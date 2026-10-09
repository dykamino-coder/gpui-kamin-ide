//! Узел-элемент в элемент GPUI: element().

use crate::render::*;

/// Блочный элемент.
pub(crate) fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    // Высота ряда от внешней колонки — только ЭТОМУ элементу (`flow::OUTER_ROW`).
    let outer_row = crate::flow::take_outer_row();
    let mut merged = inline::inherit(inherited, &e.style);
    list_item::inherited_style(e, inherited, &mut merged);
    // Якорный шаг: ключи реестров кадра (свой `node_id` для содержащего
    // блока детей, порядок сборки, ключ клетки) и размеры от якоря —
    // `anchor-size()`, растяжка в клетке `position-area` — из реестра
    // ПРОШЛОГО кадра, до раскладки.
    merged.self_node = e.node_id;
    merged.anchor_seq = crate::anchor::next_seq();
    merged.anchor_key = crate::anchor::key_of(e);
    // Вариант `position-try-fallbacks`, выбранный на прошлом кадре, — в стиль
    // ДО размеров и раскладки (§fallback: «the element keeps those styles»).
    crate::anchor::apply_chosen(&mut merged);
    crate::anchor::resolve_sizes(&mut merged, inherited);
    // `dir="auto"` — сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку содержимого.
    // Разбор двунаправленности выберет её сам при наборе, но выключка и
    // прижим текста читают `rtl` из стиля, и без этого шага блок с арабским
    // текстом прижимался влево.
    if e.attr("dir") == Some("auto") && e.style.rtl.is_none() {
        let mut text = String::new();
        gather_text(&e.children, &mut text);
        let strong = text.chars().find_map(|ch| {
            use unicode_bidi::BidiClass::*;
            match unicode_bidi::bidi_class(ch) {
                L => Some(false),
                R | AL => Some(true),
                _ => None,
            }
        });
        if let Some(rtl) = strong {
            merged.rtl = Some(rtl);
        }
    }
    // Предел ОРТОГОНАЛЬНОГО потока для строк вертикального письма внутри
    // (CSS Writing Modes §7.3). Искать его надо вверх по дереву, поэтому он
    // несётся вниз наследуемым полем. Ближе всего собственная определённая
    // высота элемента; за ней — высота ближайшего контейнера прокрутки с
    // наложенным на неё `max-height`; в самом конце — окно (его подставляет
    // потребитель в `paragraph`).
    {
        // `height: 5em` доживает сюда неразрешённым — доля считается от
        // кегля самого блока (outline-inline-vlr-006: предел колонки 5em).
        let em_base = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): переводить сюда и единицы шрифта
        // (`max-height: 8ch`), чтобы предел ортогонального потока не терялся.
        // Срез вертикального письма 1086 пар: приобретено 0, потеряно 7 —
        // `available-size-003…018` (0.05-0.11 -> «красное видно»). Тесты
        // прямо пишут: «**max**-height does not give the element a definite
        // block size» (§7.3.1 берёт предел у ОПРЕДЕЛЁННОГО размера, а
        // `max-height` определённым не делает). Значит и нынешний перевод
        // `max-height` в точках — тоже неверный источник предела.
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * em_base),
            _ => None,
        };
        // `box-sizing: border-box`: заданные высота и её пределы — это
        // РАМОЧНАЯ коробка, а предел строк — размер СОДЕРЖИМОГО (§7.3.1
        // «inner size»; Blink переводит в content-box в
        // `SetOrthogonalFallbackInlineSize`). Атомный путь это уже делает
        // (`edges` при `border_box` выше), блочный — нет: эталоны
        // `sizing-orthog-vlr-in-htb-007`, `vrl-in-htb-007/010`
        // (`box-sizing: border-box; height: 400px`, рамка 3) переносили на
        // 400, тест после вычета рамок в `aaa7d8d` — на 394.
        let own_edges = if e.style.border_box == Some(true) {
            let b = e.style.borders();
            px_of(b.top).unwrap_or(0.0)
                + px_of(b.bottom).unwrap_or(0.0)
                + px_of(e.style.padding.top).unwrap_or(0.0)
                + px_of(e.style.padding.bottom).unwrap_or(0.0)
        } else {
            0.0
        };
        let content = |v: f32| (v - own_edges).max(0.0);
        let h = px_of(e.style.height).map(content);
        let min_h = px_of(e.style.min_height).map(content);
        let max_h = px_of(e.style.max_height).map(content);
        // Предел, поставленный СВОЕЙ высотой, уже содержимый (content-box
        // по умолчанию, border-box переведён выше) — вычитать из него нечего. Вычет нужен
        // только УНАСЛЕДОВАННОМУ пределу, см. хунк ниже.
        // Доля высоты — от высоты СОДЕРЖАЩЕГО блока, если та определённая
        // (CSS 2.1 §10.5: «calculated with respect to the height of the
        // generated box's containing block»). `px_of` её не понимал, и
        // ортогональный `height: 50%` в контейнере 400px переносил строки по
        // унаследованному пределу 394, а не по своим 200
        // (`sizing-orthog-prct-vlr-in-htb-004`: ~7 колонок вместо ~13).
        // Только для потокового ребёнка БЛОЧНОГО родителя: у абсолюта база —
        // его содержащий блок, у элемента сетки — область, у гибкого — своя
        // развязка.
        let h = h.or(match (e.style.height, inherited.height) {
            (Some(Len::Pct(k)), Some(Len::Px(ph)))
                if in_flow(&e.style)
                    && matches!(inherited.display, None | Some(Display::Block)) =>
            {
                Some(k * ph)
            }
            _ => None,
        });
        let mut own_limit = false;
        if h.is_some() || min_h.is_some() || max_h.is_some() {
            // Клэмп как у CSS-высоты: max режет, min ПЕРЕБИВАЕТ max; без
            // своей высоты базой служит НАЧАЛЬНЫЙ содержащий блок, и он же —
            // общий потолок («larger than ICB» не расширяет место).
            let mut avail = h.unwrap_or(opts.viewport.1);
            if let Some(m) = max_h {
                avail = avail.min(m);
            }
            if let Some(m) = min_h {
                avail = avail.max(m);
            }
            avail = avail.min(opts.viewport.1);
            // У блока без вертикали предел ставится детям всегда; у самого
            // вертикального — только если родитель не дал своего
            // (table-cell-002: max-height ячейки).
            // Своя ОПРЕДЕЛЁННАЯ высота вертикального блока — это его строчный
            // размер, и перенос решает она, а не предел предка: запасной
            // предел §7.3.1 нужен лишь там, где места не задано. Вето
            // «предок уже дал предел» писалось под `max-height` ячейки
            // (table-cell-002), а `max-height` определённого размера не даёт
            // — поэтому вето сужено до случая без своей `height`.
            if e.style.vertical != Some(true) || merged.ortho_limit.is_none() || h.is_some() {
                merged.ortho_limit = Some(avail);
                own_limit = true;
            }
        }
        // Свои рамки и отбивки вдоль СТРОЧНОЙ оси (при вертикальном письме —
        // физически верх и низ) съедают предел, который блок передаёт детям:
        // §7.3.1 берёт запасной предел от ВНУТРЕННЕГО размера содержащего
        // блока («the containing block's **inner** max size»,
        // css-writing-modes-4 Overview.bs:2141), то есть от content-box.
        // Blink делает тот же вычет явно — `space_utils.cc:59-72`
        // `SetOrthogonalFallbackInlineSize`, комментарий «Calculate the
        // content-box size»; он берёт предел у НЕПОСРЕДСТВЕННОГО родителя, а
        // у нас предел несётся вниз наследуемым полем (`inline.rs:1012`),
        // поэтому вычет обязан идти на КАЖДОМ уровне.
        // Видно это только у `sideways-lr`: там строка начинается у
        // ПРОТИВОПОЛОЖНОГО края коробки (`VerticalText::ccw`), и лишняя
        // высота уводит весь рисунок; у письма по часовой она свисает
        // пустым хвостом (`block-flow-direction-vlr-010`, `vrl-009`,
        // `srl-049` зелены при том же дефекте).
        if !own_limit
            && merged.vertical == Some(true)
            && let Some(l) = merged.ortho_limit
        {
            let b = e.style.borders();
            let edges = px_of(b.top).unwrap_or(0.0)
                + px_of(b.bottom).unwrap_or(0.0)
                + px_of(e.style.padding.top).unwrap_or(0.0)
                + px_of(e.style.padding.bottom).unwrap_or(0.0);
            if edges > 0.0 {
                merged.ortho_limit = Some((l - edges).max(0.0));
            }
        }
    }
    pseudo_line_layers::install(e, &mut merged);
    // Единицы окна разрешаются здесь: размер окна знает только сборщик.
    merged.resolve_viewport(opts.viewport);
    orthogonal_inline::resolve(&mut merged, inherited, opts.viewport, e.tag == "html");
    PAINT_VIEWPORT.with(|v| v.set(opts.viewport));
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::forms::element(e, &merged, opts) {
        return transformed(el, &merged, inherited);
    }
    // Рамка строится ОДИН раз до match: прежний `is_some() => unwrap()`
    // читал файл с диска и разбирал вложенный документ дважды за кадр.
    // Неразобранная рамка по-прежнему падает в общий рукав.
    let mut built_iframe = if e.tag == "iframe" {
        iframe(e, opts)
    } else if e.tag == "object" && object_is_document(e) {
        // `<object data="….html">` — тот же вложенный документ, что и рамка
        // (HTML §4.8.7): адрес переносится из `data` в `src`, размеры берутся
        // уже разрешёнными (`10em` в собственном стиле — ещё `Len::Em`).
        let mut copy = e.clone();
        let url = e.attr("data").unwrap_or_default().to_string();
        copy.attrs.push(("src".to_string(), url));
        copy.style.width = merged.width;
        copy.style.height = merged.height;
        iframe(&copy, opts)
    } else {
        None
    };
    match e.tag.as_str() {
        // CSS Lists 3 §2: a block list item generates its own marker even
        // when its parent is an ordinary block rather than a list container.
        _ if (e.style.display == Some(Display::ListItem)
            || (e.tag == "li" && e.style.display.is_none()))
            && !matches!(
                e.tag.as_str(),
                "img" | "svg" | "embed" | "object" | "video" | "canvas" | "iframe"
            ) =>
        {
            list_item::render_with_style(e, inherited, &merged, opts)
        }
        // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4): слитый стиль
        // его уже несёт, а копия для замещаемой коробки — нет. Без переноса
        // блочная картинка под `body { image-orientation: none }` всё равно
        // разворачивалась по метке EXIF.
        "img" => {
            // Единицы шрифта в размерах БЛОЧНОЙ картинки разрешаются так же,
            // как у строчной (рукав `"img"` в `atom`): `image()` = `image_with(e,
            // None)` в собственном стиле держал `Len::Em`, и `height: 1em` при
            // `font-size: 3.75em` пропадал — картинка оставалась 15×15
            // (`c43-rpl-bbx-002`). CSS 2.1 §4.3.2: `em` — вычисленный кегль
            // САМОГО элемента, его и даёт `resolve_em` от кегля родителя.
            let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
            if inline_level(e) {
                replaced_used_style::inline_percentage_width(&mut copy.style, AVAIL_W.get());
            }
            copy.style.image_orient_none = merged.image_orient_none;
            // Признак определённого блока (§10.5) — от слитого стиля: держатель
            // строится из сырого, и `apply` иначе выбрасывал `height: %`
            // (см. строчный рукав `"img"` в `atom_element`; ячейка и лунки —
            // свои пути).
            if !matches!(inherited.display, Some(Display::TableCell) | Some(Display::GridLanes)) {
                copy.style.cb_height_def = merged.cb_height_def;
            }
            // Единицы окна — в точки, как у строчной картинки.
            copy.style.resolve_viewport(opts.viewport);
            image_with(&copy, Some(atom_base_font(inherited, opts)))
        }
        // Замещаемые с картинкой-источником рисуются как <img>: embed через
        // src, object через data, video через poster (css-images §5:
        // object-fit/-position действуют на всех замещаемых).
        "embed" if e.attr("src").is_some() => image(e),
        // Вложенный документ (`<iframe>` и `<object>` с документом в `data`)
        // построен выше; рукав стоит ПЕРЕД картиночным `object`, иначе
        // документ уходил в `image()` пустой коробкой. Объект с документом,
        // который не прочитался, в картинку не превращается — падает в общий
        // рукав и показывает запасное содержимое (HTML §4.8.7).
        "iframe" | "object" if built_iframe.is_some() => built_iframe.take().unwrap(),
        // БЛОЧНЫЙ кадр без пригодного документа — всё равно замещаемая
        // коробка: по умолчанию 300×150 (CSS 2.1 §10.3.2 «…the used value of
        // 'width' becomes 300px», §10.6.2 — 150px; HTML §15.4.4 у `<iframe>`
        // атрибуты `width/height` — размеры). Прежде он шёл общим рукавом
        // пустым блоком: ширина растягивалась на содержащий блок, высота
        // была нулевой, и красная рамка вылезала из-под зелёной
        // (`block-replaced-height-004/005/007`). Строчный кадр не трогаем —
        // там коробка без вклада в строку замерена и откачена (рукав
        // `"iframe"` в `atom_element`).
        // Плавающий кадр сюда не идёт: замерено — `float-replaced-height-005/
        // 007` (0.00 → «красное видно»), у флоата свой путь размеров.
        // Только кадр в обычном блочном потоке: флоат снимает `float` с
        // копии и кладёт её в синтетический гибкий ряд (`wrap_floats`) или в
        // бандовый хост (`flow-root`) — там размер считает свой путь
        // (замерено: `float-replaced-height-005/007` 0.00 → «красное видно»).
        "iframe"
            if !e.style.float.is_some_and(|f| f != 0)
                && matches!(inherited.display, None | Some(Display::Block))
                && e.style.flow_root != Some(true)
                && !matches!(
                    e.style.width,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                ) =>
        {
            let mut copy = pct_height_to_px(e, inherited);
            if !matches!(copy.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
                copy.style.width = Some(Len::Px(300.0));
            }
            let pct_ok = matches!(copy.style.height, Some(Len::Pct(_))) && merged.cb_height_def;
            if !matches!(copy.style.height, Some(Len::Px(_))) && !pct_ok {
                copy.style.height = Some(Len::Px(150.0));
            }
            copy.style.cb_height_def = merged.cb_height_def;
            styled_div(&copy).flex_shrink_0().into_any_element()
        }
        "object" if e.attr("data").is_some() && !object_is_document(e) => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: давать рамке БЕЗ адреса резервную ширину 300
        // точек при `display: block` (§10.3.4 -> §10.3.2). Замерено по срезу
        // из 416 пар семей *image*/*replaced*: приобретено 0, потеряно 1 —
        // `float-replaced-height-004` 0.00 -> «красное видно». Это второй
        // заход на резервный размер бесадресной рамки; первый (коробка
        // 300×150 целиком) стоил 13 пар, запись выше.
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => {
            // Тот же stretch-fit, что у строчного атома (`atom_element`).
            // Содержащий блок — `inherited` (родитель), а не `merged`: это
            // уже собственный стиль рисунка.
            let cb_w = match inherited.width {
                Some(Len::Px(v)) if v > 0.0 => Some(v),
                _ => CB_WIDTH.get().filter(|v| *v > 0.0),
            };
            // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`.
            svg_replaced(e, &crate::svg::stretch_fit(e, cb_w), &merged).unwrap_or_else(|| {
                styled_div_with(e, &merged)
                    .child(SharedString::from("[рисунок]"))
                    .into_any_element()
            })
        }
        // `w_full` — растяжка по СТРОЧНОЙ оси горизонтального родителя. В
        // вертикальном родителе ширина — блочный размер (css-writing-modes-4
        // §7.1): полная ширина раздувала линию `width: 10px` на всё тело
        // (`horizontal-rule-vlr-003`), а строчную ось (высоту) растягивает сам
        // ряд блочного потока.
        "hr" => {
            let d = styled_div_with(e, &merged);
            if inherited.vertical == Some(true) {
                d.into_any_element()
            } else {
                d.w_full().into_any_element()
            }
        }
        // Синтетический узел обтекания формой (см. wrap_floats).
        "shape-flow" => shape_flow(e, &merged, opts),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        // Лунки идут путём сетки taffy (`dom::lanes_as_grid`); сюда доходит
        // только узел, собранный мимо того прохода, — он переводится тем же
        // правилом. Рукописная раскладка `lanes` удалена: все оси (вертикальное
        // письмо, `rtl`) ведёт `taffy::compute::grid::lanes`.
        _ if merged.display == Some(Display::GridLanes) => {
            let mut copy = e.clone();
            crate::dom::lanes_to_grid(&mut copy.style);
            element(&copy, inherited, opts)
        }
        // Таблица — независимый контекст форматирования: её строки в бюджет
        // `line-clamp` не входят (css-overflow-4 §5.3, `webkit-line-clamp-013`).
        // Табличный путь минует общий, где стоит этот сторож (`makes_bfc`),
        // поэтому он ставится здесь.
        _ if matches!(
            e.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        ) =>
        {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Ряд, группа рядов или ячейка ВНЕ таблицы получают анонимную
        // таблицу-обёртку (css-tables-3 §3.1): иначе ячейки складывались
        // столбиком обычных блоков.
        _ if matches!(
            e.style.display,
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
        ) =>
        {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            let wrapper = anon_element("table", vec![Node::Element(e.clone())]);
            table(&wrapper, &merged, opts)
        }
        "table" => {
            let _clamp_bfc = crate::interact::clamp_context()
                .is_some()
                .then(crate::interact::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Список с заданной раскладкой — это уже не список, а контейнер:
        // на `ul` верстают навигацию и наборы чипов.
        "ul" | "ol" if e.style.display.is_none() => list_container::render(e, &merged, opts),
        // `white-space: pre*` значим не меньше тега: переводы строк сохраняет
        // именно он, и на `<div style="white-space: pre">` разметка обязана
        // вести себя так же, как на `<pre>`.
        // Преформат отдельным рисователем — ТОЛЬКО для непереносящегося
        // `white-space: pre`. Переводы строк хранят четыре режима, и три из
        // них переносят строки: `pre-wrap`, `pre-line`, `break-spaces`. Пока
        // сюда уходили все четыре, эти три шли мимо нашей строчной раскладки,
        // где и живут висящие пробелы, разрыв после сохранённого пробела и
        // правила куска. Отсюда же `break-spaces` был неотличим от `pre-wrap`.
        // Внутри преформата может стоять кусок со СВОИМ `white-space`, и он
        // переносится, хотя абзац — нет. Отдельный рисователь преформата
        // правил куска не знает, поэтому такой случай уходит в обычную
        // строчную раскладку (`white-space-pre-031`).
        _ => {
            let mut d = styled_div_with(e, &merged);
            let overflow_plan = absolute_overflow::Plan::new(&merged, inherited);
            if let Some(plan) = &overflow_plan {
                plan.prepare(d.style());
            }
            // ЗАМЕРЕНО И ЗАКРЕПЛЕНО: минимума высоты в видимую область на
            // коробке корня БОЛЬШЕ НЕТ. §10.6.3 — высота корня `auto`, ростом
            // с окно обязан быть начальный содержащий блок, а не коробка:
            // рамка и фон корня уходили полосами до низа окна
            // (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
            // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и
            // содержащим блоком им служит корневой `div` стенда.
            //
            // Замер по семьям backgrounds/*, *root*, positioning/*,
            // containing-block*, normal-flow/*, *margin*: приобретено 11,
            // потеряна одна (`background-root-024`: слой донора-тела лежит в
            // детях корня и отсчитывался от его padding-box).
            // Многоколоночный поток. Своей многоколоночной раскладки нет, но
            // сетка даёт то же расположение: число рядов считаем по числу
            // детей, а заполнение идёт по колонкам — тогда порядок совпадает
            // с браузерным (сверху вниз, затем в следующую колонку).
            // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
            // целых колонок этой ширины влезает в коробку, столько их и будет
            // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
            // считать не от чего, и остаётся прежняя дорожечная раскладка.
            // Умолчание `column-gap: normal` — один кегль (css-align §8.3).
            // Вычисленные значения, а не заданные: `resolve_em` живёт в
            // `inline::inherit`, поэтому точки лежат в `merged`
            // (`render.rs:13235`), а в `e.style` остаётся `Len::Em`. Кегль
            // для `column-gap: normal` — СОБСТВЕННЫЙ кегль элемента
            // (css-align §8.3; Blink `length_utils.cc:1356`
            // `style.GetFontDescription().ComputedPixelSize()`), и он тоже
            // разрешён только в `merged`.
            let used_gap = match merged.column_gap {
                Some(Len::Px(v)) => v,
                _ => match merged.font_size {
                    Some(Len::Px(size)) => size,
                    _ => opts.base_size(),
                },
            };
            // `column-*` — только у блочных контейнеров (css-multicol-1 §2):
            // сетка ими не режется (`grid-multicol-001`).
            let multicol = multicol_container(&e.style);
            // css-multicol-1 §3.4 шаги (05)-(07): N считается по ВЫЧИСЛЕННЫМ
            // 'column-width', 'column-gap' и используемой ширине коробки.
            // Пока брались заданные значения, `column-width: 6em` не
            // проходил гейт `Len::Px`, `count_from_width` был `None`,
            // `width_driven` — ложью, и многоколоночник не включался вовсе:
            // `multicol-width-001` рисовался одним абзацем в 30 знаков
            // вместо пяти колонок по шесть (снимок обеих сторон в
            // `target/scout-mctextflow-2026-09.md` §3.4). Blink
            // абсолютизирует 'column-width' в `float` ещё на вычисленном
            // значении (`css_properties.json5:7513`
            // `ConvertComputedLength<float>`) и в `length_utils.cc:1311`
            // `ResolveUsedColumnCount` читает уже точки.
            let column_width = merged.column_width.filter(|_| multicol);
            let column_count = merged.column_count.filter(|_| multicol);
            // Ось прогрессии колонок — СТРОЧНАЯ ось многоколоночника
            // (css-multicol-1 §2, `Overview.bs:375-379`: «The column boxes are
            // ordered in the inline base direction»). В вертикальном письме она
            // вертикальна, блочная — горизонтальна, у `vertical-rl` от ПРАВОГО
            // края (css-writing-modes-4 §3.1). `inline-size`/`block-size`
            // разложены в физические `height`/`width` ещё в каскаде, поэтому
            // строчный размер коробки здесь — `height`, блочный — `width`.
            // `direction: rtl` в вертикали (колонки снизу вверх) — прежним путём.
            let col_rl = merged.vertical_rl == Some(true);
            let col_axis = if merged.vertical == Some(true) && merged.rtl != Some(true) {
                if col_rl {
                    crate::flow::StackAxis::VerticalRl
                } else {
                    crate::flow::StackAxis::VerticalLr
                }
            } else {
                crate::flow::StackAxis::Horizontal
            };
            let col_vert = col_axis.is_vertical();
            let col_inline_size = if col_vert { merged.height } else { merged.width };
            let count_from_width = match (column_width, col_inline_size) {
                (Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
                    Some((((box_w + used_gap) / (w + used_gap)).floor().max(1.0)) as u16)
                }
                _ => None,
            };
            // Used column-count (css-multicol §3.4, как ResolveUsedColumnCount
            // в blink): заданы оба — МЕНЬШЕЕ из числа и «сколько влезает»;
            // только ширина — сколько влезает.
            let used_count = match (column_count, count_from_width) {
                (Some(c), Some(fw)) => Some(c.min(fw)),
                (Some(c), None) => Some(c),
                (None, fw) => fw,
            };
            let width_driven = column_count.is_none()
                && count_from_width.is_none()
                && matches!(column_width, Some(Len::Px(w)) if w > 0.0);
            // Заданный `column-height` без числа колонок — одна колонка, но с
            // рядами (`column-height-012`: `column-height:40px` и 80px
            // содержимого — два ряда по 40).
            let height_driven =
                e.style.column_height.is_some() && used_count.is_none_or(|n| n <= 1);
            // `column-count: 1` — тоже многоколоночник (css-multicol-1 §2; Blink
            // `ComputedStyle::SpecifiesColumns`: «!HasAutoColumnCount()»), и
            // спаннер в нём режет содержимое на ряды: ряд до спаннера — свой
            // контекст форматирования и держит свои флоаты
            // (`multicol-span-float-002`: `Pink` вставал между флоатами первой
            // строки), рамка предка режется по фрагментам
            // (`multicol-span-all-children-height-008`). Только при спаннере:
            // без него одна колонка по-прежнему рисуется обычным блоком
            // (переполнения одной колонки вбок здесь нет —
            // `scout-multicol-2026-09.md` §10). Элемент списка — мимо:
            // сегментный путь маркер не рисует
            // (`multicol-span-all-list-item-001/002`).
            let lone_span = used_count == Some(1)
                && !height_driven
                && e.style.column_wrap.is_none()
                && e.list_item.is_none()
                && has_deep_spanner(e);
            if let Some(cols) = used_count
                .filter(|n| *n > 1)
                .or(width_driven.then_some(0))
                .or(height_driven.then_some(1))
                .or(lone_span.then_some(1))
            {
                // Сплошной текст режется на колонки по строкам, а не по детям:
                // один длинный абзац иначе оставался в первой колонке целиком.
                // `columns: auto <w>` без ширины коробки решается в замере —
                // туда уходит и число, и ширина колонки (§3.4).
                let col_w_px = match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                };
                let want = (cols > 0).then_some(cols as usize);
                // Ряды колонок (css-multicol-2 §ch, §cwr): высота ряда —
                // `column-height`, а при `column-wrap: wrap` без него —
                // высота коробки (Blink `RowHeight()`:
                // `remaining_content_block_size_`, issue 11754 вариант 2);
                // `column-wrap: auto` = `wrap` при заданном
                // `column-height`. `row-gap: normal` в колонках — 1em (§rg).
                let col_h = match e.style.column_height {
                    Some(Len::Px(h)) if h >= 0.0 => Some(h),
                    _ => None,
                };
                // Блочный размер коробки: в вертикальном письме — ширина.
                let box_h = match if col_vert { e.style.width } else { e.style.height } {
                    Some(Len::Px(h)) if h > 0.0 => Some(h),
                    _ => None,
                };
                let wrap = e.style.column_wrap.unwrap_or(col_h.is_some());
                let em = match e.style.font_size {
                    Some(Len::Px(size)) => size,
                    _ => opts.base_size(),
                };
                let row_gap = match e.style.gap.and_then(|g| g.0) {
                    Some(Len::Px(v)) => v.max(0.0),
                    Some(Len::Em(k)) => k * em,
                    _ => em,
                };
                let rows = (col_h.is_some() || wrap).then_some(crate::flow::Rows {
                    h: col_h.or(if wrap { box_h } else { None }),
                    gap: row_gap,
                    wrap,
                    cap: false,
                });
                // Потолок баланса (`Rows::cap`): СОБСТВЕННАЯ высота коробки в
                // точках. ★ Прежний MC-BALANCE-CAP (float.rs, v164, +3/−9)
                // спускал внешний потолок во ВЛОЖЕННЫЕ многоколоночники без
                // своей высоты — все девять потерь такие; здесь потолок не
                // передаётся вниз (`in_stack`: внутри копии другой стопки —
                // нет), не трогает `column-fill: auto` (там `fixed`) и ряды
                // (`column-height`). `grid-container-fragmentation-004`: 350 в
                // 4 колонках по 100 — баланс уходил в 125, в четвёртой
                // колонке красное 50..100.
                // Потолок баланса — ИСПОЛЬЗУЕМАЯ высота коробки, а не голая `height`:
                // Blink `ConstrainColumnBlockSize` (`column_layout_algorithm.cc:
                // 1774-1793`) берёт `max = min(max-height, height)`, затем
                // `max = max(max, min-height)` («A specified min-block-size may
                // increase the maximum length»; CSS 2.1 §10.7). `multicol-fill-
                // balance-005`: `height:20px; max-height:40px; min-height:100px` —
                // коробка 100, баланс 200/2 = 100 ровно в неё; с потолком 20 колонки
                // выходили по 20, и красный фон 20..100 был виден. `min-height` не в
                // точках (доля, `em`) — потолка нет, как до P5: ниже используемой
                // высоты резать нельзя, а её здесь не знаем.
                let (max_block, min_block) = if col_vert {
                    (e.style.max_width, e.style.min_width)
                } else {
                    (e.style.max_height, e.style.min_height)
                };
                let cap_h = box_h.and_then(|h| {
                    let h = match max_block {
                        Some(Len::Px(m)) if m >= 0.0 => h.min(m),
                        _ => h,
                    };
                    match min_block {
                        None | Some(Len::Auto) => Some(h),
                        Some(Len::Px(m)) => Some(h.max(m)),
                        _ => None,
                    }
                });
                // Вложенный многоколоночник во внешней колонке (`flow::OUTER_ROW`):
                // ряды высотой во внешний фрагментаинер без зазора — граница
                // ряда совпадает с границей внешней колонки (css-break-4 §2.1;
                // Blink `ConstrainColumnBlockSize`, `LayoutLine` :1009-1026
                // «wrap … if we're participating in an outer fragmentation
                // context»). Последний ряд балансируется (css-multicol-1 §7.1
                // «only the last fragment is balanced»).
                let nest_phase = outer_row.map_or(0.0, |r| r.1);
                let nest_rows = outer_row.map(|r| r.0).filter(|_| {
                    col_h.is_none() && e.style.column_wrap.is_none() && !col_vert
                });
                let rows = match nest_rows {
                    Some(hh) => Some(crate::flow::Rows {
                        h: Some(hh),
                        gap: 0.0,
                        wrap: true,
                        cap: false,
                    }),
                    None => rows,
                };
                let rows = rows.or_else(|| {
                    (cap_h.is_some()
                        && e.style.column_fill_auto != Some(true)
                        && !crate::flow::in_stack())
                    .then_some(crate::flow::Rows {
                        h: cap_h,
                        gap: row_gap,
                        wrap: false,
                        cap: true,
                    })
                });
                // Спаннер среди инлайнового потока: режем детей на сегменты,
                // каждый сегмент — свой поток колонок, спаннер — блок между
                // ними (css-multicol §6). С рядами (`column-wrap: wrap` +
                // `column-height`) сегменты не годятся: у каждого свои ряды от
                // нуля, а Blink ведёт ОДИН курсор по коробке
                // (`column_layout_algorithm.cc` `intrinsic_block_size_`,
                // `LayoutSpanner`: спаннер, не влезший в остаток ряда, — со
                // следующего ряда; `column-height-006/013/017…020`). Такой
                // многоколоночник идёт единой стопкой, спаннер — её ребёнком
                // (`StackChild::span`). Внутри копии другой стопки — по-прежнему
                // сегментами: перенос ряда во внешнюю колонку не написан
                // (`column-height-029`, scout-columnwrap-2026-09b.md §2.3).
                // Спаннер бывает НЕ прямым ребёнком: css-multicol-1
                // §column-span (`Overview.bs:1497-1499`) — «A spanning element
                // may be lower than the first level of descendants as long as
                // they are part of the same formatting context, and there is
                // nothing between the spanning element and multicol container
                // that establishes a containing block for fixed position
                // descendants». Спаннер выносится ИЗ ПОТОКА и режет
                // многоколоночник на «до», «спаннер во всю ширину» и «после»,
                // а его предки внутри многоколоночника разрезаются вместе с
                // ним. Поднимаем таких потомков к прямым детям ОДИН раз, до
                // всех решений ниже: дальше и сегментный путь, и единая
                // стопка, и текстовый `column_flow` видят спаннер прямым
                // ребёнком. Blink ведёт для этого путь `ColumnSpannerPath`
                // (`column_spanner_path.h`), у нас пути нет — предки режутся
                // прямо в дереве (`hoist_spanners`).
                // Сегментный путь ниже: метка родителя предыдущего спаннера,
                // если между ними не легло ни одного ряда колонок, и выложен
                // ли уже хоть один кусок (для сторожей полей).
                let mut span_prev: Option<String> = None;
                let mut seg_open = false;
                let hoisted;
                let e = match hoist_spanners(&e.children) {
                    Some(kids) => {
                        let mut c = e.clone();
                        c.children = kids;
                        hoisted = c;
                        &hoisted
                    }
                    None => e,
                };
                let is_span = |n: &Node| matches!(n, Node::Element(c) if spanner_box(c));
                let unified = rows.is_some_and(|r| r.wrap && r.h.is_some()) && !crate::flow::in_stack();
                // Хвостовой ряд колонок при `column-fill: auto` НЕ стоит перед
                // спаннером, и §column-fill («content in a multi-column line that
                // does not immediately precede a spanner») велит заполнять его
                // подряд до высоты коробки. Сегмент ниже теряет высоту
                // (`sub.style.height = None`) и уходил в балансировку:
                // `no-balancing-after-column-span` — 200×50 двумя колонками
                // вместо 100×100 одной. Высота хвоста — остаток коробки после
                // спаннера (Blink `ConstrainColumnBlockSize`:
                // `max -= CurrentContentBlockOffset(line_offset)`). Только когда
                // до хвоста стоит ОДИН измеримый спаннер и больше ничего:
                // высоту сбалансированных рядов до спаннера здесь не знаем.
                let rest_h: Option<f32> = match (e.style.column_fill_auto, box_h, rows) {
                    (Some(true), Some(total), None) => e
                        .children
                        .iter()
                        .rposition(&is_span)
                        .and_then(|j| {
                            let px = |l: &Option<Len>| match l {
                                None => Some(0.0),
                                Some(Len::Px(v)) => Some(*v),
                                _ => None,
                            };
                            let mut used = 0.0f32;
                            let mut spans = 0usize;
                            for n in &e.children[..=j] {
                                if is_blank(n) {
                                    continue;
                                }
                                let Node::Element(sp) = n else {
                                    return None;
                                };
                                if !is_span(n) {
                                    return None;
                                }
                                spans += 1;
                                let st = inline::inherit(&merged, &sp.style);
                                used += shape_full(sp, 4, ShapeCx::COLUMNS)?.0
                                    + px(&st.margin.top)?
                                    + px(&st.margin.bottom)?;
                            }
                            (spans == 1).then(|| (total - used).max(0.0))
                        }),
                    _ => None,
                };
                if e.children.iter().any(&is_span) && !unified {
                    // `<fieldset>`: отрисованная легенда стоит ВНЕ колонок —
                    // многоколоночность получает анонимная коробка содержимого
                    // fieldset (HTML §15.3.13 «The fieldset and legend
                    // elements», пересказ; Blink `LayoutFieldset` +
                    // `FieldsetContentBox`). Прежде легенда падала в первый ряд и
                    // занимала колонку (`multicol-span-all-fieldset-001…003`:
                    // эталон — `fieldset > legend + div.inner` с колонками).
                    // Отрисованная — первая `legend` в потоке.
                    let mut kids = e.children.clone();
                    if e.tag == "fieldset" {
                        if let Some(i) = kids.iter().position(|n| {
                            matches!(n, Node::Element(c) if c.tag == "legend" && !out_of_flow(&c.style))
                        }) {
                            let legend = kids.remove(i);
                            d = d.children(blocks(&[legend], &merged, opts));
                        }
                    }
                    for chunk in kids.split_inclusive(&is_span) {
                        let (body, span) = match chunk.split_last() {
                            Some((last, head)) if is_span(last) => (head, Some(last)),
                            _ => (chunk, None),
                        };
                        if body.iter().any(|n| !is_blank(n)) {
                            // Ряд колонок между спаннерами — новый контекст
                            // форматирования: поля спаннеров сквозь него не
                            // схлопываются (§column-span: «margins on elements
                            // inside a column box will not collapse with the margin
                            // of a spanner»). Сам ряд для taffy — лист или
                            // не-блок, насквозь его поле не проходит.
                            span_prev = None;
                            seg_open = true;
                            let mut seg = e.clone();
                            seg.children = body.to_vec();
                            seg.style.column_span = None;
                            // Одна колонка со спаннером (`lone_span`) — ряд рисуется
                            // обычным блоком: `column_flow` спускается в
                            // единственного блочного ребёнка и рисует только его
                            // текст, теряя коробку (фон, рамку, высоту фрагмента
                            // `container` в `multicol-span-all-children-height-005/
                            // 008`). Клон `sub` с одной колонкой спаннеров не несёт,
                            // и гейт `lone_span` его снова не пускает.
                            let flow = if lone_span {
                                None
                            } else {
                                column_flow(&seg, &merged, opts, want, col_w_px)
                            };
                            if let Some(el) = flow {
                                d = d.child(el);
                            } else {
                                // Блочный сегмент: рекурсия в общий рендер —
                                // он сам выберет укладку колонок; коробка
                                // (фон/рамки/поля) остаётся на хосте.
                                let mut sub = seg.clone();
                                sub.style.background = None;
                                // Всё, что многоколоночник рисует и сдвигает КОРОБКОЙ,
                                // уже стоит на хосте `d` (`styled_div_with(e, ..)`);
                                // клон ряда повторял это на каждом ряду: контур и
                                // тень вокруг каждого ряда, двойная прозрачность,
                                // фильтр и трансформ, двойной сдвиг `relative`, а у
                                // `position: absolute` ряды выпадали из потока и
                                // ложились друг на друга (`multicol-span-all-
                                // fieldset-002/003`, `-button-002/003`). Обрезку
                                // переполнения делает хост: по css-multicol-1
                                // §overflow режет коробка многоколоночника, а не ряд.
                                // Ряд остаётся содержащим блоком (`relative`), как
                                // прежде.
                                sub.style.gradient = None;
                                sub.style.bg_image = None;
                                sub.style.border_image = None;
                                sub.style.shadows = Vec::new();
                                sub.style.inset_shadows = Vec::new();
                                sub.style.outline = None;
                                sub.style.opacity = None;
                                sub.style.transform = None;
                                sub.style.filter = None;
                                sub.style.mask_image = None;
                                sub.style.backdrop_blur = None;
                                if matches!(
                                    sub.style.position,
                                    Some(crate::computed::Position::Absolute)
                                        | Some(crate::computed::Position::Fixed)
                                ) {
                                    sub.style.position = Some(crate::computed::Position::Relative);
                                }
                                sub.style.inset = Default::default();
                                sub.style.z_index = None;
                                sub.style.overflow_x = None;
                                sub.style.overflow_y = None;
                                sub.style.margin = Default::default();
                                sub.style.padding = Default::default();
                                sub.style.border_width = Default::default();
                                sub.style.width = None;
                                sub.style.height = None;
                                // Хвост после спаннера при `column-fill: auto` —
                                // остаток коробки (`rest_h`): укладка заполняет
                                // колонки подряд, а не балансирует. Только
                                // измеримым блокам: неизмеримый ряд уходит в
                                // запасную сетку, где заданная высота растянула бы
                                // дорожки `auto`.
                                if span.is_none()
                                    && body.iter().all(|n| {
                                        is_blank(n)
                                            || matches!(n, Node::Element(k)
                                                if !k.inline
                                                    && shape_full(k, 4, ShapeCx::COLUMNS).is_some())
                                    })
                                {
                                    if let Some(rest) = rest_h {
                                        sub.style.height = Some(Len::Px(rest));
                                    }
                                }
                                d = d.child(div().children(blocks(
                                    &[Node::Element(sub)],
                                    &merged,
                                    opts,
                                )));
                            }
                        }
                        if let Some(Node::Element(sp)) = span {
                            // Хост сегментов — блок taffy (`Display::Block`,
                            // `vendor/gpui/src/style.rs:862`), и поля соседних детей
                            // он схлопывает сам (`vendor/taffy/src/compute/
                            // block.rs:186-210`): соседние спаннеры ОДНОГО предка —
                            // верно («the margins of two adjacent spanners will
                            // collapse with each other»; `multicol-span-all-margin-
                            // 003`). Запрещённое схлопывание гасит нулевая гибкая
                            // сторожка — её taffy насквозь не проходит
                            // (`has_styles_preventing_being_collapsed_through`:
                            // `!style.is_block()`): (1) перед ПЕРВЫМ куском-спаннером
                            // — многоколоночник сам контекст форматирования, и
                            // верхнее поле спаннера сквозь его верх не уходит;
                            // (2) между спаннерами РАЗНЫХ исходных предков (метка
                            // `kamin-span-parent`, `spanner_parts`).
                            let key = sp.attr("kamin-span-parent").unwrap_or("").to_string();
                            if !seg_open || span_prev.as_ref().is_some_and(|k| *k != key) {
                                d = d.child(div().flex());
                            }
                            seg_open = true;
                            span_prev = Some(key);
                            let inner = inline::inherit(&merged, &sp.style);
                            // Спаннер — независимый контекст форматирования
                            // (§column-span). Голый `styled_div_with` — блок taffy,
                            // и тот схлопывал поле первого/последнего ребёнка
                            // сквозь край спаннера (`vendor/taffy/src/compute/
                            // block.rs:186`): в `multicol-span-all-margin-nested-
                            // firstchild-001` `<span style="margin: 2em 0">` уводил
                            // `<h6>` вниз, и чёрного фона не было видно вовсе.
                            // Оболочка — гибкая колонка, ровно как у блока общего
                            // пути (`d.flex().flex_col()` при пустом `display`), где
                            // поля детей сводит сам `blocks()` — а он с предикатом
                            // `own_context_style` поле наружу больше не отдаёт.
                            let shell = styled_div_with(sp, &inner);
                            let shell = if matches!(inner.display, None | Some(Display::Block))
                                && inner.vertical != Some(true)
                            {
                                shell.flex().flex_col()
                            } else {
                                shell
                            };
                            d = d.child(shell.children(blocks(
                                &sp.children,
                                &inner,
                                opts,
                            )));
                        }
                    }
                    // Нижняя сторожка: нижнее поле последнего спаннера остаётся
                    // внутри многоколоночника — он независимый контекст
                    // форматирования (CSS 2.1 §8.3.1).
                    d = d.child(div().flex());
                    return d.into_any_element();
                }
                // Текстовый путь `text-box-trim` не знает (ни у краёв колонок, ни у
                // первой/последней строки) — такой многоколоночник идёт стопкой
                // со строками (`line_run_shape`; `text-box-trim-multicol-009/010`).
                let trim_host = merged.text_box_trim_start || merged.text_box_trim_end;
                if let Some(el) = column_flow(e, &merged, opts, want, col_w_px)
                    .filter(|_| nest_rows.is_none() && !trim_host)
                {
                    // Коробка элемента остаётся своей: отступы и фон
                    // принадлежат ей, поток живёт внутри.
                    return d.child(el).into_any_element();
                }
                if cols == 0 {
                    // Число колонок при `columns: auto <w>` решается только в
                    // замере текстового потока; блочный фоллбек — дорожками.
                    if let Some(w) = col_w_px {
                        d = d.grid().grid_cols_min(px(w));
                    }
                } else {
                    // Блочные дети с ИЗВЕСТНЫМИ высотами — честная укладка по
                    // колонкам с балансом и монолитами (css-break, фаза 1;
                    // план target/scout-multicol.md / scout-fragmentation.md).
                    // Коробка ребёнка и его вертикальные поля отдельно:
                    // поля схлопываются между соседями и на границах колонок.
                    // Флоаты в поддереве ломают известность высоты.
                    // Прямые абсолюты многоколоночника — не в стопку: их
                    // содержащий блок — весь контейнер, рисуются его детьми
                    // рядом со стопкой (`out-of-flow-in-multicolumn-094…097`
                    // при нулевой записи в стопке уходили в колонку).
                    // Статическая позиция (CSS 2.1 §10.6.4: «where the box
                    // would have been if position were static»; §10.3.7 — то
                    // же по строчной оси) — правило ПОЗИЦИОНИРОВАННОЙ
                    // коробки. У плавающей своё место по §9.5, и щуп ей не
                    // положен: нулевая запись в стопке меняет `kids.len()`, с
                    // ним `balance_last` и весь план `fill_avoiding`
                    // (`multicol-fill-balance-038` — монолитный флоат с
                    // полями 40/70 при `margin-bottom: -30px` у соседа:
                    // 0.32 -> «красное видно», замерено на v206). Blink
                    // перебирает в `LayoutFragmentainerDescendants` только
                    // `oof_positioned_candidates`; флоат идёт обычной
                    // укладкой (`PositionFloat`).
                    let positioned = |s: &Computed| {
                        matches!(
                            s.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    };
                    // Строчный размер колонки для меры строк (`with_lines`):
                    // css-multicol-1 §3.4 (11) «W := max(0, (U + column-gap)/N −
                    // column-gap)» при U в точках; иначе строки не меряются.
                    // Ширина `auto` блока в потоке — ширина содержимого родителя
                    // (CSS 2.1 §10.3.3), когда та в точках и у коробки нет боковых
                    // полей, рамок и отбивок (`text-box-trim-multicol-011-ref`:
                    // многоколоночник без ширины в `.container` 640).
                    let line_inline_size = col_inline_size.or_else(|| {
                        let zero = |l: &Option<Len>| matches!(l, None | Some(Len::Px(0.0)));
                        let b = e.style.borders();
                        (!col_vert
                            && matches!(e.style.width, None | Some(Len::Auto))
                            && matches!(e.style.display, None | Some(Display::Block))
                            && e.style.float.unwrap_or(0) == 0
                            && !out_of_flow(&e.style)
                            && [&e.style.margin.left, &e.style.margin.right, &e.style.padding.left, &e.style.padding.right, &b.left, &b.right]
                                .into_iter()
                                .all(zero)
                            // Только простой поток: строки и листья с высотой в точках.
                            // Блок с переполнением своей высоты (`css-break/block-max-
                            // height-001-ref`: 160 с ребёнком 200) стопка рисует иначе,
                            // чем прежний путь рисует тест с `max-height` (0.00 -> 11).
                            && e.children.iter().all(|n| match n {
                                Node::Text(_) => true,
                                Node::Element(k) => {
                                    k.inline
                                        || inline_content(k)
                                        || (k.children.iter().all(is_blank)
                                            && matches!(k.style.height, Some(Len::Px(_))))
                                }
                            }))
                        .then_some(inherited.width)
                        .flatten()
                        .filter(|w| matches!(w, Len::Px(_)))
                    });
                    let line_col_w = match line_inline_size {
                        Some(Len::Px(u)) if merged.border_box != Some(true) && cols > 0 => {
                            Some(((u + used_gap) / cols as f32 - used_gap).max(0.0))
                        }
                        _ => None,
                    };
                    // Строчные прогоны среди блоков — анонимными блоками (CSS 2.1
                    // §9.2.1.1), только когда строки можно измерить.
                    let grouped_e = line_col_w.and_then(|_| group_inline_runs(e));
                    let ge: &Element = grouped_e.as_ref().unwrap_or(e);
                    // Плавающий прямой ребёнок во всю ширину колонки рядом с
                    // собой ничего не терпит: строки и блоки встают под ним,
                    // как под блоком, — в стопку колонок он идёт БЛОКОМ и
                    // рвётся по колонкам вместе с потоком (css-break-3 §4:
                    // флоат — фрагментируемая коробка; `css-break/float-001`:
                    // флоат 200px в колонках по 100 — прежний путь рисовал его
                    // соседом стопки одним куском).
                    let zero_m = |l: &Option<Len>| match l {
                        None => true,
                        Some(Len::Px(v)) => v.abs() < 0.01,
                        _ => false,
                    };
                    let full_float = |c: &Element| {
                        c.style.float.is_some_and(|f| f != 0)
                            && matches!(c.style.width, Some(Len::Pct(k)) if (k - 1.0).abs() < 1e-4)
                            && zero_m(&c.style.margin.left)
                            && zero_m(&c.style.margin.right)
                            // Поля флоата не схлопываются и у края колонки не
                            // усекаются (CSS 2.1 §8.3.1), а у блока стопки —
                            // да: флоат с вертикальными полями — прежним путём
                            // (`multicol-fill-balance-037`: `margin: 40px 0`).
                            && zero_m(&c.style.margin.top)
                            && zero_m(&c.style.margin.bottom)
                            && c.style.shape_outside.is_none()
                            && !positioned(&c.style)
                    };
                    let floats_blocked = ge
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if full_float(c)))
                    .then(|| {
                        let mut g = ge.clone();
                        for n in g.children.iter_mut() {
                            if let Node::Element(c) = n
                                && full_float(c)
                            {
                                c.style.float = None;
                                c.style.clear = None;
                                c.attrs.push(("kamin-float-block".into(), "1".into()));
                            }
                        }
                        g
                    });
                    let ge: &Element = floats_blocked.as_ref().unwrap_or(ge);
                    // Обёртка flex/сетки с ЕДИНСТВЕННЫМ элементом-`clone`
                    // (`clone_wrapper_item`): по блочной оси такая обёртка
                    // раскладывается ровно как блок с этим ребёнком, и фрагменты
                    // клонированного украшения строятся у самого элемента.
                    let unwrapped = ge
                        .children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if clone_wrapper_item(c).is_some()))
                        .then(|| {
                            let mut g = ge.clone();
                            for n in g.children.iter_mut() {
                                if let Node::Element(c) = n
                                    && let Some(item) = clone_wrapper_item(c)
                                {
                                    *c = item;
                                }
                            }
                            g
                        });
                    let ge: &Element = unwrapped.as_ref().unwrap_or(ge);
                    // Плавающие прямые дети — как прежде: не в стопку,
                    // рисуются её соседями.
                    let direct_oof: Vec<Element> = ge
                        .children
                        .iter()
                        .filter_map(|n| match n {
                            Node::Element(c)
                                if out_of_flow(&c.style) && !positioned(&c.style) =>
                            {
                                Some(c.clone())
                            }
                            _ => None,
                        })
                        .collect();
                    // Позиционированные прямые дети несут МЕСТО В ПОТОКЕ —
                    // номер среди детей, ушедших в стопку: точка статической
                    // позиции лежит В КОЛОНКЕ, а не под стопкой, и взять её
                    // больше неоткуда — раскладка под нами про колонки не
                    // знает. Счёт идёт по тем же детям, что отбирает
                    // `stackable` ниже: непустые и не внепоточные.
                    let oof_static: Vec<(usize, Element)> = {
                        let mut at = 0usize;
                        let mut out: Vec<(usize, Element)> = Vec::new();
                        for n in ge.children.iter().filter(|n| !is_blank(n)) {
                            let Node::Element(c) = n else {
                                at += 1;
                                continue;
                            };
                            if positioned(&c.style) {
                                out.push((at, c.clone()));
                            } else if !out_of_flow(&c.style) {
                                at += 1;
                            }
                        }
                        out
                    };
                    // Высота внешнего фрагментаинера для вложенного рядами
                    // (`nested_rows_shape`): `column-fill: auto` и блочный размер
                    // в точках, без своих рядов.
                    let outer_frag = (e.style.column_fill_auto == Some(true)
                        && rows.is_none()
                        && !col_vert)
                        .then(|| col_h.or(box_h))
                        .flatten();
                    let first_flow = ge
                        .children
                        .iter()
                        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| n as *const Node);
                    // Чью меру дала `nested_rows_shape` — только их копии рядами.
                    let nested_auto: std::cell::RefCell<Vec<u64>> = Default::default();
                    // Вложенный многоколоночник `height: auto` под БАЛАНСОМ внешнего
                    // (`nested_whole`): одна сбалансированная строка колонок целиком.
                    // css-multicol-1 §2: вложенный многоколоночник — сам мультиколонный
                    // контейнер, его колонки и линейки рисуются внутри внешней колонки
                    // (Blink `ColumnLayoutAlgorithm` для внутреннего — та же раскладка).
                    // Копия стопки шла узкой веткой `styled_div_with` и рисовала его
                    // плоско — без колонок и линеек (`multicol-rule-color-inherit-001`).
                    let nested_whole: std::cell::RefCell<Vec<u64>> = Default::default();
                    let whole_ok = e.style.column_fill_auto != Some(true)
                        && rows.is_none_or(|r| r.cap)
                        && nest_rows.is_none()
                        && !col_vert;
                    // Дети, чью высоту меряет раскладка копии (`StackChild::measure`).
                    let measured_kids: std::cell::RefCell<Vec<(u64, f32)>> = Default::default();
                    let measure_ok = e.style.column_fill_auto == Some(true)
                        && rows.is_none()
                        && nest_rows.is_none()
                        && !col_vert
                        && (col_h.is_some() || matches!(e.style.height, Some(Len::Px(_))));
                    let last_flow = ge
                        .children
                        .iter()
                        .rev()
                        .find(|n| !is_blank(n) && !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| n as *const Node);
                    let stackable: Option<Vec<(Element, Shape)>> = with_lines(&merged, line_col_w, opts, || ge
                        .children
                        .iter()
                        .filter(|n| !is_blank(n))
                        .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| match n {
                            // Вложенный многоколоночник `height: auto` с верха
                            // внешней колонки — рядами (`nested_rows_shape`).
                            Node::Element(c)
                                if first_flow == Some(n as *const Node)
                                    && matches!(c.style.height, None | Some(Len::Auto))
                                    && zero_len(c.style.margin.top)
                                    && outer_frag.is_some()
                                    && line_col_w.is_some()
                                    && nested_rows_box(c) =>
                            {
                                let c = &resolved_lengths(c, &merged);
                                let (Some(hh), Some(cw)) = (outer_frag, line_col_w) else {
                                    return None;
                                };
                                match nested_rows_shape(c, &merged, hh, cw, opts) {
                                    Some(h) => {
                                        nested_auto.borrow_mut().push(c.node_id);
                                        Some(((*c).clone(), h))
                                    }
                                    None => shape_full(c, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h)),
                                }
                            }
                            Node::Element(c)
                                if whole_ok
                                    && matches!(c.style.height, None | Some(Len::Auto))
                                    && line_col_w.is_some()
                                    && nested_rows_box(c) =>
                            {
                                // Высоту даёт раскладка самой копии (`StackChild::measure`):
                                // внутренний многоколоночник балансирует себя сам, и мера
                                // `shape_full` его колонок не видит (сумма детей).
                                let c = resolved_lengths(c, &merged);
                                let w = line_col_w?;
                                let m = |l: &Option<Len>| match l {
                                    Some(Len::Px(v)) => Some(*v),
                                    None | Some(Len::Auto) => Some(0.0),
                                    _ => None,
                                };
                                let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                                nested_whole.borrow_mut().push(c.node_id);
                                measured_kids.borrow_mut().push((c.node_id, w));
                                Some((c, (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                            }
                            // `position: relative` укладке не мешает — сдвиг
                            // накладывается на месте (корень A1).
                            Node::Element(c)
                                if !c.inline
                                    && (c.style.position.is_none()
                                        || c.style.position
                                            == Some(crate::computed::Position::Relative))
                                    && (c.style.float.unwrap_or(0) == 0
                                        || block_like_float(&c.style)) =>
                            {
                                // В вертикальном письме мера — по БЛОЧНОЙ оси
                                // (css-multicol-1 §2: «The column height is the
                                // length of the column box in the block
                                // direction»): поддерево меряется ПОВЁРНУТЫМ
                                // клоном (`transpose_tree`), рисуется исходным.
                                // Длины коробки — в точках (`resolved_lengths`):
                                // и мере, и копиям, и распоркам роста — одно дерево.
                                let mut c = resolved_lengths(c, &merged);
                                // `text-box-trim` многоколоночника режет его ПЕРВУЮ и
                                // ПОСЛЕДНЮЮ отформатированную строку (css-inline-3
                                // §4.2) — у строчного блока-ребёнка с края потока они
                                // его же. Метка, а не флаг стиля: срез на разрывах
                                // решает ближайшая коробка со своим флагом
                                // (`brk_trim`), и флаг ребёнка отнял бы его у хоста
                                // (`text-box-trim-multicol-005`).
                                if inline_content(&c) {
                                    if merged.text_box_trim_start && first_flow == Some(n as *const Node) {
                                        c.attrs.push(("kamin-host-trim-start".into(), "1".into()));
                                    }
                                    if merged.text_box_trim_end && last_flow == Some(n as *const Node) {
                                        c.attrs.push(("kamin-host-trim-end".into(), "1".into()));
                                    }
                                }
                                let c = &c;
                                if col_vert {
                                    let t = transpose_tree(c, col_rl)?;
                                    shape_full(&t, 4, ShapeCx::COLUMNS).map(|h| ((*c).clone(), h))
                                } else {
                                    shape_full(c, 4, ShapeCx::COLUMNS)
                                        .map(|h| ((*c).clone(), h))
                                        .or_else(|| {
                                            // Мера `shape_full` не выразила ребёнка (флоаты,
                                            // внепоточные потомки, таблица со сросшимися
                                            // рамками …). Прежде отказ ОДНОГО ребёнка
                                            // отправлял весь многоколоночник в сетку без
                                            // фрагментации — содержимое вовсе не переходило
                                            // в следующую колонку. css-break-3 §4: любая
                                            // блочная коробка фрагментируема; её высоту даёт
                                            // сама раскладка копии в колонку (`StackChild::
                                            // measure`, Blink меряет ребёнка тем же
                                            // алгоритмом, что и кладёт), а разрез — по краю
                                            // колонки (`slice`, без точек класса A).
                                            // Только колонки с заданной высотой
                                            // (`column-fill: auto`): у баланса высота
                                            // коробки сама зависит от меры, а раскладка
                                            // копии флоаты в высоту не берёт (а коробка
                                            // многоколоночника — корень контекста — берёт).
                                            // Принудительного разрыва внутри мера без
                                            // точек тоже не видит.
                                            if !measure_ok || forced_inside(c, 6) {
                                                return None;
                                            }
                                            let w = line_col_w?;
                                            let m = |l: &Option<Len>| match l {
                                                Some(Len::Px(v)) => Some(*v),
                                                None | Some(Len::Auto) => Some(0.0),
                                                _ => None,
                                            };
                                            let (mt, mb) = (m(&c.style.margin.top)?, m(&c.style.margin.bottom)?);
                                            measured_kids.borrow_mut().push((c.node_id, w));
                                            Some(((*c).clone(), (0.0, mt, mb, Vec::new(), Vec::new(), Vec::new())))
                                        })
                                }
                            }
                            _ => None,
                        })
                        .collect());
                    if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
                        // Копий у ребёнка — сколько колонок он может занять: без
                        // рядов ровно `cols` (как прежде), с рядами — по своей
                        // высоте против высоты ряда, с запасом на поля и срезы.
                        // Спаннер между колонками не режется — копий ему не
                        // надо, и в счёт он не входит (`column-height-019`:
                        // спаннер 85px при ряде 5px).
                        let copies = match rows {
                            Some(r) => {
                                let per = r.h.unwrap_or(f32::MAX).max(1.0);
                                let span = kids
                                    .iter()
                                    .filter(|(c, _)| c.style.column_span != Some(true))
                                    .map(|(_, s)| (s.0 / per).ceil() as usize)
                                    .max()
                                    .unwrap_or(0);
                                (span + 2).max(cols as usize).min(48)
                            }
                            None => cols.max(1) as usize,
                        };
                        // `column-fill: auto`: колонки заполняются подряд до
                        // `column-height`, а без него — до высоты коробки
                        // (css-multicol-1 §3.3, как прежде).
                        // `column-fill: auto` заполняет колонку до БЛОЧНОГО
                        // размера коробки — в вертикальном письме это ширина.
                        let fixed = if let Some(hh) = nest_rows {
                            (e.style.column_fill_auto == Some(true)).then_some(hh)
                        } else if e.style.column_fill_auto == Some(true) {
                            col_h.or(match if col_vert { e.style.width } else { e.style.height } {
                                Some(Len::Px(h)) => Some(h),
                                _ => None,
                            })
                        } else {
                            None
                        };
                        // Рост от вытолкнутых монолитов — распорки в
                        // DOM-клонах до сборки копий (`grow_pushed`).
                        // Многострочный колоночный flex — строками, параллельными
                        // потоками (`split_flex_lines`). Только при заполнении
                        // `column-fill: auto` с заданной высотой и без рядов:
                        // баланс считал бы содержимое по сумме записей, а строки
                        // идут бок о бок.
                        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.10): то же при балансе
                        // (`rows.is_none()` без `fixed`) с оценкой баланса по
                        // самой длинной строке группы (`runs_guess`). css-break/
                        // flexbox 319: +0/−2 — `multi-line-row-flex-fragmentation-
                        // 037/038` 0.07 → «красное видно»; балансные 033-035, 048
                        // не взяты. Только колонки (без рядов) при балансе — 0/0.
                        // Балансу нужен свой подбор высоты по строкам, а не оценка.
                        // Строки flex и распорки роста (`grow_pushed`) меряют
                        // ФИЗИЧЕСКОЕ дерево и знают только вертикальную ось —
                        // в вертикальном письме их нет (следующий шаг).
                        let (kids, kid_par, kid_parent, kid_starts) = if fixed.is_some() && rows.is_none() && !col_vert {
                            let col_w = match merged.width {
                                Some(Len::Px(w)) if merged.border_box != Some(true) && cols > 0 => {
                                    Some((w - used_gap * (cols as f32 - 1.0)) / cols as f32)
                                }
                                _ => None,
                            };
                            split_flex_lines(kids, col_w, &merged)
                        } else {
                            let n = kids.len();
                            (kids, vec![crate::flow::Par::default(); n], vec![None; n], (0..=n).collect())
                        };
                        // `break-inside: avoid` без настоящего монолита — у любого
                        // ребёнка колонок (`flow::Par::avoid_only`): с верха колонки
                        // коробка выше колонки рвётся, а не переполняет её.
                        let mut kid_par = kid_par;
                        for (p, (c, _)) in kid_par.iter_mut().zip(kids.iter()) {
                            if p.group == 0 {
                                p.avoid_only = c.style.break_inside_avoid && avoid_only_monolith(c);
                                p.float = c.attr("kamin-float-block").is_some();
                                p.clears = c.style.clear.is_some();
                            }
                        }
                        let kids = if col_vert {
                            kids
                        } else {
                            grow_pushed(kids, cols as usize, fixed, rows, copies, &kid_par)
                        };
                        // `box-decoration-break: clone`: геометрия фрагментов —
                        // ДО сборки копий: каждая копия такой коробки строится
                        // отдельной коробкой своей высоты (`clone_fragment`).
                        // Щуп — как у `grow_pushed`, но с НАСТОЯЩИМ параллельным
                        // потоком соседей (та же мера, что у `StackChild` ниже):
                        // иначе план соседа с потоком разошёлся бы с укладкой.
                        // Без `clone` среди детей не считается вовсе.
                        let clone_plan: Vec<Vec<(f32, f32)>> =
                            if !col_vert && kids.iter().any(|(c, _)| clone_dec(c).is_some()) {
                                let probe: Vec<crate::flow::Kid> = kids
                                    .iter()
                                    .enumerate()
                                    .map(|(pi, (c, s))| {
                                        let mut m = c.clone();
                                        m.style.margin.top = None;
                                        m.style.margin.bottom = None;
                                        let (over, cuts, forced, solid) = match shape_full(
                                            &m,
                                            4,
                                            ShapeCx {
                                                unclamped: true,
                                                ..ShapeCx::COLUMNS
                                            },
                                        )
                                        .filter(|_| {
                                            fixed.is_some()
                                                && plain_block_tree(&m, 4)
                                                && visible_overflow(&m.style)
                                        })
                                        .filter(|u| u.0 > s.0 + 0.01)
                                        {
                                            Some(u) => (u.0, u.3, u.4, u.5),
                                            None => (s.0, s.3.clone(), s.4.clone(), s.5.clone()),
                                        };
                                        crate::flow::Kid {
                                            h: s.0,
                                            mt: s.1,
                                            mb: s.2,
                                            monolith: solid_box(c),
                                            cuts,
                                            force_before: edge_break(c, false),
                                            force_after: edge_break(c, true),
                                            avoid_before: edge_avoid(c, false),
                                            avoid_after: edge_avoid(c, true),
                                            forced,
                                            solid,
                                            span: c.style.column_span == Some(true) && !c.inline,
                                            over,
                                            clone_dec: clone_dec(c),
                                            // Тот же предикат, что у `StackChild` ниже:
                                            // иначе план соседа разошёлся бы с укладкой.
                                            overflow_top: fixed.is_some()
                                                && rows.is_none()
                                                && !parallel_items_inside(c, 4),
                                            repeat: repeat_leads(c, fixed, rows),
                                            par: kid_par[pi],
                                        }
                                    })
                                    .collect();
                                crate::flow::ColumnStack::frags_of(
                                    &probe,
                                    cols as usize,
                                    fixed,
                                    rows,
                                    copies,
                                )
                            } else {
                                Vec::new()
                            };
                        let rule = if e.style.column_rule_visible == Some(true) {
                            Some((
                                match e.style.column_rule_width {
                                    Some(Len::Px(v)) => v,
                                    Some(Len::Em(k)) => {
                                        k * match e.style.font_size {
                                            Some(Len::Px(fs)) => fs,
                                            _ => opts.base_size(),
                                        }
                                    }
                                    _ => 3.0,
                                },
                                e.style
                                    .column_rule_color
                                    .or(merged.color)
                                    .unwrap_or(crate::value::Color {
                                        r: 0.0,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 1.0,
                                    })
                                    .to_hsla(),
                            ))
                        } else {
                            None
                        };
                        // С рядами линейки (`column-rule` со втяжкой/разрывом,
                        // `row-rule` — css-multicol-2 §rg/§crc → css-gaps-1)
                        // красит `GapRulePainter` по границам колонок и
                        // спаннеров из стопки (`ColumnStack::gap_items`);
                        // простая полоса `rule` тогда не рисуется. Без рядов —
                        // как прежде (`multicol-rule-*` не трогаются).
                        let gap_spec = rows
                            .filter(|r| r.wrap)
                            .and_then(|_| multicol_gap_rule_spec(e, &merged, opts, used_gap, row_gap));
                        let gap_items = gap_spec
                            .as_ref()
                            .map(|_| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt ^ 0x4D43_4F4C));
                        let rule = rule.filter(|_| gap_spec.is_none());
                        // Где начинается ребёнок в первой внешней колонке — для
                        // вложенного рядами с заданной высотой (`nest_row`): все
                        // предыдущие встают целиком в первую колонку, без
                        // принудительных разрывов и параллельных строк flex.
                        // Внешний многоколоночник с БАЛАНСОМ и единственным ребёнком —
                        // вложенным заданной высоты `h` без точек разреза: баланс
                        // делит его поровну, и высота внешней колонки известна до
                        // укладки — `h / cols` (не выше потолка коробки; css-multicol-1
                        // §7.1; `multicol-breaking-005`: 300 в трёх колонках по 100).
                        let balanced_frag: Option<f32> = (fixed.is_none()
                            && rows.is_none_or(|r| r.cap)
                            && cols > 1
                            && kids.len() == 1)
                            .then(|| {
                                let (c, s) = &kids[0];
                                (nested_rows_box(c)
                                    && matches!(c.style.height, Some(Len::Px(_)))
                                    && s.3.is_empty()
                                    && s.1.abs() < 0.01)
                                    .then(|| {
                                        let per = s.0 / cols as f32;
                                        rows.and_then(|r| r.h).map_or(per, |cap| per.min(cap))
                                    })
                            })
                            .flatten()
                            .filter(|h| *h > 1.0);
                        let fixed_nest = fixed.or(balanced_frag);
                        let nest_at: Vec<Option<f32>> = {
                            let mut v = Vec::with_capacity(kids.len());
                            let (mut y, mut prev_mb, mut ok) = (0.0f32, 0.0f32, true);
                            for (i, (c, s)) in kids.iter().enumerate() {
                                let lead = if i == 0 { s.1 } else { prev_mb.max(s.1) };
                                let hh = fixed_nest.unwrap_or(0.0);
                                v.push((ok && fixed_nest.is_some() && y + lead < hh - 0.01).then_some(y + lead));
                                if edge_break(c, false)
                                    || edge_break(c, true)
                                    || kid_par.get(i).is_some_and(|p| p.group != 0)
                                    || y + lead + s.0 > hh + 0.01
                                {
                                    ok = false;
                                }
                                y += lead + s.0;
                                prev_mb = s.2;
                            }
                            v
                        };
                        let children: Vec<crate::flow::StackChild> = kids
                            .into_iter()
                            .enumerate()
                            .map(|(ix, (c, (h, mt, mb, cuts, forced, solid)))| {
                                // `box-decoration-break: clone` — фрагменты ЭТОГО
                                // ребёнка по плану. Неразрезанная коробка идёт
                                // `slice`: вид тот же, а её переполнение остаётся
                                // параллельным потоком (`clone-003`: ребёнок 185
                                // в коробке 70 продолжается во второй колонке).
                                let frag_geom: Vec<(f32, f32)> =
                                    clone_plan.get(ix).cloned().unwrap_or_default();
                                let dec = clone_dec(&c).filter(|_| frag_geom.len() > 1 && !col_vert);
                                // Элемент строки flex (`split_flex_lines`) наследует от
                                // СВОЕГО контейнера, а не от многоколоночника.
                                let merged_k = kid_parent.get(ix).cloned().flatten();
                                let merged: &Computed = merged_k.as_ref().unwrap_or(&merged);
                                let mut copy = c;
                                // Поля кладёт укладка колонок, не коробка. В
                                // вертикальном письме блочные поля — левое и
                                // правое; схлопывание сквозь верх (`strip_through_top`)
                                // там не считается вовсе (мера повёрнутая).
                                if col_vert {
                                    copy.style.margin.left = None;
                                    copy.style.margin.right = None;
                                } else {
                                    copy.style.margin.top = None;
                                    copy.style.margin.bottom = None;
                                    // И поле, схлопнутое СКВОЗЬ верх (`through` в
                                    // `mt`): стопка уже положила его `lead`-ом,
                                    // второй раз его вставил бы корень копии.
                                    strip_through_top(&mut copy, 4);
                                }
                                // Коробка из одних флоатов меряется высотой их
                                // ряда (`float_only_box`), но сама по §10.6.3
                                // высотой НОЛЬ — вне колонок это делает
                                // `collapse_margins`, а копия фрагмента
                                // строится мимо него. Без нуля в каждой
                                // колонке проступил бы фон контейнера
                                // (`floats-clear-multicol-*`: `background:
                                // red`); флоаты переполняют нулевую коробку, и
                                // маска колонки режет их по разрезам меры.
                                if !col_vert && float_only_box(&copy).is_some() && through_strut(&copy).is_some() {
                                    copy.style.height = Some(Len::Px(0.0));
                                }
                                // Параллельный поток (css-break-3 §3):
                                // содержимое, переполняющее коробку с заданной
                                // высотой, продолжается в следующей колонке
                                // САМО ПО СЕБЕ, а сосед встаёт сразу под
                                // коробкой. Протяжённость потока — та же мера
                                // без обрезки высотой; вместе с ней берём её
                                // НЕусечённые точки разреза и монолитные
                                // диапазоны (в пределах `h` они совпадают с
                                // обычными: усечение только отбрасывает записи
                                // ниже `h`).
                                // ТРОЕ ворот, все замерены:
                                //  1) `column-fill: auto` — иначе поток уходит
                                //     в балансировку и меняет высоту колонки
                                //     (`single-line-column-flex-
                                //     fragmentation-051`);
                                //  2) поддерево обычных блоков — иначе мера
                                //     `shape_full` приближённая и
                                //     протяжённость выдуманная;
                                //  3) коробка своё переполнение показывает —
                                //     у обрезающей и прокручиваемой потока нет.
                                // Четвёртые ворота — внутри меры: бюджет
                                // разжатия ОДИН на путь (`ShapeCx::unclamped`,
                                // перепривязка `cx` в `shape_full`). Без него
                                // разъезжался ЭТАЛОН четырёх пар
                                // `flex-item-content-overflow-*`.
                                let copy_m = if col_vert { transpose_tree(&copy, col_rl) } else { None };
                                let (over, cuts, forced, solid) = match with_lines(&merged, line_col_w, opts, || shape_full(
                                    copy_m.as_ref().unwrap_or(&copy),
                                    4,
                                    ShapeCx {
                                        unclamped: true,
                                        ..ShapeCx::COLUMNS
                                    },
                                ))
                                .filter(|_| {
                                    // Сетка-стопка с обычными блочными элементами
                                    // меряется так же точно, как блок (`grid_stack`:
                                    // ряд = элемент), а `overflow-x: clip` блочное
                                    // переполнение не прячет (css-overflow-3:
                                    // `clip` парой к `visible` не делает коробку
                                    // прокручиваемой). `grid-container-
                                    // fragmentation-006`: сетка 200 с содержимым
                                    // 400 в четырёх колонках.
                                    let plain = plain_block_tree(&copy, 4)
                                        || stacked_flex_tree(&copy, 4)
                                        || (grid_stack(&copy)
                                            && copy.children.iter().all(|n| match n {
                                                Node::Element(k) => k.inline || plain_block_tree(k, 3),
                                                _ => true,
                                            }));
                                    let ov = copy_m.as_ref().unwrap_or(&copy);
                                    let block_visible = matches!(
                                        ov.style.overflow_y,
                                        None | Some(crate::computed::Overflow::Visible)
                                    ) && matches!(
                                        ov.style.overflow_x,
                                        None | Some(crate::computed::Overflow::Visible)
                                            | Some(crate::computed::Overflow::Clip)
                                    );
                                    fixed.is_some() && plain && block_visible
                                })
                                .filter(|s| s.0 > h + 0.01)
                                {
                                    Some(s) => (s.0, s.3, s.4, s.5),
                                    None => (h, cuts, forced, solid),
                                };
                                // Мера с дотягом внепоточных — поток, а не
                                // коробка (`OOF_OWN`): сосед встаёт под концом
                                // коробки, абсолют продолжается в колонках.
                                let (h, over) = match OOF_OWN.with(|m| m.borrow().get(&copy.node_id).copied()) {
                                    // Только когда за коробкой в стопке есть
                                    // сосед: последней коробке её дотяг —
                                    // мера колонок многоколоночника.
                                    Some((own, full))
                                        if (full - h).abs() < 0.01
                                            && !solid_box(&copy)
                                            && ix + 1 < kid_par.len() =>
                                    {
                                        (own, over.max(full))
                                    }
                                    _ => (h, over),
                                };
                                // Относительный сдвиг — не коробке, а фрагменту
                                // (css-break-3 §5.5): его кладёт `ColumnStack`
                                // вместе со срезом.
                                let rel = hoist_relative(&mut copy);
                                // Срез строки хоста (`kamin-host-trim-*`) — рисует
                                // `blocks()` копии по её флагам.
                                if copy.attr("kamin-host-trim-start").is_some() {
                                    copy.style.text_box_trim_start = true;
                                }
                                if copy.attr("kamin-host-trim-end").is_some() {
                                    copy.style.text_box_trim_end = true;
                                }
                                // Вложенный многоколоночник с ВЕРХА внешней колонки
                                // (первый ребёнок без поля) и заданной высотой —
                                // рядами во внешний фрагментаинер (`flow::OUTER_ROW`).
                                // С верха колонки граница k-го ряда — ровно k·H, и
                                // внешняя стопка режет коробку краем колонки
                                // (`fill_at`, не монолит) точно по рядам; мера
                                // коробки — её заданная высота. Сдвинутый вниз
                                // (первый ряд = остаток колонки) — следующий шаг.
                                let nest_row = fixed_nest.filter(|hh| {
                                    *hh > 0.0
                                        && (rows.is_none() || balanced_frag.is_some())
                                        && !col_vert
                                        && kid_par.get(ix).is_none_or(|p| p.group == 0)
                                        && nested_rows_box(&copy)
                                        && match nest_at.get(ix).copied().flatten() {
                                            // С верха колонки — и заданная высота, и
                                            // `auto` с мерой рядами.
                                            Some(y0) if y0 < 0.01 => {
                                                matches!(copy.style.height, Some(Len::Px(_)))
                                                    || nested_auto.borrow().contains(&copy.node_id)
                                            }
                                            // Ниже верха — только заданная высота: мера
                                            // коробки от рядов не зависит.
                                            Some(_) => matches!(copy.style.height, Some(Len::Px(_))),
                                            None => false,
                                        }
                                });
                                let nest_phase_k = nest_at.get(ix).copied().flatten().unwrap_or(0.0);
                                let inner = inline::inherit(&merged, &copy.style);
                                // Копии на случай разреза между колонками:
                                // элемент GPUI рисуется один раз, а фрагмент
                                // нужен свой в каждой колонке. Больше, чем
                                // колонок, ребёнок занять не может.
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): строить копию
                                // через `element(&copy, &merged, opts)`, чтобы сетка
                                // и гибкий контейнер внутри стопки рисовались
                                // (`grid-container-fragmentation-*`): срез
                                // фрагментации 445 -> 405 (+12/−52) — рамки, тени,
                                // `break-between-avoid-*`, `fieldset` ушли в
                                // красное: общий путь элемента кладёт слои и
                                // выносит абсолюты иначе, чем ждёт стопка.
                                // Возвращать узкой веткой только для сетки.
                                /// css-position-3 §abspos-breaking: «User
                                /// agents must not paginate the content of
                                /// fixed-positioned boxes». Копия фрагмента —
                                /// ПОЛНЫЙ клон поддерева, и `position: fixed`
                                /// внутри неё уезжает в слой ICB из КАЖДОЙ
                                /// копии (`render.rs:1790` -> `:1823` ->
                                /// `icb_push`). Слой лежит вне коробки
                                /// многоколоночника, маска колонки
                                /// (`flow.rs:998`) его не режет — на экране
                                /// вышло бы столько зелёных коробок, сколько
                                /// колонок. Оставляем фиксированного потомка
                                /// только в ПЕРВОЙ копии: там же, где стоит
                                /// его щуп статической позиции.
                                /// Blink делает это тем же разделением —
                                /// `out_of_flow_layout_part.cc:1607`: «This
                                /// does not include repeated fixed-positioned
                                /// elements».
                                /// Устанавливает ли коробка содержащий блок для
                                /// `position: fixed` (css-position-3 §fixed-cb:
                                /// «the nearest ancestor box that establishes a
                                /// fixed positioning containing block»;
                                /// css-transforms-1 §3: трансформ даёт
                                /// «containing block for all descendants … and
                                /// fixed-position descendants»). Список ДОСЛОВНО
                                /// тот же, что в `inline::inherit`
                                /// (`transform_ancestor`): `transform`,
                                /// `contain: layout`, `contain: paint`. Шире
                                /// брать нельзя — `blocks` решает по `under_tf`
                                /// из `inherit`, и расхождение дало бы коробку и
                                /// в копии, и в слое ICB.
                                fn fixed_cb_box(c: &crate::computed::Computed) -> bool {
                                    c.transform.is_some()
                                        || c.contain_layout == Some(true)
                                        || c.contain_paint == Some(true)
                                        || c.will_change & crate::computed::wc::CB_FIXED != 0
                                }
                                /// css-position-3 §abspos-breaking: «User
                                /// agents must not paginate the content of
                                /// fixed-positioned boxes». Копия фрагмента —
                                /// ПОЛНЫЙ клон поддерева, и `position: fixed`
                                /// внутри неё уезжает в слой ICB из КАЖДОЙ
                                /// копии (`render.rs:1790` -> `:1823` ->
                                /// `icb_push`). Слой лежит вне коробки
                                /// многоколоночника, маска колонки
                                /// (`flow.rs:998`) его не режет — на экране
                                /// вышло бы столько зелёных коробок, сколько
                                /// колонок. Оставляем фиксированного потомка
                                /// только в ПЕРВОЙ копии: там же, где стоит
                                /// его щуп статической позиции.
                                /// Blink делает это тем же разделением —
                                /// `out_of_flow_layout_part.cc:1607`: «This
                                /// does not include repeated fixed-positioned
                                /// elements».
                                ///
                                /// Запрет этот — про коробки, чей содержащий
                                /// блок ОКНО. Если содержащий блок `fixed`
                                /// лежит ВНУТРИ контекста фрагментации
                                /// (трансформированный или обособленный предок
                                /// внутри копии, либо сама коробка
                                /// многоколоночника), коробка — обычный абсолют
                                /// того предка, и §abspos-breaking выше требует
                                /// обратного: «positioned relative to its
                                /// containing block ignoring any fragmentation
                                /// breaks … may subsequently be broken over
                                /// several fragmentation containers». В слой ICB
                                /// такая коробка у нас и не уходит: `blocks`
                                /// (`render.rs:3966`) считает её `abs_like` при
                                /// `under_tf` и оставляет НА МЕСТЕ, значит копия
                                /// ≥ 1 без неё теряет единственную отрисовку.
                                /// Blink делит так же:
                                /// `fixedpos_containing_block`
                                /// (`out_of_flow_layout_part.cc:1369, :2978`) —
                                /// обычный фрагментаинерный потомок, и только
                                /// оконные попадают в
                                /// `repeated_fixedpos_descendants` (:1515).
                                ///
                                /// `fixed_cb` — встретился ли по пути ВНИЗ от
                                /// коробки многоколоночника предок, который
                                /// устанавливает содержащий блок для `fixed`.
                                /// Предки ВЫШЕ многоколоночника сюда не входят:
                                /// их содержащий блок вне контекста, коробка по
                                /// спеке одна, и место ей — копия 0.
                                fn drop_viewport_fixed(n: &Node, fixed_cb: bool) -> Option<Node> {
                                    match n {
                                        Node::Element(k)
                                            if !fixed_cb
                                                && k.style.position
                                                    == Some(crate::computed::Position::Fixed) =>
                                        {
                                            None
                                        }
                                        Node::Element(k) => {
                                            let mut c = k.clone();
                                            let deeper = fixed_cb || fixed_cb_box(&k.style);
                                            c.children = k
                                                .children
                                                .iter()
                                                .filter_map(|kid| drop_viewport_fixed(kid, deeper))
                                                .collect();
                                            Some(Node::Element(c))
                                        }
                                        other => Some(other.clone()),
                                    }
                                }
                                // Линейки промежутков (css-gaps-1) у копии
                                // фрагмента: слой строится ТОЛЬКО при заданном
                                // стиле линейки (`gap_rule_spec`), как в
                                // `element()`, — ни одна старая пара сюда не
                                // попадает. Буфер проб — СВОЙ на копию: пробы
                                // всех копий одного узла иначе сливаются в один
                                // буфер, первая копия забирает всё (`take`) и
                                // строит дорожки по смеси поднятых на `from`
                                // копий. Отрезки красятся в координатах полной
                                // раскладки копии, маска колонки (`flow.rs`,
                                // `with_content_mask`) режет их вместе с
                                // содержимым — вид `slice` css-break-3 §4.
                                let copy_ix = std::cell::Cell::new(0usize);
                                let whole = nest_row.is_none()
                                    && nested_whole.borrow().contains(&copy.node_id);
                                let build = |first: bool, part: usize| {
                                    // `box-decoration-break: clone`: копия — САМ
                                    // фрагмент (`clone_fragment`), и корень, и
                                    // наследуемый стиль берутся у НЕГО. ★ Откат
                                    // 07.09 (v153) держался ровно здесь: корень
                                    // строился со стилем ИСХОДНОЙ коробки, и
                                    // каждая копия выходила полной коробкой
                                    // (`clone-007` 2.08 = ровно квадрат 100×100,
                                    // `-026` 4.98 = 240×25×4 вылета). НЕпоследний
                                    // фрагмент режется по съеденному содержимому,
                                    // последний — по концу видимого переполнения
                                    // (`over`, `clone-002`). Лишние копии (фрагментов
                                    // меньше, чем копий) не ставятся и остаются
                                    // исходной коробкой.
                                    let frag = match (dec, frag_geom.get(part)) {
                                        (Some((dt, db)), Some(&(from, fh))) => {
                                            let clip = frag_geom.get(part + 1).map_or(
                                                (over - dt - db - from).max(fh - dt - db),
                                                |n| n.0 - from,
                                            );
                                            Some(clone_fragment(&copy, dt, db, from, fh, clip))
                                        }
                                        _ => None,
                                    };
                                    let frag_inner =
                                        frag.as_ref().map(|f| inline::inherit(&merged, &f.style));
                                    let src: &Element = frag.as_ref().unwrap_or(&copy);
                                    let src_inner: &Computed = frag_inner.as_ref().unwrap_or(&inner);
                                    let kids: Vec<Node> = if first {
                                        src.children.clone()
                                    } else {
                                        // Семя — только коробка многоколоночника
                                        // и корень копии. `e` считается наравне
                                        // с предками внутри копии: содержащий
                                        // блок `fixed` на самой коробке контекста
                                        // фрагментации — это Blink
                                        // `fixedpos_containing_block`
                                        // (`out_of_flow_layout_part.cc:1369`),
                                        // фрагментаинерный потомок, а не
                                        // повторяемая коробка. Предки ВЫШЕ `e`
                                        // не в счёт: их содержащий блок вне
                                        // контекста. `hoist_relative` выше
                                        // снимает лишь ВСТАВКИ,
                                        // `transform`/`contain` остаются на
                                        // месте — проверка по `copy.style`
                                        // законна.
                                        let fixed_cb_root =
                                            fixed_cb_box(&e.style) || fixed_cb_box(&copy.style);
                                        src.children
                                            .iter()
                                            .filter_map(|n| drop_viewport_fixed(n, fixed_cb_root))
                                            .collect()
                                    };
                                    let frag_gap_rules = gap_rule_spec(&copy, &inner, opts);
                                    let frag_gap_key = frag_gap_rules.as_ref().map(|_| {
                                        let ix = copy_ix.get();
                                        copy_ix.set(ix + 1);
                                        (copy.node_id ^ opts.doc_salt)
                                            ^ (ix as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15)
                                    });
                                    let frag_gap_guard =
                                        frag_gap_key.map(crate::interact::GapGuard::enter);
                                    // css-break-3 §5.5: «Fragmentation … occurs
                                    // before relative positioning, transforms,
                                    // and any other graphical effects. Such
                                    // effects are applied per fragment». Разрезы
                                    // трансформ не двигают (`shape_full` его и не
                                    // читает), но САМ трансформ обязан быть на
                                    // каждом фрагменте. Общий путь вешает его
                                    // через `transformed()` (render.rs:1719,
                                    // :5938, :8185); узкая ветка копии шла мимо
                                    // всех трёх, и `transform` у ребёнка
                                    // многоколоночника пропадал целиком
                                    // (`transform-000…005`: `translateX(60px)`
                                    // контейнера гасил `left:-60px` потомков, а
                                    // без него содержимое уезжало из колонки).
                                    // Начало отсчёта пока общее на всю коробку,
                                    // а не своё на фрагмент, — для `translate`
                                    // это точно, для `rotate`/`scale` нет.
                                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строить эту
                                    // копию через общий `element()` вместо узкой
                                    // ветки `styled_div_with`. Срез 3029 пар:
                                    // 1900 -> 1902 (+14/-12), и семь потерь —
                                    // грубые (99.00, страница разъезжается):
                                    // `multi-line-column-flex-fragmentation-035`,
                                    // `multi-line-row-flex-fragmentation-039/040/
                                    // 059`, `multicol-nested-013/021`,
                                    // `multicol-fill-balance-nested-000`. Тот же
                                    // путь, на котором прежде мерился откат -52.
                                    // Копия ТАБЛИЦЫ — своим рисователем.
                                    // `styled_div_with` + `blocks` кладут детей
                                    // таблицы обычными блоками, и `element()`
                                    // заворачивает КАЖДЫЙ ряд в СВОЮ анонимную
                                    // таблицу (ветка `TableRowGroup | TableRow |
                                    // TableCell`, render.rs:11622): дорожки
                                    // считаются по одному ряду, ячейка сжимается
                                    // по содержимому, а фон ряда и ячейки
                                    // теряется вовсе. Фрагментация идёт ДО
                                    // графических эффектов и применяется к
                                    // каждому фрагменту (css-break-3 §5.5), но
                                    // РАСКЛАДКА фрагмента — та же табличная
                                    // (css-tables-3 §fragmentation).
                                    // ЗАМЕРЕНО пробами (`target/probe-bt/`,
                                    // стенд v150): фон САМОЙ таблицы рисуется
                                    // (`p4-2col-bgtbl` 0.00) и блок с шириной в
                                    // точках рисуется (`p5-b-w100-div50` 0.00), а
                                    // фон ячейки (`p6-td-bg`), фон ряда
                                    // (`p6-tr-bg`), `width: auto`
                                    // (`p5-c-w100-divauto`) и `width: 100%`
                                    // (`p6-div-w100pct`) не рисуются НИЧЕМ —
                                    // 15625 красных точек из 15625.
                                    // `transformed` — как в общей ветке ниже:
                                    // `table()` его не вешает (в `element()`
                                    // таблица идёт мимо него), а копия обязана
                                    // нести трансформ на каждом фрагменте.
                                    if let Some(hh) = nest_row {
                                        let mut mc = copy.clone();
                                        mc.children = kids;
                                        // Своя метка узла на каждую копию: буфер линеек
                                        // промежутков (`gap_items_for` по `node_id`) у
                                        // копий одного узла сливался в один, и первая
                                        // копия забирала линейки всех рядов — во
                                        // втором и третьем ряду их не было
                                        // (`multicol-breaking-002`, 0.65).
                                        mc.node_id ^= (part as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15);
                                        // Ширина `auto` — по колонке (CSS 2.1 §10.3.3):
                                        // копия кладётся корнем, и её многоколоночнику
                                        // нужна ширина в точках для меры строк.
                                        if matches!(mc.style.width, None | Some(Len::Auto))
                                            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
                                        {
                                            mc.style.width = Some(Len::Px(w));
                                        }
                                        // `height: auto` — высота из меры рядами
                                        // (`nested_rows_shape`): стопка с рядами
                                        // отдаёт полный последний ряд, а коробка
                                        // кончается на сбалансированном хвосте.
                                        if matches!(mc.style.height, None | Some(Len::Auto)) {
                                            let b = mc.style.borders();
                                            let px = |l: &Option<Len>| match l {
                                                Some(Len::Px(v)) => *v,
                                                _ => 0.0,
                                            };
                                            let bot = px(&mc.style.padding.bottom) + px(&b.bottom);
                                            mc.style.height = Some(Len::Px((h - bot).max(0.0)));
                                            mc.style.border_box = None;
                                        }
                                        drop(frag_gap_guard);
                                        crate::flow::set_outer_row(Some((hh, nest_phase_k)));
                                        let el = element(&mc, &merged, opts);
                                        crate::flow::set_outer_row(None);
                                        return el;
                                    }
                                    if whole {
                                        let mut mc = copy.clone();
                                        mc.children = kids;
                                        if matches!(mc.style.width, None | Some(Len::Auto))
                                            && let Some(w) = line_col_w.and_then(|cw| nested_box_w(&mc, cw))
                                        {
                                            mc.style.width = Some(Len::Px(w));
                                        }
                                        drop(frag_gap_guard);
                                        return element(&mc, &merged, opts);
                                    }
                                    if table_box(&copy) {
                                        let mut tc = copy.clone();
                                        tc.children = kids;
                                        drop(frag_gap_guard);
                                        return transformed(
                                            table(&tc, &inner, opts),
                                            &inner,
                                            &merged,
                                        );
                                    }
                                    // Абсолютный потомок ищет ближайшего
                                    // позиционированного предка (CSS 2.1
                                    // §10.1), а раскладка под нами знает только
                                    // непосредственного родителя: коробка, чей
                                    // родитель содержащим блоком НЕ является,
                                    // уезжает в слой (`cb_push`,
                                    // render.rs:4230 `to_cb`). Общий путь
                                    // `element()` слой заводит
                                    // (render.rs:13511-13529), а узкая ветка
                                    // копии фрагмента возвращается из
                                    // `element()` раньше
                                    // (`return d.into_any_element()`,
                                    // render.rs:13274) — и до сих пор такая
                                    // коробка либо всплывала в ЧУЖОЙ внешний
                                    // слой (позиционированный предок ВЫШЕ
                                    // многоколоночника), либо, слоя нет,
                                    // рисовалась на месте: от края случайного
                                    // родителя вместо содержащего блока.
                                    //
                                    // css-position-3 §abspos-breaking: «In a
                                    // fragmented flow, an absolutely positioned
                                    // box is positioned relative to its
                                    // containing block ignoring any
                                    // fragmentation breaks (as if the flow were
                                    // continuous). The box may subsequently be
                                    // broken over several fragmentation
                                    // containers». Копия и есть этот
                                    // непрерывный поток: `flow.rs` `prepaint`
                                    // кладёт её `layout_as_root(Definite(col_w),
                                    // Definite(full_h))` во всю высоту и
                                    // поднимает на срез, а колонку вырезает
                                    // маска — коробке, попавшей в слой КОРНЯ
                                    // КОПИИ, фрагментация достаётся даром. То же
                                    // деление у Blink: кандидат, чей содержащий
                                    // блок внутри контекста, идёт
                                    // `LayoutFragmentainerDescendants`
                                    // (`out_of_flow_layout_part.cc:1498`).
                                    //
                                    // Предикат — тот же `establishes_cb`, что в
                                    // `element()`, и по стилю КОПИИ:
                                    // `hoist_relative` выше снимает только
                                    // ВСТАВКИ, сам `position: relative` (как и
                                    // `transform`/`contain`) на копии остаётся.
                                    // Прямые дети копии ничего не меняют: у них
                                    // `establishes_cb(inherited)` истинно, они и
                                    // раньше рисовались на месте.
                                    // Имена областей сетки — в номера линий, как в
                                    // общем `element()` (`place_named_areas`): ни
                                    // GPUI, ни taffy имён не знают, и копия
                                    // фрагмента клала элементы автоматически
                                    // (`grid-item-fragmentation-026`: оба в
                                    // области `a`, второй уезжал во 2-й ряд).
                                    let kids = match &copy.style.grid_areas {
                                        Some(areas)
                                            if matches!(
                                                copy.style.display,
                                                Some(Display::Grid) | Some(Display::InlineGrid)
                                            ) =>
                                        {
                                            place_named_areas(areas, kids)
                                        }
                                        _ => kids,
                                    };
                                    let frag_cb_layer = crate::inline::establishes_cb(&inner);
                                    if frag_cb_layer {
                                        crate::interact::cb_open_with(fixed_cb_layer_box(&inner));
                                    }
                                    let mut body = blocks(&kids, src_inner, opts);
                                    if frag_cb_layer {
                                        body.extend(crate::interact::cb_close());
                                    }
                                    drop(frag_gap_guard);
                                    let mut d = styled_div_with(src, src_inner);
                                    // Ось блочного потока ВНУТРИ копии — горизонтальная
                                    // (css-writing-modes-4 §3.1), как у вертикального
                                    // блока в `element()`: гибкий ряд, у `vertical-rl`
                                    // обратный. Гибкому и сеточному ось ставит `apply`.
                                    if col_vert && matches!(src.style.display, None | Some(Display::Block)) {
                                        d = d.flex();
                                        d = if col_rl { d.flex_row_reverse() } else { d.flex_row() };
                                    }
                                    // Голый `styled_div_with` — БЛОК taffy (`apply.rs`
                                    // `apply_layout`: блоку вызова нет, gpui `Display::Block`
                                    // → taffy Block), а блок общего пути — гибкая колонка
                                    // (`d.flex().flex_col()` при пустом `display`, та же
                                    // оболочка у спаннера выше). На колонку опирается
                                    // `blocks()`: коробке с `aspect-ratio`, auto-шириной и
                                    // высотой в точках он ставит `Align::Start` (css-sizing-4
                                    // §5.1 «calculated the same as for a replaced element
                                    // with a natural aspect ratio»; Blink `length_utils.cc:
                                    // 535-562` → `FitContent`), а блочный алгоритм taffy
                                    // `align-self` не читает и тянет её во всю ширину
                                    // родителя. Прежде вылет прятала маска шириной в
                                    // колонку; после multicol-rest P7 (css-multicol-1 §8.1:
                                    // «visibly overflows and is not clipped to the column
                                    // box») он виден (`block-aspect-ratio-052`: зелёный 345
                                    // вместо 25, четыре фрагмента — 420×100). Гейт узкий —
                                    // только копия с таким ребёнком; колонка для ЛЮБОЙ
                                    // копии блока — отдельным замером.
                                    let ratio_kid = |n: &Node| {
                                        matches!(n, Node::Element(k)
                                            if !k.inline
                                                && k.style
                                                    .aspect_ratio
                                                    .is_some_and(|r| r.is_finite() && r > 0.0)
                                                && matches!(k.style.width, None | Some(Len::Auto))
                                                && matches!(k.style.height, Some(Len::Px(_))))
                                    };
                                    if src.style.display.is_none()
                                        && src_inner.vertical != Some(true)
                                        && kids.iter().any(ratio_kid)
                                    {
                                        d = d.flex().flex_col();
                                    }
                                    // Для ЛЮБОЙ flex/grid-копии, не только с линейками:
                                    // эталоны css-gaps (`…-fragmentation-008-ref`) кладут
                                    // ту же сетку без правил, и с гейтом «только с
                                    // линейками» тест рисовал сетку, а эталон — нет
                                    // (v93: 008 3.75, 009 5.18, 010 4.50).
                                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v94): то же для
                                    // flex-копий. css-break 2874: +26/−15, и девять потерь
                                    // — 99.00 (`multi-line-row-flex-fragmentation-084…090`,
                                    // `multi-line-column-flex-fragmentation-056/057`:
                                    // страница разъезжается), ещё 065–071 на 1.5–13.
                                    // Сетка даёт +17 в css-break без потерь.
                                    // Flex-копия — во всю ширину колонки, как и
                                    // сетка: flex-корень с `width: auto` taffy
                                    // кладёт шириной СОДЕРЖИМОГО (`flexbox.rs`
                                    // `determine_container_main_size`, ветвь
                                    // `Definite` → `longest_line_length`), и
                                    // элемент `width: 100%` выходил нулевым, а
                                    // `width: 100px` в колонке 50 не сжимался.
                                    // Замер одного этого (v94): +9/−15, все
                                    // потери — `row-gap`, их закрывает мера
                                    // гибкой стопки (`flex_items` в `shape_full`).
                                    if matches!(copy.style.width, None | Some(Len::Auto))
                                        && matches!(
                                            copy.style.display,
                                            Some(Display::Grid)
                                                | Some(Display::InlineGrid)
                                                | Some(Display::Flex)
                                        )
                                    {
                                        d = d.w_full();
                                    }
                                    // Пустая сетка: taffy раскладывает бездетный
                                    // узел ЛИСТОМ (vendor/taffy/src/tree/
                                    // taffy_tree.rs: `(_, false) =>
                                    // compute_leaf_layout`), явные дорожки ему
                                    // не видны, и копия выходила нулевой при
                                    // мере 200 (`grid-container-fragmentation-
                                    // 002`: `grid-template-rows: 200px`). Дорожка
                                    // существует без элементов (css-grid-1
                                    // §7.1) — высота копии та же, что в мере.
                                    if kids.is_empty()
                                        && matches!(
                                            copy.style.display,
                                            Some(Display::Grid) | Some(Display::InlineGrid)
                                        )
                                        && grid_rows_px(&copy.style).is_some()
                                    {
                                        d = d.min_h(px(h));
                                    }
                                    if let (Some(key), Some(spec)) = (frag_gap_key, frag_gap_rules) {
                                        // Копия кладётся `layout_as_root(Definite(col_w), …)`
                                        // (`flow.rs` `ColumnStack::prepaint`), а taffy у
                                        // flex/grid-КОРНЯ с `width: auto` берёт размер
                                        // содержимого, не доступное место
                                        // (`vendor/taffy/src/compute/flexbox.rs`
                                        // `determine_container_main_size`, ветвь
                                        // `Definite` → `longest_line_length`): дорожки
                                        // `1fr` выходили нулевыми, и вся сетка была
                                        // невидима (`grid-gap-decorations-fragmentation-
                                        // 008/010/016`: только серый фон). Блок
                                        // растягивается сам; flex/grid получают 100% —
                                        // корень разрешает долю против `available_space`
                                        // (`taffy/src/compute/mod.rs` `compute_root_layout`).
                                        // Под детьми копии — как в `element()`
                                        // (css-gaps-1: «just above the border»).
                                        body.insert(
                                            0,
                                            crate::interact::GapRulePainter::new(
                                                crate::interact::gap_items_for(key),
                                                spec,
                                            )
                                            .into_any_element(),
                                        );
                                    }
                                    transformed(
                                        d.children(body).into_any_element(),
                                        &inner,
                                        &merged,
                                    )
                                };
                                // Монолиты (css-break-3 §4.1) — их разрыв
                                // запрещён, и в следующую колонку они уходят
                                // целиком: `break-inside: avoid`,
                                // прокручиваемая или обрезающая коробка,
                                // замещаемый элемент, таблица и ячейка,
                                // атомарная строчная коробка. Сюда же —
                                // сплошной СТРОЧНЫЙ набор: резать его можно
                                // только между строками, а строк укладка
                                // колонок не видит, и разрез приходился бы
                                // посреди строки.
                                // Монолитен ПРОКРУЧИВАЕМЫЙ контейнер (css-break-4
                                // §4.1 «scroll containers»); `hidden`/`clip` —
                                // обрезка, не прокрутка, и режется как блок
                                // (корень A4).
                                let scrolls = |o: Option<crate::computed::Overflow>| {
                                    matches!(o, Some(crate::computed::Overflow::Scroll))
                                };
                                let block_kid = |n: &Node| {
                                    matches!(n, Node::Element(k)
                                        if !k.inline || k.style.display == Some(Display::Block))
                                };
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): `contain: size` как
                                // монолит (Blink `IsMonolithic`) — срез фрагментации
                                // 469 -> 467 (+1/−3): `single-line-column-flex-
                                // fragmentation-051/063` режутся у Blink иначе (рост
                                // элемента от фрагментации, корень R5 скаута).
                                // Рост лёг `705fd58`; `contain: size` — `size_monolith`,
                                // тот же предикат, что у `solid_box` в мере и пробе
                                // `grow_pushed` (`scout-break-2026-09e.md`).
                                let monolith = nest_row.is_none() && (size_monolith(&copy)
                                    || copy.style.break_inside_avoid
                                    || scrolls(copy.style.overflow_x)
                                    || scrolls(copy.style.overflow_y)
                                    || matches!(
                                        copy.tag.as_str(),
                                        "img"
                                            | "svg"
                                            | "canvas"
                                            | "video"
                                            | "embed"
                                            | "object"
                                            | "iframe"
                                    )
                                    // Таблица и ячейка — не монолиты
                                    // (css-break-4 §4.1).
                                    || matches!(
                                        copy.style.display,
                                        Some(Display::InlineBlock)
                                            | Some(Display::InlineFlex)
                                            | Some(Display::InlineGrid)
                                    )
                                    // Сплошной СТРОЧНЫЙ набор тоже монолит:
                                    // резать его можно лишь между строками, а
                                    // строк укладка колонок не видит, и разрез
                                    // приходился бы посреди строки.
                                    // ПУСТАЯ коробка с высотой режется по своей
                                    // высоте (css-break-4 §4.2; корень A3).
                                    || (copy.children.iter().any(|n| !is_blank(n))
                                        && !copy.children.iter().any(block_kid)
                                        // Строки измерены (`line_run_shape`) — режется
                                        // между строк, не монолит.
                                        && cuts.is_empty()));
                                // Высоту меряет раскладка копии (`StackChild::measure`):
                                // точек разреза мера не дала, и строчный набор без них
                                // не монолит — режется краем колонки. Монолит — только
                                // по собственным причинам коробки (css-break-3 §4.1).
                                let measure = measured_kids
                                    .borrow()
                                    .iter()
                                    .find(|(id, _)| *id == copy.node_id)
                                    .map(|(_, w)| *w);
                                let monolith = if whole {
                                    true
                                } else if measure.is_some() {
                                    nest_row.is_none()
                                        && (size_monolith(&copy)
                                            || copy.style.break_inside_avoid
                                            || scrolls(copy.style.overflow_x)
                                            || scrolls(copy.style.overflow_y))
                                } else {
                                    monolith
                                };
                                // Пока строятся копии — «внутри стопки»: вложенный
                                // многоколоночник со спаннером остаётся на
                                // сегментном пути (см. `unified` выше).
                                let _nested = crate::flow::StackScope::enter();
                                let span = copy.style.column_span == Some(true) && !copy.inline;
                                // Переполняющие колонки (css-multicol-1 §8.2: «A multicol
                                // container can have more columns than it has room for due
                                // to: a declaration that constrains the column height … In
                                // this case, additional column boxes are created in the
                                // inline direction») — ТОЛЬКО ребёнку, который несёт
                                // абсолютного потомка: его содержащий блок сплошной, и
                                // абсолют режется по колонкам сам (css-position-3
                                // §abspos-breaking), а копий у ребёнка было ровно
                                // `column-count` — хвост уходил «за кадр» (`flow.rs`
                                // `fill_at`, `copy + 1 >= limit`;
                                // `out-of-flow-in-multicolumn-007`: CB 300 при колонке 100,
                                // копий 2 из 3). ★ Прежний патч без гейта «несёт абсолют»
                                // (scout-fragoof-2026-09d §7) замерен +7/−14: все потери —
                                // дети БЕЗ абсолютов (вложенные многоколоночники, флекс,
                                // `multicol-fill-balance-*`), у которых мера `shape_full`
                                // не совпадает с рисунком. Гейт `plain_block_tree` — тот же,
                                // что у параллельного потока выше. Прочим детям — прежнее
                                // число копий, и стопка без такого ребёнка байт-в-байт
                                // прежняя (`ColumnStack::new` берёт наибольшее число копий).
                                let kid_copies = match fixed {
                                    Some(per)
                                        if rows.is_none()
                                            && !span
                                            && per > 0.0
                                            && visible_overflow(&e.style)
                                            && visible_overflow(&copy.style)
                                            && plain_block_tree(&copy, 4)
                                            && carries_abspos(&copy, 4) =>
                                    {
                                        ((h.max(over) / per).ceil() as usize + 1)
                                            .min(16)
                                            .max(copies)
                                    }
                                    // Высота неизвестна до раскладки: копий — на
                                    // переполняющие колонки (css-multicol-1 §8.2).
                                    Some(per) if measure.is_some() && rows.is_none() && !span && per > 0.0 => {
                                        copies.max(8).min(16)
                                    }
                                    _ => copies,
                                };
                                crate::flow::StackChild {
                                    measure,
                                    el: side_margin_wrap(build(true, 0), &copy, col_vert),
                                    frags: if span {
                                        Vec::new()
                                    } else {
                                        (1..kid_copies)
                                            .map(|i| side_margin_wrap(build(false, i), &copy, col_vert))
                                            .collect()
                                    },
                                    monolith,
                                    cuts,
                                    // Тот же подъём, что в мере (Х5): иначе
                                    // укладка колонок не увидит разрыва,
                                    // который мера уже посчитала.
                                    force_before: edge_break(&copy, false),
                                    force_after: edge_break(&copy, true),
                                    avoid_before: edge_avoid(&copy, false),
                                    avoid_after: edge_avoid(&copy, true),
                                    forced,
                                    solid,
                                    h,
                                    mt,
                                    mb,
                                    span,
                                    over,
                                    rel,
                                    clone_dec: dec,
                                    // Монолит с верха колонки переполняет её
                                    // (`flow.rs` `fill_at`, `overflow_to`) —
                                    // только при `column-fill: auto` без рядов
                                    // и без элементов ряда в поддереве.
                                    overflow_top: fixed.is_some()
                                        && rows.is_none()
                                        && !parallel_items_inside(&copy, 4),
                                    // Вложенный многоколоночник — маска режет вбок
                                    // (`flow.rs` `StackChild::nested_cols`).
                                    nested_cols: multicol_inside(&copy, 4),
                                    par: kid_par[ix],
                                    positioned: !span
                                        && (matches!(
                                            copy.style.position,
                                            Some(crate::computed::Position::Relative)
                                                | Some(crate::computed::Position::Sticky)
                                        ) || copy.style.transform.is_some())
                                        && copy.style.z_index.unwrap_or(0) == 0,
                                    // Хвост непоследнего фрагмента таблицы — её фоном
                                    // (`flow.rs` `StackChild::slack`).
                                    slack: if col_vert {
                                        None
                                    } else if table_box(&copy) {
                                        copy.style.background.map(|c| c.to_hsla())
                                    } else if dec.is_none() && over <= h + 0.01 {
                                        // Продолжение одного лишь параллельного
                                        // потока (`over`) — не продолжение коробки:
                                        // она кончилась, хвоста у неё нет.
                                        slack_fill(&copy)
                                    } else {
                                        None
                                    },
                                    laid_w: Default::default(),
                                    // Повтор шапки/подвала таблицы — полосы своими
                                    // копиями (`flow::Repeat`); та же мера, что у
                                    // щупов (`repeat_leads`).
                                    repeat: repeat_bands(&copy, fixed, rows)
                                        .filter(|_| !span && !col_vert)
                                        .map(|(head, foot, geom)| crate::flow::Repeat {
                                            head,
                                            foot,
                                            geom,
                                            head_els: match head {
                                                Some(_) => (1..kid_copies).map(|i| build(false, i)).collect(),
                                                None => Vec::new(),
                                            },
                                            foot_els: match foot {
                                                Some(_) => (0..kid_copies).map(|i| build(false, i)).collect(),
                                                None => Vec::new(),
                                            },
                                        }),
                                }
                            })
                            .collect();
                        // Щуп статической позиции — НУЛЕВОЙ записью стопки на
                        // месте позиционированного ребёнка: высоты нет, полей
                        // нет, точек разреза нет, монолитных диапазонов нет —
                        // план укладки от него не двигается (`fill_at`:
                        // `rest = 0` всегда влезает в остаток колонки), а
                        // холст `interact::spot_probe` запоминает экранную
                        // дырку. Сама коробка в стопку НЕ идёт: она рисуется
                        // после стопки заместителем `spot_place` и сдвигается
                        // в эту дырку. Тем это отличается от прежней пробы
                        // «нулевая запись в стопке», о которой говорит
                        // комментарий у `direct_oof`: там в колонку уходил САМ
                        // элемент, и его резала маска
                        // (`out-of-flow-in-multicolumn-094…097`).
                        //
                        // Blink берёт статическую позицию оттуда же —
                        // `out_of_flow_layout_part.cc`,
                        // `LayoutFragmentainerDescendants`: позиция кандидата
                        // считается относительно ФРАГМЕНТАИНЕРА.
                        let mut children = children;
                        let oof_spots: Vec<crate::interact::SpotCell> =
                            oof_static.iter().map(|_| Default::default()).collect();
                        for (i, (at, oof)) in oof_static.iter().enumerate().rev() {
                            // Заданную ось считает раскладка от содержащего
                            // блока, щуп правит только ПУСТУЮ (CSS 2.1
                            // §10.3.7) — тот же гейт `fixed_axes`, что у слоёв
                            // в `blocks()`.
                            oof_spots[i].set(crate::interact::Spot {
                                fixed_axes: (
                                    edge_set(oof.style.inset.left)
                                        || edge_set(oof.style.inset.right),
                                    edge_set(oof.style.inset.top)
                                        || edge_set(oof.style.inset.bottom),
                                ),
                                rtl: merged.rtl == Some(true),
                                vertical: merged.vertical == Some(true),
                                vertical_rl: merged.vertical_rl == Some(true),
                                own_vertical: oof.style.vertical == Some(true),
                                ..Default::default()
                            });
                            let probe = crate::flow::StackChild {
                                measure: None,
                                el: crate::interact::spot_probe(oof_spots[i].clone(), true),
                                frags: Vec::new(),
                                monolith: false,
                                cuts: Vec::new(),
                                force_before: false,
                                force_after: false,
                                avoid_before: false,
                                avoid_after: false,
                                forced: Vec::new(),
                                solid: Vec::new(),
                                h: 0.0,
                                mt: 0.0,
                                mb: 0.0,
                                span: false,
                                over: 0.0,
                                rel: (0.0, 0.0),
                                clone_dec: None,
                                overflow_top: false,
                                nested_cols: false,
                                repeat: None,
                                par: crate::flow::Par::default(),
                                slack: None,
                                laid_w: Default::default(),
                                positioned: false,
                            };
                            // Номер — среди ДЕТЕЙ ДО раскрытия строк flex (`split_flex_lines`).
                            let at = kid_starts.get(*at).copied().unwrap_or(children.len()).min(children.len());
                            children.insert(at, probe);
                        }
                        // Стопка тянется по СТРОЧНОЙ оси: в вертикальном письме
                        // это высота, значит коробка кладёт её гибким рядом
                        // (поперечная ось растягивает высоту), у `vertical-rl` —
                        // от ПРАВОГО края (`flex_row_reverse`), там начало
                        // блочной оси.
                        let d = if col_vert {
                            let d = d.flex();
                            if col_rl { d.flex_row_reverse() } else { d.flex_row() }
                        } else {
                            d
                        };
                        let mut d = d.child(
                            crate::flow::ColumnStack::new(
                                children,
                                cols as usize,
                                used_gap,
                                fixed,
                                rule,
                                rows,
                                gap_items.clone(),
                                intrinsic_inline_size(&e.style, inherited).then(|| {
                                    crate::flow::Intrinsic(match column_width {
                                        Some(Len::Px(w)) if w > 0.0 => Some(w),
                                        _ => None,
                                    })
                                }),
                            )
                            .with_axis(col_axis)
                            .with_row_phase(if nest_rows.is_some() { nest_phase } else { 0.0 })
                            // Линейки последней линии — до низа содержимого коробки
                            // заданной высоты (Blink `PaintColumnRules`), без
                            // спаннеров и рядов (`multicol-rule-nested-balancing-001`).
                            .with_rule_stretch(
                                match merged.height {
                                    Some(Len::Px(h))
                                        if h > 0.0
                                            && !col_vert
                                            && nest_rows.is_none()
                                            && e.style.border_box != Some(true)
                                            && !e.children.iter().any(|n| matches!(n, Node::Element(c) if spanner_box(c))) =>
                                    {
                                        Some(h)
                                    }
                                    _ => None,
                                },
                            ),
                        );
                        // Флоаты — прежним ходом, соседями стопки.
                        for oof in &direct_oof {
                            d = d.child(element(oof, &merged, opts));
                        }
                        // Заместитель на месте щупа: рисуется ПОСЛЕ стопки и
                        // после флоатов (позиционированная коробка выше и
                        // поточного содержимого, и плавающих — CSS 2.1 §9.9,
                        // шаг 8 против шагов 4 и 5; на этом держится
                        // `abspos-after-spanner`, где под зеленью поточная
                        // красная коробка), а встаёт туда, где щуп стоял в
                        // колонке.
                        // Процентная высота абсолюта — от высоты отбивки
                        // содержащего блока (CSS 2.1 §10.5, §10.1 п. 4), а здесь им
                        // служит САМ многоколоночник. Заместитель же кладёт коробку
                        // в нулевую обёртку (`spot_place`), и раскладка под нами
                        // считала проценты от неё — коробка схлопывалась в ноль
                        // (`single-line-row-flex-fragmentation-019/020`: `height:
                        // 50%` без `top`). Пересчитываем в точки заранее, когда
                        // высота многоколоночника известна в точках.
                        let cb_h: Option<f32> = (e.style.position.is_some()
                            && e.style.position != Some(crate::computed::Position::Static)
                            && !col_vert)
                            .then(|| {
                                let px = |l: Option<Len>| match l {
                                    None => Some(0.0),
                                    Some(Len::Px(v)) => Some(v),
                                    _ => None,
                                };
                                let b = e.style.borders();
                                let pad = px(e.style.padding.top)? + px(e.style.padding.bottom)?;
                                let bor = px(b.top)? + px(b.bottom)?;
                                match e.style.height {
                                    Some(Len::Px(h)) if e.style.border_box == Some(true) => Some((h - bor).max(pad)),
                                    Some(Len::Px(h)) => Some(h.max(0.0) + pad),
                                    _ => None,
                                }
                            })
                            .flatten();
                        for (i, (_, oof)) in oof_static.iter().enumerate() {
                            let mut oof = oof.clone();
                            if let Some(ch) = cb_h
                                && oof.style.position == Some(crate::computed::Position::Absolute)
                            {
                                for l in [&mut oof.style.height, &mut oof.style.min_height, &mut oof.style.max_height] {
                                    if let Some(Len::Pct(k)) = *l {
                                        *l = Some(Len::Px(k * ch));
                                    }
                                }
                            }
                            d = d.child(crate::interact::spot_place(
                                oof_spots[i].clone(),
                                element(&oof, &merged, opts),
                            ));
                        }
                        if let (Some(buf), Some(spec)) = (gap_items, gap_spec) {
                            d = d.child(crate::interact::GapRulePainter::new(buf, spec).into_any_element());
                        }
                        return d.into_any_element();
                    }
                    let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
                    let rows = count.div_ceil(cols as usize).max(1) as u16;
                    let gap = used_gap;
                    d = d
                        .grid()
                        .grid_template_cols(
                            (0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect(),
                        )
                        .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
                        .gap_x(px(gap));
                    d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
                }
            } else if let Some(Len::Px(w)) = column_width {
                // Ширина колонки без их числа — это «сколько влезет»: ровно
                // то, что умеет короткая форма дорожек в GPUI.
                d = d.grid().grid_cols_min(px(w));
            // Ось блочного потока не зависит от `display` (css-writing-modes-4
            // §3.1): inline-block, ячейка, list-item с вертикальным письмом
            // раскладывают детей той же горизонтальной осью, что и голый блок
            // (`block-flow-direction-*`, `line-box-direction-*`).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: явный `Some(Block)` — он же стоит у
            // блокифицированных (абсолют в сетке), и
            // `grid-positioned-children-writing-modes-001` 0.39 -> 1.32 даже с
            // гейтом «родитель не сетка».
            // `list-item` сюда НЕ пускать (★ ЗАМЕРЕНО: `li` с одним текстом
            // становился рядом — `line-box-direction-vrl-019/vlr-020`
            // 6.58 -> 12.55).
            } else if merged.vertical == Some(true)
                // `display: table-caption` — это `Display::Block` С МЕТКОЙ
                // (`computed.rs:3125`), и голый `Some(Block)` сюда пускать
                // нельзя (замеренный откат выше). Метку же ставит ТОЛЬКО
                // само объявление `display: table-caption`, тег `<caption>`
                // её не несёт, а подпись ВНУТРИ стола сюда не приходит вовсе
                // — её строит `table()`. Письмо применяется к подписи
                // (css-writing-modes-4 §3.1, «Applies to: all elements
                // except table row groups, column groups, rows, columns»),
                // значит ось её блочного потока задаёт письмо, а не `display`.
                // Blink: `table_layout_algorithm.cc:67`
                // `ConstraintSpaceBuilder(space, caption.Style().GetWritingDirection(), true)`.
                && (matches!(
                    e.style.display,
                    None | Some(Display::InlineBlock) | Some(Display::TableCell)
                ) || e.style.is_caption == Some(true)
                    // Объявленный `display: flow-root` — тот же блок со своим
                    // контекстом (`computed.rs` ставит ему `Some(Block)` с
                    // меткой `flow_root`); блокифицированный абсолют метки не
                    // несёт, откат выше его не касается. Без этого
                    // `flow-root` в `vertical-rl` раскладывал детей
                    // горизонтальным блоком: хост полос мерился по
                    // min-content, флоаты-колонки эталона
                    // `css-break/background-image-001` вставали поперёк строки
                    // и пропадали.
                    || (e.style.flow_root == Some(true)
                        && e.style.display == Some(Display::Block)))
            {
                // Вертикальное письмо: ось блочного потока — горизонтальная.
                // Дети идут слева направо (`vertical-lr`) или справа налево
                // (`vertical-rl`).
                d = d.flex();
                d = if merged.vertical_rl == Some(true) {
                    d.flex_row_reverse()
                } else {
                    d.flex_row()
                };
                // `sideways-lr`: строка идёт снизу вверх — начало строчной
                // оси у НИЖНЕГО края (css-writing-modes-4 §block-flow).
                // Прижим коробки к низу — ЗАПЛАТКА того времени, когда абзац
                // вертелся по часовой и его содержимое росло от верха.
                // С поворотом против часовой (`VerticalText::ccw`) строка сама
                // начинается у нижнего края, и второй прижим снова уводит
                // рисунок. Снимать ВМЕСТЕ с патчем поворота и мерить
                // `wm-propagation-body-047`, `abs-pos-border-offset-002` —
                // ровно те две пары, ради которых заплатка ставилась.
                if e.style.width.is_none() {
                    d = d.flex_shrink_0();
                }
            } else if e.style.display.is_none() {
                // Блок без явного `display` — блочная раскладка taffy, а не
                // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
                // выше родителя ужимался, тогда как браузер даёт ему вылезти.
                // Схлопывание вертикальных отступов при этом делает сама
                // раскладка — включая протекание через пустой блок.
                d = d.flex().flex_col();
                // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
                // детям с заданной шириной (в blocks): items_end на контейнере
                // снимал stretch у всех, и абзац в rtl ужимался до текста —
                // text-align внутри пустел (text-align-end-001: bw=124.8
                // вместо 300). ЗАМЕРЕНО: +18 css-text при −2..3 wm и −4 mix
                // (spot-механика abs-pos-border-offset полагалась на
                // items_end — новый след) — нетто +10.
            }
            // Ряд по умолчанию — но не тогда, когда письмо справа налево:
            // там ряд обязан идти в обратную сторону, и общая ветка его
            // разворот отменяла. Письмо — СЛИТОЕ: `direction` наследуется
            // (css-writing-modes-4 §2.1), и ряд под `body { direction: rtl }`
            // без своего `direction` шёл слева направо — поля
            // `margin-inline-start` эталонов вставали не между элементами
            // (`gap-001-rtl-ref`, `gap-003-rtl-ref`).
            if e.style.display == Some(Display::Flex)
                && e.style.flex_dir.is_none()
                && merged.rtl != Some(true)
            {
                // При вертикальном письме умолчание `row` — это ось строки, а
                // она идёт сверху вниз.
                d = if merged.vertical == Some(true) {
                    d.flex_col()
                } else {
                    d.flex_row()
                };
            }
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: отдавать субсетке обычной сетки
            // РАЗРЕШЁННЫЙ кусок родительских дорожек — тем же кодом, что и
            // проход лунок (`if item.style.subgrid` в `lanes`), с вычетом
            // своих краёв и правкой зазора. Кусок брался из ЯВНЫХ линий
            // ребёнка (в обычной сетке размещение делает вендор, и другого
            // источника `at`/`span` до раскладки нет). Срез css-grid (1133
            // пары, 646 зелёных): 645, приобретено НОЛЬ, и
            // `row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`
            // ушла 0.00 → HUNG (вложенная субсетка зацикливает раскладку).
            // Возвращать только вместе с размещением субсетки НА НАШЕЙ
            // стороне: пока `at`/`span` берутся из линий, вложенный случай
            // получает кусок от куска и сходится не всегда.
            // Имена областей разворачиваются здесь: контейнер и его дети
            // видны одновременно только на этом уровне.
            let children = match &e.style.grid_areas {
                Some(areas) => place_named_areas(areas, e.children.clone()),
                None => e.children.clone(),
            };
            // Resolve physical margins before preparing the vertical formatting context.
            let children = if merged.vertical == Some(true) {
                // Поле самого контейнера по ведущей стороне оси потока —
                // для схлопывания с первым ребёнком (§8.3.1). Ведущая
                // сторона: левая у `vertical-lr`/`sideways-*`, правая у
                // `vertical-rl`. Открыта, если там нет ни рамки, ни
                // внутреннего отступа; у корня поля не схлопываются вовсе.
                let reverse = merged.vertical_rl == Some(true);
                let lead_margin = if e.tag == "html" {
                    None
                } else {
                    let b = e.style.borders();
                    let (border, pad, own) = if reverse {
                        (b.right, e.style.padding.right, e.style.margin.right)
                    } else {
                        (b.left, e.style.padding.left, e.style.margin.left)
                    };
                    // Независимый контекст форматирования (overflow не
                    // `visible`, флоат, `display: flow-root`, `contain`) не
                    // схлопывает своё поле с детьми (CSS2 §8.3.1, css-writing-
                    // modes-4 §7.4): `margin-collapse-vlr-017`/`vrl-016`
                    // (`overflow: hidden`, v100: 0.00 → «красное видно»).
                    let bfc = !matches!(
                        e.style.overflow_x,
                        None | Some(crate::computed::Overflow::Visible)
                    ) || !matches!(
                        e.style.overflow_y,
                        None | Some(crate::computed::Overflow::Visible)
                    ) || e.style.float.is_some()
                        || e.style.display.is_some()
                        || e.style.flow_root == Some(true)
                        // css-align-3 §align-block — тот же список, что и в
                        // `own_context`: своё поле такая коробка с полем
                        // первого ребёнка не схлопывает.
                        || e.style.align_content_block
                        || e.style.contain_layout == Some(true)
                        || e.style.contain_paint == Some(true);
                    let sealed = bfc
                        || margin_px(border, &e.style).unwrap_or(0.0) > 0.0
                        || margin_px(pad, &e.style).unwrap_or(0.0) > 0.0;
                    if sealed {
                        None
                    } else {
                        Some(margin_px(own, &e.style).unwrap_or(0.0))
                    }
                };
                // Доли полей/отступов — в точки от высоты контейнера ДО
                // схлопывания (см. `resolve_inline_pct`).
                vertical_hug::children(
                    orthogonal_children(
                        vertical_flow_margins::children(
                            resolve_inline_pct(children, &merged, true),
                            &merged,
                            reverse,
                            lead_margin,
                        ),
                        &merged,
                        opts.viewport.0,
                    ),
                    &e.style,
                    &merged,
                )
            } else {
                orthogonal_vertical_children(resolve_inline_pct(children, &merged, false), &merged)
            };
            let mut kids: Vec<AnyElement> = Vec::new();
            kids.extend(clip_layer(&merged, opts));
            // Бюджет строк обрезки: сторожа контекста живут, пока строится
            // поддерево — пробы детей пишут строки в буфер контейнера.
            // Многоколонник клэмпом не режется (`continue: collapse` там как
            // `auto`, §5.2; `styled_div_with` срез не ставит): без этого гейта
            // бюджет абзаца счётного режима поставил бы «…» (`line-clamp-039`).
            let is_clamp = (e.style.clamp_lines().is_some()
                || (e.style.clamp_auto == Some(true) && auto_clamp_limit(&merged).is_some()))
                && !multicol_container(&e.style);
            let _clamp_guard = is_clamp.then(|| crate::interact::ClampGuard::enter(e.node_id));
            let makes_bfc = matches!(
                merged.overflow_x,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || matches!(
                merged.overflow_y,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || merged.float.is_some()
                || merged.flow_root == Some(true)
                // Независимый контекст форматирования и без overflow/float/
                // flow-root: гибкий контейнер, сетка, таблица — своя
                // раскладка по определению, и строки внутри в бюджет
                // `line-clamp` не входят (css-overflow-4: «skip lines in
                // independent formatting contexts»; `webkit-line-clamp-012/013`).
                || matches!(
                    merged.display,
                    Some(Display::Flex)
                        | Some(Display::InlineFlex)
                        | Some(Display::Grid)
                        | Some(Display::InlineGrid)
                        | Some(Display::GridLanes)
                        | Some(Display::Table)
                        | Some(Display::InlineTable)
                        | Some(Display::TableCell)
                )
                // `<fieldset>` — тоже отдельная раскладка
                // (`webkit-line-clamp-027`).
                || e.tag == "fieldset";
            let _bfc_guard = (!is_clamp && makes_bfc && crate::interact::clamp_context().is_some())
                .then(crate::interact::ClampGuard::enter_bfc);
            // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
            // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
            // ранний `return` не съел запись. Абсолютные дети дорожек не
            // занимают (css-grid-1 §9), а пустой анонимный блок — это
            // распорка лент (`spacer()`), не элемент.
            if let Some(key) = crate::interact::gap_context()
                && !matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                )
                && !(e.node_id == 0 && e.children.is_empty())
            {
                // Проба ложится на паддинг-бокс; линейкам нужен рамочный.
                let fs = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let bw = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    Some(Len::Em(k)) => k * fs,
                    _ => 0.0,
                };
                let b = e.style.borders();
                kids.push(crate::interact::gap_item_probe(
                    crate::interact::gap_items_for(key),
                    [bw(b.top), bw(b.right), bw(b.bottom), bw(b.left)],
                ));
            }
            if let Some((key, skip)) = crate::interact::clamp_context() {
                // Строки дают пробы абзацев (paragraph_probed); здесь — только
                // коробка с краской: блок прячется целиком, если срез внутри.
                // Поточная коробка со СВОИМ контекстом форматирования точек
                // среза внутри не имеет (css-overflow-4 §5.3: строки
                // независимых контекстов не считаются, точка — только между
                // блоками): пересечённая потолком, она уходит целиком, как
                // коробка с заданной высотой (`line-clamp-auto-033`:
                // `flow-root` под «Line 4»). Флоат и абсолют — не поточные,
                // строчный атом — внутри строки.
                let monolithic = makes_bfc
                    && !merged.float.is_some_and(|f| f != 0)
                    && !matches!(
                        merged.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    )
                    && !matches!(
                        merged.display,
                        Some(Display::InlineBlock)
                            | Some(Display::InlineFlex)
                            | Some(Display::InlineGrid)
                            | Some(Display::InlineTable)
                    );
                if !is_clamp && (has_box_style_probe(&e.style) || monolithic) {
                    // Нижние рамка и паддинг фрагментированной коробки
                    // остаются в потоке (css-overflow-4 §5.3): проба несёт
                    // их вместе с границами паддинг-бокса.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bp_after = side(e.style.borders().bottom) + side(e.style.padding.bottom);
                    kids.push(crate::interact::clamp_probe(
                        crate::interact::clamp_lines_for(key),
                        0.0,
                        skip,
                        e.style.height.is_some() || e.style.min_height.is_some() || monolithic,
                        bp_after,
                        // Коробка — не абзац: знак обрыва на неё не садится
                        // (он всегда в конце строки, css-overflow-4 §5.3).
                        None,
                        None,
                    ));
                } else if !is_clamp
                    && !skip
                    && e.children.iter().all(is_blank)
                    && !merged.float.is_some_and(|f| f != 0)
                    && !matches!(
                        merged.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    )
                    && matches!(merged.display, None | Some(Display::Block))
                {
                    kids.push(crate::interact::clamp_empty_probe(
                        crate::interact::clamp_lines_for(key),
                    ));
                }
            }
            // Абсолютный потомок ищет ближайшего позиционированного предка
            // (§10.1), а раскладка под нами знает только непосредственного
            // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
            // содержащим блоком не является, переезжает сюда.
            let cb_layer = crate::inline::establishes_cb(&merged);
            if cb_layer {
                crate::interact::cb_open_with(fixed_cb_layer_box(&merged));
            }
            // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
            // задан стиль хотя бы одной линейки: начальное `none` означает,
            // что рисовать нечего, и ни одна старая пара сюда не попадает
            // (см. `gap_rule_spec`).
            let gap_rules = gap_rule_spec(e, &merged, opts);
            let gap_buf = gap_rules
                .is_some()
                .then(|| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt));
            let _gap_guard = gap_buf
                .as_ref()
                .map(|_| crate::interact::GapGuard::enter(e.node_id ^ opts.doc_salt));
            // css-gaps-1 §gap-decorations: «Gap decorations are painted just
            // above the border of the container» — ПОД детьми. Слой идёт до
            // них: буфер проб он всё равно читает в `paint`, а prepaint всего
            // дерева у gpui проходит раньше (эталоны `grid-gap-decorations-042`
            // и `flex-033` кладут линейки `z-index: -1`, `008/023` — элементы
            // `z-index: 2`).
            if let (Some(buf), Some(spec)) = (gap_buf, gap_rules) {
                kids.push(crate::interact::GapRulePainter::new(buf, spec).into_any_element());
            }
            kids.extend(blocks(&children, &merged, opts));
            if cb_layer {
                kids.extend(crate::interact::cb_close());
            }
            if is_clamp {
                // Бюджет среза — высота ПОЛЯ СОДЕРЖИМОГО: при `box-sizing:
                // border-box` из `max-height` вычитаются вертикальные рамки и
                // паддинги (`line-clamp-auto-003`: 138 − 2×(1+4) = 128).
                let bb_y = if merged.border_box == Some(true) {
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bw = merged.borders();
                    side(bw.top)
                        + side(bw.bottom)
                        + side(merged.padding.top)
                        + side(merged.padding.bottom)
                } else {
                    0.0
                };
                // Потолок высоты участвует в выборе точки среза только в
                // авто-режиме (`line-clamp: auto` / `4 auto`); счётный
                // `line-clamp: 4` и `-webkit-line-clamp` режут ТОЛЬКО по числу
                // строк, а не влезшее в `max-height` переполняет коробку
                // (`line-clamp-035`, `webkit-line-clamp-with-max-height`).
                let max_h = match auto_clamp_limit(&merged) {
                    Some(v) if e.style.clamp_auto == Some(true) => Some((v - bb_y).max(0.0)),
                    _ => None,
                };
                // `text-box-trim: trim-end` клампа: последняя строка перед
                // обрывом срезается (css-inline-3 §4.2; Blink триммит строку у
                // точки клампа). Метрика — по стилю контейнера.
                let clamp_trim = if merged.text_box_trim_end {
                    text_box_trim_px(&merged, false, opts)
                } else {
                    0.0
                };
                kids.push(
                    crate::interact::ClampCut::new(
                        e.node_id,
                        crate::interact::clamp_lines_for(e.node_id),
                        e.style.clamp_lines(),
                        max_h,
                    )
                    .trim_end(clamp_trim)
                    .into_any_element(),
                );
            }
            // Абсолют с `anchor()`-вставками: раскладка поставила его к краю
            // содержащего блока (нулевая вставка), сдвиг до края якоря
            // считает заместитель на подготовке кадра. Без якорных вставок
            // коробка возвращается как есть.
            let child = d.children(kids).into_any_element();
            let child = match overflow_plan { Some(plan) => plan.wrap(child), None => child };
            crate::anchor::place(child, &merged, inherited)
        }
    }
}
