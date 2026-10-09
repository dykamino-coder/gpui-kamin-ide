//! Доводка вычисленного стиля: логические стороны, единицы окна и шрифта, текстовые и смешанные стили.

use crate::style::computed::*;
use crate::style::values::value::{Color, Len};

impl Computed {
    // ПРОБОВАЛИ И ОТКАТИЛИ: блокификация под `float` и абсолютным
    // позиционированием (CSS 2.1 §9.7) — сворачивать `inline`, `inline-block`
    // и внутренние табличные виды в блок после каскада.
    //
    // Замер со всеми элементами: CSS2 4614 -> 4615 (+29/-28), oldfront
    // 2336 -> 2324. Без замещаемых (их размеры считает свой путь): CSS2
    // 4614 -> 4618, oldfront 2336 -> 2325. Потери обоих заходов — семья
    // `left-applies-to-*` и плавающие куски строки: у нас плавающий кусок
    // остаётся в общей строке текста нарочно, и блочным он рвёт соединение
    // букв (тот же корень, что у отката в `render.rs::wrap_floats`).
    //
    // Возвращаться, когда у флоатов появится своя коробка блока
    // (`bands.rs` + `BfcFlow`), а не ряд флекса.

    /// Сдвиг из `transform`, который раскладка берёт на себя как
    /// относительное смещение — тем же путём, что и свойство `translate`
    /// (`apply::apply_box`). Чистый сдвиг — это смена начала координат
    /// (css-transforms-1 §transform-rendering), и разложенная на сдвинутом
    /// месте коробка обязана рисоваться байт в байт как сдвинутая: иначе
    /// при дробном масштабе экрана округление раскладки (до сдвига) и
    /// дробный сдвиг матрицей (после) расходились на пиксель — края коробки и
    /// глифы (Blink так же проносит дробное смещение сквозь 2D-сдвиг:
    /// `PaintPropertyTreeBuilder`, subpixel accumulation). Только статичная
    /// блочная коробка — её путь отрисовки один (`render.rs`, блочная ветка
    /// `transformed(animated(e))`), и края у неё не заданы.
    pub fn folded_shift(&self) -> Option<(f32, f32)> {
        use crate::style::computed::inh;
        if !matches!(self.position, None | Some(Position::Static))
            || self.hoisted_block
            || self.rotate_prop.is_some()
            || self.scale_prop.is_some()
            || self.animation.is_some()
            || self.inherit_bits & inh::TRANSFORM != 0
            || !self.plain_block_box
            // A fragmented box is shifted per fragment, but the column
            // layout places only its first fragment by the relative offset
            // (css-break-3 §box-splitting; `css-break/transform-000`): keep
            // the shift in the transform there.
            || self.in_multicol
        {
            return None;
        }
        if let Some((x, y)) = self.translate
            && !matches!((x, y), (Len::Px(_), Len::Px(_)))
        {
            return None;
        }
        self.transform
            .as_ref()?
            .pure_px_shift()
            .filter(|&(x, y)| x != 0.0 || y != 0.0)
    }

    /// Место под логические значения — заводится по первому обращению: у
    /// подавляющего большинства узлов их нет вовсе.
    pub(crate) fn logical(&mut self) -> &mut Logical {
        self.logical.get_or_insert_with(Default::default)
    }

