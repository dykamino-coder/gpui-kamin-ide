//! Типы потока фрагментов: полосы, дети, строки, ось стопки.
// owner: A

use crate::layout::float::shapes::FloatShape;
use gpui::{AnyElement, Bounds, IntoElement, Pixels, point, px, size};

/// Полоса выреза: на строках, пересекающих [y0, y1), начало (или конец)
/// строки занято на `left`/`right` точек.
#[derive(Clone, Copy, Debug, Default)]
pub struct ExBand {
    pub y0: f32,
    pub y1: f32,
    pub left: f32,
    pub right: f32,
}

/// Ребёнок потока: элемент и его известный размер.
pub struct FlowChild {
    pub el: AnyElement,
    pub w: f32,
    pub h: f32,
}

pub struct FlowRow {
    pub(crate) children: Vec<FlowChild>,
    pub(crate) shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
    /// Направление письма: rtl кладёт коробки от правого края.
    pub(crate) rtl: bool,
    /// `writing-mode: vertical-rl`: строки — колонки справа налево, поток в
    /// колонке — сверху вниз. Раскладка идёт в ТРАНСПОНИРОВАННОМ мире
    /// (инлайн-ось строкой), физика восстанавливается при укладке.
    pub(crate) vertical_rl: bool,
    /// Известный инлайн-размер содержащего блока (в вертикальном письме —
    /// его высота): запасной предел строк, когда замер его не даёт.
    pub(crate) inline_limit: Option<f32>,
    /// `vertical-lr`: колонки идут СЛЕВА направо (блок-старт — левый край).
    pub(crate) block_lr: bool,
    /// `sideways-lr`: инлайн-ось снизу вверх.
    pub(crate) inline_up: bool,
    /// Позиции детей, вычисленные замером (в точках от угла коробки).
    pub(crate) slots: std::cell::RefCell<Vec<(f32, f32)>>,
}

impl FlowRow {
    pub fn new(
        children: Vec<FlowChild>,
        shapes: std::sync::Arc<(Vec<FloatShape>, Vec<FloatShape>)>,
        rtl: bool,
    ) -> Self {
        FlowRow {
            children,
            shapes,
            rtl,
            vertical_rl: false,
            inline_limit: None,
            block_lr: false,
            inline_up: false,
            slots: std::cell::RefCell::new(Vec::new()),
        }
    }

    pub fn vertical_rl(mut self) -> Self {
        self.vertical_rl = true;
        self
    }

    pub fn inline_up(mut self) -> Self {
        self.inline_up = true;
        self
    }

    pub fn block_lr(mut self) -> Self {
        self.block_lr = true;
        self
    }

    pub fn inline_limit(mut self, v: f32) -> Self {
        self.inline_limit = Some(v);
        self
    }

    /// Размер ребёнка в осях раскладки: в вертикальном письме инлайн-ось —
    /// физическая высота.
    pub(crate) fn tdims(&self, c: &FlowChild) -> (f32, f32) {
        if self.vertical_rl {
            (c.h, c.w)
        } else {
            (c.w, c.h)
        }
    }

    /// Вырез на полосе [y, y+h): точный экстент форм с обеих сторон.
    pub(crate) fn cut(&self, y: f32, h: f32) -> (f32, f32) {
        let l = self
            .shapes
            .0
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        let r = self
            .shapes
            .1
            .iter()
            .map(|f| f.cut(y, y + h))
            .fold(0.0f32, f32::max);
        (l, r)
    }

    /// Нижний край всех форм: ниже него вырезов нет.
    pub(crate) fn shapes_bottom(&self) -> f32 {
        self.shapes
            .0
            .iter()
            .chain(self.shapes.1.iter())
            .map(|f| match *f {
                FloatShape::Band { top, h, .. } => top + h,
                FloatShape::Circle { top, cy, r, .. } => top + cy + r,
                FloatShape::Ellipse { top, cy, ry, .. } => top + cy + ry,
                FloatShape::Poly { top, ref pts } => {
                    top + pts.iter().map(|p| p.1).fold(0.0f32, f32::max)
                }
                FloatShape::Profile { top, ref ext } => top + ext.len() as f32,
                FloatShape::RoundedBox { top, ref shape, .. } => top + shape.bottom(),
            })
            .fold(0.0f32, f32::max)
    }

