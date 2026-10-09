//! Правки дерева после разбора: quirks-проценты, цвета правил, run-in, флоаты во флексе, выравнивание, руби.

use crate::dom::*;

/// Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме quirks
/// доля высоты элемента в потоке ищет опору через предков-блоков с
/// `height: auto` до ближайшего с заданной высотой. Сводится к точкам ЗДЕСЬ, в
/// стиле узла: флоаты и картинки строятся из сырого стиля (`wrap_floats`,
/// `image_with`), и пересчёт только в слитом (`inline::inherit`) до них не
/// доходил (`float-percentage-resolution-quirks-mode`,
/// `intrinsic-percent-replaced-003`). `base` — высота содержимого опоры для
/// детей; гибкий/сеточный/табличный предок с `auto` и абсолют цепочку рвут.
pub(crate) fn quirks_percent_heights(nodes: &mut [Node], base: Option<f32>) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        let st = &mut e.style;
        let out_of_flow = matches!(st.position, Some(Position::Absolute) | Some(Position::Fixed));
        let blockish = matches!(st.display, None | Some(Display::Block) | Some(Display::InlineBlock) | Some(Display::ListItem));
        if let (Some(Len::Pct(k)), Some(b), false, true) = (st.height, base, out_of_flow, blockish) {
            st.height = Some(Len::Px(k * b));
        }
        // Табличные коробки квирка не дают: доля внука ячейки с заданной
        // высотой остаётся `auto` (`percentages-grandchildren-quirks-mode-001`).
        let tabular = matches!(
            st.display,
            Some(Display::Table | Display::InlineTable | Display::TableCell | Display::TableRow | Display::TableRowGroup)
        );
        let child_base = match st.height {
            _ if tabular => None,
            Some(Len::Px(h)) => Some(h),
            None | Some(Len::Auto) if blockish && !out_of_flow && e.tag != "html" => base,
            _ => None,
        };
        quirks_percent_heights(&mut e.children, child_base);
    }
}

pub(crate) type RuleColors = (
    Option<crate::value::Color>,
    Option<crate::computed::GapList<Option<crate::value::Color>>>,
    Option<crate::value::Color>,
    Option<crate::computed::GapList<Option<crate::value::Color>>>,
);

/// `column-rule-color: inherit` / `row-rule-color: inherit` — ненаследуемое
/// свойство берёт ВЫЧИСЛЕННОЕ значение ДОМ-родителя (css-cascade-4 §7.2).
/// Отрисовка линеек читает собственный стиль коробки, поэтому слово решается
/// здесь, в дереве, как `display: inherit` выше (`multicol-rule-color-inherit-001`:
/// родитель `column-rule-color: green` при `column-rule-style: none`, ребёнок
/// `inherit` — зелёные линейки, а не `currentcolor` красного текста).
pub(crate) fn resolve_rule_color_inherit(nodes: &mut [Node], parent: &RuleColors) {
    use crate::computed::inh;
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let s = &mut el.style;
        if s.inherit_bits & inh::COLUMN_RULE_C != 0 {
            s.column_rule_color = parent.0;
            s.column_rule_colors = parent.1.clone();
            s.inherit_bits &= !inh::COLUMN_RULE_C;
        }
        if s.inherit_bits & inh::ROW_RULE_C != 0 {
            s.row_rule_color = parent.2;
            s.row_rule_colors = parent.3.clone();
            s.inherit_bits &= !inh::ROW_RULE_C;
        }
        let own: RuleColors = (
            s.column_rule_color,
            s.column_rule_colors.clone(),
            s.row_rule_color,
            s.row_rule_colors.clone(),
        );
        resolve_rule_color_inherit(&mut el.children, &own);
    }
}

/// `filter: url(#id)` дешёвым слоем рисуется только у коробки БЕЗ содержимого
/// (`interact::FilterLayer`): у коробки с детьми слой лёг бы поверх них.
/// Решается здесь, пока дерево целое — при сборке абсолютные дети уже
/// вынесены в свои слои, и родитель выглядит пустым
/// (`filter-region-transformed-composited-child-001`).
pub(crate) fn filter_ref_only_empty(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if el.style.filter_ref.is_some()
            && el.children.iter().any(|c| match c {
                Node::Element(_) => true,
                Node::Text(t) => !t.trim().is_empty(),
            })
        {
            el.style.filter_ref = None;
        }
        filter_ref_only_empty(&mut el.children);
    }
}

