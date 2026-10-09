//! Блочный поток: дети блока в элементы (blocks).

use crate::render::*;
pub(crate) mod flow;
pub(crate) mod positioned;
pub(crate) mod canvas;
pub(crate) use crate::render::blocks::flow::*;
pub(crate) use crate::render::blocks::positioned::*;
pub(crate) use crate::render::blocks::canvas::*;

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): разворачивать `text-emphasis` в поштучные
// руби (по знаку-аннотации над каждой буквой базы, кроме пробелов и
// пунктуации — css-text-decor-3 §5.3). Срез руби и акцентов, 167 пар:
// 125 -> 125, приобретено 6 (`text-emphasis-line-height-001a/002a/002b`,
// `-position-over-left-002`, `-position-under-left-002`, `-punctuation-3`),
// потеряно 6 — `-line-height-004a..d` 0.07 -> 0.7 и `-punctuation-1/2`
// 0.00 -> 6.31/3.12: поштучный атом меняет разбивку строки и подъём базовой
// линии, а эталоны семьи считают её по-своему. Возвращать вместе с
// настоящей надстрочной аннотацией (сдвиг базовой линии без атома).
pub(crate) fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    // Only the cell's own content list is the BFC root's (see `CELL_BFC`).
    let cell_bfc = CELL_BFC.with(|c| c.replace(false));
    let cb_prev = CB_WIDTH.get();
    if let Some(Len::Px(w)) = inherited.width
        && w > 0.0
    {
        CB_WIDTH.set(Some(w));
    }
    let _cb_guard = scopeguard_cb(cb_prev);
    // Доступная ширина блочных детей этого уровня (см. `AVAIL_W`).
    let avail_prev = AVAIL_W.get();
    AVAIL_W.set(available_width::inner(inherited, avail_prev));
    let _avail_guard = AvailWGuard(avail_prev);
    // `content-visibility: hidden`: содержимое пропускается целиком
    // (css-contain-2 §4) — коробка остаётся, детей нет.
    let stripped: Vec<Node>;
    let nodes = if nodes.iter().any(
        |n| matches!(n, Node::Element(e) if e.style.skip_content == Some(true) && !e.children.is_empty()),
    ) {
        stripped = nodes
            .iter()
            .map(|n| match n {
                Node::Element(e) if e.style.skip_content == Some(true) => {
                    let mut copy = e.clone();
                    copy.children.clear();
                    Node::Element(copy)
                }
                other => other.clone(),
            })
            .collect();
        &stripped
    } else {
        nodes
    };
    // `order` в CSS работает ТОЛЬКО внутри гибкого контейнера и сетки; в
    // обычном потоке он не значит ничего. Раньше сортировались дети любого
    // родителя — блоки меняли порядок там, где браузер их не трогает.
    // Барьер `fixed` — не только СВОЙ трансформ родителя, но и его
    // `contain: layout|paint` (css-contain-2 §3.2 п.5, §3.3: «establishes …
    // a fixed positioning containing block»). `transform_ancestor` родителя
    // несёт лишь ЕГО предков (`inline::inherit`), поэтому прямой ребёнок
    // обособленной коробки уходил в слой окна и садился в угол экрана
    // (`contain-layout-007`, `contain-paint-010`), а внук — нет
    // (`contain-*-containing-block-fixed-001` = 0.00). Список совпадает с
    // `fixed_cb_box` — расхождение он прямо запрещает.
    // Обещанное свойство, дающее блок для `fixed` (css-will-change-1 §2.1), —
    // тот же барьер, что `transform` самого родителя (`will-change-fixpos-cb-*`).
    let under_tf = inherited.transform_ancestor
        || inherited.transform.is_some()
        || inherited.contain_layout == Some(true)
        || inherited.contain_paint == Some(true)
        || inherited.will_change & crate::computed::wc::CB_FIXED != 0;
    let ordered_context = matches!(
        inherited.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            // Поток лунок — сеточный контекст: схлопывания отступов нет
            // (css-grid-3), и `order` действует.
            | Some(Display::GridLanes)
    );
    // Схлопывание вертикальных отступов есть ТОЛЬКО в обычном потоке: в
    // гибком контейнере и сетке CSS его запрещает, а мы схлопывали везде —
    // элементы ряда съезжали друг к другу против браузера.
    // Блок внутри строчного разрывает его на анонимные коробки (CSS 2.1
    // §9.2.1.1) — разбиение идёт ДО схлопывания полей: вынесенный блок
    // обязан схлопнуть свои поля с новыми соседями. В гибком контейнере и
    // сетке разрыва нет вовсе: там дети блокифицируются, и куски уехали бы
    // по чужим дорожкам.
    let split = if ordered_context {
        nodes.to_vec()
    } else {
        // Анонимная таблица вокруг ПРОГОНА табличных братьев (§17.2.1 шаг 3)
        // — до разбиения блока в строчном и до схлопывания полей, как это
        // делает и сборщик дерева в браузере.
        split_block_in_inline(&hoist_inset_abs(&wrap_anon_tables(nodes)))
    };
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v158, `scout-flex-2026-09e.md` патч №1):
    // снятие АВТОРСКОГО `align-self` у блока в обычном потоке здесь, в начале
    // `blocks()` — «до внутренних постановщиков». Полный свод против v35:
    // +3 (`align-self-013`, `flexbox-align-self-vert-001`, `-horiz-002`) /
    // −5 (`absolute-replaced-width-020` 0.00 → 3.84, `left-offset-003`
    // 0.00 → 0.96, `left-offset-percentage-001` 0.00 → 1.05,
    // `anchor-position-inline-005/-006`). Первая тройка — ровно та, что
    // названа в записи `inline.rs:1001`: место в конвейере не спасло,
    // замещаемому абсолюту `align_self` нужен ещё ДО `blocks()`. Возвращать
    // только с явным признаком «значение авторское» в `Computed`.
    // §10.3.3: у блока в потоке с `width: auto` боковое `auto`-поле
    // используется НУЛЁМ, а коробка занимает всю ширину. У нас блок — гибкая
    // колонка, и любое auto-поле на поперечной оси отменяет растяжение до
    // дорожки: абзац сжимался по содержимому и уезжал к краю.
    let split = if ordered_context {
        split
    } else {
        split
            .into_iter()
            .map(|n| match n {
                // `justify-self` блока в потоке (css-align-3 §6.1 «Block-Level
                // Boxes»): не-`normal`/`stretch` значение меряет коробку с
                // `width: auto` по содержимому (fit-content — обёртка
                // `content_sized`), auto-поля имеют приоритет над
                // выравниванием; без auto-полей выравнивание выражается ими же
                // (флекс-колонка блока их исполняет). `left`/`right` разбор
                // уже свёл к `start`/`end`; сторона — по письму родителя
                // (`justify-self-auto-margins-2`: `margin: auto` центрирует
                // 100 в 200). Таблица и замещаемый размер по содержимому уже
                // имеют — им только поля.
                Node::Element(mut e)
                    if in_flow(&e.style)
                        && !inline_level_box(&e)
                        && inherited.vertical != Some(true)
                        && matches!(
                            e.style.justify_self,
                            Some(Align::Start) | Some(Align::Center) | Some(Align::End)
                        ) =>
                {
                    let auto = |l: Option<Len>| l == Some(Len::Auto);
                    let table = e.tag == "table" || e.style.display == Some(Display::Table);
                    if matches!(e.style.width, None | Some(Len::Auto)) && !table && !replaced_tag(&e) {
                        e.style.width = Some(Len::FitContent);
                    }
                    if !auto(e.style.margin.left) && !auto(e.style.margin.right) {
                        let rtl = inherited.rtl == Some(true);
                        match (e.style.justify_self, rtl) {
                            (Some(Align::Center), _) => {
                                e.style.margin.left = Some(Len::Auto);
                                e.style.margin.right = Some(Len::Auto);
                            }
                            (Some(Align::End), false) | (Some(Align::Start), true) => {
                                e.style.margin.left = Some(Len::Auto);
                            }
                            _ => e.style.margin.right = Some(Len::Auto),
                        }
                    }
                    Node::Element(e)
                }
                Node::Element(mut e)
                    if in_flow(&e.style)
                        && matches!(e.style.width, None | Some(Len::Auto))
                        && (e.style.margin.left == Some(Len::Auto)
                            || e.style.margin.right == Some(Len::Auto)) =>
                {
                    if e.style.margin.left == Some(Len::Auto) {
                        e.style.margin.left = Some(Len::Px(0.0));
                    }
                    if e.style.margin.right == Some(Len::Auto) {
                        e.style.margin.right = Some(Len::Px(0.0));
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    let nodes: &[Node] = &split;
    let collapsed = if ordered_context {
        reorder(nodes.to_vec())
    } else {
        // Схлопывание идёт ДО наследования стилей, поэтому кегль уровня
        // передаётся отдельно: `margin: 1em` без своего `font-size` меряется
        // от родительского.
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let prev = COLLAPSE_FONT_PX.with(|c| c.replace(base));
        // Ширина содержащего блока для ПРОЦЕНТНЫХ полей (§8.3: проценты полей
        // считаются от ширины содержащего блока, схлопывание — по уже
        // разрешённым значениям). Известна только заданная в точках.
        let cb_w = match inherited.width {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let prev_w = COLLAPSE_CB_WIDTH_PX.with(|c| c.replace(cb_w));
        // Определённость высоты блока для ДОЛЕЙ высоты детей — тем же
        // предикатом, что у слитого стиля (`inline::inherit`): он зависит
        // только от родителя, а сырой стиль ребёнка признака ещё не несёт.
        let cb_h_def = inline::inherit(inherited, &Computed::default()).cb_height_def;
        let prev_h = COLLAPSE_CB_HEIGHT_DEF.with(|c| c.replace(cb_h_def));
        let mut out = collapse_margins(
            nodes,
            matches!(
                inherited.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ),
        );
        margin_height::zero_float_blocks(&mut out, inherited);
        COLLAPSE_FONT_PX.with(|c| c.set(prev));
        COLLAPSE_CB_WIDTH_PX.with(|c| c.set(prev_w));
        COLLAPSE_CB_HEIGHT_DEF.with(|c| c.set(prev_h));
        out
    };
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed: Vec<Node> = if ordered_context {
        collapsed
            .into_iter()
            // `visibility: collapse` на элементе гибкого контейнера убирает
            // его из строки, НО оставляет РАСПОРКУ (strut, css-flexbox §4.4):
            // поперечный размер и базовая линия ряда меряются как при нём
            // (flexbox-collapsed-item-baseline-001). Распорка — тот же
            // элемент с нулевой ГЛАВНОЙ осью и невидимой краской.
            .map(|n| match n {
                Node::Element(mut e) if e.style.collapsed == Some(true) => {
                    match inherited.flex_dir {
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                            e.style.height = Some(Len::Px(0.0));
                            e.style.max_height = Some(Len::Px(0.0));
                            e.style.min_height = Some(Len::Px(0.0));
                            e.style.margin.top = Some(Len::Px(0.0));
                            e.style.margin.bottom = Some(Len::Px(0.0));
                            e.style.padding.top = Some(Len::Px(0.0));
                            e.style.padding.bottom = Some(Len::Px(0.0));
                            e.style.border_width.top = Some(Len::Px(0.0));
                            e.style.border_width.bottom = Some(Len::Px(0.0));
                        }
                        // Распорка в главной оси — ноль ЦЕЛИКОМ: элемент «as
                        // if display:none» (css-flexbox-1 §4.4), значит и его
                        // поля, отбивки и рамки по главной оси соседей не
                        // раздвигают (`flexbox_visibility-collapse`: между
                        // соседями только их собственные поля).
                        _ => {
                            e.style.width = Some(Len::Px(0.0));
                            e.style.max_width = Some(Len::Px(0.0));
                            e.style.min_width = Some(Len::Px(0.0));
                            e.style.margin.left = Some(Len::Px(0.0));
                            e.style.margin.right = Some(Len::Px(0.0));
                            e.style.padding.left = Some(Len::Px(0.0));
                            e.style.padding.right = Some(Len::Px(0.0));
                            e.style.border_width.left = Some(Len::Px(0.0));
                            e.style.border_width.right = Some(Len::Px(0.0));
                        }
                    }
                    e.style.hidden = Some(true);
                    Node::Element(e)
                }
                other => other,
            })
            // ПРОБЕЛЬНЫЙ текст между детьми ряда/сетки не рождает анонимный
            // элемент (css-flexbox §4): переводы строк разметки давали
            // лишние 2-3px между коробками
            // (flexbox-baseline-align-self-baseline-horiz-001: тест дышит
            // щелями, эталон написан слитно).
            .filter(|n| !matches!(n, Node::Text(_)) || !is_blank(n))
            .map(|n| match n {
                Node::Element(mut e) => {
                    e.style.float = None;
                    e.style.clear = None;
                    e.style.vertical_align = None;
                    e.style.flex_item = matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    );
                    // Элемент КОЛОНКИ: определён ли главный размер
                    // контейнера (css-flexbox-1 §9.8 п.1). От этого зависит,
                    // определён ли блок у ЕГО детей — доля высоты внутри
                    // элемента колонки без высоты решается как `auto`
                    // (Blink `flex_layout_algorithm.cc`:
                    // `is_initial_block_size_indefinite`).
                    if matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    ) && matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    ) {
                        // Абсолют с ОБОИМИ вертикальными отступами и авто-высотой
                        // тоже определён: высота выходит из уравнения
                        // css-position-3 §4.1 (`top + height + bottom` = блок
                        // содержащего, а он у абсолюта всегда определён,
                        // css-sizing-3 §4.1 «definite»). Без этого колонка
                        // `position: absolute; top: 0; bottom: 0` считалась
                        // неопределённой, и основа-доля ребёнка снималась
                        // (`percentage-heights-002`: синяя полоса по содержимому,
                        // красный фон контейнера под ней).
                        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
                        let abs_both_insets = matches!(
                            inherited.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        ) && matches!(inherited.height, None | Some(Len::Auto))
                            && edge(inherited.inset.top)
                            && edge(inherited.inset.bottom);
                        let definite = matches!(inherited.height, Some(Len::Px(_)))
                            || (matches!(inherited.height, Some(Len::Pct(_)))
                                && inherited.cb_height_def)
                            || abs_both_insets
                            || inherited.stretched
                            || inherited.root_box;
                        e.style.flex_main_def = Some(definite);
                    }
                    // Доля высоты элемента РЯДА при неопределённой высоте
                    // контейнера ведёт себя как `auto` (CSS 2.1 §10.5;
                    // css-flexbox-1 §9.8: определённой поперечную ось делает
                    // лишь определённый размер контейнера), но вычисленное
                    // значение — не `auto`, поэтому `stretch` к ней не
                    // применяется и работает как `flex-start` (§9.4 п.11,
                    // css-align-3 §6.1). Прежде доля решалась от высоты
                    // строки (`stretch-requires-computed-auto-size`: красная
                    // коробка в полвысоты соседа).
                    if matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    ) && matches!(
                        inherited.flex_dir,
                        None | Some(FlexDir::Row) | Some(FlexDir::RowReverse)
                    ) && inherited.vertical != Some(true)
                        && e.style.vertical != Some(true)
                        && matches!(e.style.height, Some(Len::Pct(_)))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        let edge = |l: Option<Len>| l.is_some_and(|v| v != Len::Auto);
                        let cross_definite = matches!(inherited.height, Some(Len::Px(_)))
                            || (matches!(inherited.height, Some(Len::Pct(_)))
                                && inherited.cb_height_def)
                            || (matches!(
                                inherited.position,
                                Some(crate::computed::Position::Absolute)
                                    | Some(crate::computed::Position::Fixed)
                            ) && edge(inherited.inset.top)
                                && edge(inherited.inset.bottom))
                            || inherited.stretched
                            || inherited.root_box
                            || inherited.aspect_ratio.is_some();
                        if !cross_definite {
                            e.style.height = None;
                            let stretch = match e.style.align_self {
                                Some(a) => a == crate::computed::Align::Stretch,
                                None => matches!(
                                    inherited.align_items,
                                    None | Some(crate::computed::Align::Stretch)
                                ),
                            };
                            if stretch {
                                e.style.align_self = Some(crate::computed::Align::Start);
                            }
                        }
                    }
                    // ★ Эти три правила жили в ветке ОБЫЧНОГО потока (`else`
                    // ниже) с проверками на Flex/Grid-родителя — и были
                    // недостижимы по построению (скаут flexbox: пробы
                    // `flex2-colbasis-*` показали, что компенсация основы не
                    // действует). Их место — здесь, среди детей ряда/сетки.
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    // `<canvas>` в сетке: атрибуты `width/height` — природный
                    // размер и соотношение сторон, а не CSS-размер (HTML
                    // §4.12.5, §15.3.10). Ось, растянутая выравниванием
                    // (`stretch`, css-align-3 §6.1) или переносимая из
                    // заданной автором другой оси (css-sizing-4 «transferred
                    // size»), становится `auto`, соотношение — на коробку
                    // (`replaced-element-011`, `grid-item-inline-contribution-*`,
                    // `replaced-alignment-with-aspect-ratio-001`).
                    if e.tag == "canvas"
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && (e.style.attr_sized.0 || e.style.attr_sized.1)
                    {
                        let stretch = |own: Option<Align>, items: Option<Align>| {
                            own == Some(Align::Stretch)
                                || (own.is_none() && items == Some(Align::Stretch))
                        };
                        let sx = stretch(e.style.justify_self, inherited.justify_items);
                        let sy = stretch(e.style.align_self, inherited.align_items);
                        // Обе оси пришли из атрибутов, а явный `stretch` — ровно
                        // у одной: вторая ось тоже `auto` и выводится из
                        // растянутой через соотношение (css-sizing-4 «transferred
                        // size»; Blink `length_utils.cc` — `kStretchExplicit` по
                        // блочной оси включает соотношение для строчной). Раньше
                        // она оставалась атрибутом, и taffy выводил из неё
                        // растянутую: 10×10 вместо 100×100
                        // (`replaced-alignment-with-aspect-ratio-001/002`).
                        // Вертикальную сетку не трогаем: оси там переставлены.
                        let both_attrs = e.style.attr_sized.0 && e.style.attr_sized.1;
                        let transfer = both_attrs && sx != sy && inherited.vertical != Some(true);
                        let free_x = e.style.attr_sized.0
                            && (sx || !e.style.attr_sized.1 || transfer);
                        let free_y = e.style.attr_sized.1
                            && (sy || !e.style.attr_sized.0 || transfer);
                        if free_x || free_y {
                            // Явный `stretch` по ОБЕИМ осям задаёт обе стороны
                            // растяжением — соотношение не действует
                            // (`-003.tentative`: 10×20 в области 100×100 давал
                            // 100×200; то же правило — `grid-aspect-ratio-032/033`).
                            if let (Some(Len::Px(w)), Some(Len::Px(h))) =
                                (e.style.attr_width, e.style.attr_height)
                                && h > 0.0
                                && e.style.aspect_ratio.is_none()
                                && !(both_attrs && sx && sy)
                            {
                                e.style.aspect_ratio = Some(w / h);
                            }
                            // Нерастянутая ось при `normal` у коробки с
                            // соотношением — `start` (css-grid-2 §6.6.1), иначе
                            // taffy растянет её сам (`alignment.rs:122-128`: без
                            // заданной ширины умолчание — `Stretch`) и выведет
                            // растянутую из неё. Авторское значение не трогаем.
                            if transfer {
                                if sy
                                    && e.style.justify_self.is_none()
                                    && inherited.justify_items.is_none()
                                {
                                    e.style.justify_self = Some(Align::Start);
                                }
                                if sx
                                    && e.style.align_self.is_none()
                                    && inherited.align_items.is_none()
                                {
                                    e.style.align_self = Some(Align::Start);
                                }
                            }
                            if free_x {
                                e.style.width = None;
                            }
                            if free_y {
                                e.style.height = None;
                            }
                        }
                    }
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    e.style.flex_item_ratio = ratio_ok
                        && !positioned_out
                        && matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex));
                    // `flex-basis` задаёт размер СОДЕРЖИМОГО (css-flexbox-1 §7.2.3:
                    // «flex-basis determines the size of the content box, unless
                    // otherwise specified such as by box-sizing»), а в раскладку
                    // уходит внешний размер — как `width`/`height` в `apply`, основа
                    // получает отбивку и рамку по ГЛАВНОЙ оси родителя
                    // (`flexbox-mbp-horiz-*`, `flexbox-justify-content-horiz-002`).
                    if matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex))
                        && inherited.vertical.is_none()
                        && e.style.border_box != Some(true)
                        && let Some(Len::Px(b)) = e.style.flex_basis
                    {
                        let px_of = |l: Option<Len>| match l {
                            Some(Len::Px(v)) => v,
                            _ => 0.0,
                        };
                        let bd = e.style.borders();
                        let row = matches!(
                            inherited.flex_dir,
                            None
                                | Some(crate::computed::FlexDir::Row)
                                | Some(crate::computed::FlexDir::RowReverse)
                        );
                        let extra = if row {
                            px_of(e.style.padding.left)
                                + px_of(e.style.padding.right)
                                + px_of(bd.left)
                                + px_of(bd.right)
                        } else {
                            px_of(e.style.padding.top)
                                + px_of(e.style.padding.bottom)
                                + px_of(bd.top)
                                + px_of(bd.bottom)
                        };
                        e.style.flex_basis = Some(Len::Px(b + extra));
                    }
                    // Элемент сетки с `aspect-ratio` при `normal` (css-grid-2
                    // §6.6.1): «sized consistent with the size calculation
                    // rules for block-level elements» — строчная ось заполняет
                    // область (как stretch), а БЛОЧНАЯ идёт из соотношения, не
                    // растягиваясь на ряд: там `start`
                    // (`grid-aspect-ratio-001/007/010/038`). ★ ЗАМЕРЕНО: `start`
                    // и по строчной оси — `grid-aspect-ratio-018/038` в красное.
                    if ratio_ok
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && inherited.vertical.is_none()
                    {
                        let auto_w = matches!(e.style.width, None | Some(Len::Auto));
                        let auto_h = matches!(e.style.height, None | Some(Len::Auto));
                        // Обе оси auto: строчная заполняет область, блочная — из
                        // соотношения. Блочная определена: строчная — из
                        // соотношения (CSS2 §10.3.2 для замещаемого с
                        // соотношением; css-sizing-4 §5.1).
                        if auto_w
                            && !auto_h
                            && e.style.justify_self.is_none()
                            && inherited.justify_items != Some(Align::Stretch)
                        {
                            e.style.justify_self = Some(Align::Start);
                        }
                        if auto_h
                            && e.style.align_self.is_none()
                            && inherited.align_items != Some(Align::Stretch)
                        {
                            e.style.align_self = Some(Align::Start);
                        }
                    }
                    // `flex-basis: content` — основа по содержимому, и
                    // заданный ГЛАВНЫЙ размер при ней не действует. Какая ось
                    // главная, знает только родитель, поэтому размер снимается
                    // здесь, а не в стиле самого элемента.
                    // У ЗАМЕЩАЕМОГО элемента содержимое — он сам, и его
                    // размер задаёт собственный пиксель или атрибут: снимать
                    // его нельзя, иначе `<canvas width=20>` схлопывается в
                    // ноль (`flexbox-flex-basis-content-001a`).
                    let own = matches!(
                        e.tag.as_str(),
                        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "svg"
                    );
                    if e.style.basis_content == Some(true) {
                        match inherited.flex_dir {
                            Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                                e.style.height = own.then_some(e.style.attr_height).flatten();
                            }
                            _ => e.style.width = own.then_some(e.style.attr_width).flatten(),
                        }
                    }
                    // Основа-ДОЛЯ у элемента колонки, чей контейнер не определён
                    // по главной оси, — это `content` (css-flexbox-1 §7.2.3: «if
                    // that containing block's size is indefinite, the used value
                    // for flex-basis is content»; Blink `IsItemFlexBasisDefinite`).
                    // Taffy не решает долю и падает на заданную высоту
                    // (`flexbox.rs` `flex_basis.or(main_size)`): `flex: 0 0 0%;
                    // height: 500px` давал 500 вместо содержимого 100
                    // (`flex-basis-010`), а `flex: 1 1; height: 100px` делал
                    // блок детей определённым, и `height: 100%` ребёнка
                    // закрашивал красное (`percentage-heights-017/018`).
                    if e.style.flex_main_def == Some(false)
                        && matches!(e.style.flex_basis, Some(Len::Pct(_)))
                    {
                        e.style.flex_basis = None;
                        e.style.height = own.then_some(e.style.attr_height).flatten();
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
    };
    let flex_ctx = matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex));
    let letter_scope = first_letter_scope::Scope::new(&collapsed, inherited);
    // Буквица `initial-letter` расшивается в плавающий узел ДО обтекания —
    // дальше её ведёт `wrap_floats` наравне с авторскими флоатами. В гибком
    // контейнере и сетке `::first-letter` не действует — там не трогаем.
    let collapsed = if ordered_context {
        collapsed
    } else {
        initial_letter_float(
            first_letter_descendants::route(
                first_line_descendants::route(collapsed, inherited),
                inherited,
            ),
            inherited,
            opts,
        )
    };
    // §8.3.1: поле первого ребёнка примыкает к верхнему полю содержащего
    // блока, только если того не отделяют ни рамка, ни отбивка и он не
    // заводит своего контекста форматирования.
    let cb_top_open = !own_context_style(inherited)
        && zero_len(inherited.padding.top)
        && zero_len(inherited.borders().top);
    // Измеряемый бандовый хост (`band_flow.rs`) — только в БЛОЧНОМ контейнере
    // горизонтального письма: в гибком и сетке `float` не
    // действует (css-flexbox-1 §3, css-grid-1 §6.1).
    // Хост работает и в вертикальном письме (шаг F10): план в
    // логических осях, перевод в физику при сборке (`band_flow::VERT`).
    // Horizontal float sides are physical (CSS 2.1 §9.5.1); paragraphs
    // handle RTL within those bands. Vertical RTL still needs axis conversion.
    let vert_host = inherited.vertical == Some(true)
        && inherited.sideways != Some(true);
    // Вне хоста и там, где у раскладки свой счёт строк и разрывов: под
    // `line-clamp` (точка среза считает строки и флоаты за ней —
    // `line-clamp-with-floats-003/004`, `webkit-line-clamp-025`; отложенный
    // ряд `float_flow` для этого и заведён) и на печатных листах (монолитная
    // коробка хоста не режется между страницами —
    // `monolithic-overflow-020-print`).
    let measured_ok = !flex_ctx
        && inherited.line_clamp.is_none()
        && crate::interact::clamp_context().is_none()
        && !PAGED.with(std::cell::Cell::get)
        && !matches!(
            inherited.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        )
        && (inherited.vertical != Some(true) || vert_host)
        && (inherited.vertical_rl != Some(true) || vert_host)
        && (inherited.rtl != Some(true) || inherited.vertical != Some(true));
    let _fl_guard = BandFlGuard(BAND_FL.with(|f| f.replace(inherited.first_line.as_deref().cloned())));
    let _cbh_guard = BandCbhGuard(BAND_CBH.with(|h| {
        h.replace(match inherited.height {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        })
    }));
    let _cbw_guard = BandCbwGuard(BAND_CBW.with(|w| {
        w.replace(match inherited.width {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        })
    }));
    let _wm_guard = BandWmGuard(BAND_WM.with(|w| {
        w.replace(match (inherited.vertical, inherited.vertical_rl) {
            (Some(true), Some(true)) => 1,
            (Some(true), _) => 2,
            _ => 0,
        })
    }));
    let collapsed = replaced_used_style::inline_nodes(collapsed, AVAIL_W.get());
    let collapsed = by_layer(
        wrap_floats(
            collapsed,
            inherited,
            cb_top_open,
            match inherited.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            },
            measured_ok,
            own_context_style(inherited) || inherited.flex_item,
            cell_bfc,
        ),
        flex_ctx,
    );
    // Блок мы изображаем гибкой колонкой, а её дети по умолчанию сжимаются —
    // в обычном потоке этого нет: ребёнок выше родителя обязан вылезти, а не
    // ужаться. Поэтому в потоке сжатие детям выключается, если разметка не
    // просила обратного.
    let flex_context = matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let collapsed: Vec<Node> = if ordered_context {
        // Элемент гибкого контейнера сжимается по умолчанию — это его
        // начальное значение в CSS. Проставляем его явно, потому что
        // `display: inline-block` в другом месте выключает сжатие: строчная
        // коробка В СТРОКЕ и правда не жмётся, а тот же элемент В РЯДУ —
        // обязан. Без этого ряд из `<span>`-ов держал свою ширину и не
        // ужимался до минимального размера содержимого.
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) if flex_context => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(1.0);
                    }
                    // `vertical-align` на элементе гибкого контейнера не
                    // действует (css-flexbox-1 §4): он выравнивается своими
                    // свойствами, а не как кусок строки.
                    e.style.vertical_shift = None;
                    e.style.vertical_shift_px = None;
                    // Элемент ряда под обособлением строчной оси: главный
                    // размер берётся из `contain-intrinsic-size`, а не от
                    // содержимого. В колонке главная ось блочная — её уже
                    // держит подмена высоты.
                    let row = !matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    );
                    if row
                        && e.style.contains_width()
                        && matches!(e.style.width, None | Some(Len::Auto))
                    {
                        e.style.width = Some(Len::Px(e.style.contain_intrinsic.0.unwrap_or(0.0)));
                    }
                    // Поперечный размер элемента ряда с `height: auto` при
                    // растяжке даёт строка (css-flexbox-1 §9.4 п.11), и
                    // обособление высоты обязано ей уступить (`apply_box`).
                    // `auto`-поле по поперечной оси растяжку отменяет.
                    let stretches = e.style.align_self_normal
                        || match e.style.align_self {
                            Some(Align::Stretch) => true,
                            None => matches!(inherited.align_items, None | Some(Align::Stretch)),
                            _ => false,
                        };
                    if row
                        && inherited.vertical != Some(true)
                        && stretches
                        && e.style.contains_height()
                        && matches!(e.style.height, None | Some(Len::Auto))
                        && e.style.margin.top != Some(Len::Auto)
                        && e.style.margin.bottom != Some(Len::Auto)
                    {
                        e.style.cross_stretched = true;
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(0.0);
                    }
                    // rtl: переполняющий блок с ЗАДАННОЙ шириной прижат к
                    // правому краю и вылезает влево (csswg-drafts#5572);
                    // только горизонтальное письмо — в вертикали cross-ось
                    // иная (abs-pos-border-offset-001/002).
                    if inherited.rtl == Some(true)
                        && inherited.vertical_rl.is_none()
                        && e.style.width.is_some()
                        && e.style.align_self.is_none()
                        // Блочный по ВЫЧИСЛЕННОМУ `display`, а не по тегу:
                        // `span { display: block; width: … }` в rtl-блоке —
                        // тоже блок (замер 1393 пар с rtl/картинками: +0/−0,
                        // `block-in-inline-margins-002a/b` 0.12 -> 0.00).
                        // Эталон `flexbox-writing-mode-013-ref` держит
                        // слева другое (не этот путь).
                        && (!e.inline
                            || matches!(
                                e.style.display,
                                Some(Display::Block)
                                    | Some(Display::ListItem)
                                    | Some(Display::Flex)
                                    | Some(Display::Grid)
                                    | Some(Display::Table)
                            ))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `sideways-lr` — единственное письмо, где строчная ось
                    // идёт СНИЗУ ВВЕРХ: таблица Abstract-Physical Mapping
                    // (css-writing-modes-4, Overview.bs:1795-1830) даёт ему
                    // `line-left` = НИЗ, всем прочим вертикальным — верх.
                    // Значит начало строчной оси содержащего блока — его
                    // нижний край, и ребёнок с ОПРЕДЕЛЁННЫМ поперечным
                    // (физически вертикальным) размером стоит там:
                    // `body { height: 9em }` под корнем `sideways-lr` прижат
                    // к низу окна (`block-flow-direction-043-ref`: стол
                    // y 412…591 из 600), квадрат `height: 100px` — в НИЖНЕМ
                    // левом углу (`wm-propagation-body-035-ref`: y 492…592).
                    // Растянутого ребёнка правило не касается: `align-self`
                    // без определённого поперечного размера снимает растяжку.
                    // ★ ЗАМЕРЕНО: срез всего вертикального письма
                    // (`target/L-wm-wide.txt`, 1798 пар) 1335 -> 1354,
                    // +20/−1. Единственная потеря — `abs-pos-border-
                    // offset-002` 0.45 -> 2.22: там 68 коробок всех
                    // сочетаний письма и направления, и порог она
                    // держала не правотой, а усреднением; статическое
                    // место абсолюта при `sideways-lr` остаётся долгом
                    // корня WM-OVERCONSTRAINED-AXIS.
                    if inherited.vertical == Some(true)
                        && inherited.sideways == Some(true)
                        && inherited.vertical_rl != Some(true)
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Pct(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // `vertical-lr`/`vertical-rl`/`sideways-rl` при
                    // `direction: rtl`: строчная ось идёт СНИЗУ вверх —
                    // inline-start содержащего блока у НИЖНЕГО края
                    // (css-writing-modes-4 §6.4: line-right = низ, rtl
                    // ставит start на line-right). Переполненная по строчной
                    // оси коробка стоит у inline-start и вылезает к inline-end
                    // (CSS 2.2 §10.3.3 в логических осях, §7.1) — то есть
                    // низом к низу и ВВЕРХ. Без правила тело `height: 100vh`
                    // с рамками под корнем `vertical-lr; direction: rtl`
                    // лежало от верха, и красная верхняя рамка оставалась в
                    // окне (`contain-{body,html}-t-o-*` — ломался и эталон).
                    // `sideways-lr` исключён: у него line-left = низ, и при rtl
                    // start — ВЕРХ (правило выше его не касается rtl).
                    if inherited.vertical == Some(true)
                        && inherited.rtl == Some(true)
                        && !(inherited.sideways == Some(true) && inherited.vertical_rl != Some(true))
                        && matches!(
                            e.style.height,
                            Some(Len::Px(_))
                                | Some(Len::Em(_))
                                | Some(Len::Pct(_))
                                | Some(Len::Vh(_))
                                | Some(Len::Vw(_))
                        )
                        && e.style.align_self.is_none()
                        && !e.inline
                        && matches!(e.style.display, None | Some(Display::Block))
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // Коробка с `aspect-ratio` при auto-ширине и определённой
                    // высоте — fit-content, а не растяжка (css-sizing-4 §5.1:
                    // «automatic sizes are calculated the same as for a replaced
                    // element with a natural aspect ratio»; Blink length_utils
                    // `may_apply_aspect_ratio` → FitContent). Блок у нас —
                    // колонка flex, и `stretch` тянул ширину на всю строку
                    // (`block-aspect-ratio-002/006/…`).
                    // Нулевое и бесконечное отношение — как `auto`
                    // (css-sizing-4 §5.1; `zero-or-infinity-002`).
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    if ratio_ok
                        && !ordered_context
                        && matches!(e.style.width, None | Some(Len::Auto))
                        // Доля высоты от блока с высотой в точках — тоже
                        // определённая высота (CSS 2.1 §10.5), и ширина
                        // идёт из соотношения, а не растяжкой
                        // (`percentage-resolution-005`: 50×100 вместо 100×100).
                        && (matches!(e.style.height, Some(Len::Px(_)))
                            || (matches!(e.style.height, Some(Len::Pct(_)))
                                && matches!(inherited.height, Some(Len::Px(_)))))
                        && e.style.align_self.is_none()
                        && inherited.vertical.is_none()
                        && !e.inline
                        && !positioned_out
                    {
                        e.style.align_self = Some(Align::Start);
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    // §9.9 шаг 8: позиционированная коробка рисуется ПОВЕРХ содержимого
    // потока, а порядок краски у нас — порядок детей. В обычном потоке это
    // делает верхний слой, но в сетке и гибком контейнере он выключен
    // (`!ordered_context` у гейтов ниже), и абсолютный ребёнок оказывался под
    // соседями. Здесь его достаточно переставить в конец: место он берёт не
    // из потока (в раскладку сетки такой ребёнок не входит), поэтому
    // перестановка меняет только краску.
    // Вынесенный абсолют всё же РВЁТ прогон текста: каждая непрерывная
    // последовательность текстовых детей — свой анонимный элемент
    // (css-flexbox-1 §4, css-grid-2 §6.1), а абсолютный ребёнок в неё не
    // входит. После перестановки куски «Two » и «lines» оказывались соседями
    // и склеивались в один абзац (`anonymous-flex-item-004`,
    // `anonymous-grid-item-001`). `run_breaks` — индексы в новом списке, перед
    // которыми накопленный абзац закрывается.
    let mut run_breaks: Vec<usize> = vec![];
    let collapsed: Vec<Node> = if ordered_context {
        let in_flow = |n: &Node| match n {
            Node::Element(e) => {
                e.style.position != Some(crate::computed::Position::Absolute)
                    || e.style.z_index.is_some_and(|z| z < 0)
            }
            Node::Text(_) => true,
        };
        let mut flow: Vec<Node> = vec![];
        let mut over: Vec<Node> = vec![];
        for n in collapsed {
            if in_flow(&n) {
                flow.push(n);
            } else {
                if run_breaks.last() != Some(&flow.len()) {
                    run_breaks.push(flow.len());
                }
                over.push(n);
            }
        }
        flow.into_iter().chain(over).collect()
    } else {
        collapsed
    };
    let mut out = vec![];
    // Порядок краски подслоя (§9.9 шаг 3): соседние распорки отрицательного
    // `z-index` стоят в порядке разметки, а рисоваться обязаны по z. Высота у
    // них нулевая и y общий, поэтому перестановка СОСЕДЕЙ раскладку не меняет
    // — в отличие от перестановки в общем списке детей, замеренной в минус
    // (см. `movable`). Прогон рвётся сам, как только между распорками встаёт
    // что-то ещё.
    let below_run_start = 0usize;
    let below_run_end = usize::MAX;
    let below_zs: Vec<i32> = vec![];
    // Липкому ребёнку нужны две вещи, которых он сам не видит: коробка
    // родителя и видимая часть ленты. Их снимает распорка — она идёт первой,
    // потому что готовит замер до отрисовки детей.
    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => e.style.position == Some(crate::computed::Position::Sticky),
        _ => false,
    });
    let frame: crate::interact::StickyCell = Default::default();
    if sticky {
        out.push(sticky_probe(frame.clone()));
    }
    let pending: Vec<Node> = vec![];
    // Слой верхней отрисовки этого контейнера: позиционированные элементы
    // складывают сюда содержимое, а забирается оно последними детьми.
    crate::interact::late_open();
    let nodes = collapsed.as_slice();
    blocks_flow(
        nodes,
        run_breaks,
        pending,
        out,
        letter_scope,
        inherited,
        opts,
        ordered_context,
        under_tf,
        frame,
        below_run_end,
        below_run_start,
        below_zs,
    )
}