    /// Разложить детей в ширину `limit`; вернуть высоту и позиции.
    pub(crate) fn layout(&self, limit: f32) -> (f32, Vec<(f32, f32)>) {
        let mut slots = Vec::with_capacity(self.children.len());
        let mut y = 0.0f32;
        let mut x = 0.0f32;
        let mut line_h = 0.0f32;
        let mut cut = self.cut(0.0, 1.0);
        for c in &self.children {
            let (cw, ch) = self.tdims(c);
            let avail = (limit - cut.0 - cut.1).max(0.0);
            // ЗАМЕРЕНО И ОТКАЧЕНО: гасить перенос при `white-space: nowrap`
            // (поля `nowrap` у ряда не было вовсе). Полный свод CSS3: 0 и 0.
            // Заявленные пары в своде не нашлись под названными именами —
            // полосный ряд включается только при обтекании, а тесты гибкой
            // раскладки флоатов не содержат.
            // Не влезает — новая строка; коробка шире строки стоит одна.
            if x + cw > avail + 0.01 && x > 0.0 {
                y += line_h;
                x = 0.0;
                line_h = 0.0;
                cut = self.cut(y, ch.max(1.0));
            } else if x == 0.0 {
                cut = self.cut(y, ch.max(1.0));
            }
            // Коробка не помещается даже в начале строки — строка съезжает
            // ниже, пока вырез не отпустит (float-retry-push): плавающий
            // блок толкает СЛИШКОМ ШИРОКОЕ содержимое под себя.
            if x == 0.0 && cw > (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                let bottom = self.shapes_bottom();
                while y < bottom {
                    y += 1.0;
                    cut = self.cut(y, ch.max(1.0));
                    if cw <= (limit - cut.0 - cut.1).max(0.0) + 0.01 {
                        break;
                    }
                }
            }
            // Вырез мог смениться выше по строке — пересчитать после переноса.
            let (sx, sy) = if self.rtl {
                (limit - cut.1 - x - cw, y)
            } else {
                (cut.0 + x, y)
            };
            slots.push((sx, sy));
            x += cw;
            line_h = line_h.max(ch);
        }
        (y + line_h, slots)
    }
}