/// Вбегание `display: run-in` (CSS 2.1 §9.2.3): элемент без блочного
/// содержимого, за которым (сквозь пробельный текст) идёт обычная блочная
/// коробка, становится её ПЕРВЫМ СТРОЧНЫМ ребёнком; во всех остальных
/// случаях он ведёт себя как блок (это уже так — разбор дал Block).
pub(crate) fn fold_run_ins(nodes: &mut Vec<Node>, parent: Option<&Computed>) {
    // Пробельный текст прозрачен для вбегания, только если он СХЛОПНЕТСЯ:
    // при `white-space: pre*` контейнера пробел — настоящий строчный кусок
    // (анонимная строка), и за run-in идёт уже не блок (css-display-3 §4.1:
    // «intervening white space» — схлопываемый). `run-in-basic-014`: эталон —
    // run-in блоком, строка сохранённого пробела, затем блок.
    let keep = parent.is_some_and(|p| p.keep_spaces == Some(true));
    let is_blank = |n: &Node| {
        matches!(n, Node::Text(t) if t.is_empty() || (!keep && t.trim().is_empty()))
    };
    let mut i = 0;
    while i < nodes.len() {
        // Сначала вглубь: вложенные run-in решаются в своём контейнере.
        if let Node::Element(e) = &mut nodes[i] {
            let own = e.style.clone();
            fold_run_ins(&mut e.children, Some(&own));
        }
        // Вне потока элемент вбеганию не мешает и сам не вбегает.
        fn out_of_flow(e: &Element) -> bool {
            e.style.float.is_some()
                || matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                )
        }
        // Блочная коробка В ПОТОКЕ где угодно внутри (в том числе за
        // строчными обёртками) запрещает вбегание.
        fn holds_block(nodes: &[Node]) -> bool {
            nodes.iter().any(|c| match c {
                Node::Element(ch) => {
                    if out_of_flow(ch) || ch.style.display == Some(Display::None) {
                        return false;
                    }
                    if ch.inline
                        || matches!(
                            ch.style.display,
                            Some(Display::InlineBlock)
                                | Some(Display::InlineFlex)
                                | Some(Display::InlineGrid)
                                | Some(Display::InlineTable)
                        )
                    {
                        return holds_block(&ch.children);
                    }
                    true
                }
                _ => false,
            })
        }
        let runs_in = match &nodes[i] {
            Node::Element(e) => {
                e.style.run_in == Some(true) && !out_of_flow(e) && !holds_block(&e.children)
            }
            _ => false,
        };
        if !runs_in {
            i += 1;
            continue;
        }
        // Следующая непустая коробка: подходит только обычный блок — не
        // run-in, не строчный. Плавающие и позиционированные соседи
        // ПРОЗРАЧНЫ: они вне потока и вбеганию не мешают (§9.2.3).
        let Some(j) = (i + 1..nodes.len()).find(|&j| {
            !is_blank(&nodes[j]) && !matches!(&nodes[j], Node::Element(t) if out_of_flow(t))
        }) else {
            i += 1;
            continue;
        };
        let target_ok = matches!(&nodes[j], Node::Element(t)
        if !t.inline
            && t.style.run_in != Some(true)
            && !matches!(
                t.style.display,
                Some(Display::None)
                    | Some(Display::InlineBlock)
                    | Some(Display::InlineFlex)
                    | Some(Display::InlineGrid)
                    | Some(Display::InlineTable)
                    | Some(Display::Table)
                    | Some(Display::TableRow)
                    | Some(Display::TableRowGroup)
                    | Some(Display::TableCell)
            ));
        if !target_ok {
            i += 1;
            continue;
        }
        let Node::Element(mut run) = nodes.remove(i) else {
            unreachable!()
        };
        run.inline = true;
        run.style.display = None;
        run.style.run_in = None;
        // Наследование от ИСХОДНОГО родителя (§9.2.3) НЕ запекается:
        // inline::inherit сливает и рендерные поля, и замер показал минус
        // (run-in-inherit-001: 5.88 -> 7.35). Цвет нового блока вбёгнутый
        // перенимает неправильно — хвост запаркован.
        let _ = parent;
        let Node::Element(target) = &mut nodes[j - 1] else {
            unreachable!()
        };
        target.children.insert(0, Node::Element(run));
        // На месте i теперь стоит бывший j-1 — им и продолжаем.
    }
}