    /// Разложить логические стороны и размеры по физическим.
    ///
    /// Зовётся ПОСЛЕ того, как письмо унаследовано: до этого неизвестно, какая
    /// ось строчная. Физическое значение, если оно задано, не трогается —
    /// логическое лишь заполняет пустое место.
    /// Стиль без СВОЕЙ краски: `visibility: hidden` прячет коробку, но не
    /// поддерево — потомок с `visibility: visible` обязан рисоваться (§11.2).
    /// Гасить целиком нельзя: раскладка обязана остаться прежней, поэтому
    /// снимается только краска, а размеры и рамки по толщине не трогаются.
    pub fn paint_off(&self) -> Computed {
        let mut c = self.clone();
        c.hidden = None;
        c.background = None;
        c.bg_image = None;
        c.gradient = None;
        c.gradient_raw = None;
        c.border_color = Some(crate::style::values::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 0.0,
        });
        c.border_colors = [c.border_color; 4];
        c.outline = None;
        c.shadows.clear();
        c.text_shadow = None;
        c.text_shadow_rest.clear();
        c.underline = None;
        c.line_through = None;
        c
    }

    pub fn resolve_logical(&mut self, parent_vertical: Option<bool>, is_cell: bool) {
        let Some(logical) = self.logical.take() else {
            return;
        };
        // ОСИ НЕ ПЕРЕСТАВЛЯЮТСЯ, и это не упрощение, а следствие устройства
        // вертикального письма: оно рисуется ПОВОРОТОМ блока, то есть внутри
        // повёрнутого поддерева физические ширина и высота уже поменялись
        // местами. Перевод логических сторон «как в спецификации» переставил
        // бы их второй раз — замерено: writing-modes 191 → 189. Переставлять
        // здесь можно будет только вместе с отказом от поворота.
        // ПОВТОРНО ЗАМЕРЕНО И ОТКАЧЕНО (2026-08-09), уже на новом
        // вертикальном письме: перестановка ЦЕЛИКОМ — writing-modes 200 → 201,
        // flexbox 352 → 349; перестановка ТОЛЬКО размеров — те же числа.
        // Теряются `css-flexbox/gap-001-lr`, `gap-007-lr`, `gap-007-rl`
        // (получает `gap-002-rl`): размер там задан логическим свойством, и
        // после перестановки он ложится поперёк уже повёрнутой раскладки.
        // ОТКАТ ПРОТУХ (проверено 30.08 по свежему своду): из трёх названных
        // выше потерь `gap-007-lr` (0.99) и `gap-007-rl` (0.56) красные и без
        // перестановки, а «приобретение» `gap-002-rl` зелено само.
        // ЗАМЕРЕНО И ОТКАЧЕНО: исключать отсюда ячейку таблицы, чтобы вернуть
        // `table-cell-align-005` и `table-cell-valign-003`. Полный свод CSS3:
        // те же 4 потери — на этом шаге у ячейки ещё нет `display` (табличность
        // движок держит по ТЕГУ, а `Display::TableCell` приходит только из
        // авторского CSS).
        //
        // Переставляем только у УНАСЛЕДОВАВШЕГО письмо: у ортогонального узла
        // (письмо объявлено на нём самом, родитель горизонтален) перестановку
        // ниже по течению делает табличный и блочный код, и вторая здесь
        // складывалась с ней в поворот на месте.
        // РАЗМЕРЫ отображаются по СВОЕМУ письму, а не по письму родителя.
        // Таблица Abstract-Physical Mapping (css-writing-modes-4,
        // Overview.bs:1790-1827) даёт `block-size` -> width и
        // `inline-size` -> height ВСЕМ вертикальным письмам, а колонку
        // выбирает used-значение `writing-mode` САМОГО элемента: письма
        // родителя в таблице нет. Blink делает ровно это одним предикатом —
        // `computed_style.h:1270` `LogicalWidth() = IsHorizontalWritingMode()
        // ? Width() : Height()` (и так же Logical{Min,Max}{Width,Height},
        // строки 1275-1287).
        // Нашей поворотной модели это не мешает: коробки физические на всех
        // уровнях, вертикальный контейнер кладёт детей `flex_row`
        // (`render.rs:14461`), поэтому у ортогонального узла (письмо
        // объявлено на нём, родитель горизонтален) физическая ширина — его
        // БЛОЧНАЯ ось, а высота — СТРОЧНАЯ, ровно как у унаследовавшего
        // письмо. Стороны так считаются давно — `side_vertical` ниже.
        // Ячейка таблицы — единственное исключение: у неё логический
        // inline-size перекладывает в высоту сам табличный код
        // (`render.rs:16790-16800` по флагу `width_from_inline`), и второй
        // перевод здесь сложился бы с ним в поворот на месте
        // (`table-cell-align-005`, `table-cell-valign-003` — замеренный
        // откат в шапке функции).
        let vertical = self.vertical == Some(true) && (parent_vertical == Some(true) || !is_cell);
        let rtl = self.rtl == Some(true);
        // Стороны (поля/отступы/края) переставляются ПО-НАСТОЯЩЕМУ: блочный
        // поток вертикального письма собирается транспонированным рядом
        // (`flex_row(_reverse)`), а не поворотом — `margin-block` абзаца в
        // vertical-rl обязан лечь горизонтально (wm-propagation-body-*).
        // Размеры остаются без перестановки (замерено дважды, см. выше).
        let side_vertical = self.vertical == Some(true);
        let side_rl = self.vertical_rl == Some(true);
        let side_sw_lr = self.sideways == Some(true) && !side_rl;
        // Логическое значение ПЕРЕКРЫВАЕТ физическое, а не только заполняет
        // пустое. Так вело себя прежнее разложение при разборе (оно писало
        // прямо в физическое поле), и порядок объявлений в правиле от этого
        // сохранялся. Заполнение только пустого меняло исход там, где заданы
        // оба (замерено на `grid-self-alignment-baseline-with-grid-002`).
        let set = |slot: &mut Option<Len>, val: Option<Len>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // Размеры: строчная ось горизонтальна при обычном письме и
        // вертикальна при вертикальном.
        let set_ci = |slot: &mut Option<f32>, val: Option<f32>| {
            if val.is_some() {
                *slot = val;
            }
        };
        // `contain-intrinsic-*-size` переставляется по СВОЕМУ письму, а не по
        // общему гейту `vertical` (тот требует ещё и вертикального родителя).
        // Причина: читается пара через `contains_width()`/`contains_height()`,
        // а те смотрят ТОЛЬКО на `self.vertical`. У ортогонального узла
        // (письмо объявлено на нём, родитель горизонтален) условия расходились,
        // и `contain-intrinsic-inline-size` приезжал поперёк — так падали
        // `contain-intrinsic-size-logical-002` и
        // `grid-lanes-contain-intrinsic-size-logical-001`. Размеры и стороны
        // ниже остаются на прежнем гейте: их перестановку у ортогонального
        // узла делает код ниже по течению (замер описан выше по функции).
        if side_vertical {
            set_ci(&mut self.contain_intrinsic.1, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.0, logical.ci_block);
        } else {
            set_ci(&mut self.contain_intrinsic.0, logical.ci_inline);
            set_ci(&mut self.contain_intrinsic.1, logical.ci_block);
        }
        if vertical {
            set(&mut self.height, logical.inline_size);
            set(&mut self.width, logical.block_size);
            set(&mut self.min_height, logical.min_inline);
            set(&mut self.min_width, logical.min_block);
            set(&mut self.max_height, logical.max_inline);
            set(&mut self.max_width, logical.max_block);
        } else {
            if self.vertical == Some(true) && logical.inline_size.is_some() {
                self.width_from_inline = true;
            }
            set(&mut self.width, logical.inline_size);
            set(&mut self.height, logical.block_size);
            set(&mut self.min_width, logical.min_inline);
            set(&mut self.min_height, logical.min_block);
            set(&mut self.max_width, logical.max_inline);
            set(&mut self.max_height, logical.max_block);
        }
        // Стороны. Начало строчной оси: слева (обычное письмо), справа (оно же
        // справа налево) или сверху (вертикальное). Начало оси блока: сверху,
        // а в вертикальном — справа при `vertical-rl` и слева при `-lr`.
        // Логическая сторона ложится на физическую, только если объявлена
        // ПОЗЖЕ её (порядок каскада): UA `padding-inline-start: 40px` у `ul`
        // против авторского `padding-top: 0` в `vertical-rl`
        // (`line-box-direction-vrl-019`, `block-flow-direction-vrl-021`).
        let phys_seq = self.side_seq;
        let sides = [
            (&logical.padding, 0u8, phys_seq.padding),
            (&logical.margin, 1, phys_seq.margin),
            (&logical.inset, 2, phys_seq.inset),
        ];
        for (from, which, pseq) in sides {
            let to = match which {
                0 => &mut self.padding,
                1 => &mut self.margin,
                _ => &mut self.inset,
            };
            let (i_start, i_end, b_start, b_end) = if side_vertical {
                // Начало оси блока: `vertical-rl` — ПРАВЫЙ край (поток блоков
                // идёт справа налево), `vertical-lr` — левый.
                let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
                // `sideways-lr`: строка идёт СНИЗУ ВВЕРХ (css-writing-modes-4
                // §3) — начало строчной оси у нижнего края, а не верхнего.
                let flip = side_sw_lr != rtl;
                let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
                (is, ie, bs, be)
            } else if rtl {
                (1u8, 3u8, 0u8, 2u8)
            } else {
                (3u8, 1u8, 0u8, 2u8)
            };
            for (side, val, lseq) in [
                (i_start, from.inline_start, from.seq[0]),
                (i_end, from.inline_end, from.seq[1]),
                (b_start, from.block_start, from.seq[2]),
                (b_end, from.block_end, from.seq[3]),
            ] {
                let slot = match side {
                    0 => &mut to.top,
                    1 => &mut to.right,
                    2 => &mut to.bottom,
                    _ => &mut to.left,
                };
                if lseq >= pseq[side as usize] {
                    set(slot, val);
                }
            }
        }
        // Логические кромки: раскладываются той же картой сторон — сырое
        // значение прогоняется через обычный разбор кромки на настоящую
        // физическую сторону.
        let (i_start, i_end, b_start, b_end) = if side_vertical {
            let (bs, be) = if side_rl { (1u8, 3u8) } else { (3, 1) };
            let flip = side_sw_lr != rtl;
            let (is, ie) = if flip { (2u8, 0u8) } else { (0, 2) };
            (is, ie, bs, be)
        } else if rtl {
            (1u8, 3u8, 0u8, 2u8)
        } else {
            (3u8, 1u8, 0u8, 2u8)
        };
        let borders = logical.border.clone();
        for (raw, side) in [
            (&borders[0], b_start),
            (&borders[1], i_end),
            (&borders[2], b_end),
            (&borders[3], i_start),
        ] {
            if let Some(v) = raw {
                self.apply_border_shorthand(v, Some(side as usize));
            }
        }
    }

    /// Перевести `em` в точки по размеру шрифта.
    ///
    /// Размер шрифта известен только после каскада, поэтому длины в `em`
    /// доживают до этого места неразрешёнными. Для самого `font-size` база —
    /// РОДИТЕЛЬСКИЙ размер, для остального — свой собственный.
    /// Перевести единицы окна в точки.
    ///
    /// Размер окна известен только сборщику дерева, поэтому `vh`/`vw` доживают
    /// до него неразрешёнными — как и `em` до размера шрифта.
    pub fn resolve_viewport(&mut self, viewport: (f32, f32)) {
        let fix = |l: &mut Option<Len>| match *l {
            Some(Len::Vw(k)) => *l = Some(Len::Px(k * viewport.0)),
            Some(Len::Vh(k)) => *l = Some(Len::Px(k * viewport.1)),
            Some(Len::Calc(i)) => {
                let mut s = crate::style::values::value::calc_get(i);
                // Без слагаемых окна складывать нечего: индекс остаётся
                // (арена append-only, `resolve_viewport` идёт на каждом
                // слитом стиле), а `collapse` стёр бы процентную смесь
                // `calc(50% - 3px)` в `None` уже после разбора.
                if s.vw != 0.0 || s.vh != 0.0 {
                    s.px += s.vw * viewport.0 + s.vh * viewport.1;
                    s.vw = 0.0;
                    s.vh = 0.0;
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                match *one {
                    Some(Len::Vw(k)) => *one = Some(Len::Px(k * viewport.0)),
                    Some(Len::Vh(k)) => *one = Some(Len::Px(k * viewport.1)),
                    Some(Len::Calc(i)) => {
                        let mut s = crate::style::values::value::calc_get(i);
                        // То же, что у размеров: смесь с долей доживает.
                        if s.vw != 0.0 || s.vh != 0.0 {
                            s.px += s.vw * viewport.0 + s.vh * viewport.1;
                            s.vw = 0.0;
                            s.vh = 0.0;
                            *one = s.collapse_mixed();
                        }
                    }
                    _ => {}
                }
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.font_size,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        // Толщина рамки в единицах окна (`border-bottom: 50vh solid`,
        // `monolithic-overflow-021`): без перевода рамка выходила нулевой.
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
    }

    /// Во сколько раз ИСПОЛЬЗУЕМЫЙ кегль отличается от вычисленного.
    ///
    /// css-fonts-5 §font-size-adjust: `u = (m / m′) s` — желаемая доля `m`,
    /// делённая на ту же метрику шрифта `m′`. `none` и `from-font` (метрика
    /// своего же первого доступного шрифта, отношение ровно 1) кегль не
    /// трогают. Метрику, которую не удалось снять, спека велит не подгонять.
    pub fn used_font_factor(&self, family: &str) -> f32 {
        // Дескриптор `size-adjust` масштабирует ВСЕ метрики лица; поверх него
        // `font-size-adjust` сводит метрику к заданной доле, и множитель
        // дескриптора сокращается: `m / (m′·k) · k = m / m′`
        // (`size-adjust-02/03`). `from-font` — метрика того же лица, то есть
        // остаётся один дескриптор.
        // Множитель — свойство лица, лицо выбирает наклон запроса.
        let slope = if self.oblique == Some(true) {
            2
        } else if self.italic == Some(true) {
            1
        } else {
            0
        };
        let size_adjust = crate::text::fonts::size_adjust(family, slope);
        match self.font_size_adjust {
            Some((metric, want)) if want.is_finite() => {
                match crate::text::metrics::adjust_aspect(family, metric) {
                    Some(have) if have > 0.0 => want / have,
                    _ => size_adjust,
                }
            }
            _ => size_adjust,
        }
    }

    pub fn resolve_em(&mut self, parent_font_px: f32) {
        // Сначала свой размер шрифта: от него считается всё остальное. Для
        // него самого единицы шрифта считаются от РОДИТЕЛЬСКОГО кегля.
        // Родовое `monospace` имени семейства не даёт, а меряться должно по
        // тому шрифту, которым текст в самом деле наберётся.
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::text::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        // Дорожки сетки в единицах шрифта: считаются от СВОЕГО кегля, он к
        // этому моменту уже разрешён вызывающим (см. ниже по функции).
        let own_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => parent_font_px,
        };
        // Длины в единицах шрифта внутри transform/transform-origin: свой
        // кегль известен только теперь.
        if self.transform_raw.is_some()
            || self.transform_origin_raw.is_some()
            || self.shadow_raw.is_some()
            || self.text_shadow_raw.is_some()
        {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::text::metrics::ch_ex_px(&family, own_font);
            if let Some(raw) = self.transform_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform", &px);
            }
            if let Some(raw) = self.transform_origin_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform-origin", &px);
            }
            if let Some(raw) = self.shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("box-shadow", &px);
            }
            // `text-shadow` наследуется ВЫЧИСЛЕННЫМ значением: `em` решается
            // кеглем того элемента, где тень объявлена, а потомки получают уже
            // точки (сырая запись есть только у своего стиля).
            if let Some(raw) = self.text_shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("text-shadow", &px);
            }
        }
        if let Some(raw) = self.gradient_em.take() {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::text::metrics::ch_ex_px(&family, own_font);
            let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
            if let Some(g) = parse_gradient(&px) {
                self.gradient = Some(g);
                if self.gradient_raw.is_some() {
                    self.gradient_raw = Some(px);
                }
            }
        }
        for list in [self.grid_tracks.as_mut(), self.grid_rows.as_mut()]
            .into_iter()
            .flatten()
        {
            for t in list.iter_mut() {
                t.resolve_font(&family, own_px);
            }
        }
        match self.font_size {
            Some(Len::Em(k)) => self.font_size = Some(Len::Px(k * parent_font_px)),
            Some(Len::Ch(k)) => {
                let (ch, _) = crate::text::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ch));
            }
            Some(Len::Ex(k)) => {
                let (_, ex) = crate::text::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ex));
            }
            Some(Len::Ic(k)) => {
                self.font_size = Some(Len::Px(
                    k * crate::text::metrics::ic_px(&family, parent_font_px),
                ));
            }
            _ => {}
        }
        let base = match self.font_size {
            Some(Len::Px(px)) => px,
            _ => parent_font_px,
        };
        // `ch` и `ex` меряются по ГЛИФАМ семейства, а не по кеглю: у Ahem
        // нуль занимает целый кегль, у текстового шрифта — около половины.
        // Семейство здесь уже унаследовано, поэтому замер возможен только на
        // этом шаге, вместе с `em`.
        // Повторный вызов на уже подогнанном стиле: база — сохранённый
        // вычисленный кегль, а не подогнанный.
        let base = self.font_adjust_base.map_or(base, |b| b.0);
        // Используемый кегль (css-fonts-5 §font-size-adjust): «affects the
        // size of relative units that are based on font metrics such as ex
        // and ch but does not affect the size of em units». `em` и числовой
        // `line-height` остаются от `base`, метрики шрифта — от `used`, и сам
        // текст набирается `used` (`font_size`), а детям уходит `base`.
        let used = base * self.used_font_factor(&family);
        if used != base && self.font_adjust_base.is_none() {
            self.font_adjust_base = Some((base, self.line_height));
            if let Some(Len::Pct(m)) = self.line_height {
                self.line_height = Some(Len::Px(m * base));
            }
            self.font_size = Some(Len::Px(used.max(0.01)));
        }
        let (mut ch, ex) = crate::text::metrics::ch_ex_px(&family, used);
        // `ch` — продвижение нуля вдоль оси строки. При стоящих глифах в
        // вертикальном письме строка идёт сверху вниз, и продвижение равно
        // кеглю, а не ширине глифа (CSS Writing Modes §7.4).
        if self.vertical == Some(true) && self.upright == Some(true) {
            ch = used;
        }
        // `ic` меряется по тому же семейству и тем же шагом, что `ch` и `ex`.
        let ic = crate::text::metrics::ic_px(&family, used);
        // `cap` — высота прописной того же лица (css-values-4 §6.1.4). Щуп
        // вертикальных метрик её уже отдаёт третьим числом (по нему
        // `text-box-trim` считает срез `cap`), своего замера не нужно.
        let cap = crate::text::metrics::vmetrics_px(&family, used).2;
        let to_px = move |l: &mut Option<Len>| match *l {
            Some(Len::Em(k)) => *l = Some(Len::Px(k * base)),
            Some(Len::EmPx(k, add)) => *l = Some(Len::Px(k * base + add)),
            Some(Len::Ch(k)) => *l = Some(Len::Px(k * ch)),
            Some(Len::Ex(k)) => *l = Some(Len::Px(k * ex)),
            Some(Len::Ic(k)) => *l = Some(Len::Px(k * ic)),
            // Смешанный calc: шрифтовые слагаемые складываются здесь — база
            // и метрики известны; остаток сворачивается заново.
            Some(Len::Calc(i)) => {
                let mut s = crate::style::values::value::calc_get(i);
                // Без шрифтовых слагаемых складывать нечего — индекс остаётся
                // прежним: арена append-only, а `resolve_em` идёт на каждом
                // наследовании, и повторное хранение раздувало бы её впустую.
                if s.em != 0.0 || s.ch != 0.0 || s.ex != 0.0 || s.ic != 0.0 || s.cap != 0.0 {
                    s.px += s.em * base + s.ch * ch + s.ex * ex + s.ic * ic + s.cap * cap;
                    s.em = 0.0;
                    s.ch = 0.0;
                    s.ex = 0.0;
                    s.ic = 0.0;
                    s.cap = 0.0;
                    // Процентная смесь обязана ДОЖИТЬ: `collapse` вернул бы
                    // `None` и стёр `text-indent: calc(1em + 50%)`. Для всего,
                    // что пришло из `Len::parse`, `pct == 0`, и обе свёртки
                    // совпадают.
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let fix = to_px;
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                to_px(one);
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.letter_spacing,
            &mut self.word_spacing,
            &mut self.text_indent,
            &mut self.line_height,
            &mut self.column_width,
            &mut self.column_height,
            &mut self.column_gap,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
        if let Some(o) = self.outline.as_mut() {
            fix(&mut o.width);
            fix(&mut o.offset);
        }
    }

    /// Тот же стиль без коробки — только то, что относится к тексту.
    ///
    /// Нужен там, где абзац разбит на куски: фон, отступы, рамка и размеры
    /// принадлежат абзацу целиком, и повторять их на каждом слове нельзя —
    /// иначе у каждого слова появляется своя подложка и своё поле.
    /// Итоговые возможности OpenType куска — в порядке старшинства
    /// css-fonts-4 §7.2: `font-variant-*` и прочие свойства, затем свойство
    /// `font-feature-settings`. Повтор тега схлопывается, побеждает
    /// последний: прежде в gpui уходило `liga 0, …, liga 1`, а
    /// `apply_font_features` дописывал после них ещё `liga 0`
    /// (`font-features-across-space-3`).
    pub fn used_features(&self) -> Vec<(String, u32)> {
        // Шаг 2 §7.2 — дескриптор правила `@font-face`, МЛАДШЕ свойств.
        let mut all: Vec<(String, u32)> =
            crate::text::fonts::face_features(self.font_family.as_deref().unwrap_or(""));
        all.extend(self.font_features.iter().cloned());
        all.extend(crate::text::fonts::alternates::resolve(
            self.font_family.as_deref().unwrap_or(""),
            self.font_alternates.as_ref(),
        ));
        self.add_kerning_feature(&mut all);
        // Шаг 4 §7.2: «setting a non-default value for the letter-spacing
        // property disables optional ligatures» (css-text-3 §8.2). Старше
        // `font-variant-ligatures`, младше `font-feature-settings`
        // (`font-feature-resolution-001/002`: `fvl-1 ls-1` — без лигатуры,
        // `ls-1 ffs-1` — с ней).
        if matches!(self.letter_spacing, Some(Len::Px(v) | Len::Em(v)) if v != 0.0) {
            for tag in ["liga", "clig", "dlig", "hlig"] {
                all.push((tag.to_string(), 0));
            }
        }
        if let Some(settings) = &self.font_settings {
            all.extend(settings.iter().cloned());
        }
        let mut out: Vec<(String, u32)> = Vec::with_capacity(all.len());
        for (tag, value) in all {
            out.retain(|(t, _)| *t != tag);
            out.push((tag, value));
        }
        out
    }

    pub fn text_only(&self) -> Computed {
        Computed {
            color: self.color,
            // Видимость — свойство ТЕКСТА тоже: скрытый кусок держит место, но
            // не красится, а запасная ветка «строка из слов» флаг теряла.
            hidden: self.hidden,
            font_size: self.font_size,
            // Гарнитура — свойство ТЕКСТА: без неё кусок в строчном ряду
            // набирался подменным системным шрифтом, и `@font-face` (в том
            // числе Ahem у стенда) не доезжал никуда, где рядом стоит
            // картинка или иной атом.
            font_family: self.font_family.clone(),
            font_families: self.font_families.clone(),
            font_weight: self.font_weight,
            font_weight_step: self.font_weight_step,
            italic: self.italic,
            oblique: self.oblique,
            underline: self.underline,
            line_through: self.line_through,
            line_height: self.line_height,
            text_align: self.text_align,
            text_align_last: self.text_align_last,
            break_word: self.break_word,
            balance_lines: self.balance_lines,
            bidi_override: self.bidi_override,
            bidi_isolate: self.bidi_isolate,
            bidi_embed: self.bidi_embed,
            hanging: self.hanging,
            nowrap: self.nowrap,
            monospace: self.monospace,
            letter_spacing: self.letter_spacing,
            font_features: self.font_features.clone(),
            font_kerning: self.font_kerning,
            font_settings: self.font_settings.clone(),
            font_alternates: self.font_alternates.clone(),
            ruby_merge: self.ruby_merge,
            text_transform: self.text_transform,
            ellipsis: self.ellipsis,
            overflow_marker: self.overflow_marker.clone(),
            clamp_mark: self.clamp_mark.clone(),
            line_clamp: self.line_clamp,
            clamp_legacy: self.clamp_legacy,
            clamp_auto: self.clamp_auto,
            svg_fill: self.svg_fill.clone(),
            // `stroke` и `stroke-width` в SVG НАСЛЕДУЮТСЯ (SVG 2 §Painting),
            // как и `fill`. Геометрия (`x`, `y`) — нет, её здесь нет намеренно.
            svg_stroke: self.svg_stroke.clone(),
            svg_stroke_width: self.svg_stroke_width.clone(),
            webkit_box: self.webkit_box,
            webkit_box_vertical: self.webkit_box_vertical,
            // Сдвиг от базовой линии — свойство ТЕКСТА: без него строчный
            // кусок в общем прогоне остаётся на базовой линии.
            vertical_shift: self.vertical_shift,
            vertical_shift_pct: self.vertical_shift_pct,
            vertical_shift_px: self.vertical_shift_px,
            vertical_shift_len: self.vertical_shift_len,
            vertical_align_text: self.vertical_align_text,
            vertical_align_base: self.vertical_align_base,
            rel_shift: self.rel_shift,
            text_fit: self.text_fit,
            hyphen_char: self.hyphen_char.clone(),
            // Кусок, собранный из `text_only`, бывает родителем: без базы он
            // отдал бы детям ПОДОГНАННЫЙ кегль, и подгонка накопилась бы.
            font_size_adjust: self.font_size_adjust,
            font_adjust_base: self.font_adjust_base,
            ..Computed::default()
        }
    }

    /// Смесь этого стиля с наведённым по доле перехода.
    ///
    /// Смешиваются свойства, которые в наведении и меняют: цвета, заливка,
    /// прозрачность, толщина рамки. Остальное берётся у наведённого стиля,
    /// как только доля переваливает половину — ступенькой, потому что
    /// промежуточного значения у них нет.
    pub fn blend(&self, hover: &Computed, k: f32) -> Computed {
        let k = k.clamp(0.0, 1.0);
        if k <= 0.0 {
            return self.clone();
        }
        let mut out = if k >= 0.5 {
            hover.clone()
        } else {
            self.clone()
        };
        let mix = |a: Option<Color>, b: Option<Color>| -> Option<Color> {
            match (a, b) {
                (Some(a), Some(b)) => Some(Color {
                    r: a.r + (b.r - a.r) * k,
                    g: a.g + (b.g - a.g) * k,
                    b: a.b + (b.b - a.b) * k,
                    a: a.a + (b.a - a.a) * k,
                }),
                (a, b) => b.or(a),
            }
        };
        out.background = mix(self.background, hover.background);
        out.color = mix(self.color, hover.color);
        out.border_color = mix(self.border_color, hover.border_color);
        out.opacity = match (self.opacity, hover.opacity) {
            (Some(a), Some(b)) => Some(a + (b - a) * k),
            (a, b) => b.or(a),
        };
        out
    }
}