impl IntoElement for FlowRow {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Ребёнок колонок: элемент, высота коробки и вертикальные поля
/// (схлопываются между соседями по правилам потока).
pub struct StackChild {
    pub el: AnyElement,
    /// Высоту и параллельный поток даёт РАСКЛАДКА копии шириной в колонку
    /// (`Some(ширина колонки)`), а не мера `render::shape_full`: та ребёнка не
    /// выразила (флоаты, внепоточные потомки …). Ставится в
    /// `request_column_layout` до укладки; точек разреза у такого ребёнка нет —
    /// он режется краем колонки.
    pub measure: Option<f32>,
    /// Запасные копии ТОГО ЖЕ ребёнка. Разрез между колонками рисует по
    /// копии на фрагмент: элемент GPUI рисуется ровно один раз, и показать
    /// одну коробку в двух колонках иначе нечем. Копий столько же, сколько
    /// колонок, — больше ребёнок занять не может.
    pub frags: Vec<AnyElement>,
    /// Монолит — коробка, которую нельзя разрывать (css-break-3 §4.1):
    /// `break-inside: avoid`, прокручиваемая коробка, ячейка таблицы,
    /// замещаемый элемент и сплошной строчный набор. Такая уходит в
    /// следующую колонку целиком.
    pub monolith: bool,
    /// Точки ЗАКОННОГО разреза (css-break-3 §4.3, класс A) — границы
    /// вложенных блочных детей, рекурсивно: `(need, from)` — сколько
    /// ребёнка от верха должно уместиться до разреза и с какого смещения
    /// продолжать в следующей колонке (поле на границе усекается,
    /// css-break-3 §5.2). По возрастанию.
    pub cuts: Vec<(f32, f32)>,
    /// Принудительные разрывы (css-break-4 §3.1): перед коробкой, после неё
    /// и внутри — смещения от верха, где `break-before/after` вложенных
    /// блочных детей требуют новой колонки.
    pub force_before: bool,
    pub force_after: bool,
    /// Запрет разрыва на ГРАНИЦЕ с соседом (css-break-4 §4.3, правило 1):
    /// `break-before: avoid*` этой коробки и `break-after: avoid*`
    /// предыдущей запрещают разрез ровно в этой точке — но не внутри самих
    /// коробок (это `monolith`). С переносом по §break-propagation:
    /// значение снимается `render::edge_avoid`, как принудительное —
    /// `edge_break`.
    pub avoid_before: bool,
    pub avoid_after: bool,
    pub forced: Vec<f32>,
    /// Диапазоны, внутри которых разрыв запрещён (css-break-4 §4.1 —
    /// монолиты-потомки; рамка и отбивка самой коробки): `[a, b)` от верха.
    pub solid: Vec<(f32, f32)>,
    pub h: f32,
    pub mt: f32,
    pub mb: f32,
    /// `column-span: all` (css-multicol-1 §6): кладётся во всю ширину между
    /// линиями колонок; в стопке с рядами — по курсору Blink
    /// `LayoutSpanner` (не влез в остаток ряда — со следующего ряда).
    pub span: bool,
    /// Низ ПАРАЛЛЕЛЬНОГО потока (css-break-3 §3) от верха коробки:
    /// содержимое, переполняющее заданную высоту, продолжается в следующем
    /// фрагментаинере само по себе. Равен `h`, когда потока нет. Поток
    /// режется по нему, а шагом для СОСЕДА остаётся `h`.
    pub over: f32,
    /// Общий сдвиг `position: relative`, снятый с копии (`hoist_relative`).
    /// css-break-3 §5.5: «Fragmentation occurs before relative positioning …
    /// Such effects are applied per fragment» — сдвиг накладывается НА
    /// фрагмент, а значит двигает и его срез. Внутри копии он бы уехал из
    /// маски колонки и погас (`out-of-flow-in-multicolumn-042/045`), а у
    /// КОРНЯ копии `layout_as_root` его и вовсе не читает (проба `pm1`).
    pub rel: (f32, f32),
    /// `box-decoration-break: clone`: блочное украшение `(верх, низ)`.
    /// Взведённое поле значит, что КАЖДАЯ копия — уже готовый фрагмент своей
    /// высоты (`render.rs::clone_fragment`): её не поднимают на срез и не
    /// режут маской.
    pub clone_dec: Option<(f32, f32)>,
    /// Монолит-ПОТОМОК, начатый на верху колонки, переполняет её, а не режется
    /// краем (`fill_at`, `overflow_to`). Только `column-fill: auto` без рядов и
    /// только ребёнку без элементов ряда (`render::parallel_items_inside`):
    /// баланс подобрал бы высоту ниже монолита, а у ряда flex/сетки/таблицы в
    /// переполнение ушли бы соседи по ряду.
    pub overflow_top: bool,
    /// В поддереве ребёнка есть многоколоночник (`render::multicol_inside`). Внешними
    /// колонками он у нас не фрагментируется (нет Blink
    /// `is_constrained_by_outer_fragmentation_context_`,
    /// `column_layout_algorithm.cc:1743-1746`): копия — ПЛОСКИЙ рисунок его
    /// собственного баланса, и её боковой вылет выдуман. Маска такого ребёнка
    /// режет вбок по колонке, как до вылета (`multicol-nested-013/014/021`,
    /// `multicol-fill-balance-nested-000`).
    pub nested_cols: bool,
    /// Повторяемые шапка/подвал таблицы (css-tables-3 §repeated-headers; Blink
    /// `table_layout_algorithm.cc:1082-1150`). `None` — повтора нет.
    pub repeat: Option<Repeat>,
    /// Параллельный поток строки flex (`Par`).
    pub par: Par,
    /// Позиционированный ребёнок (или контекст наложения с `z-index: auto`):
    /// красится ПОСЛЕ всех непозиционированных фрагментов стопки (CSS 2.1
    /// Appendix E, шаг 8 после шагов 4–7).
    pub positioned: bool,
    /// Фон таблицы для «хвоста» непоследнего фрагмента: секции и ряды до низа
    /// фрагментаинера не тянутся, а коробка таблицы — тянется (css-break-3
    /// §box-splitting; Blink `table_layout_algorithm.cc` — фрагмент таблицы
    /// занимает остаток, `fragmentation_utils.cc:563` «Consumed block-size …
    /// is always stretched to the fragmentainers»). Распорка роста
    /// (`grow_pushed`) в пустой ячейке коробки не находит, и в хвосте было
    /// пусто. Хвост красится цветом фона по ширине разложенной копии.
    pub slack: Option<gpui::Hsla>,
    /// Ширина разложенной копии (для `slack`), ставится в `prepaint`.
    pub laid_w: std::cell::Cell<f32>,
}

/// Повтор секций таблицы во фрагментах. Фрагмент в нашей модели — СРЕЗ одной
/// нарисованной копии `[from, from + h)`, а повторённой шапки в срезе нет:
/// она рисуется ОТДЕЛЬНОЙ копией той же таблицы, поднятой так, что полоса
/// шапки встаёт на верх фрагмента, и режется маской по этой полосе. Тело
/// фрагмента-продолжения уезжает вниз на высоту полосы (`Frag::head`), у
/// непоследнего фрагмента снизу оставлено место под подвал (`Frag::foot`) —
/// это Blink `reserved_space = repeated_header_block_size +
/// repeated_footer_block_size` (`:1323-1327`).
pub struct Repeat {
    /// Полоса шапки в координатах коробки: `(верх, высота)`; высота — шапка
    /// плюс `border-spacing` под ней (Blink `repeated_header_block_size`,
    /// `:1393-1395`: продолжение шапку кладёт без зазора над ней, `:942`).
    pub head: Option<(f32, f32)>,
    /// Полоса подвала: `(верх, высота)`; высота — подвал плюс зазор над ним
    /// (Blink `repeated_footer_block_size`, `:1147-1149`).
    pub foot: Option<(f32, f32)>,
    /// Где полосам место (`RepeatGeom`).
    pub geom: RepeatGeom,
    /// Копии для полос: шапка — фрагменту `c ≥ 1` (`head_els[c - 1]`),
    /// подвал — НЕпоследнему фрагменту `c` (`foot_els[c]`).
    pub head_els: Vec<AnyElement>,
    pub foot_els: Vec<AnyElement>,
}

/// Геометрия повтора для укладки (`fill_at`), в координатах коробки. Нули —
/// повтора нет, укладка тождественна прежней.
#[derive(Clone, Copy, Default)]
pub struct RepeatGeom {
    /// Место полосы шапки сверху у продолжения и полосы подвала снизу.
    pub head: f32,
    pub foot: f32,
    /// Конец полосы шапки на её родном месте: продолжение, начатое раньше,
    /// шапку ещё не прошло.
    pub head_end: f32,
    /// Точка разрыва перед подвалом на родном месте и конец подвала: фрагмент,
    /// куда родной подвал влезает целиком, полосы не берёт — подвал в нём
    /// последний (Blink `has_pending_repeated_footer = false`, `:1315`).
    pub foot_at: f32,
    pub foot_end: f32,
    /// Коробка рядов (без подписей): повтор — только во фрагментах, где она
    /// есть (Blink: «If this isn't the first fragment for the table box …»,
    /// `:1108-1114`; подписи о повторе не знают, `:1248-1249`).
    pub box_top: f32,
    pub box_end: f32,
}

impl RepeatGeom {
    /// Полоса шапки у фрагмента-продолжения, начатого с `from`.
    pub(crate) fn head_at(&self, from: f32) -> f32 {
        if self.head > 0.0 && from >= self.head_end - 0.01 && from < self.box_end - 0.01 {
            self.head
        } else {
            0.0
        }
    }