pub(crate) fn flex_items_lose_float(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        flex_items_lose_float(&mut el.children);
        if !matches!(
            el.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            continue;
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else { continue };
            if matches!(
                child.style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            ) {
                continue;
            }
            child.style.float = None;
            child.style.clear = None;
        }
    }
}

/// `align-self` от ДОМ-родителя: слово `inherit` и неприменимость к блоку.
///
/// 1. `align-self: inherit` (css-cascade-4 §7.3) — вычисленное значение
///    родителя. Прежде слово отбрасывалось разбором, и элемент брал
///    `align-items` контейнера (`flexbox-align-self-vert-001`,
///    `-horiz-001-block`: `inherit` ждёт `flex-end` от `.flexbox`).
///    Берётся АВТОРСКОЕ значение родителя (`align_self_decl`), даже когда
///    у самого родителя оно погашено пунктом 2: гашение — про
///    использованное значение, вычисленное остаётся.
/// 2. css-align-3 §6.1: `align-self` «Applies to: flex items, grid items,
///    and absolutely-positioned boxes». Наш блок собран колонкой flex, и
///    авторское `align-self: flex-end` у блока в `body` уводило его к
///    правому краю (`flexbox-align-self-vert-001`, `-vert-rtl-001`).
///    Blink читает свойство только в раскладке flex/grid
///    (`C:\Users\MSI\Projects\refs\chromium-blink\third_party\blink\renderer\core\layout\flex\flex_layout_algorithm.cc:266`
///    `ResolvedAlignSelf`). Гасится только авторское значение и ДО сборки:
///    приёмы сборки пишут в то же поле позже (`render.rs`: rtl-прижим
///    блока с шириной, `blocks()`, флоаты, столы) и видят пустое поле, как
///    без автора. Запись отката 04.09 (`inline.rs`: гашение в
///    `inline::inherit` без признака авторства, −50 в своде v19) — этот
///    путь гасит только авторское. Родитель `display: contents` — настоящий
///    контейнер выше, такие дети не трогаются; корневой уровень тоже.
pub(crate) fn align_self_from_dom_parent(nodes: &mut [Node], parent: Option<&ParentAlign>) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        let s = &mut el.style;
        match parent {
            Some(p) => {
                if s.align_self_inherit {
                    s.align_self = p.value;
                    s.align_self_decl = Some(p.value);
                    s.align_self_inherit = false;
                    (
                        s.align_self_safe,
                        s.align_self_normal,
                        s.align_self_own_axis,
                        s.align_self_flex_kw,
                        s.align_self_last,
                    ) = p.flags;
                }
                if p.block
                    && s.align_self.is_some()
                    && s.align_self_decl == Some(s.align_self)
                    && !matches!(s.position, Some(Position::Absolute) | Some(Position::Fixed))
                {
                    s.align_self = None;
                    s.align_self_normal = false;
                }
            }
            None => s.align_self_inherit = false,
        }
        let me = ParentAlign {
            // Вычисленное значение для `inherit` детей — авторское, если было.
            value: s.align_self_decl.unwrap_or(s.align_self),
            flags: (
                s.align_self_safe,
                s.align_self_normal,
                s.align_self_own_axis,
                s.align_self_flex_kw,
                s.align_self_last,
            ),
            block: matches!(
                s.display,
                None | Some(Display::Block)
                    | Some(Display::ListItem)
                    | Some(Display::InlineBlock)
                    | Some(Display::TableCell)
            ) && s.webkit_box != Some(true),
        };
        align_self_from_dom_parent(&mut el.children, Some(&me));
    }
}

/// Что дети берут у родителя в `align_self_from_dom_parent`.
pub(crate) struct ParentAlign {
    pub(crate) value: Option<crate::computed::Align>,
    /// safe, normal, own_axis, flex_kw, last.
    pub(crate) flags: (bool, bool, bool, bool, bool),
    /// Родитель — блочный контейнер (не flex/grid/contents/таблица).
    pub(crate) block: bool,
}

