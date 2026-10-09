//! Цикл по детям блока: строчные прогоны, блочные дети, позиционированные (blocks_flow).

use crate::render::*;

#[allow(clippy::too_many_arguments)]
pub(crate) fn blocks_flow(
    nodes: &[Node],
    run_breaks: Vec<usize>,
    mut pending: Vec<Node>,
    mut out: Vec<AnyElement>,
    mut letter_scope: crate::render::first_letter_scope::Scope,
    inherited: &Computed,
    opts: &RenderOpts,
    ordered_context: bool,
    under_tf: bool,
    frame: std::rc::Rc<std::cell::Cell<crate::interact::StickyFrame>>,
    mut below_run_end: usize,
    mut below_run_start: usize,
    mut below_zs: Vec<i32>,
) -> Vec<AnyElement> {
    for (idx, n) in nodes.iter().enumerate() {
        if run_breaks.contains(&idx) && !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(letter_scope.paragraph(&taken, inherited, opts)));
        }
        let is_inline = match n {
            // Пробельный узел между инлайн-соседями — часть строки, а не
            // разрыв: `<button>A</button> <button>B</button>` в разметке с
            // переносами давал два абзаца, и кнопки вставали столбиком.
            // Под `white-space: pre*` пробельный узел — содержимое: узел из
            // одного перевода строки это ПУСТАЯ СТРОКА перед `</pre>`
            // (block-plaintext-006), отбрасывание съедало её высоту.
            Node::Text(t) => {
                inherited.preserve_newlines == Some(true)
                    || !blank_text(t)
                    || (!pending.is_empty() && t.contains(' '))
            }
            // Элемент с ЗАДАННЫМИ краями строчным не бывает: края он считает
            // от позиционированного предка, а не от строки. Куском абзаца он
            // получал содержащим блоком сам абзац — и `inset: 0` растягивал
            // его на одну строку вместо всей коробки родителя. На этом стоит
            // приём эталонов WPT: `::after` с `content: ""` и `inset: 0`
            // накрывает красное зелёным (`overflow-wrap-anywhere-001`).
            // Только когда заданы ОБЕ оси: у коробки с одним краем свободная
            // ось остаётся статической, а статическая позиция строчного — в
            // строке, не в блочном потоке. Такую коробку ведёт щуп в
            // `atom_element` (`x_set != y_set`).
            Node::Element(e)
                if matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) && !at_static_position(&e.style)
                    && {
                        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                        (edge(e.style.inset.left) || edge(e.style.inset.right))
                            && (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                    }
                    // Поле формы и заменяемый элемент строит СВОЙ путь
                    // (`forms::element`, картинка), и краями он распоряжается
                    // сам. Выведенный из строки, он терял свою коробку —
                    // `<button>` с четырьмя краями переставал растягиваться
                    // (`position-absolute-semi-replaced-stretch-button`).
                    // Исключение снимается ровно там, где оно даёт НЕ ТОТ
                    // прямоугольник: содержащий блок абсолюта — внутренний
                    // край рамки родителя (§10.1 п.4.2), а куском строки
                    // замещаемый считает край от содержимого. Признак —
                    // `cb_padding_shifts_replaced`.
                    && (!matches!(
                        e.tag.as_str(),
                        "input" | "textarea" | "select" | "button" | "img" | "svg" | "canvas"
                    ) || cb_padding_shifts_replaced(e, inherited)) =>
            {
                false
            }
            // Абсолют строчного уровня (до блокификации — `inline-block` и
            // родня) с РОВНО ОДНОЙ заданной осью: свободная ось берётся от
            // гипотетической коробки при `position: static` (CSS 2.1 §10.3.7,
            // §10.6.4), а та стоит в строке, не под ней. Блокифицированный, он
            // уходил блочным ребёнком ниже абзаца, и `left: 0; top: auto`
            // вставал на следующую строку (`border-left-width-thin`: белая
            // заплатка под красным вместо поверх). Щуп строки ведёт такую
            // коробку в `atom_element` (`x_set != y_set`). Без строчного
            // содержимого ДО коробки строка пуста, и гипотетическая коробка
            // стоит в её начале — там же, где блочная статическая позиция;
            // такой абсолют остаётся прежним блочным путём
            // (`left-applies-to-012/014`: абсолют — единственный ребёнок).
            Node::Element(e)
                if e.style.abs_inline_level
                    && !ordered_context
                    && pending.iter().any(|p| match p {
                        Node::Text(t) => !t.trim().is_empty(),
                        Node::Element(x) => !matches!(
                            x.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        ),
                    })
                    && {
                        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                        (edge(e.style.inset.left) || edge(e.style.inset.right))
                            != (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                    } =>
            {
                true
            }
            Node::Element(e) => match e.style.display {
                // Явно заявленная инлайновая коробка остаётся в строке даже у
                // блочного по природе тега — но НЕ внутри гибкого контейнера
                // или сетки: там каждый ребёнок сам себе элемент раскладки
                // («блокирование» из CSS). Иначе колонка из таких коробок
                // выкладывалась рядом: они склеивались в один абзац.
                Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable) => !ordered_context,
                // `display: inline grid-lanes` — такая же строчная коробка:
                // разбор держит её как `GridLanes` с пометкой `lanes_inline`,
                // и по css-display-3 внешний вид у неё `inline`. Эталоны
                // семьи `grid-lanes-intrinsic-sizing-*` написаны на
                // `display: inline-grid`, и без этой строки девять сеток
                // вставали столбиком вместо ряда.
                Some(Display::GridLanes) if e.style.lanes_inline => !ordered_context,
                // `display: contents` without block-level descendants: its
                // children are inline-level boxes and text runs of THIS
                // container (css-display-3 §2.5 «as if they replaced the
                // element»), so they join the surrounding inline run — the
                // inline collector dissolves the element (`inline.rs`,
                // `Display::Contents`). Flushing the run here split one line
                // `<div contents>abc</div><br>` into an anonymous block plus a
                // run starting with `<br>` — an extra empty line
                // (`text-autospace-elements-002`).
                Some(Display::Contents) => !ordered_context && !contains_block(&e.children),
                // Прежний откат этой строки СНЯТ (03.09). Он мерился, когда
                // строчный атом строил лунки голым `blocks()` и терял их
                // целиком — оттого вся восьмёрка `flow-tolerance-*` и уходила
                // в красное (0.00 -> 5.66 и родня). Теперь `atom_element`
                // отдаёт лунки блочному пути (`element()`), и обе правки
                // вместе дают по всему CSS3 2420 -> 2442: приобретено 27,
                // потеряно 5 (`row-line-names-007/008/010/012`,
                // `row-subgrid-abs-pos-002` — рядные лунки, они ждут обтяжку
                // по РЯДАМ, корень R4 из `target/scout-subgrid-orthogonal-
                // 2026-09.md`).
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строчный путь для абсолюта
                // с объявленным `display: inline` на статической позиции
                // (css-position-3 §staticpos-rect). Срез 12086 пар вместе с
                // патчем барьера `contain`: 9343 -> 9346 (+9/-6), причём вся
                // шестёрка потерь — этого рукава:
                // `inline-level-absolute-in-block-level-context-002`
                // (0.26->0.52), `-007` (0.00->0.54), `-010` (0.00->1.04),
                // `position-absolute-dynamic-static-position-inline`
                // (0.00->2.10), `abs-pos-border-offset-003` (0.46->1.75),
                // `css-flexbox-height-animation-stretch` (0.10->1.90), против
                // всего двух приобретений (`-009`, `-012`). Строчная ветка
                // теряет полосу обтекания и рамочные смещения — рукав нужен
                // не здесь, а в `atom_element`.
                Some(_) => false,
                // Дети гибкого контейнера и сетки блокируются по CSS: каждый
                // сам себе элемент раскладки. Без оговорки `<span>` без
                // объявленного `display` оставался строчным, склеивался с
                // соседями в ОДИН абзац, и четыре элемента раскладки
                // превращались в один.
                //
                // Плавающий кусок строчным не бывает: `float` вынимает элемент
                // из строки и делает блоком (CSS 2.1 §9.7). Пока картинка с
                // `float: right` оставалась куском абзаца, до неё не доходило
                // поле родителя, и она вылезала за край страницы.
                // Перевод строки коробки не создаёт: в гибком контейнере и
                // сетке он остаётся ВНУТРИ безымянного элемента раскладки
                // вместе с соседним текстом, а не становится своим элементом
                // (`position-absolute-root-element-flex`: два предложения,
                // разделённые `<br><br>`, вставали бок о бок и переносились
                // раньше времени).
                None => {
                    e.inline
                        && (!ordered_context || e.tag == "br")
                        && !e.style.float.is_some_and(|f| f != 0)
                }
            },
        };
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paint_inline_step7(letter_scope.paragraph(&taken, inherited, opts)));
        }
        // Позиционированные с `z-index: auto` красятся В ПОРЯДКЕ ДЕРЕВА
        // (CSS 2.1 прил. E, шаг 8; Blink `paint_layer_paint_order_iterator.h`
        // — один список). Абсолют на статической позиции живёт в верхнем слое
        // (`late_push`), и тот выпускался только в конце контейнера — ПОВЕРХ
        // позиционированных соседей, идущих в дереве позже
        // (`position-sticky-stacking-context-002`: `#overlapped-red` накрывал
        // липкий и `relative`-брата). Перед таким соседом слой выпускается:
        // пустые заместители нулевой высоты раскладку не трогают, а щупы
        // накопленных абсолютов стоят раньше по списку — дырки к подготовке
        // их заместителей уже известны. Отрицательный `z-index` не трогаем:
        // у него своя сортировка прогона (`below_run_*`).
        if let Node::Element(e) = n
            && crate::interact::late_pending()
            && !e.style.z_index.is_some_and(|z| z < 0)
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Relative) | Some(crate::computed::Position::Sticky)
            )
        {
            out.extend(crate::interact::late_close());
            crate::interact::late_open();
        }
        if let Node::Element(e) = n {
            // Ключ краски шага 8 — до сборки детей (см. `next_paint_key`).
            let paint_key = next_paint_key();
            // CSS2 Appendix E: descendants paint within their nearest stacking context.
            let layer_ok = !inside_deferred();
            let geometry_layer_ok = !paint_scope::deferred();
            let _deferred_guard = paint_scope::Guard::enter(
                defers(&e.style, inherited, under_tf), stacking_context(&e.style),
            );
            // Ряд обтекания: текст рядом с плавающим блоком и остаток под ним.
            if e.tag == "kamin-float" {
                out.push(letter_scope.flow(&e.children, inherited, |s| float_flow(e, s, opts)));
                continue;
            }
            if let Some(el) = scrollable(e, inherited, opts) {
                out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
                continue;
            }
            if let Some(el) = resizable(e, inherited, opts) {
                out.push(el);
                continue;
            }
            if let Some(el) = transitioned(e, inherited, opts) {
                // Наложение считается и для узла с переходом: раньше ветка
                // уходила мимо, и `z-index` у него пропадал.
                out.push(layered(el, &e.style, inherited, layer_ok, under_tf));
                continue;
            }
            // `display: contents` — своей коробки у элемента нет: дети
            // становятся детьми родителя, и стиль самого элемента исчезает.
            if e.style.display == Some(Display::Contents) {
                let mut merged = inline::inherit(inherited, &e.style);
                // Своей коробки нет — значит и объёмный контекст она не
                // обрывает: дети берут ячейки ДЕДА (css-display-3
                // §box-generation; transform3d-preserve3d-014 — `rotateX(90)`
                // над `display: contents` над `rotateX(90) scale(2)`).
                if merged.frame_3d.is_none() {
                    merged.frame_3d = inherited.frame_3d.clone();
                }
                if merged.perspective_frame.is_none() {
                    merged.perspective_frame = inherited.perspective_frame.clone();
                }
                out.extend(blocks(&e.children, &merged, opts));
                continue;
            }
            // Ключевое слово содержимого в `min-width`/`max-width` при
            // ширине в точках (css-sizing-3 §4.1, зажим §5.1): used =
            // max(W, kw) либо min(W, kw) — то же самое, что `width: kw` с
            // пределом W. Перестановка отдаёт ключевое слово обёртке-сетке
            // (`content_sized`), а точки — пределу в раскладке (`apply`);
            // блочная ось решается в `apply` (`min-height: max-content`).
            let swapped;
            let e = if let Some(copy) = content_limit_swapped(e) {
                swapped = copy;
                &swapped
            } else {
                e
            };
            // Обёртка `content_sized` — сетка, а дорожка сетки НЕ считает
            // боковые поля ребёнка: коробка `width: max-content` с полем
            // теряла его и уезжала (`pre-wrap-017`: зелёный блок пропадал
            // вовсе). Поэтому элемент строится БЕЗ боковых полей, а поля
            // берёт на себя обёртка.
            // Переносится только ОТРИЦАТЕЛЬНОЕ поле: положительное внутри
            // дорожки работает как надо, а отрицательное дорожка съедает —
            // коробка `width: max-content` с `margin-left: -1em` пропадала
            // вовсе (`pre-wrap-017`).
            let negative = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::Px(v) | Len::Em(v) | Len::Ch(v) | Len::Ex(v)) if v < 0.0
                )
            };
            let hoist_margins = content_sized_wraps(e)
                && !replaced_tag(e)
                && (negative(e.style.margin.left) || negative(e.style.margin.right));
            // Размещение в сетке тоже уезжает на обёртку (см.
            // `content_sized`): в дорожках родителя стоит она. Внутри обёртки
            // (своя сетка в одну дорожку) элемент с прежним `grid-row: 2`
            // уходил бы в её неявный ряд. Прежде обёртка без размещения
            // ставилась авто-размещением (`row-fill-reverse-align-self-001`:
            // `width: min-content; grid-row: 2` в лунках вставал в ряд 1).
            let placement = crate::apply::grid_item_placement(&e.style);
            let hoist_place = content_sized_wraps(e)
                && !replaced_tag(e)
                && (placement.0.is_some() || placement.1.is_some());
            let stripped;
            let e = if hoist_margins || hoist_place {
                let mut copy = e.clone();
                if hoist_margins {
                    copy.style.margin.left = None;
                    copy.style.margin.right = None;
                }
                if hoist_place {
                    copy.style.grid_row = None;
                    copy.style.grid_col = None;
                    copy.style.grid_row_named = [None, None];
                    copy.style.grid_col_named = [None, None];
                }
                stripped = copy;
                &stripped
            } else {
                e
            };
            let placement = if hoist_place { placement } else { (None, None) };
            // Коробка по содержимому (`width: min-content | max-content |
            // fit-content`) стоит в обёртке-сетке `content_sized`, и её доли
            // решались бы ОТ ОБЁРТКИ, а не от содержащего блока: `height: 100%`
            // — от неявного ряда (по содержимому: коробка схлопывалась в ноль),
            // `padding-top: 100%` — от области сетки (заливала всё окно).
            // Блочный родитель с известными сторонами решает их сразу
            // (CSS 2.1 §10.5 высота — от высоты содержащего блока, §8.4
            // отступы и §8.3 поля — от его ширины;
            // `intrinsic-percent-replaced-012/013`).
            let resolved_pct;
            let e = match pct_resolved_for_wrapper(e, inherited) {
                Some(copy) => {
                    resolved_pct = copy;
                    &resolved_pct
                }
                None => e,
            };
            // Анимация оборачивает ЛЮБОЙ элемент: таблицу, список, картинку —
            // раньше она доставалась только простому блоку.
            // Фон КАНВАСА (CSS 2.2 §14.2): фон корневого html — а без него
            // фон body — красит всю область просмотра, включая место за
            // полями. Слой absolute от родителя-корня растягивается на всё
            // окно, с самой коробки краска снимается (иначе двойная альфа).
            let canvas_paint = e.style.canvas_bg;
            // Тело под корнем-донором фона холста при `vertical-rl`: его
            // margin-box (плюс рамка/отбивка корня) — коробка корня по
            // содержимому; её левый край пишется на подготовке тела.
            let record_root = e.tag == "body"
                && inherited.canvas_bg
                && inherited.vertical_rl == Some(true)
                && !matches!(inherited.width, Some(Len::Px(_)));
            let canvas_stripped;
            let e = if canvas_paint {
                canvas_layer(
                    e,
                    opts,
                    &mut out,
                );
                let mut copy = e.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                copy.style.bg_image = None;
                canvas_stripped = copy;
                &canvas_stripped
            } else {
                e
            };
            // Абсолют с КЛЮЧЕВЫМ СЛОВОМ содержимого по оси и краями с обеих
            // сторон этой оси (css-position-3 §3.7-3.8): растяжение краями —
            // только для автоматического размера; заданный ключевым словом
            // размер — по содержимому, а остаток делят auto-поля. Держатель =
            // inset-modified containing block (абсолют с краями элемента,
            // гибкий контейнер вдоль оси), внутри — та же коробка статической,
            // без краёв и полей (`div-{min,max,fit}-content-block-size`,
            // `div-*-auto-margin-*`).
            let kw_len = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                )
            };
            let positioned_out = matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            );
            // Предел ключевым словом содержимого при АВТОМАТИЧЕСКОМ размере —
            // тот же держатель: css-sizing-3 §fit-content в блочной оси даёт
            // высоту содержимого, и used = min(растяжение краями, содержимое)
            // (`position-absolute-fit-content`: `top: 0; bottom: 0;
            // max-height: fit-content` — 100, а не 200). `apply.rs` такой
            // предел умеет только при `height` в точках и иначе пропускает.
            // Содержимое выше растяжения держатель не зажмёт — приближение.
            let auto_len = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let holder_axis = if positioned_out
                && e.style.vertical.is_none()
                && auto_len(e.style.height) && kw_len(e.style.max_height)
                && edge_set(e.style.inset.top)
                && edge_set(e.style.inset.bottom)
            {
                Some(true)
            } else if positioned_out
                // Письмо коробки держателю не мешает: после `280d0d4` размеры
                // ортогонального узла ложатся по СВОЕМУ письму, и у
                // `vertical-rl` `block-size: min-content` — физическая ширина
                // (css-writing-modes-4, Abstract-Physical Mapping). Гейт
                // `vertical.is_none()` ставился, пока размеры
                // транспонировались; без держателя ширина тянулась краями
                // `left/right` во всё окно (`div-{min,max,fit}-content-
                // orthogonal-*`: 13.5 %). Ветка высоты выше гейт сохраняет: в
                // вертикальном письме это строчная ось, её пары не разбирались.
                && auto_len(e.style.width) && kw_len(e.style.max_width)
                && edge_set(e.style.inset.left)
                && edge_set(e.style.inset.right)
            {
                Some(false)
            } else {
                None
            };
            let built = if let Some(block_axis) = holder_axis {
                inset_holder_box(
                    e,
                    block_axis,
                    inherited,
                    opts,
                    &kw_len,
                )
            } else if stacking_context(&e.style)
                && e.style.isolate != Some(true)
                && blends_inside(&e.children, 0)
            {
                // css-compositing-1 §mix-blend-mode: смешиваемый потомок
                // смешивается только с содержимым СВОЕГО контекста наложения.
                // Контекст обязан сложиться отдельной группой, иначе подложкой
                // становится весь кадр: белая страница вокруг родителя давала
                // красное кольцо (`-blended-element-with-transparent-pixels`),
                // lime вместо fuchsia (`-blended-with-3D-transform`). Blink —
                // `PaintLayer::HasNonIsolatedDescendantWithBlendMode`.
                let mut iso = e.style.clone();
                iso.isolate = Some(true);
                grouped(
                    transformed(animated(e, inherited, opts), &e.style, inherited),
                    &iso,
                )
            } else {
                grouped(
                    transformed(animated(e, inherited, opts), &e.style, inherited),
                    &e.style,
                )
            };
            let built = vertical_hug(built, e, inherited);
            let built = sticky_wrap(built, &e.style, &frame, layer_ok);
            // Таблица сжимается по содержимому (§17.5.2.2), и выражено это у
            // нас гибким рядом. В контейнере с БЛОЧНОЙ раскладкой гибкого
            // ряда нет, `align_self` мёртв, и таблица растягивалась на всю
            // ширину родителя — видно на `<span style="display:block">` с
            // табличными детьми.
            let table_child = e.tag == "table"
                || matches!(
                    e.style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            let block_parent = matches!(
                inherited.display,
                Some(Display::Block) | Some(Display::ListItem) | Some(Display::TableCell)
            );
            let built = if table_child && block_parent && e.style.width.is_none() {
                div().flex().flex_row().child(built).into_any_element()
            } else {
                built
            };
            // Абсолютный блок без заданных краёв стоит на СТАТИЧЕСКОЙ позиции —
            // там, где он оказался бы в потоке, а не в углу содержащего блока.
            // Пустышка нулевой высоты держит это место в потоке, элемент висит
            // от её угла. Без неё такой блок уезжал к началу родителя и
            // накрывал собой всё, что стояло выше.
            // Только в обычном потоке: в сетке и гибком контейнере пустышка
            // стала бы ЯЧЕЙКОЙ и сдвинула соседей, а по CSS абсолютный
            // ребёнок из раскладки родителя выключен.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ — наследование,
            // шрифт, письмо и маски остаются верными, — а готовый уходит
            // последним ребёнком документа, где края решит уже вьюпорт.
            //
            // Пока только при заданных ОБЕИХ осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ (наследование,
            // шрифт, письмо и маски остаются верными), а готовый уходит
            // последним ребёнком документа, где края решает уже вьюпорт.
            //
            // Предок считается ВКЛЮЧАЯ непосредственного родителя:
            // `inherited.cb_ancestor` отвечает за предков строго выше него.
            // ЗАМЕРЕНО без этого слагаемого: CSS2 4636 -> 4626, все двенадцать
            // потерь — абсолют внутри `position: relative`-РОДИТЕЛЯ.
            //
            // Пока только при заданных обеих осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Достаточно ОДНОЙ заданной оси: по ней край считает раскладка от
            // области просмотра, по пустой элемент стоит на СТАТИЧЕСКОЙ
            // позиции (§10.3.7, §10.6.4), и её сообщает щуп, оставшийся на
            // месте элемента. Ось задана, если задана хотя бы одна сторона.
            //
            // Внутри отложенного поддерева щуп готовится ПОЗЖЕ слоя, и дырка
            // была бы пуста — такие остаются на месте.
            let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
            let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
            // `fixed` считается ОТ ОКНА всегда (§10.1 п.3): позиционированный
            // предок ему не содержащий блок, и заданной оси от него не
            // требуется — незаданная сторона держит статическое место. Пока он
            // шёл общим путём, коробка висела от края родителя.
            let fixed = e.style.position == Some(crate::computed::Position::Fixed) && !under_tf;
            // `fixed` под трансформом — абсолют относительно этого предка.
            let abs_like = e.style.position == Some(crate::computed::Position::Absolute)
                || (e.style.position == Some(crate::computed::Position::Fixed) && under_tf);
            // В стопке страниц абсолют корня уходит в слой и без заданных
            // сторон: на месте его резала бы маска фрагмента кида
            // (`monolithic-overflow-013`); статическую позицию копии 0 даёт
            // щуп, копии ≥ 1 идут непрерывным потоком от верха листа.
            let orphan_abs = abs_like
                && !(inherited.cb_ancestor || crate::inline::establishes_cb(inherited))
                && (x_set || y_set || PAGED.with(|p| p.get()));
            // Позиционированный предок ЕСТЬ, но это не родитель: коробку
            // забирает слой ближайшего содержащего блока (§10.1).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09, шесть заходов): пускать в слой
            // РОДИТЕЛЯ коробку, у которой родитель сам образует содержащий
            // блок (снять вето `!establishes_cb`). Замысел верный —
            // `Spot::fixed_axes` писался под смешанный случай «одна ось от
            // края, другая статическая», и без выноса такой коробке
            // статическую позицию не считает никто (`probe/svpc.html`: y = 30
            // вместо 90). Целевой срез 650 пар (305 зелёных): 258 при ЛЮБОМ
            // гейте — по позиции родителя, по флагу «слой открыт», без
            // табличных видов, только для одной оси. Приобретено 9, и это
            // ровно те пары, которые ждал прежний откат: `abspos-009`,
            // `position-absolute-007`, `right-offset-003`, `abs-pos-non-
            // replaced-vlr-087/089`, `-vrl-086/088/158/164`. Потеряно 46 —
            // `table-anonymous-objects-011..091`: родитель там обычный
            // `position: relative` div (`display: None`, слой открыт), и
            // коробка с ОДНОЙ заданной осью в его слое встаёт не туда, где
            // стояла в потоке. Значит неверно не условие входа, а сама
            // статическая позиция, которую слой считает горизонтальной
            // одноосной коробке. Возвращать вместе с проверкой щупа на
            // `table-anonymous-objects-011` (три абсолюта, у одного задан
            // лишь `top`).
            // `fixed` под трансформом: содержащий блок — ближайший предок,
            // содержащий `fixed` (css-transforms-1 §transform-rendering,
            // css-contain-2 §3.2), а позиционированные между ними — нет
            // (`out-of-flow-in-multicolumn-029`: `fixed` внутри абсолюта
            // внутри трансформа). Родитель-трансформ держит его на месте.
            let tf_fixed = e.style.position == Some(crate::computed::Position::Fixed) && under_tf;
            // Без заданных сторон — тоже: на месте раскладка разрешила бы
            // проценты размеров от РОДИТЕЛЯ (`width: 100%` у абсолютного
            // родителя нулевой ширины, `out-of-flow-in-multicolumn-044`), а
            // статическую позицию по обеим осям даёт щуп.
            let far_fixed = tf_fixed && !fixed_cb_layer_box(inherited);
            let far_abs = !tf_fixed
                && abs_like
                && inherited.cb_ancestor
                && !crate::inline::establishes_cb(inherited)
                && (x_set || y_set);
            let far_abs = far_abs || far_fixed;
            // A negative `z-index` box whose containing block is the ICB goes
            // to the ICB layer too, painted in the bottom layer (`Underlay`,
            // CSS 2.1 §9.9 step 3): in place it was positioned from its
            // parent's box, e.g. a `body` lowered by a collapsed margin
            // (spec-examples `shape-outside-001`: `#failure-container`).
            let below_icb = orphan_abs
                && !fixed
                && e.style.z_index.is_some_and(|z| z < 0)
                && !stacking_context(inherited);
            let to_icb = !ordered_context
                && geometry_layer_ok
                && (fixed || orphan_abs)
                && (e.style.z_index.unwrap_or(0) >= 0 || below_icb)
                && !stays_positioned(&nodes[idx + 1..]);
            // В гибком контейнере и сетке слой содержащего блока закрыт: там
            // нет щупа статической позиции. Но при ОБЕИХ заданных осях щуп и не
            // нужен (CSS 2.1 §10.1 п.4 — содержащий блок ближайший
            // позиционированный предок, а не flex/grid-родитель): иначе коробка
            // раскладывалась от родителя (`align-self-with-flex-grid-parent`:
            // розовый квадрат уезжал с `.inner` на 270 px).
            let to_cb = !to_icb
                && (!ordered_context || (x_set && y_set))
                && geometry_layer_ok
                && far_abs
                && e.style.z_index.unwrap_or(0) >= 0
                && !stays_positioned(&nodes[idx + 1..]);
            let built = if to_icb || to_cb {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    fixed_axes: (x_set, y_set),
                    free_margin: (
                        if x_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.left, &e.style).unwrap_or(0.0)
                        },
                        if y_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.top, &e.style).unwrap_or(0.0)
                        },
                    ),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    replaced: matches!(
                        e.tag.as_str(),
                        "img" | "iframe" | "video" | "canvas" | "object" | "embed" | "svg"
                    ),
                    ..Default::default()
                });
                let sent = if to_icb && fixed && PAGED.with(|p| p.get()) {
                    // Стопка страниц: фиксированный — в свой слой, по копии
                    // на лист без сдвига (`FIXED_LAYER`). Обёртка `LatePlace`
                    // та же, что у слоя ICB, — через вложенный слой.
                    crate::interact::icb_open();
                    let _ = crate::interact::icb_push(spot.clone(), built);
                    FIXED_LAYER.with(|f| f.borrow_mut().extend(crate::interact::icb_close()));
                    None
                } else {
                    // Слой содержащего блока рисуется после его потока, но
                    // позиционированные красятся в порядке разметки (шаг 8):
                    // ключ ставит коробку слоя среди них. `fixed` в слое ICB —
                    // тоже: без ключа он красился раньше собирателя, и
                    // поднятый в собиратель предок ложился поверх.
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: `fixed` без ключа — кусок 1704
                    // пары +4/−2 (`static-fixed-inside-abspos`,
                    // `position-fixed-001`: 0.00 -> «красное видно»); с
                    // ключом +4/−0.
                    // A positive `z-index` orders the box above the layer's
                    // auto/0 boxes (CSS 2.1 §9.9 steps 8–9): the deferral the
                    // in-place path gets from `layered` below
                    // (`shape-image-009`: `#test` z-index 2 under a z-index 1
                    // failure box, both hoisted).
                    let built = if !fixed && e.style.z_index.is_some_and(|z| z > 0) {
                        layered(built, &e.style, inherited, layer_ok, under_tf)
                    } else {
                        built
                    };
                    let built = if paint_last_ok(e, &nodes[idx + 1..]) {
                        gpui::PaintLast::new(built).key(paint_key).into_any_element()
                    } else {
                        built
                    };
                    let built = if to_icb && below_icb {
                        crate::interact::Underlay::new(built).into_any_element()
                    } else {
                        built
                    };
                    if to_icb {
                        crate::interact::icb_push(spot.clone(), built)
                    } else if far_fixed {
                        crate::interact::cb_push_fixed(spot.clone(), built)
                    } else {
                        crate::interact::cb_push(spot.clone(), built)
                    }
                };
                match sent {
                    None => {
                        // Пустая ось требует щупа: статическую позицию взять
                        // больше неоткуда. При заданных обеих осях на месте
                        // не остаётся ничего.
                        if !(x_set && y_set) {
                            out.push(crate::interact::spot_probe(spot, true));
                        }
                        continue;
                    }
                    Some(kept) => kept,
                }
            } else {
                built
            };
            // Абсолютная коробка с ОТРИЦАТЕЛЬНЫМ `z-index` не идёт ни в слой
            // ICB, ни в верхний слой: её место ПОД потоком (§9.9 шаг 3). Но
            // пустая ось у неё считается от СТАТИЧЕСКОЙ позиции, а гибкая
            // раскладка такой коробке её не даёт и ставит в начало содержимого
            // родителя. Нулевая распорка держит место в потоке, и коробка
            // висит от её угла — там, где написана.
            // Исключение — коробка блочного уровня, у которой задана БЛОЧНАЯ ось
            // (`top`/`bottom`), а свободна строчная, в горизонтальном письме
            // слева направо, и содержащий блок — сам родитель. Статическая
            // позиция по строчной оси здесь — левый край содержимого
            // родителя (§10.3.7), её раскладка на месте даёт и так, а
            // заданную ось §10.6.4 считает от СОДЕРЖАЩЕГО БЛОКА. На распорке
            // `top: 1px` отсчитывался от статической позиции — коробка
            // съезжала под весь поток (`margin-collapse-clear-012..016`:
            // красная подложка `z-index: -1` под жёлтым блоком).
            let below_cb_axis = e.style.position == Some(crate::computed::Position::Absolute)
                && e.style.z_index.is_some_and(|z| z < 0)
                && y_set
                && !x_set
                && !e.inline
                && inherited.rtl != Some(true)
                && inherited.vertical != Some(true)
                && e.style.vertical != Some(true)
                && matches!(
                    inherited.position,
                    Some(crate::computed::Position::Relative)
                        | Some(crate::computed::Position::Absolute)
                )
                && crate::inline::establishes_cb(inherited)
                && !inherited.cb_ancestor
                && !stacking_context(inherited)
                && !ordered_context;
            let below_free_axis = e.style.position == Some(crate::computed::Position::Absolute)
                && e.style.z_index.is_some_and(|z| z < 0)
                && !(x_set && y_set)
                && !below_cb_axis;
            // ЗАМЕРЕНО И ОТКАЧЕНО: уводить в верхний слой ВСЯКУЮ абсолютную
            // коробку с одной свободной осью (§9.9 шаг 8) — по симметрии с
            // `below_free_axis`. Полный свод CSS2: приобретено 3, ПОТЕРЯНО
            // 165 (вся семья `vertical-align-0NN` уходит в «красное видно»,
            // `floats-wrap-bfc-outside-001` 0.08 -> 7.28). Распорка держит
            // место свободной оси только там, где элемент и так вне строки;
            // в абзаце она рвёт строку. Возвращаться только с настоящей
            // статической позицией внутри строки.
            if !ordered_context && (at_static_position(&e.style) || below_free_axis) {
                (below_run_start, below_run_end) = static_position_layer(
                    e,
                    inherited,
                    built,
                    layer_ok,
                    under_tf,
                    nodes,
                    idx,
                    paint_key,
                    &mut out,
                    &mut below_zs,
                    below_run_start,
                    below_run_end,
                );
                continue;
            }
            let _ = hoist_margins;
            // Замещаемому дорожка по содержимому не нужна: его размер по
            // ключевому слову — природный, считается в `image_with`.
            let layered_built = layered(built, &e.style, inherited, layer_ok, under_tf);
            let mut done = content_wrapper::for_element(layered_built, e, inherited, placement);
            // Тело под корнем-донором фона холста при `vertical-rl`: записать
            // левый край коробки корня для отрисовки холста (см. `canvas_paint`).
            // Корень по содержимому = margin-box тела плюс рамка и отбивка
            // корня слева.
            if record_root {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let root_b = inherited.borders();
                let offset = side(e.style.margin.left) + side(inherited.padding.left) + side(root_b.left);
                done = crate::interact::record_root_left(done, opts.doc_salt, offset);
            }
            // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
            // flow): свой анкор-ряд вокруг ОДНОГО узла — соседей не трогает.
            // Корню с фоном-картинкой не ставится (гасил canvas-слой).
            // Прижим — свойство ГЛАВНОГО потока, а он на документ один. Если
            // обособление на `html` или на `body` погасило распространение
            // письма тела в область просмотра (css-contain-2
            // §containment-types), главным потоком тело не стало: оно
            // остаётся обычным блоком в потоке горизонтального корня и к
            // правому краю окна не жмётся
            // (contain-body-w-m-001..004, contain-html-w-m-001..004).
            if matches!(e.tag.as_str(), "html" | "body")
                && e.style.vertical_rl == Some(true)
                && !e.style.wm_contained
            {
                if e.style.bg_image.is_none() {
                    done = div()
                        .w_full()
                        .flex()
                        .justify_end()
                        .child(done)
                        .into_any_element();
                } else if let Some(Len::Px(w)) = e.style.width {
                    // Корню с фоном-картинкой флекс-обёртка гасила слой
                    // краски — прижим вправо считается сдвигом по известной
                    // ширине (background-size-document-root-vrl-*).
                    let shift = (opts.viewport.0 - w).max(0.0);
                    if shift > 0.0 {
                        done = div().ml(px(shift)).child(done).into_any_element();
                    }
                }
            }
            // Релятивный элемент с отрицательным `z-index`: место в потоке —
            // своё, краска — под содержимым до него (CSS 2.1 §9.9, шаг 3).
            // Расширение на элементы сетки без `position` ЗАМЕРЕНО В МИНУС
            // (display-inline-grid 0.08 -> 8.20, inline-z-axis-002/004) —
            // подложка в строчной сетке рвёт свою же краску.
            // `<body>` исключён: его родитель — корневой элемент, а тот
            // всегда образует КОРНЕВОЙ контекст наложения. Шаги 1-2
            // приложения E — собственные фон и рамка корня, шаг 3 —
            // отрицательный `z-index` ПОВЕРХ них, а не под всем окном; братьев
            // у `<body>` нет, уходить не подо что
            // (`root-element-creates-stacking-context`).
            // Родитель — СВОЙ контекст наложения (transform, opacity < 1,
            // позиционированный с z-index, isolation, filter; CSS 2.1 прил. E,
            // css-transforms-1 §transform-rendering): отрицательный z-index
            // ребёнка ложится под его содержимое, но ПОВЕРХ его фона — не под
            // весь документ (`individual-transform/stacking-context-00*`,
            // `transform-stacking-001`).
            if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
                && e.tag != "body"
                && !stacking_context(inherited)
            {
                done = crate::interact::Underlay::new(done).into_any_element();
            }
            // Абсолют с `z-index < 0`, оставленный на месте (`below_cb_axis`):
            // краска — шаг 3 корневого контекста, под потоком родителя, как и
            // у держателя на распорке выше.
            if below_cb_axis {
                done = crate::interact::Underlay::new(done).into_any_element();
            }
            // Позиционированный блок с `z-index: auto | 0` рисуется на шаге 8
            // приложения E CSS 2.1 — ПОСЛЕ блоков и строк потока, — а у нас
            // порядок краски был порядком детей: следующий блок закрашивал
            // сдвинутый `relative` (`position-relative-035`) и абсолют с одной
            // свободной осью (`right-offset-003`). `PaintLast` меняет только
            // краску — раскладка и место в потоке те же, поэтому строки он не
            // рвёт (запись про распорку выше): внутри строки элементы идут
            // через `pending`, а не сюда. Положительный `z-index` и `fixed`
            // уже отложены `layered`, отрицательный — подложка выше.
            let step8 = matches!(
                e.style.position,
                Some(crate::computed::Position::Relative)
                    | Some(crate::computed::Position::Absolute)
                    | Some(crate::computed::Position::Sticky)
            ) && e.style.z_index.unwrap_or(0) == 0
                && !matches!(e.tag.as_str(), "html" | "body")
                && paint_last_ok(e, &nodes[idx + 1..]);
            // Непозиционированный элемент с `opacity` < 1 красится на том же слое, что
            // позиционированные с `z-index: 0` (css-color-4 §opacity: «painted
            // on the same layer … as positioned elements with stacking order
            // 0»; Blink кладёт такой слой в список z-порядка с нулём): после
            // блоков и строк потока, в порядке разметки (`t32-opacity-zorder-c`).
            let step8 = step8
                || (e.style.position.is_none_or(|p| p == crate::computed::Position::Static)
                    // `z-index` у непозиционированного не действует
                    // (CSS 2.1 §9.9.1 «Applies to: positioned elements»).
                    && (e.style.z_index.unwrap_or(0) == 0
                        || !z_index_applies(&e.style, inherited))
                    // Только прозрачность: у `contain`/`will-change`/
                    // `transform` положительный `z-index` потомков держится
                    // на краске на месте (`contain-paint-stacking-context-*`).
                    && (e.style.opacity.is_some_and(|o| o < 1.0)
                        // css-transforms-1 §transform-rendering: a transformed
                        // box establishes a stacking context and is painted
                        // as a positioned `z-index: 0` layer (Blink puts it in
                        // the z-order list with 0): `perspective-zero` — a
                        // static transformed box after a `relative` one.
                        || e.style.transform.is_some()
                        || e.style.translate.is_some())
                    && !e.style.z_index.is_some_and(|z| z > 0 && z_index_applies(&e.style, inherited))
                    && !matches!(e.tag.as_str(), "html" | "body")
                    && paint_last_ok(e, &nodes[idx + 1..]));
            if step8 {
                done = gpui::PaintLast::new(done).key(paint_key).into_any_element();
            }
            out.push(done);
        }
    }
    if !pending.is_empty() {
        out.push(paint_inline_step7(letter_scope.paragraph(&pending, inherited, opts)));
    }
    // `text-box-trim` (css-inline-3 §4.2): у блочного контейнера срезается
    // блочно-начальная сторона ПЕРВОЙ отформатированной строки и
    // блочно-конечная — ПОСЛЕДНЕЙ. Выражается отрицательным полем на первом и
    // последнем ребёнке: коробка ужимается ровно на срез, а содержимое
    // остаётся на месте.
    //
    // Строку ищет `text_box_line_style` по css-pseudo-4: у контейнера с
    // блочным содержимым это первая строка ПЕРВОГО in-flow блочного ребёнка,
    // и если у того строки нет (пустой `<div>`, пустая анонимная коробка) —
    // срезать нечего (`half-leading-block-box-001/003`). «Intervening
    // non-zero padding or borders» — отступы и рамки ПОТОМКОВ между
    // контейнером и строкой (`-004/-005`), а не самого контейнера: его
    // собственный отступ срезу не мешает (`-006`). Метрики — от корневой
    // строчной коробки найденной строки, то есть от стиля её блока.
    if (inherited.text_box_trim_start || inherited.text_box_trim_end)
        && !out.is_empty()
        && !ordered_context
    {
        // Срез с одной стороны: полулидинг строки плюс расстояние от
        // подъёма/спуска до заданной метрики края (`text` — ноль, `cap`/`ex`
        // — остаток над прописной/строчной, `alphabetic` — весь спуск).
        let trim_for = |line_style: &Computed, start: bool| -> f32 {
            let size = match line_style.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            let family = line_style.font_family.clone().unwrap_or_default();
            let (ascent, descent, cap) = crate::metrics::vmetrics_px(&family, size);
            let line = match line_style.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
                _ => ascent + descent,
            };
            // Полулидинг — половина разницы между высотой строки и метрикой
            // содержимого (CSS 2.1 §10.8.1).
            let half = (line - (ascent + descent)) / 2.0;
            // Край — у КОРНЕВОЙ СТРОЧНОЙ КОРОБКИ найденной строки (css-inline-3
            // §text-box-trim: «to the specified metric of its root inline
            // box»): `text-box-edge` наследуемое, `inline::inherit` его несёт,
            // а явное `auto` на блоке строки перекрывает `ex` контейнера
            // (`not-ignore-nested-text-box-edge`; Blink `AdjustEdges`:
            // kAuto = kText).
            if start {
                let over = match line_style.text_box_over {
                    crate::computed::TextEdge::Cap => ascent - cap,
                    crate::computed::TextEdge::Ex => {
                        ascent - crate::metrics::ch_ex_px(&family, size).1
                    }
                    _ => 0.0,
                };
                half + over
            } else {
                let under = match line_style.text_box_under {
                    // Алфавитная линия может стоять НАД нулём глифа (BASE `romn`,
                    // `BaselineDiagnostic`: +50/1000 — `text-box-trim-end-002`).
                    crate::computed::TextEdge::Alphabetic => {
                        descent + crate::fonts::alphabetic_em(&family) * size
                    }
                    _ => 0.0,
                };
                half + under
            }
        };
        if inherited.text_box_trim_start
            && let Some(line_style) = text_box_line_style(nodes, inherited, true)
        {
            let trim = trim_for(&line_style, true);
            if trim > 0.0 {
                let first = out.remove(0);
                // Вертикальный блок кладёт детей рядом (`flex_row` у
                // `vertical-lr`, `flex_row_reverse` у `vertical-rl`): блок-старт —
                // левый или правый край, верхнее поле двигало строку ВДОЛЬ неё
                // (`text-box-trim-half-leading-block-box-002`).
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mt(px(-trim)),
                    (true, false) => div().ml(px(-trim)),
                    (true, true) => div().mr(px(-trim)),
                };
                out.insert(0, holder.child(first).into_any_element());
            }
        }
        if inherited.text_box_trim_end
            && let Some(line_style) = text_box_line_style(nodes, inherited, false)
        {
            let trim = trim_for(&line_style, false);
            if trim > 0.0 {
                let last = out.pop().expect("список не пуст");
                // Блок-конец вертикали: правый край у `vertical-lr`, левый у
                // `vertical-rl`.
                let holder = match (
                    inherited.vertical == Some(true),
                    inherited.vertical_rl == Some(true),
                ) {
                    (false, _) => div().mb(px(-trim)),
                    (true, false) => div().mr(px(-trim)),
                    (true, true) => div().ml(px(-trim)),
                };
                out.push(holder.child(last).into_any_element());
            }
        }
    }
    // Верхний слой: то, что обязано рисоваться поверх соседей, идёт последним
    // и возвращается на своё место замеренным сдвигом.
    out.extend(crate::interact::late_close());
    containment_paint::collect(out, inherited)
}