    /// Место под подвал у фрагмента с `from`, которому доступно `room`.
    pub(crate) fn foot_for(&self, from: f32, room: f32) -> f32 {
        if self.foot > 0.0
            && from < self.foot_at - 0.01
            && from + room > self.box_top + 0.01
            && from + room < self.foot_end - 0.01
        {
            self.foot
        } else {
            0.0
        }
    }
}

/// Мера ребёнка для укладки колонок.
#[derive(Clone)]
pub struct Kid {
    pub h: f32,
    pub mt: f32,
    pub mb: f32,
    pub monolith: bool,
    pub cuts: Vec<(f32, f32)>,
    pub force_before: bool,
    pub force_after: bool,
    /// css-break-4 §4.3 правило 1 — см. `StackChild::avoid_before`. Стопка
    /// СТРАНИЦ ставит сюда `false`: правило 1 в печати пока не применяется.
    pub avoid_before: bool,
    pub avoid_after: bool,
    pub forced: Vec<f32>,
    pub solid: Vec<(f32, f32)>,
    pub span: bool,
    /// Низ параллельного потока от верха коробки (`StackChild::over`); `h`,
    /// когда потока нет.
    pub over: f32,
    /// `box-decoration-break: clone` — блочное украшение `(верх, низ)`,
    /// повторяемое в КАЖДОМ фрагменте (css-break-4 §break-decoration).
    /// `None` — `slice`, прежний путь до последней строки.
    pub clone_dec: Option<(f32, f32)>,
    /// См. `StackChild::overflow_top`; у страниц — `false`.
    pub overflow_top: bool,
    /// Повтор секций таблицы (`RepeatGeom`): место шапки сверху у
    /// фрагмента-продолжения и подвала снизу у непоследнего; нули — нет.
    pub repeat: RepeatGeom,
    /// Параллельный поток (`Par`); `Par::default()` — обычный ребёнок.
    pub par: Par,
}

/// Строки многострочного КОЛОНОЧНОГО flex-контейнера — параллельные потоки
/// (Blink `flex_layout_algorithm.cc:2096-2120`: свой `FlexColumnBreakInfo` на
/// строку, `:2504-2515` — разрыв элемента уводит к следующей строке, а не
/// рвёт контейнер). Контейнер раскрыт в стопке на элементы
/// (`render::split_flex_lines`): строка — подряд идущие дети с общим началом
/// группы, каждая следующая строка укладывается С ТОГО ЖЕ места, что первая,
/// а за группой курсор встаёт на самый дальний конец строки. Поля элементов
/// не схлопываются (css-flexbox-1 §4.2).
#[derive(Clone, Copy, Default, Debug)]
pub struct Par {
    /// Номер группы (контейнера); 0 — не в группе.
    pub group: u32,
    /// Первый ребёнок группы / первый ребёнок строки / последний ребёнок группы.
    pub group_start: bool,
    pub line_start: bool,
    pub group_end: bool,
    /// Сдвиг строки по оси x от края колонки.
    pub dx: f32,
    /// Монолит только из-за `break-inside: avoid`: это пожелание (css-break-4
    /// §4.4), и элемент выше целого фрагментаинера с его верха рвётся как
    /// обычный, а не с верха переносится (Blink: avoid лишь снижает
    /// привлекательность разрыва; `multi-line-column-flex-fragmentation-017`:
    /// элементы 250/200 при колонке 100). Не с верха — переносится целиком.
    pub avoid_only: bool,
    /// Плавающая коробка во всю ширину колонки, поставленная в стопку блоком
    /// (`render.rs`, `full_float`). Перенесённая в следующую колонку, она не
    /// уводит за собой поток: следующие коробки продолжают остаток текущей
    /// колонки, а в колонке флоата встают под ним (css-break-3 §4: флоат —
    /// своя фрагментируемая коробка; Blink кладёт флоат в следующий
    /// фрагментаинер, а соседа-BFC — в возможность размещения под ним;
    /// `css-break/float-005…008`).
    pub float: bool,
    /// Коробка с `clear`: встаёт под перенесённым флоатом, а не в остаток
    /// колонки перед ним (CSS 2.1 §9.5.2). Следующий флоат — тоже: его верх
    /// не выше верха предыдущего (§9.5.1 п.5).
    pub clears: bool,
}

/// Кусок ребёнка в колонке: чей он, какая по счёту копия, в какой колонке
/// стоит, на сколько отступает от её верха, какая часть содержимого видна
/// (`from` — от собственного верха ребёнка) и какой она высоты.
#[derive(Clone, Copy)]
pub(crate) struct Frag {
    pub(crate) kid: usize,
    pub(crate) copy: usize,
    pub(crate) col: usize,
    pub(crate) y: f32,
    pub(crate) from: f32,
    pub(crate) h: f32,
    /// Полосы повтора таблицы (`Repeat`): шапка над `y` и подвал под `y + h`;
    /// `y`/`h` — по-прежнему само содержимое среза.
    pub(crate) head: f32,
    pub(crate) foot: f32,
}

/// Колонки многоколоночного потока для БЛОЧНЫХ детей с известными
/// высотами (css-multicol §7.4 + css-break): жадная укладка сверху вниз,
/// балансировка «оценка + добавка на минимальный недолаз» (как в blink
/// ResolveColumnAutoBlockSize), монолиты уходят в следующую колонку
/// целиком. Разрез ДЕТЕЙ (строк/рамок) — следующая фаза.
///
/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): разрез ребёнка по краю колонки написан и
/// работает. Устройство: `StackChild` несёт по копии элемента на колонку
/// (элемент GPUI рисуется ровно один раз, и показать одну коробку в двух
/// колонках больше нечем), `fill` возвращает фрагменты
/// `{кто, копия, колонка, отступ, срез, высота}`, раскладка ставит каждую
/// копию ЦЕЛИКОМ и поднимает её на срез, а отрисовка режет маской
/// `with_content_mask` — это и есть вид `slice` из css-break-3 §4.
/// Замерено ЧЕТЫРЕ раза на срезе css-break+css-multicol (1498 пар):
///
/// * голый разрез, маска на каждом ребёнке — 333 -> 342 (+29/−20);
/// * плюс монолиты (`break-inside: avoid`, прокрутка, замещаемый, таблица,
///   ячейка, сплошной строчный набор) — 341 -> 338 (+18/−21);
/// * плюс маска ТОЛЬКО разрезанному ребёнку — 341 -> 342 (+2/−1);
/// * то же без отсечки «сплошной строчный набор» — 341 -> 341 (+5/−5).
///
/// Отсюда главный вывод, который и надо помнить: почти весь «выигрыш»
/// первого захода был ЛОЖНЫЙ — его давала маска, прятавшая переполнение у
/// НЕразрезанных детей (`overflow-clip-004`, `overflow-unsplittable-*`,
/// `overflowing-block-003` и родня возвращаются, как только маску ограничить
/// разрезанными). Сам разрез по высотам детей стоит +2 и упирается в то,
/// что укладка колонок не видит СТРОК: смелая отсечка режет посреди строки
/// и теряет ровно столько же, сколько приобретает.
///
/// Пятый замер, уже против СВЕЖЕЙ базы (342 зелёных): 342, +20/−20 — и
/// это самое важное наблюдение. Двигаются РОВНО те же двадцать пар, что и
/// от правки «у `<canvas>` есть коробка», только в обратную сторону:
/// `overflow-clip-004`, `table-cell-expansion-006`,
/// `flex-container-fragmentation-008/009`, `monolithic-with-overflow`,
/// `tall-line-in-short-fragmentainer-002` возвращаются в зелёное, а
/// `borders-001/002`, `out-of-flow-in-multicolumn-077..080`,
/// `table-*-paint-v*` уходят в красное. Значит эта двадцатка держится не на
/// разрезе, а на том, ЕСТЬ ЛИ у замещаемого коробка и считается ли он
/// монолитом, — и любой частичный шаг просто перекладывает её из кармана в
/// карман. Три разных правки (разрез, `column-fill: auto` с высотой,
/// коробка `<canvas>`) дали на этом срезе +1, +1 и +1.
///
/// Возвращать вместе с фрагментацией ПО СТРОКАМ (высота строки и её
/// границы), а не по высотам детей, и сразу с монолитами по css-break-3
/// §4.1 — одним куском. Разбор `break-inside` написан в том же патче и тоже
/// откачен: без разреза он мёртвый. Патч целиком:
/// `target/frag-slice.patch`, разборы раздела —
/// `target/scout-cssbreak-2026-09.md` и `target/scout-linefrag-2026-09.md`
/// (второй пересчитал потолок построчной фрагментации: не 200 пар, а 40-52,
/// зато «разрыв между блочными детьми РЕКУРСИВНО» — 183 пары).
/// Ширина колонки для внутренних размеров многоколоночного контейнера:
/// `Some(w)` — `column-width` в точках, `None` — `column-width: auto`.
/// Само наличие значения означает «ширину коробки решает содержимое».
#[derive(Clone, Copy)]
pub struct Intrinsic(pub Option<f32>);