/// Стол — элемент СЕТКИ: растяжка по дорожке остаётся за ним.
///
/// css-align-3 §6.2: начальное `align-self: normal` у элемента сетки
/// «behaves as stretch», и растянутый элемент получает размер ОБЛАСТИ.
/// Сжатие стола по содержимому (CSS 2.1 §17.5.2.2) у нас выражено
/// `align_self = FlexStart` (`render.rs: table`), а у элемента сетки эта ось —
/// БЛОЧНАЯ: стол переставал расти до дорожки (пустой стол выходил нулевой
/// высоты и ронял базовую линию контейнера — `grid-container-baseline-
/// synthesized-001…004`), а сжатия по строчной оси приём там и не давал:
/// её ведёт `justify-self`. Явная растяжка на самом элементе снимает приём
/// ровно в сетке и нигде больше.
///
/// Гейты: только обычная сетка (у лунок свой проход дорожек), только
/// потоковый ребёнок (внепоточный элементом сетки не является,
/// css-grid-1 §9), только когда контейнер не задал своего `align-items`
/// и автор не задал `align-self` — чужое выравнивание не перебиваем.
pub(crate) fn grid_table_items_keep_stretch(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        grid_table_items_keep_stretch(&mut el.children);
        if !matches!(
            el.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            continue;
        }
        if !matches!(
            el.style.align_items,
            None | Some(crate::computed::Align::Stretch)
        ) {
            continue;
        }
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else { continue };
            if matches!(
                child.style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            ) {
                continue;
            }
            let is_table = child.tag == "table"
                || matches!(
                    child.style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            if is_table && child.style.align_self.is_none() {
                child.style.align_self = Some(crate::computed::Align::Stretch);
            }
        }
    }
}

/// Сумма двух длин. Складываются только точки: смешивать доли и кегли здесь
/// не с чем — контейнера в этот момент нет.
/// Руби-роль коробки (css-ruby-1 §2.1): своё `display: ruby*`, иначе тег
/// без авторского `display` (A.1). Зеркало `render::ruby_role`.
pub(crate) fn ruby_box_role(tag: &str, style: &Computed) -> Option<crate::computed::RubyRole> {
    use crate::computed::RubyRole;
    if let Some(role) = style.ruby_role {
        return Some(role);
    }
    if style.display.is_some() {
        return None;
    }
    match tag {
        "ruby" => Some(RubyRole::Container),
        "rb" => Some(RubyRole::Base),
        "rt" => Some(RubyRole::Text),
        "rbc" => Some(RubyRole::BaseContainer),
        "rtc" => Some(RubyRole::TextContainer),
        _ => None,
    }
}

/// css-ruby-1 §2.2 п.2: подряд идущие базы, аннотации и их контейнеры ВНЕ
/// руби-контейнера (вместе с пробелами между ними) оборачиваются в
/// анонимный руби-контейнер. Прежде `<rt>` прямо в `<p>` рисовалась мелким
/// строчным текстом в ряду (`ruby-box-generation-*`, вторая строка: эталон
/// пишет те же коробки внутри `<ruby>`). Краевые пробелы серии остаются
/// снаружи, строчное содержимое серию обрывает.
pub(crate) fn wrap_misparented_ruby(children: Vec<Node>) -> Vec<Node> {
    use crate::computed::RubyRole;
    let internal = |n: &Node| {
        matches!(n, Node::Element(e)
            if ruby_box_role(&e.tag, &e.style).is_some_and(|r| r != RubyRole::Container))
    };
    if !children.iter().any(internal) {
        return children;
    }
    let blank = |n: &Node| matches!(n, Node::Text(t) if t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n' | '\x0c')));
    let mut out: Vec<Node> = Vec::with_capacity(children.len());
    let mut run: Vec<Node> = Vec::new();
    // Пробелы после последней руби-коробки серии: войдут в серию, только
    // если за ними снова руби-коробка.
    let mut pending: Vec<Node> = Vec::new();
    let flush = |run: &mut Vec<Node>, out: &mut Vec<Node>| {
        if !run.is_empty() {
            out.push(Node::Element(Element {
                list_item: None,
                node_id: 0,
                anim: None,
                inline: true,
                tag: "ruby".to_string(),
                style: Computed::default(),
                hover: None,
                first_letter: None,
                first_line: None,
                children: std::mem::take(run),
                attrs: vec![],
            }));
        }
    };
    for n in children {
        if internal(&n) {
            run.append(&mut pending);
            run.push(n);
        } else if blank(&n) && !run.is_empty() {
            pending.push(n);
        } else {
            flush(&mut run, &mut out);
            out.append(&mut pending);
            out.push(n);
        }
    }
    flush(&mut run, &mut out);
    out.append(&mut pending);
    out
}

/// Задаёт ли элемент отсчёт для абсолютных потомков.
pub(crate) fn own_containing_block(c: &Computed) -> bool {
    matches!(
        c.position,
        Some(Position::Relative)
            | Some(Position::Absolute)
            | Some(Position::Fixed)
            | Some(Position::Sticky)
    )
}