/// Ряды колонок (css-multicol-2 §column-wrap, §column-height). Blink
/// (`column_layout_algorithm.cc`): ряды — сетка с шагом `h + gap` по
/// содержимому коробки; линия колонок ставится в текущий ряд, её высота —
/// остаток ряда (`RemainingRowHeightAtOffset`), лишние колонки уходят в
/// следующий ряд (`OffsetToNextRow`). Спаннеры внутри рядов — следующий шаг
/// (`target/scout-columnwrap-2026-09.md` §5).
#[derive(Clone, Copy)]
pub struct Rows {
    /// Высота ряда: `column-height`, либо высота коробки при `column-wrap:
    /// wrap` (Blink `RowHeight`); `None` — ряд не ограничен, новые ряды
    /// родятся только от принудительных разрывов
    /// (`column-wrap-no-constraints-002`).
    pub h: Option<f32>,
    /// `row-gap` между рядами (умолчание `normal` = 1em, §rg).
    pub gap: f32,
    /// `wrap` — лишние колонки в новый ряд; иначе (`nowrap` с заданным
    /// `column-height`) — вбок, за край коробки (css-multicol-1 §8.2).
    pub wrap: bool,
    /// Рядов нет — это ПОТОЛОК баланса: `h` = заданная высота коробки
    /// многоколоночника (Blink `ConstrainColumnBlockSize`: колонка не выше
    /// used block-size контейнера). Линия одна, высотой в баланс, лишние
    /// колонки — вбок.
    pub cap: bool,
}

thread_local! {
    /// Глубина построения копий детей стопки (`render.rs`, `StackChild`).
    static STACK_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Сторож «строится копия ребёнка стопки». Многоколоночник со спаннером
/// внутри другой стопки остаётся на сегментном пути: единой стопке нужен
/// перенос ряда/спаннера во ВНЕШНЮЮ колонку (Blink `LayoutSpanner`: «The new
/// row doesn't fit in the outer fragmentainer»), которого нет
/// (`column-height-029`, `target/scout-columnwrap-2026-09b.md` §2.3).
pub struct StackScope;

impl StackScope {
    pub fn enter() -> Self {
        STACK_DEPTH.with(|d| d.set(d.get() + 1));
        StackScope
    }
}

impl Drop for StackScope {
    fn drop(&mut self) {
        STACK_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
    }
}

thread_local! {
    /// Высота ряда вложенного многоколоночника, заданная ВНЕШНЕЙ колонкой
    /// (`set_outer_row` → `take_outer_row` первой строкой `render::element`).
    static OUTER_ROW: std::cell::Cell<Option<(f32, f32)>> = const { std::cell::Cell::new(None) };
    /// Сколько первых колонок укладки стоят НЕ с верха фрагментаинера (первый
    /// ряд вложенного многоколоночника, начатого ниже верха внешней колонки):
    /// не влезший с верха такой колонки монолит уходит дальше, а не
    /// переполняет её (Blink: `is_at_fragmentainer_start` ложно —
    /// `BreakBeforeChildIfNeeded`, css-break-3 §4.1 «may be pushed»).
    pub(crate) static NOT_TOP: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Передать следующему `element()` высоту внешнего фрагментаинера: копия
/// вложенного многоколоночника строится рядами этой высоты (css-break-4 §2.1:
/// «when a multi-column container breaks across pages, it generates a new row
/// of columns on the next page»; Blink `column_layout_algorithm.cc:1741-1748`
/// `ConstrainColumnBlockSize` → `min(size, available_outer_space)`).
/// Сторож `NOT_TOP` на время укладки.
pub(crate) struct NotTop(pub(crate) usize);

impl NotTop {
    pub(crate) fn set(n: usize) -> Self {
        NotTop(NOT_TOP.with(|c| c.replace(n)))
    }
}

impl Drop for NotTop {
    fn drop(&mut self) {
        NOT_TOP.with(|c| c.set(self.0));
    }
}

pub fn set_outer_row(h: Option<(f32, f32)>) {
    OUTER_ROW.with(|r| r.set(h));
}

/// Забрать переданную высоту (одноразово).
pub fn take_outer_row() -> Option<(f32, f32)> {
    OUTER_ROW.with(|r| r.take())
}

pub fn in_stack() -> bool {
    STACK_DEPTH.with(|d| d.get() > 0)
}

/// Ось стопки колонок. css-multicol-1 §2 (`Overview.bs:375-379`): «The
/// column boxes are ordered in the inline base direction of the multicol
/// container … The column width is the length of the column box in the inline
/// direction. The column height is the length of the column box in the block
/// direction»; note `:544-549`: «In text set using a vertical writing mode, the
/// block direction runs horizontally». Blink держит укладку логической
/// (`column_layout_algorithm.cc:982` `LogicalOffset logical_offset(
/// column_inline_offset, line_offset)`) и переводит в физику при сборке
/// фрагмента (`WritingModeConverter`). Вся арифметика стопки (`fill_at`,
/// `balance*`, `Rows`, `place`) у нас тоже логическая: «высота» в ней — блочный
/// размер, «x колонки» — строчное смещение. Физика — только в раскладке и
/// отрисовке (`prepaint_axis`/`paint_axis`).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum StackAxis {
    /// `horizontal-tb`: прогрессия колонок вправо, блочная ось вниз.
    Horizontal,
    /// `vertical-lr`: прогрессия колонок вниз, блочная ось вправо.
    VerticalLr,
    /// `vertical-rl`/`sideways-rl`: прогрессия вниз, блочная ось ВЛЕВО от
    /// правого края коробки (css-writing-modes-4 §3.1 «block flow direction»).
    VerticalRl,
}

impl StackAxis {
    pub fn is_vertical(self) -> bool {
        !matches!(self, StackAxis::Horizontal)
    }
}

/// Логическая коробка стопки → физическая. `io`/`ie` — смещение и размер по
/// СТРОЧНОЙ оси (по ней идёт прогрессия колонок), `bo`/`be` — по БЛОЧНОЙ;
/// `block` — блочный размер всей стопки (нужен `vertical-rl`: там блочная ось
/// отсчитывается от правого края). Blink `WritingModeConverter::ToPhysical`
/// (`writing_mode_converter.cc`): у `vertical-rl` `x = outer.width - offset -
/// inner.width`.
pub fn axis_box(
    axis: StackAxis,
    origin: gpui::Point<Pixels>,
    block: f32,
    io: f32,
    ie: f32,
    bo: f32,
    be: f32,
) -> Bounds<Pixels> {
    match axis {
        StackAxis::Horizontal => Bounds {
            origin: point(origin.x + px(io), origin.y + px(bo)),
            size: size(px(ie), px(be)),
        },
        StackAxis::VerticalLr => Bounds {
            origin: point(origin.x + px(bo), origin.y + px(io)),
            size: size(px(be), px(ie)),
        },
        StackAxis::VerticalRl => Bounds {
            origin: point(origin.x + px(block - bo - be), origin.y + px(io)),
            size: size(px(be), px(ie)),
        },
    }
}
