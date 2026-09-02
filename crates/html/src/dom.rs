//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

use crate::computed::{Computed, Display, Position};
use crate::css::{
    Decls, Keyframes, Media, Rule, Selector, parse_decls, parse_keyframes, parse_stylesheet_media,
};
use crate::value::Len;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;

/// Узел документа: либо текст, либо элемент со своими детьми.
#[derive(Clone, Debug)]
pub enum Node {
    Text(String),
    Element(Element),
}

#[derive(Clone, Debug)]
pub struct Element {
    /// Номер пункта списка из счётчика `list-item`; `None` — не пункт.
    pub list_item: Option<i32>,
    /// Устойчивый номер узла в документе.
    ///
    /// Нужен анимации: GPUI хранит её состояние по идентификатору элемента, а
    /// он обязан совпадать от кадра к кадру, иначе анимация каждый раз
    /// начинается заново.
    pub node_id: u64,
    /// Кадры анимации, уже разрешённые в стиль: доля времени → стиль.
    pub anim: Option<Vec<(f32, Computed)>>,
    pub tag: String,
    pub style: Computed,
    /// Стиль наведения, собранный из правил с `:hover`. Пустой, если таких
    /// правил не было.
    pub hover: Option<Computed>,
    /// Стиль первой буквы абзаца (`::first-letter`) — отдельным слоем поверх
    /// базового: это буквица, а не стиль всего блока.
    pub first_letter: Option<Computed>,
    /// Стиль первой строки абзаца (`::first-line`).
    pub first_line: Option<Computed>,
    pub children: Vec<Node>,
    /// Атрибуты, которые нужны при отрисовке: `src`, `href`, `colspan`.
    pub attrs: Vec<(String, String)>,
    /// Инлайн ли элемент по своей природе (`<span>`, `<code>`, `<a>`): от
    /// этого зависит, попадёт ли он в строку текста или станет блоком.
    pub inline: bool,
}

impl Element {
    pub fn attr(&self, name: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    }
}

/// Теги, которые в HTML участвуют в строке текста, а не разрывают её.
pub(crate) const INLINE_TAGS: &[&str] = &[
    "a", "abbr", "b", "bdi", "bdo", "br", "cite", "code", "data", "dfn", "em", "i", "kbd", "mark",
    "q", "rp", "rt", "ruby", "s", "samp", "small", "span", "strong", "sub", "sup", "time", "u",
    "var", "wbr", "img", "svg",
    // Правки текста: без них `~~зачёркнутое~~` из markdown разрывало абзац.
    "del", "ins",
    // Управление формой стоит В СТРОКЕ: иначе «Согласен» уезжает под флажок,
    // а ряд кнопок выстраивается в столбик.
    "button", "label", "input", "select", "textarea", "output", "meter", "progress",
];

/// Теги, которым таблица агента даёт блочный вид (HTML §15.3.2-15.3.12).
///
/// Правило для НЕИЗВЕСТНОГО тега — строчный: своей записи в листе агента у
/// него нет, а начальное значение `display` — `inline`. Прежде блочным
/// становилось всё, чего нет в `INLINE_TAGS`, и `<foo>` внутри абзаца рвал
/// строку (`line-breaking-font-size-zero-001`).
pub(crate) const BLOCK_TAGS: &[&str] = &[
    "html", "body", "address", "blockquote", "center", "div", "figure", "figcaption", "footer",
    "form", "header", "hr", "legend", "listing", "main", "p", "plaintext", "pre", "xmp", "article",
    "aside", "h1", "h2", "h3", "h4", "h5", "h6", "hgroup", "nav", "section", "search", "dir", "dd",
    "dl", "dt", "ol", "ul", "menu", "li", "table", "caption", "colgroup", "col", "thead", "tbody",
    "tfoot", "tr", "td", "th", "fieldset", "details", "summary", "dialog", "optgroup", "option",
    "frameset", "frame", "noframes", "head", "title", "meta", "link", "base", "script", "style",
    "noscript", "template", "slot", "map", "area", "source", "track", "param",
];

/// Теги, содержимое которых не рисуется НИКОГДА (код и стили).
///
/// `head`/`title`/`meta`/`link` сюда не входят: их прячет таблица агента
/// `display: none`, и авторское `head { display: block }` её перебивает
/// (CSS2/generated-content content-067 и родня).
pub(crate) const DROP_TAGS: &[&str] = &["script", "style", "noscript"];

/// Имя тега без пространственного префикса.
///
/// В XHTML рисунок и формулы часто пишут с префиксом (`<svg:svg
/// xmlns:svg="…">`), а разборщик HTML держит двоеточие частью имени. Движок
/// сверяет теги по коротким именам, поэтому `svg:svg` не опознавался как
/// рисунок вовсе — вся семья замещаемых тестов CSS2 рисовала пустоту.
fn local_name(name: &str) -> String {
    match name.split_once(':') {
        Some((_, local)) if !local.is_empty() => local.to_string(),
        _ => name.to_string(),
    }
}

/// Стиль по умолчанию для тега — то, что браузер берёт из своей таблицы.
/// Без него `<b>` не жирный, а `<h1>` неотличим от абзаца.
/// ПРОВЕРЕНО ДВАЖДЫ: этот лист чатовый (`pre { padding: 8px }`,
/// `td { padding: 4px 8px }`), но на своде он не сказывается НИ НА ЧЁМ —
/// стенд подаёт `BROWSER_CSS` вторым листом того же происхождения, и по
/// порядку каскада выигрывает он. Проба с браузерными значениями здесь:
/// 71 пара семей `white-space-pre-*`, `text-indent-intrinsic-*`,
/// `border-style-applies-to-*`, `table-visual-layout-*` — ни одного сдвига.
/// Красное в этих семьях держит что-то другое.
fn user_agent_css() -> &'static str {
    r#"
head, title, meta, link { display: none }
    h1 { font-size: 24px; font-weight: 700; margin: 12px 0 6px }
    h2 { font-size: 20px; font-weight: 700; margin: 10px 0 5px }
    h3 { font-size: 17px; font-weight: 600; margin: 9px 0 4px }
    h4 { font-size: 15px; font-weight: 600; margin: 8px 0 4px }
    h5, h6 { font-size: 13px; font-weight: 600; margin: 8px 0 4px }
    p { margin: 6px 0 }
    b, strong { font-weight: 700 }
    del, s { text-decoration: line-through }
    ins { text-decoration: underline }
    mark { background: #ffe066; color: #1a1c23 }
    button { background: #3d3f51; border: 1px solid #4a4a5a; color: #e6e6ee }
    dd { margin-left: 32px }
    dt { font-weight: 700; margin: 6px 0 2px }
    figure { margin: 8px 0 }
    figcaption { font-size: 11px; color: #9aa0b4; margin: 4px 0 0 }
    caption { font-weight: 600; margin: 0 0 4px }
    canvas { background: #2a2b36; border: 1px dashed #4a4a5a }
    i, em { font-style: italic }
    u { text-decoration: underline }
    s, del { text-decoration: line-through }
    small { font-size: 11px }
    a { color: #8ab4f8; text-decoration: underline }
    code, kbd, samp { font-family: monospace; font-size: 12px }
    pre { font-family: monospace; margin: 6px 0; padding: 8px; overflow-x: auto }
    /* Заранее размеченный текст зазоров `text-autospace` не получает: правка
       ширины ломает выравнивание в столбик, ради которого его и пишут
       (css-text-4 §7, таблица стилей агента). */
    pre, code, kbd, samp, tt, textarea, input { text-autospace: no-autospace }
    ul, ol { margin: 6px 0; padding-left: 18px }
    li { margin: 2px 0 }
    blockquote { margin: 6px 0; padding-left: 10px; border-left: 3px solid #4a4a5a }
    hr { height: 1px; margin: 8px 0; background: #4a4a5a }
    table { margin: 6px 0 }
    th { font-weight: 700; padding: 4px 8px; text-align: left }
    td { padding: 4px 8px }
    button { padding: 4px 10px; border-radius: 4px }
    "#
}

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn parse(html: &str, extra_css: &str) -> Vec<Node> {
    parse_media(html, extra_css, Media::default())
}

/// То же, но с известными условиями окружения для `@media`.
pub fn parse_media(html: &str, extra_css: &str, media: Media) -> Vec<Node> {
    // Правила `@page` — от последнего РАЗОБРАННОГО документа: почистить,
    // чтобы прошлый лист не красил страницу нового.
    let _ = crate::css::take_page_decls();
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });

    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    let mut rules = parse_stylesheet_media(user_agent_css(), media);
    let base = rules.len();
    for (i, r) in parse_stylesheet_media(extra_css, media)
        .into_iter()
        .enumerate()
    {
        rules.push(Rule {
            order: base + i,
            ..r
        });
    }
    // Каждый `<style>` — ОТДЕЛЬНАЯ таблица: конец каждой закрывает свои
    // незакрытые конструкции (CSS 2.1 §4.2, `uri-017`). В склейке незакрытая
    // запись первой таблицы съедала правила второй.
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(&dom.document, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_stylesheet_media(css, media).into_iter().enumerate() {
            rules.push(Rule {
                order: base + i,
                // Таблица ДОКУМЕНТА: её происхождение старше нашего умолчания и
                // темы приложения, поэтому она перебивает их независимо от
                // специфичности (CSS Cascade §6.4.4).
                origin: 1,
                ..r
            });
        }
    }

    // Переменные темы: `:root { --x: … }` и `--x` в инлайн-стиле корня.
    // Собираются до обхода, потому что нужны каждому узлу.
    // Пользовательские свойства НАСЛЕДУЮТСЯ и каскадируют, поэтому корень
    // стартует пустым: каждый узел добавляет свои и передаёт вниз.
    let vars = Decls::new();

    let mut out = vec![];
    // Наборы кадров собираются из тех же источников, что и правила.
    let mut frames = parse_keyframes(&user_agent_css());
    frames.extend(parse_keyframes(extra_css));
    for css in &sheets {
        frames.extend(parse_keyframes(css));
    }
    // `:has()`: аргументы собираются со всех селекторов, правила с
    // вложенным `:has` выкидываются (спека: cannot be nested), отметки
    // считаются отдельным проходом до обхода.
    let mut has_args_raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut has_args_raw));
    let has_args: Vec<HasArg> = has_args_raw
        .iter()
        .filter_map(|a| parse_has_arg(a))
        .collect();
    HAS_MARKS.with(|m| m.borrow_mut().clear());
    if !has_args.is_empty() {
        mark_has(&dom.document, &has_args, &mut vec![]);
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::counters::Counters::default();
    walk_children(
        &dom.document,
        &rules,
        &vars,
        &frames,
        &mut counter,
        &mut counters,
        &[],
        false,
        &mut out,
    );
    hoist_grid_abspos(&mut out);
    content_box_static_position(&mut out);
    flex_items_lose_float(&mut out);
    subgrid_takes_parent_tracks(&mut out);
    fold_run_ins(&mut out, None);
    out
}

/// Вбегание `display: run-in` (CSS 2.1 §9.2.3): элемент без блочного
/// содержимого, за которым (сквозь пробельный текст) идёт обычная блочная
/// коробка, становится её ПЕРВЫМ СТРОЧНЫМ ребёнком; во всех остальных
/// случаях он ведёт себя как блок (это уже так — разбор дал Block).
fn fold_run_ins(nodes: &mut Vec<Node>, parent: Option<&Computed>) {
    let is_blank = |n: &Node| matches!(n, Node::Text(t) if t.trim().is_empty());
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

/// Ребёнок гибкого контейнера или сетки не плавает и не очищает.
///
/// css-flexbox-1 §4: «`float` and `clear` do not create floating or clearance
/// for flex item», то же в css-grid-2 §6 для элемента сетки — обе величины
/// вычисляются в `none` у элемента В ПОТОКЕ (абсолютный ребёнок элементом
/// контейнера не является и правило его не касается).
///
/// Без этого правило §10.6.3 «блок из одних флоатов высотой ноль» считало
/// гибкий контейнер пустым и обнуляло его высоту (`flex-box-wrap` и родня).
/// Начало и длина пролёта подсетки в дорожках родителя.
///
/// Только явные формы: у подсетки без размещения среза нет, и трогать её
/// нельзя — авто-размещение считает уже раскладка.
fn subgrid_slot(
    place: &Option<(crate::computed::Placement, crate::computed::Placement)>,
    count: usize,
) -> Option<(usize, usize)> {
    use crate::computed::Placement;
    let line = |n: i16| -> usize {
        if n > 0 {
            (n as usize - 1).min(count.saturating_sub(1))
        } else {
            count.saturating_sub((-n) as usize)
        }
    };
    // Конечная линия — это КРАЙ, а не дорожка: у сетки из N дорожек линий
    // N+1, и потолок у неё `count`, а не `count - 1`. С общим потолком
    // подсетка, упирающаяся в последнюю линию родителя, теряла дорожку:
    // `grid-column: 2 / 5` при четырёх колонках давало пролёт 2 вместо 3, а
    // проверка длины среза этого не ловит.
    let edge = |n: i16| -> usize {
        if n > 0 {
            (n as usize - 1).min(count)
        } else {
            count.saturating_sub(((-n) as usize).saturating_sub(1))
        }
    };
    match place {
        Some((Placement::Line(a), Placement::Line(b))) => {
            let (s, t) = (line(*a), edge(*b));
            Some((s.min(t), (t as i32 - s as i32).unsigned_abs() as usize))
        }
        Some((Placement::Line(a), Placement::Span(k))) => Some((line(*a), *k as usize)),
        Some((Placement::Line(a), Placement::Auto)) => Some((line(*a), 1)),
        Some((Placement::Span(k), Placement::Line(b))) => {
            Some((edge(*b).saturating_sub(*k as usize), *k as usize))
        }
        _ => None,
    }
}

/// Пролёт подсетки, когда НАЧАЛЬНОЙ линии нет: `span k`, голое `auto` и
/// отсутствие записи вовсе. Начало такой подсетки знает только
/// авто-размещение, а сколько дорожек она занимает — видно сразу
/// (css-grid-2 §subgrid-size-contribution: число дорожек авто-размещённой
/// подсетки берётся из её пролёта).
fn subgrid_span(place: &Option<(crate::computed::Placement, crate::computed::Placement)>) -> usize {
    use crate::computed::Placement;
    match place {
        Some((Placement::Span(k), Placement::Auto)) | Some((Placement::Auto, Placement::Span(k))) => {
            (*k as usize).max(1)
        }
        _ => 1,
    }
}

/// Поправка среза на РАЗНИЦУ зазоров (css-grid-2 §subgrids).
///
/// Свой зазор у подсетки остаётся, но дорожка получает половину разницы
/// зазоров с каждой стороны, обращённой к ВНУТРЕННЕМУ стыку среза: три
/// эталона WPT выписывают результат числами (`grid-gap-larger-001-ref`
/// `70px 130px 70px` при родительских 100/190/100).
pub(crate) fn subgrid_gap_slice(
    slice: &mut [crate::computed::TrackSize],
    parent: Option<Len>,
    own: Option<Len>,
) {
    use crate::computed::{Track, TrackSize};
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let d = px(parent) - px(own);
    let n = slice.len();
    if d == 0.0 || n < 2 {
        return;
    }
    for (i, t) in slice.iter_mut().enumerate() {
        let sides = u8::from(i > 0) + u8::from(i + 1 < n);
        if let TrackSize::Single(Track::Px(w)) = t {
            *w = (*w + d / 2.0 * f32::from(sides)).max(0.0);
        }
    }
}

/// Дорожки родительской сетки — вниз, в ПОДСЕТКУ (css-grid-2 §subgrids).
///
/// Своих дорожек в подсеточной оси у подсетки нет: она берёт СРЕЗ
/// родительских по своему пролёту. У раскладки лунок это уже сделано
/// (`render::lanes`), а обычная сетка разбирала `grid-template-columns:
/// subgrid` как «одну колонку» (`count_tracks` считает слово дорожкой), и
/// тест с эталоном гоняли одно свойство разным кодом.
///
/// Обход СВЕРХУ ВНИЗ: вложенная подсетка обязана увидеть уже проставленные
/// дорожки внешней.
fn subgrid_takes_parent_tracks(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if matches!(
            el.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            for row_dir in [false, true] {
                let tracks = if row_dir {
                    el.style.grid_rows.clone()
                } else {
                    el.style.grid_tracks.clone()
                }
                .unwrap_or_default();
                if tracks.is_empty()
                    || !tracks.iter().all(|t| {
                        matches!(
                            t,
                            crate::computed::TrackSize::Single(crate::computed::Track::Px(_))
                        )
                    })
                {
                    continue;
                }
                // Курсор авто-размещения (§8.5, разрежённая укладка): у
                // подсетки без начальной линии срез всё равно ЕСТЬ — она
                // встаёт в следующее свободное место своего пролёта. Курсор
                // ведут ВСЕ дети, а не только подсеточные: место занимает
                // каждый.
                let mut cur = 0usize;
                for child in el.children.iter_mut() {
                    let Node::Element(child) = child else { continue };
                    let place = if row_dir {
                        &child.style.grid_row
                    } else {
                        &child.style.grid_col
                    };
                    let slot = match subgrid_slot(place, tracks.len()) {
                        Some((at, span)) => {
                            cur = (at + span).min(tracks.len());
                            Some((at, span))
                        }
                        None => {
                            let span = subgrid_span(place);
                            if cur + span > tracks.len() {
                                cur = 0;
                            }
                            let at = cur;
                            cur = (cur + span).min(tracks.len());
                            (at + span <= tracks.len()).then_some((at, span))
                        }
                    };
                    if !child.style.subgrid {
                        continue;
                    }
                    let Some((at, span)) = slot else {
                        continue;
                    };
                    let slice: Vec<crate::computed::TrackSize> = (at..at + span)
                        .filter_map(|i| tracks.get(i).cloned())
                        .collect();
                    if slice.len() != span || span == 0 {
                        continue;
                    }
                    // Свои края подсетки вычитаются из первой и последней
                    // дорожки куска — ровно как в раскладке лунок.
                    let px = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let bs = child.style.borders();
                    let (lead, trail) = if row_dir {
                        (
                            px(child.style.margin.top) + px(bs.top) + px(child.style.padding.top),
                            px(child.style.margin.bottom)
                                + px(bs.bottom)
                                + px(child.style.padding.bottom),
                        )
                    } else {
                        (
                            px(child.style.margin.left) + px(bs.left) + px(child.style.padding.left),
                            px(child.style.margin.right)
                                + px(bs.right)
                                + px(child.style.padding.right),
                        )
                    };
                    let mut slice = slice;
                    if let Some(crate::computed::TrackSize::Single(crate::computed::Track::Px(
                        w,
                    ))) = slice.first_mut()
                    {
                        *w = (*w - lead).max(0.0);
                    }
                    if let Some(crate::computed::TrackSize::Single(crate::computed::Track::Px(
                        w,
                    ))) = slice.last_mut()
                    {
                        *w = (*w - trail).max(0.0);
                    }
                    // ЗАМЕРЕНО И ОТКАЧЕНО: брать зазор подсеточной оси у
                    // РОДИТЕЛЯ. Полный свод CSS3: приобретено 0, потеряно 3 —
                    // `grid-lanes-subgrid-001c` 0.03 -> 0.53, `-002c`
                    // 0.50 -> 0.56, `row-subgrid-grid-gap-005` 0.32 -> 0.56.
                    // Причина ОДНОСТОРОННОСТЬ, а не двойной счёт: гейт выше
                    // пропускает только обычную сетку, и во всех трёх парах
                    // тест написан на ЛУНКАХ (свой срез — `render.rs`), а
                    // эталон на сетке — стороны разъехались. Само правило
                    // тоже иное: при разнице зазоров дорожка получает половину
                    // разницы с каждой стороны внутреннего стыка, а свой зазор
                    // остаётся. Возвращаться симметрично обоим путям.
                    let (prow, pcol) = el.style.gap.unwrap_or((None, None));
                    let (crow, ccol) = child.style.gap.unwrap_or((None, None));
                    let par = if row_dir { prow } else { pcol };
                    let own = if row_dir { crow } else { ccol };
                    // Незаданный зазор подсетки — это `normal`, а он по
                    // css-grid-2 §subgrid-gaps значит «такие же зазоры, как у
                    // родителя», то есть разница НОЛЬ. Пока `None` считался
                    // нулём, разница выходила равной родительскому зазору и
                    // дорожки раздувались на его половину.
                    let own = own.or(par);
                    if own != Some(Len::Px(0.0)) && crow.is_none() && ccol.is_none() {
                        child.style.gap = Some(if row_dir {
                            (par, ccol)
                        } else {
                            (crow, par)
                        });
                        if !row_dir {
                            child.style.column_gap = par;
                        }
                    }
                    subgrid_gap_slice(&mut slice, par, own);
                    // В подсеточной оси SELF-выравнивание не действует:
                    // подсетка держит всю дорожку.
                    if row_dir {
                        child.style.grid_rows = Some(slice);
                        child.style.align_self = None;
                    } else {
                        child.style.grid_tracks = Some(slice);
                        child.style.grid_cols = Some(span as u16);
                        child.style.justify_self = None;
                    }
                }
            }
        }
        subgrid_takes_parent_tracks(&mut el.children);
    }
}

fn flex_items_lose_float(nodes: &mut [Node]) {
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

/// Абсолютный ребёнок СЕТКИ или ГИБКОГО контейнера без заданных краёв стоит
/// на статической позиции, а она отсчитывается от СОДЕРЖИМОГО контейнера
/// (css-grid-2 §9.1, css-flexbox-1 §4.1), тогда как раскладка под нами кладёт
/// такого ребёнка в коробку ПОЛЕЙ. Разницу забирает поле элемента: при
/// выравнивании к началу оно даёт левый отступ, к концу — правый, по центру —
/// сдвиг на половину разницы, при растяжении — обе стороны сразу.
fn content_box_static_position(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        content_box_static_position(&mut el.children);
        if !matches!(
            el.style.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        ) {
            continue;
        }
        let pad = el.style.padding;
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if child.style.position != Some(Position::Absolute) {
                continue;
            }
            // Элемент с заданными линиями стоит не на статической позиции, а в
            // СВОЕЙ области сетки — поля туда добавлять нечего.
            if child.style.grid_col.is_some() || child.style.grid_row.is_some() {
                continue;
            }
            // `left: auto` — это ОТСУТСТВИЕ края, а не заданный край: именно
            // при `auto` с обеих сторон элемент стоит на статической позиции.
            let auto = |l: Option<Len>| matches!(l, None | Some(Len::Auto));
            let inset = child.style.inset;
            if auto(inset.left) && auto(inset.right) {
                child.style.margin.left = add_len(child.style.margin.left, pad.left);
                child.style.margin.right = add_len(child.style.margin.right, pad.right);
            }
            if auto(inset.top) && auto(inset.bottom) {
                child.style.margin.top = add_len(child.style.margin.top, pad.top);
                child.style.margin.bottom = add_len(child.style.margin.bottom, pad.bottom);
            }
        }
    }
}

/// Сумма двух длин. Складываются только точки: смешивать доли и кегли здесь
/// не с чем — контейнера в этот момент нет.
fn add_len(a: Option<Len>, b: Option<Len>) -> Option<Len> {
    match (a, b) {
        (Some(Len::Px(x)), Some(Len::Px(y))) => Some(Len::Px(x + y)),
        (None, b) => b,
        (a, _) => a,
    }
}

/// Абсолютный ПОТОМОК сетки размещается по её линиям, а не по статической
/// позиции: если содержащий блок такого элемента — сама сетка, то `grid-row`
/// и `grid-column` задают ему прямоугольник области (css-grid-2 §9). Раскладка
/// знает только ПРЯМЫХ детей сетки, поэтому потомок поднимается к ней. Стиль к
/// этому моменту уже вычислен, и переезд по дереву его не меняет.
fn hoist_grid_abspos(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        hoist_grid_abspos(&mut el.children);
        if !is_grid(&el.style) || !own_containing_block(&el.style) {
            continue;
        }
        let mut taken = vec![];
        for child in el.children.iter_mut() {
            let Node::Element(child) = child else {
                continue;
            };
            if own_containing_block(&child.style) || is_grid(&child.style) {
                continue;
            }
            steal_placed(&mut child.children, &mut taken);
        }
        el.children.extend(taken);
    }
}

/// Забрать из поддерева абсолютные элементы с заданными линиями сетки.
fn steal_placed(children: &mut Vec<Node>, out: &mut Vec<Node>) {
    let mut kept = Vec::with_capacity(children.len());
    for mut node in children.drain(..) {
        if let Node::Element(el) = &mut node {
            let placed = el.style.grid_col.is_some() || el.style.grid_row.is_some();
            if el.style.position == Some(Position::Absolute) && placed {
                out.push(node);
                continue;
            }
            // Свой содержащий блок — дальше уже чужие абсолютные элементы.
            // Останавливает и ПОДСЕТКА: она размещает своих абсолютных детей
            // сама, и счёт с конца (`grid-column: 3 / -1`) идёт по её
            // собственному числу дорожек (`subgrid/abs-pos-001`). А вот обычная
            // вложенная сетка без своего отсчёта помехой не служит: её потомок
            // по-прежнему принадлежит внешней сетке (css-grid-2 §9).
            if !own_containing_block(&el.style) && !el.style.subgrid {
                steal_placed(&mut el.children, out);
            }
        }
        kept.push(node);
    }
    *children = kept;
}

/// Контейнер сетки — это и `inline-grid`: разница только в том, как коробка
/// встаёт в поток снаружи.
fn is_grid(c: &Computed) -> bool {
    matches!(c.display, Some(Display::Grid) | Some(Display::InlineGrid))
}

/// Задаёт ли элемент отсчёт для абсолютных потомков.
fn own_containing_block(c: &Computed) -> bool {
    matches!(
        c.position,
        Some(Position::Relative)
            | Some(Position::Absolute)
            | Some(Position::Fixed)
            | Some(Position::Sticky)
    )
}

/// Кастомные свойства из правил. Селектор не важен: в документе переменные
/// почти всегда объявлены на корне, а разбирать их область видимости — это

/// Содержимое `<style>`-тегов документа, каждый — отдельной таблицей;
/// html5ever кладёт его текстом внутрь.
/// ПРОБОВАЛИ И ОТКАТИЛИ: раскрывать ссылки на знаки (`&gt;`, `&amp;`) в
/// содержимом `<style>`, когда документ XHTML. По разбору XML это верно —
/// построитель HTML держит `<style>` сырым текстом, и селектор `body &gt; div`
/// не совпадает ни с чем; в наборе таких файлов 103. Замерено по всему CSS2:
/// 5233 -> 5230, приобретено 0, потеряно 3. Раскрытые правила доезжают до
/// раскладки и вскрывают ошибки НИЖЕ (`text-indent-intrinsic-001` 0.00 -> 1.10,
/// `-003` 0.00 -> 0.68), а `content-177` 0.42 -> «красное видно»: раскрытая
/// `&quot;` закрывает строку CSS раньше времени. Возвращаться сюда после
/// разбора этих трёх корней.
fn collect_style_tags(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, .. } = &handle.data
        && name.local.as_ref() == "style"
    {
        let mut sheet = String::new();
        for child in handle.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                // Обёртка `<![CDATA[ … ]]>` встречается в эталонах XHTML: там
                // она прячет стиль от разбора XML. Разбору CSS она мусор, и
                // правила до первой закрывающей скобки пропадали вместе с ней.
                let text = contents.borrow();
                let trimmed = text.trim();
                let body = trimmed
                    .strip_prefix("<![CDATA[")
                    .and_then(|rest| rest.strip_suffix("]]>"))
                    .unwrap_or(&text);
                sheet.push_str(body);
                sheet.push('\n');
            }
        }
        out.push(sheet);
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
}

/// Цепочка предков для сопоставления `.card .title`: тег + классы + id.
#[derive(Clone)]
pub(crate) struct Ancestor {
    tag: String,
    id: Option<String>,
    classes: Vec<String>,
    /// Все атрибуты узла: нужны атрибутным селекторам.
    attrs: Vec<(String, String)>,
    /// Место среди соседей: нужно структурным псевдоклассам.
    spot: Spot,
    /// Адрес ссылки: нужен `:link`/`:visited`.
    href: Option<String>,
    /// Атрибут `dir` самого узла (true = rtl): нужен `:dir()`.
    dir: Option<bool>,
    /// Отметки `:has()`: хеши аргументов, для которых узел — якорь с
    /// совпадением. Считаются отдельным проходом до обхода (см. `mark_has`).
    has_marks: Vec<u64>,
}

/// Отметки `:has()` текущего документа: адрес узла - хеши аргументов.
///
/// Поток разбирает документ целиком, поэтому склад потоко-локальный:
/// заполняется перед обходом, чистится по его окончании. Протаскивать его
/// параметром через всю цепочку обхода - шесть сигнатур ради одной ветки.
thread_local! {
    static HAS_MARKS: std::cell::RefCell<HashMap<usize, Vec<u64>>> =
        std::cell::RefCell::new(HashMap::new());
}

/// Хеш одного аргумента `:has(...)` - ключ отметки.
fn has_id(arg: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    arg.hash(&mut h);
    h.finish()
}

fn has_marks_of(handle: &Handle) -> Vec<u64> {
    let key = std::rc::Rc::as_ptr(handle) as usize;
    HAS_MARKS.with(|m| m.borrow().get(&key).cloned().unwrap_or_default())
}

/// Направление письма, заданное АТРИБУТОМ: `<div dir="rtl">`.
///
/// В разметке направление задают именно атрибутом, а не стилем: он и есть
/// обычный способ написать страницу справа налево. Тег `<bdo>` вдобавок
/// ОТМЕНЯЕТ разбор двунаправленности — знаки идут ровно в заданную сторону.
fn apply_direction(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let Some((_, value)) = attrs.iter().find(|(k, _)| k == "dir") else {
        if tag == "bdo" {
            style.bidi_override = Some(true);
        }
        return;
    };
    match value.to_ascii_lowercase().as_str() {
        "rtl" => {
            if style.rtl.is_none() {
                style.rtl = Some(true)
            }
        }
        "ltr" => {
            if style.rtl.is_none() {
                style.rtl = Some(false)
            }
        }
        // `dir="auto"` — сторону выбирает первый сильный знак текста; это
        // делает разбор двунаправленности сам, поэтому здесь ничего не ставим.
        _ => {}
    }
    if tag == "bdo" {
        style.bidi_override = Some(true);
    }
}

/// Размер, заданный АТРИБУТОМ: `<img width="100" height="36">`.
///
/// В HTML это «представленческая подсказка» — стиль самого слабого веса, и
/// без него картинка в разметке без CSS выходит по своему пикселю, а не по
/// заявленному размеру. Атрибут проигрывает любому правилу CSS, поэтому
/// применяется, только если размера ещё нет.
/// Дорешать `display: inline` после каскада (CSS 2.1).
///
/// §9.7: плавающий или абсолютный элемент блокифицируется. §10.2: на
/// незамещаемом строчном width/height/min/max не применяются — раньше
/// `div { display: inline; width: 1in }` рисовался коробкой (наш строчный
/// уровень выражается через inline-block, который размеры принимает).
/// Презентационные цвета разметки: `bgcolor` и `text` — хинты ниже
/// авторского CSS (каскад уже слит, поэтому «ниже» выражается как
/// «только если стиль цвета не задал»).
fn apply_presentational_colors(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    let color_of = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| crate::value::Color::parse(v.trim()))
    };
    if matches!(tag, "body" | "table" | "tr" | "td" | "th")
        && style.background.is_none()
        && style.gradient.is_none()
        && let Some(c) = color_of("bgcolor")
    {
        style.background = Some(c);
    }
    if tag == "body"
        && style.color.is_none()
        && let Some(c) = color_of("text")
    {
        style.color = Some(c);
    }
}

fn finish_inline_display(style: &mut Computed, tag: &str) {
    use crate::computed::Display;
    let out_of_flow = style.float.is_some()
        || matches!(
            style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        );
    // Блокификация СТРОЧНЫХ вариантов под float/abspos (§9.7): каждый
    // получает свой блочный аналог, а не только `inline`.
    // §9.7: у абсолютно позиционированной коробки `float` вычисляется в
    // `none`. Пока сброса не было, `float: right; position: fixed` уезжал в
    // ряд обтекания и до выноса в слой окна не доходил (`position-fixed-007`).
    if matches!(
        style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        style.float = None;
    }
    if out_of_flow {
        match style.display {
            Some(Display::InlineFlex) => style.display = Some(Display::Flex),
            Some(Display::InlineGrid) => style.display = Some(Display::Grid),
            Some(Display::InlineTable) => style.display = Some(Display::Table),
            Some(Display::InlineBlock) => style.display = Some(Display::Block),
            // ВНУТРЕННИЕ табличные виды блокифицируются в `block`, а не в
            // `table` (§9.7 вместе с §9.2.4): вне потока ряд, группа рядов и
            // ячейка своей таблицы уже не образуют. Колонка коробки не
            // порождала вовсе (`Display::None`), и вне потока квадрат просто
            // не рисовался (`top-applies-to-006`).
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell) => {
                style.display = Some(Display::Block)
            }
            Some(Display::None) if style.col_role.is_some() => style.display = Some(Display::Block),
            _ => {}
        }
        // Метки табличных ролей снимаются вместе с видом: иначе таблица
        // подобрала бы вне-поточный узел обратно в решётку (§17.2.1).
        if matches!(style.display, Some(Display::Block)) {
            style.col_role = None;
            style.row_group_kind = None;
            style.is_caption = None;
        }
    }
    // §9.5.2 «Applies to: block-level»: у коробки НЕ блочного уровня `clear`
    // не действует. Первый заход давал 0 и 0, потому что размещение самой
    // коробки рядом с флоатом тогда ещё было сломано.
    if matches!(
        style.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineTable)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
    ) || style.is_caption == Some(true)
        // Абсолютная коробка вне потока: флоатов выше неё в её контексте нет,
        // и очищать нечего (§9.5.2 действует на поток).
        || matches!(
            style.position,
            Some(Position::Absolute) | Some(Position::Fixed)
        )
    {
        style.clear = None;
        style.clear_inherit = false;
    }
    //
    // Поля к внутренним табличным видам НЕ применяются (§8.3), `clear` — только
    // к коробкам блочного УРОВНЯ (§9.5.2). Заголовок сюда не входит: он
    // блочная коробка, и поля у него законны.
    if matches!(
        style.display,
        Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
    ) || style.col_role.is_some()
    {
        style.margin = Default::default();
        style.clear = None;
        style.clear_inherit = false;
    }
    // Боковые auto-поля ПЛАВАЮЩЕГО используются нулём (§10.3.5): распирать
    // флоат от его края им нечем. До taffy `auto` доезжало как есть и уводило
    // коробку на всю свободную ширину. `float: none` разбирается в `Some(0)` —
    // поэтому сравнение со значением, а не `is_some`.
    // ПРОБОВАЛИ И ОТКАТИЛИ: обнулять боковое `auto`-поле и у коробок
    // СТРОЧНОГО уровня (§10.3.2, §10.3.9). CSS2 +1, но oldfront 2343 -> 2341:
    // у ЭЛЕМЕНТА ГИБКОГО контейнера `auto`-поле законно и забирает свободное
    // место (css-flexbox §8.1), а родителя эта функция не видит.
    // Возвращать вместе с признаком «ребёнок гибкого контейнера».
    if style.float.is_some_and(|f| f != 0) {
        if style.margin.left == Some(Len::Auto) {
            style.margin.left = Some(Len::Px(0.0));
        }
        if style.margin.right == Some(Len::Auto) {
            style.margin.right = Some(Len::Px(0.0));
        }
    }
    // §9.7: плавающий блокифицируется — и тот, чей строчный уровень идёт от
    // ИМЕНИ ТЕГА, а не от объявленного `display`. Пометка `inline_display`
    // ставится только на дословный `display: inline`, поэтому голый
    // `<span style="float:left">` до блокификации не доезжал вовсе: ширина и
    // высота на нём не применялись, и вместо коробки 120x120 рисовался кусок
    // строки по кеглю (`absolute-non-replaced-width-020/024`).
    // Только ПЛАВАЮЩИЙ: у абсолютного статическая позиция считается по
    // гипотезе §10.3.7 «если бы position был static», и строчный уровень ей
    // нужен (`render.rs`: `inline_level(e) || inline_display`).
    //
    // ★ ЗАМЕРЕНО: CSS2 5316 -> 5320, CSS3 2419 -> 2416 (в своде было 2415,
    // но `css-flexbox-height-animation-stretch` мигает: 1.33 / 0.00 / 1.10 на
    // одном и том же бинаре). Итого +1. Приобретено: `absolute-non-replaced-
    // width-020`, `float-non-replaced-width-008`, `clear-float-004`,
    // `floats-025`, `floats-145`. Потеряно: `float-nowrap-3/-9`,
    // `float-nowrap-hyphen-rewind-1` и тройка `text-justify-*-001` (у них
    // плавающий `<span>` стоит в ЭТАЛОНЕ) — все по одной причине: наш флоат
    // уходит в отдельный ряд обтекания и рядом со своей строкой уже не стоит.
    // Убирается настоящей коробкой флоата В строке, а не откатом блокификации.
    if style.float.is_some_and(|f| f != 0) && style.display.is_none() && INLINE_TAGS.contains(&tag)
    {
        style.display = Some(Display::Block);
    }
    if style.inline_display != Some(true) {
        return;
    }
    if out_of_flow {
        style.display = Some(Display::Block);
        return;
    }
    let replaced = matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input"
    );
    if !replaced {
        style.width = None;
        style.height = None;
        style.min_width = None;
        style.min_height = None;
        style.max_width = None;
        style.max_height = None;
    }
}

fn apply_presentational_size(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    if !matches!(
        tag,
        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "table"
    ) {
        return;
    }
    let value = |name: &str| {
        attrs
            .iter()
            .find(|(k, _)| k == name)
            .and_then(|(_, v)| match v.strip_suffix('%') {
                Some(pct) => pct.trim().parse::<f32>().ok().map(|p| Len::Pct(p / 100.0)),
                None => v.trim().parse::<f32>().ok().map(Len::Px),
            })
    };
    style.attr_width = value("width");
    style.attr_height = value("height");
    // Своего пикселя у холста нет, но размер по умолчанию задан разметкой:
    // 300 на 150 (HTML §4.12.5). Без него `<canvas width="20">` выходил
    // нулевой высоты, а холст без атрибутов — пустым местом.
    // Таблица замещаемой не является: у неё намёком служит только `width`
    // (HTML §15.3.2), а `attr_*` держит соотношение сторон замещаемого и
    // таблице не принадлежит. Без этого `<table width="300">` вовсе не
    // доходил до стиля, и таблица сжималась по содержимому.
    if tag == "table" {
        let w = style.attr_width.take();
        style.attr_height = None;
        if matches!(style.width, None | Some(Len::Auto)) {
            style.width = w;
        }
        return;
    }
    if tag == "canvas" {
        style.attr_width = style.attr_width.or(Some(Len::Px(300.0)));
        style.attr_height = style.attr_height.or(Some(Len::Px(150.0)));
    }
    if style.width.is_none() {
        style.width = style.attr_width;
    }
    if style.height.is_none() {
        style.height = style.attr_height;
    }
}

/// Место элемента среди соседей — по нему считаются структурные псевдоклассы.
#[derive(Clone, Copy, Default)]
pub(crate) struct Spot {
    /// Номер среди соседей-элементов, с единицы.
    pub(crate) index: usize,
    /// Сколько всего соседей-элементов.
    total: usize,
    /// То же, но среди соседей с ТЕМ ЖЕ тегом (`:nth-of-type`).
    of_type: usize,
    of_type_total: usize,
}

/// Братья узла: ВСЕ дети-элементы родителя и позиция узла среди них.
///
/// Соседним комбинаторам `+`/`~` хватает предыдущих, но
/// `:nth-last-child(… of S)` считает совпавших среди ПОСЛЕДУЮЩИХ
/// (селекторы-4 §child-index) — поэтому список полный.
#[derive(Clone, Copy)]
pub(crate) struct Sibs<'a> {
    pub(crate) all: &'a [Ancestor],
    /// Сколько элементов стоит ДО узла; сам узел-элемент = `all[pos]`.
    pub(crate) pos: usize,
    /// Узел — элемент и присутствует в `all[pos]`.
    pub(crate) is_elem: bool,
}

impl<'a> Sibs<'a> {
    pub(crate) const EMPTY: Sibs<'static> = Sibs {
        all: &[],
        pos: 0,
        is_elem: false,
    };

    /// Предыдущие соседи-элементы — для `+` и `~`.
    fn prev(&self) -> &'a [Ancestor] {
        &self.all[..self.pos]
    }

    /// Последующие соседи-элементы.
    fn next(&self) -> &'a [Ancestor] {
        &self.all[self.pos + usize::from(self.is_elem)..]
    }

    /// Те же братья глазами элемента с номером `i` в общем списке.
    pub(crate) fn at(&self, i: usize) -> Sibs<'a> {
        Sibs {
            all: self.all,
            pos: i,
            is_elem: true,
        }
    }
}

/// Паспорт элемента для сопоставления селекторов.
pub(crate) fn ancestor_of(child: &Handle, spot: Spot) -> Option<Ancestor> {
    let NodeData::Element { name, attrs, .. } = &child.data else {
        return None;
    };
    let attrs = attrs.borrow();
    let find = |key: &str| {
        attrs
            .iter()
            .find(|a| a.name.local.as_ref() == key)
            .map(|a| a.value.to_string())
    };
    Some(Ancestor {
        tag: local_name(&name.local),
        id: find("id"),
        classes: find("class")
            .map(|v| v.split_whitespace().map(str::to_string).collect())
            .unwrap_or_default(),
        attrs: attrs
            .iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect(),
        spot,
        href: find("href"),
        dir: find("dir").and_then(|v| match v.to_ascii_lowercase().as_str() {
            "rtl" => Some(true),
            "ltr" => Some(false),
            _ => None,
        }),
        has_marks: has_marks_of(child),
    })
}

/// Разобранный аргумент `:has()`: части списка, каждая с ведущим
/// комбинатором, уже пришитым якорем-стражем к самому левому компаунду.
struct HasArg {
    id: u64,
    /// (свой ли уровень: `+`/`~` - братья, иначе поддерево; селектор).
    parts: Vec<(bool, Selector)>,
}

/// Имя атрибута-стража: NUL из html5ever не приходит, коллизий нет.
const HAS_SENTINEL: &str = "\u{0}scope";

/// Пришить якорь-стража к самому левому компаунду цепочки.
fn attach_anchor(sel: &mut Selector, lead: char) {
    if let Some(p) = sel.prev.as_mut() {
        return attach_anchor(&mut p.0, lead);
    }
    if let Some(a) = sel.ancestor.as_mut() {
        return attach_anchor(&mut a.0, lead);
    }
    let sentinel = Selector {
        tag: None,
        id: None,
        classes: vec![],
        attrs: vec![crate::css::AttrSel {
            name: HAS_SENTINEL.to_string(),
            op: None,
            ci: false,
        }],
        pseudo: None,
        also: vec![],
        ancestor: None,
        prev: None,
    };
    match lead {
        '>' => sel.ancestor = Some(Box::new((sentinel, true))),
        '+' => sel.prev = Some(Box::new((sentinel, true))),
        '~' => sel.prev = Some(Box::new((sentinel, false))),
        _ => sel.ancestor = Some(Box::new((sentinel, false))),
    }
}

/// Разобрать аргумент `:has(...)`. Список НЕпрощающий (селекторы-4):
/// битая часть делает недействительным весь аргумент - `None`.
fn parse_has_arg(arg: &str) -> Option<HasArg> {
    let mut parts = vec![];
    for one in crate::css::split_selector_list(arg) {
        let one = one.trim();
        let (lead, rest) = match one.chars().next()? {
            c @ ('>' | '+' | '~') => (c, &one[1..]),
            _ => (' ', one),
        };
        let mut sel = Selector::parse(rest)?;
        attach_anchor(&mut sel, lead);
        parts.push((matches!(lead, '+' | '~'), sel));
    }
    (!parts.is_empty()).then(|| HasArg {
        id: has_id(arg),
        parts,
    })
}

/// Собрать строки-аргументы всех `:has()` селектора, включая вложенные в
/// `:not()` и `of S`. Возвращает false, если встретился НЕдопустимый -
/// вложенный `:has` (правило целиком недействительно, спека: cannot be
/// nested).
fn collect_has_args(sel: &Selector, out: &mut Vec<String>) -> bool {
    for p in sel.pseudo.iter().chain(sel.also.iter()) {
        if let Some(rest) = p.strip_prefix("has(").and_then(|r| r.strip_suffix(')')) {
            if rest.contains("has(") {
                return false;
            }
            if !out.iter().any(|a| a == rest) {
                out.push(rest.to_string());
            }
        } else if let Some(inner) = p.strip_prefix("not(").and_then(|r| r.strip_suffix(')')) {
            if let Some(s) = Selector::parse(inner)
                && !collect_has_args(&s, out)
            {
                return false;
            }
        } else if let Some((_, arg)) = p.split_once('(')
            && let Some(arg) = arg.strip_suffix(')')
            && let Some((_, list)) = crate::css::nth_of_parts(arg)
        {
            for s in &list {
                if !collect_has_args(s, out) {
                    return false;
                }
            }
        }
    }
    if let Some(a) = &sel.ancestor
        && !collect_has_args(&a.0, out)
    {
        return false;
    }
    if let Some(pr) = &sel.prev
        && !collect_has_args(&pr.0, out)
    {
        return false;
    }
    true
}

/// Перепись детей уровня: места и паспорта всех элементов.
pub(crate) fn census_of(children: &[Handle]) -> (Vec<Spot>, Vec<Ancestor>) {
    let tags: Vec<Option<String>> = children
        .iter()
        .map(|c| match &c.data {
            NodeData::Element { name, .. } => Some(name.local.to_string()),
            _ => None,
        })
        .collect();
    let total = tags.iter().filter(|t| t.is_some()).count();
    let mut seen = 0usize;
    let mut seen_of_type: HashMap<String, usize> = HashMap::new();
    let mut spots: Vec<Spot> = Vec::with_capacity(children.len());
    let mut all: Vec<Ancestor> = Vec::with_capacity(total);
    for (child, tag) in children.iter().zip(&tags) {
        let spot = match tag {
            Some(tag) => {
                seen += 1;
                let of_type = seen_of_type.entry(tag.clone()).or_insert(0);
                *of_type += 1;
                Spot {
                    index: seen,
                    total,
                    of_type: *of_type,
                    of_type_total: tags.iter().filter(|t| t.as_deref() == Some(tag)).count(),
                }
            }
            None => Spot::default(),
        };
        spots.push(spot);
        if let Some(a) = ancestor_of(child, spot) {
            all.push(a);
        }
    }
    (spots, all)
}

/// Паспорт якоря с пришитым атрибутом-стражем.
fn with_sentinel(a: &Ancestor) -> Ancestor {
    let mut out = a.clone();
    out.attrs.push((HAS_SENTINEL.to_string(), String::new()));
    out
}

/// Есть ли в СТРОГОМ поддереве узла предмет селектора с якорем в `path`.
fn has_in_subtree(handle: &Handle, sel: &Selector, path: &mut Vec<Ancestor>) -> bool {
    let children = handle.children.borrow();
    let (spots, all) = census_of(&children);
    let mut pos = 0usize;
    for (child, spot) in children.iter().zip(&spots) {
        if spot.index == 0 {
            continue;
        }
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem: true,
        };
        if matches(sel, &all[pos], path, sibs) {
            return true;
        }
        path.push(all[pos].clone());
        let hit = has_in_subtree(child, sel, path);
        path.pop();
        if hit {
            return true;
        }
        pos += 1;
    }
    false
}

/// Проход-разметчик `:has()`: на каждый элемент и каждый аргумент решает,
/// найдётся ли предмет - в поддереве либо среди последующих братьев - и
/// складывает отметку. Вложенный `:has` запрещён, поэтому матчи внутри
/// аргумента в отметки не заглядывают и циклов нет.
fn mark_has(handle: &Handle, args: &[HasArg], path: &mut Vec<Ancestor>) {
    let children = handle.children.borrow();
    let (spots, all) = census_of(&children);
    let mut pos = 0usize;
    for (child, spot) in children.iter().zip(&spots) {
        if spot.index == 0 {
            continue;
        }
        let mut marks: Vec<u64> = vec![];
        for arg in args {
            let hit = arg.parts.iter().any(|(sibling, sel)| {
                if *sibling {
                    let mut peers = all.clone();
                    peers[pos] = with_sentinel(&all[pos]);
                    (pos + 1..peers.len()).any(|i| {
                        let sibs = Sibs {
                            all: &peers,
                            pos: i,
                            is_elem: true,
                        };
                        matches(sel, &peers[i], path, sibs)
                    })
                } else {
                    path.push(with_sentinel(&all[pos]));
                    let hit = has_in_subtree(child, sel, path);
                    path.pop();
                    hit
                }
            });
            if hit {
                marks.push(arg.id);
            }
        }
        if !marks.is_empty() {
            let key = std::rc::Rc::as_ptr(child) as usize;
            HAS_MARKS.with(|m| m.borrow_mut().insert(key, marks));
        }
        path.push(all[pos].clone());
        mark_has(child, args, path);
        path.pop();
        pos += 1;
    }
}

/// Обойти детей узла, посчитав каждому его место среди соседей.
#[allow(clippy::too_many_arguments)]
fn walk_children(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::counters::Counters,
    path: &[Ancestor],
    preserve: bool,
    out: &mut Vec<Node>,
) {
    let children = handle.children.borrow();
    // Перепись братьев ЦЕЛИКОМ до обхода: `:nth-last-child(… of S)` смотрит
    // и на последующих, поэтому паспорта всех детей-элементов собираются
    // заранее, а каждый узел получает свою позицию в общем списке.
    let (spots, all) = census_of(&children);
    let mut pos = 0usize;
    for (idx, (child, spot)) in children.iter().zip(&spots).enumerate() {
        let is_elem = spot.index != 0;
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem,
        };
        walk(
            child, rules, vars, frames, counter, counters, path, *spot, preserve, sibs, &children,
            &spots, idx, out,
        );
        pos += usize::from(is_elem);
    }
}

#[allow(clippy::too_many_arguments)]
fn walk(
    handle: &Handle,
    rules: &[Rule],
    vars: &Decls,
    frames: &HashMap<String, Keyframes>,
    counter: &mut u64,
    counters: &mut crate::counters::Counters,
    path: &[Ancestor],
    spot: Spot,
    preserve: bool,
    sibs: Sibs,
    level: &[Handle],
    spots: &[Spot],
    level_pos: usize,
    out: &mut Vec<Node>,
) {
    match &handle.data {
        NodeData::Text { contents } => {
            let text = contents.borrow().to_string();
            // Пустой узел ПО CSS — только схлопываемые пробелы
            // (`space`/`tab`/`CR`/`LF`). `str::trim` снимает весь юникодный
            // пробел, и узел из идеографических U+3000 отбрасывался прямо на
            // разборе: строка из них не доезжала до раскладки вовсе
            // (`trailing-ideographic-space-017`).
            let collapsible = |c: char| matches!(c, ' ' | '\t' | '\r' | '\n');
            // Под `white-space: pre*` схлопывания нет вовсе: узел из одного
            // перевода строки — это жёсткий разрыв, и выбрасывать его нельзя
            // (`word-space-transform-011`: `あ<wbr>い<wbr>\n<wbr>う` шло одной
            // строкой, потому что перевод пропадал ещё на разборе).
            if preserve || !text.chars().all(collapsible) || text.contains(' ') {
                // Комментарий разрезает пробельный кусок надвое, а схлопывание
                // работает по одному узлу — выходило два пробела подряд.
                // Соседние текстовые узлы склеиваются в один отрезок.
                match out.last_mut() {
                    Some(Node::Text(prev)) if !preserve => prev.push_str(&text),
                    _ => out.push(Node::Text(text)),
                }
            }
        }
        NodeData::Element { name, attrs, .. } => {
            let tag = local_name(&name.local);
            if DROP_TAGS.contains(&tag.as_str()) {
                return;
            }
            let attrs: Vec<(String, String)> = attrs
                .borrow()
                .iter()
                .map(|a| (a.name.local.to_string(), a.value.to_string()))
                .collect();
            let id = attrs
                .iter()
                .find(|(k, _)| k == "id")
                .map(|(_, v)| v.clone());
            let classes: Vec<String> = attrs
                .iter()
                .find(|(k, _)| k == "class")
                .map(|(_, v)| v.split_whitespace().map(str::to_string).collect())
                .unwrap_or_default();
            let me = Ancestor {
                tag: tag.clone(),
                id: id.clone(),
                classes: classes.clone(),
                attrs: attrs.clone(),
                spot,
                href: attrs
                    .iter()
                    .find(|(k, _)| k == "href")
                    .map(|(_, v)| v.clone()),
                dir: attrs.iter().find(|(k, _)| k == "dir").and_then(|(_, v)| {
                    match v.to_ascii_lowercase().as_str() {
                        "rtl" => Some(true),
                        "ltr" => Some(false),
                        _ => None,
                    }
                }),
                has_marks: has_marks_of(handle),
            };

            let inline_decls: Decls = attrs
                .iter()
                .find(|(k, _)| k == "style")
                .map(|(_, v)| parse_decls(v))
                .unwrap_or_default();
            let mut matched: Vec<&Rule> = rules
                .iter()
                .filter(|r| matches(&r.sel, &me, path, sibs))
                .collect();
            // Свои переменные: родительские, поверх них объявления
            // совпавших правил в порядке каскада, поверх — свои же в
            // атрибуте. Пока словарь был один на документ, `:root{--c:red}`
            // и `.dark{--c:blue}` складывались в него подряд, и последнее
            // объявление красило ВЕСЬ документ — переключение темы классом
            // не работало в принципе.
            let own_vars = {
                let mut own = vars.clone();
                let mut by_cascade: Vec<&&Rule> = matched.iter().collect();
                by_cascade.sort_by_key(|r| (r.origin, r.sel.specificity(), r.order));
                for rule in by_cascade {
                    for (k, v) in &rule.decls {
                        if k.starts_with("--") {
                            own.insert(k.clone(), v.clone());
                        }
                    }
                }
                for (k, v) in &inline_decls {
                    if k.starts_with("--") {
                        own.insert(k.clone(), v.clone());
                    }
                }
                own
            };
            let vars = &own_vars;
            let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
            apply_presentational_size(&mut style, &tag, &attrs);
            apply_presentational_colors(&mut style, &tag, &attrs);
            finish_inline_display(&mut style, &tag);
            // Язык — свойство узла, а не CSS: по нему выбираются образцы
            // слогораздела (`hyphens: auto`).
            if let Some((_, v)) = attrs.iter().find(|(k, _)| k == "lang") {
                style.lang = Some(v.clone());
            }
            apply_direction(&mut style, &tag, &attrs);
            // Правила с `:hover` собираются отдельным слоем: в базовый стиль
            // им нельзя, иначе элемент выглядел бы всегда наведённым.
            let mut hovered: Vec<&Rule> = rules
                .iter()
                .filter(|r| r.sel.pseudo.as_deref() == Some("hover"))
                .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                .collect();
            hovered.sort_by_key(|r| (r.sel.specificity(), r.order));
            // Слой наведения собирается ПОВЕРХ базового стиля, а не с нуля:
            // правило `:hover` меняет два-три свойства, а не весь стиль. Со
            // сборкой «с нуля» плавный переход на середине пути показывал
            // голый огрызок — без отступов, размеров и шрифта.
            let hover = (!hovered.is_empty()).then(|| {
                let mut merged = style.clone();
                for rule in hovered.iter() {
                    merged.apply_decls_with_vars(&rule.decls, vars);
                }
                merged
            });
            // Псевдоэлементы первой буквы и первой строки — тем же слоем
            // поверх базового стиля: они меняют начертание куска, а не блок.
            let layer = |name: &str| {
                let mut found: Vec<&Rule> = rules
                    .iter()
                    .filter(|r| r.sel.pseudo.as_deref() == Some(name))
                    .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                    .collect();
                found.sort_by_key(|r| (r.sel.specificity(), r.order));
                (!found.is_empty()).then(|| {
                    let mut merged = style.clone();
                    for rule in found.iter() {
                        merged.apply_decls_with_vars(&rule.decls, vars);
                    }
                    merged
                })
            };
            let first_letter = layer("first-letter");
            let first_line = layer("first-line");

            if style.display == Some(Display::None) {
                // Колонка — единственный `display: none`, который таблице
                // НУЖЕН живым: из неё берутся ширина дорожки, слой краски и
                // рамка для разбора сросшихся кромок. Собирается отдельной
                // веткой: счётчики, псевдоэлементы, `dir="auto"` и кадры
                // анимации у безкоробочного узла не действуют, а общий путь
                // ниже применил бы их все.
                // Фон КОРНЯ красит канвас, даже когда коробок документ не
                // даёт вовсе (§14.2: «the canvas background is the root
                // element's background»). Узел остаётся пустышкой с одним
                // стилем: коробку `display: none` ему всё равно не соберут, а
                // пометка канваса без него не ставится.
                if me.tag == "html" {
                    out.push(Node::Element(Element {
                        tag: "html".to_string(),
                        inline: false,
                        node_id: 0,
                        style,
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children: vec![],
                        attrs: vec![],
                        anim: Default::default(),
                        list_item: None,
                    }));
                    return;
                }
                let Some(role) = style.col_role else {
                    return;
                };
                // §17.2.1: у колонки детей нет вовсе, у группы колонок
                // остаются только колонки.
                let mut kids: Vec<Node> = vec![];
                if role == 1 {
                    let mut path2 = path.to_vec();
                    path2.push(me.clone());
                    let mut raw: Vec<Node> = vec![];
                    walk_children(
                        handle,
                        rules,
                        vars,
                        frames,
                        counter,
                        counters,
                        &path2,
                        style.preserve_newlines.unwrap_or(preserve),
                        &mut raw,
                    );
                    kids = raw
                        .into_iter()
                        .filter(|n| matches!(n, Node::Element(c) if c.style.col_role == Some(0)))
                        .collect();
                }
                *counter += 1;
                out.push(Node::Element(Element {
                    list_item: None,
                    node_id: *counter,
                    anim: None,
                    inline: false,
                    tag,
                    style,
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: kids,
                    attrs,
                }));
                return;
            }

            // Счётчики: свои директивы узел применяет ДО детей и до своих
            // псевдоэлементов (css-lists §5: сброс, увеличение, установка).
            // Адрес узла — в дереве КОРОБОК: `display: contents` своего
            // уровня не даёт, поэтому его дети остаются братьями соседей.
            let box_level = style.display != Some(Display::Contents);
            if box_level {
                counters.enter();
            }
            // Обратный счётчик без числа: начальное значение — итог
            // предварительного обхода области (css-lists-3
            // §instantiating-counters). Считается ЗДЕСЬ, до применения
            // директив: запись создаётся уже готовым числом.
            let reversed_start = |nm: &str, counters: &mut crate::counters::Counters| {
                crate::counters_scan::reversed_initial(
                    rules, vars, nm, handle, &me, path, sibs, level, spots, level_pos,
                )
            };
            let mut is_list_item = false;
            apply_counter_decls(
                &style,
                counters,
                &tag,
                &attrs,
                &mut is_list_item,
                &reversed_start,
            );
            // Номер пункта снимается СРАЗУ после своих директив — до
            // псевдоэлементов и детей, которые счётчик двигают дальше.
            let list_item = is_list_item.then(|| counters.value_of("list-item"));

            let mut path2 = path.to_vec();
            path2.push(me.clone());
            let mut children = vec![];
            // Псевдоэлементы: коробка появляется, только если у правила есть
            // `content`. Значками, стрелками и разделителями в вёрстке
            // занимаются именно они, и без них разметка теряет часть смысла.
            // `::before` строится ДО детей, `::after` — после: счётчики они
            // видят в том же порядке, что и браузер (css-lists §counters).
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                Sibs::EMPTY,
                "before",
                &attrs,
            ) {
                children.push(Node::Element(el));
            }
            walk_children(
                handle,
                rules,
                vars,
                frames,
                counter,
                counters,
                &path2,
                style.preserve_newlines.unwrap_or(preserve),
                &mut children,
            );
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                Sibs::EMPTY,
                "after",
                &attrs,
            ) {
                children.push(Node::Element(el));
            }
            // Колонка значит что-то ТОЛЬКО внутри таблицы или группы
            // колонок. У любого другого родителя она исчезает ровно так же,
            // как исчезала до сих пор: `empty-cells-applies-to-012` ставит
            // `display: table-column` с красным фоном ВНУТРИ ряда и требует
            // «no red». Без этой отсечки такой узел уехал бы в анонимную
            // ячейку и покрасился.
            let holds_columns = matches!(tag.as_str(), "table" | "colgroup")
                || style.col_role == Some(1)
                || matches!(
                    style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            if !holds_columns {
                children.retain(|n| !matches!(n, Node::Element(c) if c.style.col_role.is_some()));
            }
            // Область счётчика НЕ закрывается на выходе из элемента: по
            // §12.4.1 она включает элемент, его потомков И СЛЕДУЮЩИХ СЕСТЁР.
            // Ровно это делает ленивая чистка `remove_stale` — она держит
            // запись, пока обход не вышел за РОДИТЕЛЯ создателя. Жадное
            // снятие здесь её опережало, и `counter-reset` на спане умирал
            // вместе с ним (`content-counter-008`: после `XLIX` шло `XIII`
            // вместо `L`).
            if box_level {
                counters.leave();
            }

            *counter += 1;
            // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
            // уже нет, а интерполировать нужно готовые стили, а не текст.
            let anim = style.animation.as_ref().and_then(|a| {
                let track = frames.get(&a.name)?;
                let resolved: Vec<(f32, Computed)> = track
                    .iter()
                    .map(|(at, decls)| {
                        let mut c = style.clone();
                        c.apply_decls_with_vars(decls, vars);
                        (*at, c)
                    })
                    .collect();
                (resolved.len() >= 2).then_some(resolved)
            });
            // `dir="auto"` — сторону задаёт ПЕРВЫЙ СИЛЬНЫЙ знак содержимого
            // (HTML §3.2.6.4). Раньше здесь не ставилось ничего в расчёте на
            // разбор двунаправленности, но он берёт сторону абзаца, а не
            // куска: строка `1;234;56א;` внутри `<span dir=auto>` выходила
            // слева направо (`empty-span-001`).
            if attrs
                .iter()
                .any(|(k, v)| k == "dir" && v.eq_ignore_ascii_case("auto"))
            {
                // `dir="auto"` в HTML — это `unicode-bidi: plaintext`: сторона
                // решается для КАЖДОГО абзаца между жёсткими разрывами, а не
                // для элемента целиком (`text-align-end-016`).
                style.bidi_plaintext = Some(true);
                if style.rtl.is_none()
                    && let Some(rtl) = first_strong(&children)
                {
                    style.rtl = Some(rtl);
                }
            }
            out.push(Node::Element(Element {
                list_item,
                node_id: *counter,
                anim,
                inline: INLINE_TAGS.contains(&tag.as_str())
                    || !BLOCK_TAGS.contains(&tag.as_str()),
                tag,
                style,
                hover,
                first_letter,
                first_line,
                children,
                attrs,
            }));
        }
        _ => walk_children(
            handle, rules, vars, frames, counter, counters, path, preserve, out,
        ),
    }
}

/// Применить `counter-reset`/`counter-increment`/`counter-set` узла.
///
/// Порядок именно такой (css-lists-3 §5): сперва создаются счётчики, затем
/// накапливаются увеличения, затем присваиваются значения. Имена, которые
/// узел СБРОСИЛ, возвращаются: на выходе из него область надо закрыть.
fn apply_counter_decls(
    style: &Computed,
    counters: &mut crate::counters::Counters,
    tag: &str,
    attrs: &[(String, String)],
    item_flag: &mut bool,
    reversed_start: &dyn Fn(&str, &mut crate::counters::Counters) -> i32,
) {
    let num_attr = |key: &str| -> Option<i32> {
        attrs
            .iter()
            .find(|(k, _)| k == key)
            .and_then(|(_, v)| v.trim().parse().ok())
    };
    // Списочный контейнер заводит счётчик `list-item` для своих пунктов:
    // у нумерованного отсчёт начинается с `start` (css-lists-3 §ua-stylesheet
    // задаёт это правилом `ol[start] { counter-reset: list-item calc(attr(start) - 1) }`).
    // Правило таблицы агента `ol, ul, menu, dir { counter-reset: list-item }`
    // живёт в общем каскаде: авторский `counter-reset` на том же узле его
    // ЗАМЕНЯЕТ целиком, а не дополняет.
    let reversed_list = tag == "ol" && attrs.iter().any(|(k, _)| k == "reversed");
    if matches!(tag, "ol" | "ul" | "menu" | "dir") && style.counter_reset.is_none() {
        // У обратного списка отсчёт идёт вниз и начинается на единицу ВЫШЕ
        // названного, у обычного — на единицу ниже (§ua-stylesheet).
        let start = match (tag, num_attr("start")) {
            ("ol", Some(v)) if reversed_list => v + 1,
            ("ol", Some(v)) => v - 1,
            // У обратного списка без `start` отсчёт начинается с числа его
            // пунктов.
            ("ol", None) if reversed_list => reversed_start("list-item", counters),
            _ => 0,
        };
        counters.reset_flagged("list-item", start, reversed_list);
    }
    // Пункт списка увеличивает `list-item` сам, если этого не сказано явно
    // (css-lists-3 §list-item-counter). Порядок строгий: явное увеличение,
    // затем неявное, затем присваивание — иначе `<li value>` считался бы
    // от уже сдвинутого значения.
    let is_item = tag == "li" || style.display == Some(Display::ListItem);
    *item_flag = is_item;
    let explicit_item = style
        .counter_increment
        .as_deref()
        .is_some_and(|t| t.split_whitespace().any(|w| w == "list-item"));
    for (decl, kind) in [
        (&style.counter_reset, 0u8),
        (&style.counter_increment, 1),
        (&style.counter_set, 2),
    ] {
        if kind == 2 && is_item && !explicit_item {
            // Пункт обратного списка считает ВНИЗ (css-lists-3
            // §list-item-counter).
            let step = if counters.is_reversed("list-item") {
                -1
            } else {
                1
            };
            counters.update("list-item", step, false);
        }
        let Some(text) = decl else { continue };
        // `reversed( имя )` — одна запись, а не три слова.
        let text = squeeze_parens(text);
        let mut it = text.split_whitespace().peekable();
        while let Some(name) = it.next() {
            // `none` — ключевое слово «ничего не делать», а не имя счётчика.
            if name.eq_ignore_ascii_case("none") {
                continue;
            }
            // Обратный счётчик: имя в скобках, значение по умолчанию узнаётся
            // предварительным обходом области (пока — ноль).
            let (name, reversed) = match name
                .strip_prefix("reversed(")
                .and_then(|r| r.strip_suffix(')'))
            {
                Some(inner) if kind == 0 && !inner.is_empty() => (inner, true),
                Some(_) => continue,
                None => (name, false),
            };
            let value = match it.peek().and_then(|n| n.parse::<i32>().ok()) {
                Some(v) => {
                    it.next();
                    v
                }
                None if reversed => reversed_start(name, counters),
                None => match kind {
                    0 | 2 => 0,
                    _ => 1,
                },
            };
            match kind {
                0 => {
                    counters.reset_flagged(name, value, reversed);
                }
                1 => counters.update(name, value, false),
                _ => counters.update(name, value, true),
            }
        }
    }
    // `<li value>` задаёт номер пункта прямо (css-lists-3 §ua-stylesheet:
    // `li[value] { counter-set: list-item attr(value) }`).
    if is_item && let Some(v) = num_attr("value") {
        counters.update("list-item", v, true);
    }
}

/// Коробка псевдоэлемента `::before`/`::after`, если правила её создают.
///
/// В CSS это настоящий потомок с собственным стилем; так его и собираем —
/// обычным инлайновым элементом с текстовым содержимым. `attr(имя)`
/// подставляется значением атрибута хозяина.
fn pseudo_box(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    which: &str,
    attrs: &[(String, String)],
) -> Option<Element> {
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some(which))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    // Нет содержимого или коробки — нет и псевдоэлемента: его директивы
    // счётчиков тогда не действуют вовсе (у него нет объекта раскладки).
    let list = style.content.clone()?;
    if style.display == Some(Display::None) {
        return None;
    }
    // Псевдоэлемент — настоящий брат содержимого хозяина: у него свой
    // уровень пути, свои директивы и своя область видимости.
    counters.enter_pseudo(which == "before");
    // У псевдоэлемента-создателя предварительного обхода нет: своей области
    // в дереве коробок он не открывает, и таких пар в наборе не встречается.
    apply_counter_decls(
        &style,
        counters,
        "",
        &[],
        &mut false,
        &|_, _| 0,
    );
    // Составляющие склеиваются по порядку (css-content-3 §2): строки как
    // есть, счётчики — знаками своего стиля, `attr()` — значением атрибута.
    let mut text = String::new();
    for item in &list {
        match item {
            crate::computed::ContentItem::Str(sv) => text.push_str(sv),
            crate::computed::ContentItem::Counter(name, style_name) => {
                let value = counters.value_of(name);
                text.push_str(&crate::counter_style::repr(value, style_name));
            }
            crate::computed::ContentItem::Counters(name, sep, style_name) => {
                // Вся цепочка области — от внешнего счётчика к внутреннему,
                // склеенная разделителем (css-lists-3 §counters).
                let chain: Vec<String> = counters
                    .chain_of(name)
                    .into_iter()
                    .map(|v| crate::counter_style::repr(v, style_name))
                    .collect();
                text.push_str(&chain.join(sep));
            }
            crate::computed::ContentItem::Attr(name) => {
                if let Some((_, v)) = attrs.iter().find(|(k, _)| k == name) {
                    text.push_str(v);
                }
            }
        }
    }
    counters.leave();
    Some(Element {
        list_item: None,
        // Псевдоэлемент своей анимации не несёт: правило `::before` задаёт
        // содержимое, а не движение.
        node_id: 0,
        anim: None,
        inline: !matches!(
            style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ),
        tag: format!("::{which}"),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![Node::Text(text)],
        attrs: vec![],
    })
}

/// Сопоставление селектора с узлом и его цепочкой предков.
///
/// `sibs` — предыдущие соседи-элементы узла в порядке разметки: по ним
/// решаются соседние комбинаторы `+` и `~`.
pub(crate) fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    if let Some(pseudo) = &sel.pseudo {
        // `:not(...)` — отрицание вложенного селектора. Разбирается здесь, а
        // не среди структурных: внутри скобок может стоять тег или класс, а им
        // нужен сам узел, а не только его место среди соседей.
        if let Some(inner) = pseudo
            .strip_prefix("not(")
            .and_then(|rest| rest.strip_suffix(')'))
        {
            let Some(inner) = Selector::parse(inner) else {
                return false;
            };
            if matches(&inner, me, path, sibs) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // Структурный псевдокласс — часть обычного каскада: он зависит только
        // от места узла в дереве. Остальные (`:hover`, `::before`) сюда не
        // попадают: их применяет отдельный слой при отрисовке.
        // `:root` — корень документа, то есть `<html>`. Он не структурный по
        // месту среди соседей, поэтому решается здесь: без него объявления
        // вроде `:root { font: 25px/1 Ahem }` не доезжали НИКУДА, и страница
        // набиралась шрифтом по умолчанию (`text-align-last-015`).
        // Ссылки: `:visited` — адрес уже в истории. Свою страницу браузер в
        // историю кладёт по определению, поэтому пустой `href` и якорь на
        // себя — посещённые; остальное для нас непосещённое.
        if pseudo == "link" || pseudo == "visited" {
            let Some(href) = &me.href else { return false };
            let visited = href.is_empty() || href.starts_with('#');
            if (pseudo == "visited") != visited {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        // `:dir(rtl|ltr)` — направление узла: свой атрибут `dir`, иначе
        // ближайшего предка с ним; по умолчанию письмо слева направо
        // (селекторы-4 §direction-pseudo).
        if let Some(want) = pseudo
            .strip_prefix("dir(")
            .and_then(|r| r.strip_suffix(')'))
        {
            let rtl = me
                .dir
                .or_else(|| path.iter().rev().find_map(|a| a.dir))
                .unwrap_or(false);
            if want.trim().eq_ignore_ascii_case("rtl") != rtl {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if pseudo == "root" {
            if me.tag != "html" {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(want) = pseudo
            .strip_prefix("lang(")
            .and_then(|r| r.strip_suffix(')'))
        {
            if !lang_matches(want, me, path) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(arg) = pseudo
            .strip_prefix("has(")
            .and_then(|r| r.strip_suffix(')'))
        {
            if !me.has_marks.contains(&has_id(arg)) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        if let Some(ok) = nth_of_holds(pseudo, me, path, sibs) {
            if !ok {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
        let Some(ok) = structural(pseudo, me.spot) else {
            return false;
        };
        if !ok {
            return false;
        }
    }
    matches_ignoring_pseudo(sel, me, path, sibs)
}

/// Псевдокласс — of-форма `:nth-child(… of S)`?
fn nth_of_form(pseudo: &str) -> bool {
    let Some((name, arg)) = pseudo.split_once('(') else {
        return false;
    };
    matches!(name, "nth-child" | "nth-last-child")
        && arg
            .strip_suffix(')')
            .is_some_and(|a| crate::css::nth_of_parts(a).is_some())
}

/// `:nth-child(An+B of S)` / `:nth-last-child(An+B of S)` (селекторы-4):
/// узел обязан сам совпасть с S, а номер считается только среди совпавших
/// братьев — с начала либо с конца. `None` — псевдокласс не of-формы.
fn nth_of_holds(pseudo: &str, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> Option<bool> {
    let (name, arg) = pseudo.split_once('(')?;
    let backwards = match name {
        "nth-child" => false,
        "nth-last-child" => true,
        _ => return None,
    };
    let arg = arg.strip_suffix(')')?;
    let (anb, list) = crate::css::nth_of_parts(arg)?;
    let hit = |a: &Ancestor, s: Sibs| list.iter().any(|sel| matches(sel, a, path, s));
    if !sibs.is_elem || list.is_empty() || !hit(me, sibs) {
        return Some(false);
    }
    let (peers, base) = if backwards {
        (sibs.next(), sibs.pos + 1)
    } else {
        (sibs.prev(), 0)
    };
    let idx = 1 + peers
        .iter()
        .enumerate()
        .filter(|(i, a)| hit(a, sibs.at(base + i)))
        .count();
    Some(nth_matches(&anb, idx))
}

/// `:lang(x)` — язык узла: свой атрибут `lang`, иначе ближайшего предка.
/// Совпадение — точное или по префиксу до дефиса, ASCII-регистронезависимо
/// (селекторы-4 §lang-pseudo; `fi` не совпадает с `fil`).
fn lang_matches(want: &str, me: &Ancestor, path: &[Ancestor]) -> bool {
    let lang_of = |a: &Ancestor| {
        a.attrs
            .iter()
            .find(|(k, _)| k.eq_ignore_ascii_case("lang") || k.eq_ignore_ascii_case("xml:lang"))
            .map(|(_, v)| v.clone())
    };
    let Some(lang) = lang_of(me).or_else(|| path.iter().rev().find_map(lang_of)) else {
        return false;
    };
    let want = want
        .trim()
        .trim_matches(|c| c == '"' || c == char::from(39));
    if want.is_empty() || want == "*" {
        return !lang.is_empty();
    }
    lang.eq_ignore_ascii_case(want)
        || (lang.len() > want.len()
            && lang.as_bytes()[want.len()] == b'-'
            && lang[..want.len()].eq_ignore_ascii_case(want))
}

/// Выполняется ли ОДИН псевдокласс на узле — для дополнительных
/// псевдоклассов компаунда (основной решает `matches`, слои — отбор
/// по имени). Неизвестный или слойный (`:hover`) здесь считается
/// НЕвыполненным: базовый каскад такое правило не применяет.
fn pseudo_holds(pseudo: &str, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    if let Some(inner) = pseudo
        .strip_prefix("not(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return Selector::parse(inner).is_some_and(|inner| !matches(&inner, me, path, sibs));
    }
    if pseudo == "link" || pseudo == "visited" {
        let Some(href) = &me.href else { return false };
        let visited = href.is_empty() || href.starts_with('#');
        return (pseudo == "visited") == visited;
    }
    if let Some(want) = pseudo
        .strip_prefix("dir(")
        .and_then(|r| r.strip_suffix(')'))
    {
        let rtl = me
            .dir
            .or_else(|| path.iter().rev().find_map(|a| a.dir))
            .unwrap_or(false);
        return want.trim().eq_ignore_ascii_case("rtl") == rtl;
    }
    if pseudo == "root" {
        return me.tag == "html";
    }
    if let Some(want) = pseudo
        .strip_prefix("lang(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return lang_matches(want, me, path);
    }
    if let Some(arg) = pseudo
        .strip_prefix("has(")
        .and_then(|r| r.strip_suffix(')'))
    {
        return me.has_marks.contains(&has_id(arg));
    }
    if let Some(ok) = nth_of_holds(pseudo, me, path, sibs) {
        return ok;
    }
    structural(pseudo, me.spot).unwrap_or(false)
}

/// Структурные псевдоклассы: место узла среди соседей.
///
/// `None` — псевдокласс не структурный, решение принимает вызывающий.
fn structural(pseudo: &str, spot: Spot) -> Option<bool> {
    let (name, arg) = match pseudo.split_once('(') {
        Some((n, rest)) => (n, rest.trim_end_matches(')').trim()),
        None => (pseudo, ""),
    };
    let (index, total) = match name {
        "first-child" | "last-child" | "only-child" | "nth-child" | "nth-last-child" => {
            (spot.index, spot.total)
        }
        "first-of-type" | "last-of-type" | "only-of-type" | "nth-of-type" | "nth-last-of-type" => {
            (spot.of_type, spot.of_type_total)
        }
        _ => return None,
    };
    // Узел без места — не элемент; таким структурные правила не адресуются.
    if index == 0 {
        return Some(false);
    }
    Some(match name {
        "first-child" | "first-of-type" => index == 1,
        "last-child" | "last-of-type" => index == total,
        "only-child" | "only-of-type" => total == 1,
        "nth-child" | "nth-of-type" => nth_matches(arg, index),
        "nth-last-child" | "nth-last-of-type" => nth_matches(arg, total + 1 - index),
        _ => false,
    })
}

/// Запись `an+b` из `:nth-child()`: подходит ли номер.
fn nth_matches(arg: &str, index: usize) -> bool {
    let arg = arg.trim().to_ascii_lowercase();
    let (a, b) = match arg.as_str() {
        "odd" => (2i64, 1i64),
        "even" => (2, 0),
        _ => match arg.split_once('n') {
            None => match arg.parse::<i64>() {
                Ok(b) => (0, b),
                Err(_) => return false,
            },
            Some((head, tail)) => {
                // Пробел между множителем и `n` запрещён (css-syntax
                // §the-anb-type): `1 n` — не An+B.
                if head.ends_with(char::is_whitespace) {
                    return false;
                }
                let a = match head {
                    "" | "+" => 1,
                    "-" => -1,
                    other => match other.parse::<i64>() {
                        Ok(v) => v,
                        Err(_) => return false,
                    },
                };
                // Сдвиг после `n` обязан нести явный знак: `2n 1` — не An+B,
                // пробелы допустимы только вокруг самого знака.
                let tail = tail.trim();
                let b = if tail.is_empty() {
                    0
                } else {
                    let (sign, num) = match tail.strip_prefix('+') {
                        Some(rest) => (1i64, rest),
                        None => match tail.strip_prefix('-') {
                            Some(rest) => (-1, rest),
                            None => return false,
                        },
                    };
                    let num = num.trim_start();
                    // Второй знак у сдвига (`2n--1`) — не число.
                    if !num.bytes().all(|c| c.is_ascii_digit()) {
                        return false;
                    }
                    match num.parse::<i64>() {
                        Ok(v) => sign * v,
                        Err(_) => return false,
                    }
                };
                (a, b)
            }
        },
    };
    let index = index as i64;
    if a == 0 {
        return index == b;
    }
    let diff = index - b;
    diff % a == 0 && diff / a >= 0
}

/// То же сопоставление, но без отсева по псевдоклассу — для слоя наведения.
pub(crate) fn matches_ignoring_pseudo(
    sel: &Selector,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
) -> bool {
    if !matches_compound(sel, me) {
        return false;
    }
    // Дополнительные псевдоклассы компаунда (`li:first-child:last-child`)
    // обязаны выполниться ВСЕ; раньше выживал только последний.
    if !sel.also.iter().all(|p| pseudo_holds(p, me, path, sibs)) {
        return false;
    }
    // Соседний комбинатор: `+` — ровно предыдущий сосед-элемент, `~` — любой
    // раньше. Сосед проверяется ПОЛНЫМ сопоставлением со своими соседями
    // слева и тем же путём предков (соседи его делят).
    if let Some(prev) = &sel.prev {
        let (prev_sel, adjacent) = (&prev.0, prev.1);
        let prev = sibs.prev();
        let hit = |i: usize| matches(prev_sel, &prev[i], path, sibs.at(i));
        let found = if adjacent {
            !prev.is_empty() && hit(prev.len() - 1)
        } else {
            (0..prev.len()).rev().any(hit)
        };
        if !found {
            return false;
        }
    }
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    // Сосед ПРЕДКА: его соседей здесь уже не восстановить — такое правило
    // честно не совпадает, чем совпадать наугад.
    if parent_sel.prev.is_some() {
        return false;
    }
    if direct {
        return path.last().is_some_and(|p| {
            matches_compound(parent_sel, p) && matches_chain(parent_sel, path, path.len() - 1)
        });
    }
    (0..path.len())
        .rev()
        .any(|i| matches_compound(parent_sel, &path[i]) && matches_chain(parent_sel, path, i))
}

/// Продолжение цепочки вверх для `.a .b .c`.
fn matches_chain(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if parent_sel.prev.is_some() {
        return false;
    }
    if direct {
        return at > 0
            && matches_compound(parent_sel, &path[at - 1])
            && matches_chain(parent_sel, path, at - 1);
    }
    (0..at)
        .rev()
        .any(|i| matches_compound(parent_sel, &path[i]) && matches_chain(parent_sel, path, i))
}

fn matches_compound(sel: &Selector, node: &Ancestor) -> bool {
    if let Some(t) = &sel.tag
        && t != &node.tag
    {
        return false;
    }
    if let Some(id) = &sel.id
        && node.id.as_deref() != Some(id.as_str())
    {
        return false;
    }
    // `:has()` НЕ-предметного компаунда (`div:has(.x) p`): отметка лежит
    // на самом узле - раньше псевдокласс здесь пропускался, и правило
    // красило все `div p` подряд.
    if let Some(p) = &sel.pseudo
        && let Some(arg) = p.strip_prefix("has(").and_then(|r| r.strip_suffix(')'))
        && !node.has_marks.contains(&has_id(arg))
    {
        return false;
    }
    // Структурный псевдокласс НЕ-предметного компаунда
    // (`td:nth-child(2) div`): место предка среди братьев известно из
    // `spot` — без проверки любой `td` подходил под любой номер, и
    // последнее правило перекрашивало все колонки
    // (logical-physical-mapping-001). Нестуктурные (`:hover`) здесь
    // по-прежнему пропускаются, of-форма — тоже: её решает предметный
    // путь по списку братьев, а по одному `spot` она не считается.
    if let Some(p) = &sel.pseudo
        && !nth_of_form(p)
        && let Some(ok) = structural(p, node.spot)
        && !ok
    {
        return false;
    }
    if !sel.attrs.iter().all(|a| {
        a.matches(
            node.attrs
                .iter()
                .find(|(k, _)| k.eq_ignore_ascii_case(&a.name))
                .map(|(_, v)| v.as_str()),
        )
    }) {
        return false;
    }
    sel.classes.iter().all(|c| node.classes.contains(c))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Цвета детей по порядку — короткая запись для проверок каскада.
    fn child_colors(html: &str) -> Vec<Option<crate::value::Color>> {
        fn find<'a>(nodes: &'a [Node], id: &str) -> Option<&'a Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.attr("id") == Some(id) {
                        return Some(e);
                    }
                    if let Some(found) = find(&e.children, id) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let nodes = parse(html, "");
        find(&nodes, "box")
            .map(|e| {
                e.children
                    .iter()
                    .filter_map(|n| match n {
                        Node::Element(c) => Some(c.style.color),
                        Node::Text(_) => None,
                    })
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Тексты псевдоэлементов документа в порядке обхода.
    fn pseudo_texts(html: &str) -> Vec<String> {
        fn walk(nodes: &[Node], out: &mut Vec<String>) {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag.starts_with("::") {
                        let text = e.children.iter().find_map(|c| match c {
                            Node::Text(t) => Some(t.clone()),
                            _ => None,
                        });
                        out.push(text.unwrap_or_default());
                    }
                    walk(&e.children, out);
                }
            }
        }
        let mut out = vec![];
        walk(&parse(html, ""), &mut out);
        out
    }

    #[test]
    fn list_item_counter_is_implicit() {
        // Пункт списка двигает `list-item` сам; `start` задаёт начало,
        // `value` — номер конкретного пункта (css-lists-3 §ua-stylesheet).
        let texts = pseudo_texts(
            "<style>li::after { content: counter(list-item) }</style>             <ol start=\"5\"><li></li><li value=\"9\"></li><li></li></ol>",
        );
        assert_eq!(texts, vec!["5", "9", "10"]);
        // Явное упоминание `list-item` отменяет неявное увеличение.
        let texts = pseudo_texts(
            "<style>li { counter-increment: list-item 3 } li::after { content: counter(list-item) }</style>             <ol><li></li><li></li></ol>",
        );
        assert_eq!(texts, vec!["3", "6"]);
    }

    #[test]
    fn has_relational_pseudo() {
        let red = crate::value::Color::parse("red");
        let green = crate::value::Color::parse("green");
        // Предметная позиция: якорь с потомком-предметом.
        let colors = child_colors(
            "<style>div { color: red } div:has(span) { color: green }</style>             <div id=\"box\"><div><span></span></div><div><b></b></div></div>",
        );
        assert_eq!(colors, vec![green, red]);
        // Ведущий `>`: только прямой ребёнок; `+`: следующий брат.
        let colors = child_colors(
            "<style>p { color: red } p:has(> em) { color: green }             p:has(+ p) { background: yellow }</style>             <div id=\"box\"><p><i><em>x</em></i></p><p><em>y</em></p></div>",
        );
        assert_eq!(colors, vec![red, green]);
        // Непредметная позиция: `div:has(.x) b` красит b только в div с .x.
        let colors = child_colors(
            "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i class=\"x\"></i><b></b></div></div>",
        );
        assert_eq!(colors, vec![None, green]);
        let colors = child_colors(
            "<style>b { color: red } div:has(.x) b { color: green }</style>             <div><div id=\"box\"><i></i><b></b></div></div>",
        );
        assert_eq!(colors, vec![None, red]);
    }

    #[test]
    fn nth_child_of_selector_list() {
        let red = crate::value::Color::parse("red");
        let green = crate::value::Color::parse("green");
        // Индекс считается среди совпавших с S братьев, а не среди всех:
        // второй `.a` — это :nth-child(2 of .a), хотя среди детей он третий.
        let colors = child_colors(
            "<style>p { color: red } p:nth-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
        );
        assert_eq!(colors, vec![red, red, green]);
        // nth-last-child(of S): совпавшие считаются с конца.
        let colors = child_colors(
            "<style>p { color: red } p:nth-last-child(2 of .a) { color: green }</style>             <div id=\"box\"><p class=\"a\"></p><p></p><p class=\"a\"></p></div>",
        );
        assert_eq!(colors, vec![green, red, red]);
    }

    #[test]
    fn custom_properties_cascade_and_inherit() {
        let red = crate::value::Color::parse("red");
        let blue = crate::value::Color::parse("blue");
        // Переключение темы классом: у потомка внутри `.dark` своё значение
        // переменной, у остальных — корневое. Пока переменные собирались в
        // один плоский словарь на документ, последнее объявление красило ВЕСЬ
        // документ, и тема классом не переключалась в принципе.
        let colors = child_colors(
            "<style>:root { --c: red } .dark { --c: blue } i { color: var(--c) }</style>             <div id=box><i></i><i class=dark></i></div>",
        );
        assert_eq!(colors, vec![red, blue], "получено {colors:?}");
    }

    #[test]
    fn nth_child_selects_by_position() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2) { color: red }</style>\
             <div id=box><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red, None], "получено {colors:?}");
    }

    #[test]
    fn nth_child_understands_an_plus_b() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:nth-child(2n+1) { color: red }</style>\
             <div id=box><i></i><i></i><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![red, None, red, None], "получено {colors:?}");
    }

    #[test]
    fn last_child_counts_from_the_end() {
        let red = crate::value::Color::parse("red");
        let colors = child_colors(
            "<style>i:last-child { color: red }</style>\
             <div id=box><i></i><i></i></div>",
        );
        assert_eq!(colors, vec![None, red], "получено {colors:?}");
    }

    #[test]
    fn of_type_counts_only_the_same_tag() {
        let red = crate::value::Color::parse("red");
        // Второй `<i>` — четвёртый ребёнок, но второй своего тега.
        let colors = child_colors(
            "<style>i:nth-of-type(2) { color: red }</style>\
             <div id=box><b></b><i></i><b></b><i></i></div>",
        );
        assert_eq!(colors, vec![None, None, None, red], "получено {colors:?}");
    }

    fn first_element(nodes: &[Node]) -> &Element {
        fn find(nodes: &[Node]) -> Option<&Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "body" || e.tag == "html" {
                        if let Some(inner) = find(&e.children) {
                            return Some(inner);
                        }
                        continue;
                    }
                    return Some(e);
                }
            }
            None
        }
        find(nodes).expect("нет элементов")
    }

    #[test]
    fn tag_defaults_apply() {
        let nodes = parse("<h1>Заголовок</h1>", "");
        let h1 = first_element(&nodes);
        assert_eq!(h1.tag, "h1");
        assert_eq!(h1.style.font_weight, Some(700));
    }

    #[test]
    fn inline_style_beats_stylesheet() {
        let nodes = parse(
            r#"<style>.c { color: red }</style><div class="c" style="color: #00ff00">x</div>"#,
            "",
        );
        let div = first_element(&nodes);
        assert_eq!(div.style.color.map(|c| c.g), Some(1.0));
    }

    #[test]
    fn descendant_selector_needs_the_ancestor() {
        let html = r#"<style>.card .t { color: #0000ff }</style>
            <div class="card"><span class="t">внутри</span></div><span class="t">снаружи</span>"#;
        let nodes = parse(html, "");
        let mut found = vec![];
        collect_spans(&nodes, &mut found);
        assert_eq!(found.len(), 2);
        assert_eq!(
            found[0].style.color.map(|c| c.b),
            Some(1.0),
            "внутри карточки — покрашен"
        );
        assert_eq!(
            found[1].style.color, None,
            "снаружи — правило не применяется"
        );
    }

    fn collect_spans<'a>(nodes: &'a [Node], out: &mut Vec<&'a Element>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "span" {
                    out.push(e);
                }
                collect_spans(&e.children, out);
            }
        }
    }

    #[test]
    fn css_variables_are_substituted() {
        // На переменных построены все современные темы: без подстановки такое
        // объявление терялось молча.
        let nodes = parse(":root { --brand: #00ff00 } .b { color: var(--brand) }", "");
        let _ = &nodes;
        let nodes = parse(
            "<style>:root { --brand: #00ff00 } .b { color: var(--brand) }</style>             <div class=\"b\">текст</div>",
            "",
        );
        assert_eq!(first_element(&nodes).style.color.map(|c| c.g), Some(1.0));
    }

    #[test]
    fn variable_fallback_is_used_when_undefined() {
        let nodes = parse(
            "<style>.b { color: var(--missing, #0000ff) }</style><div class=\"b\">t</div>",
            "",
        );
        assert_eq!(first_element(&nodes).style.color.map(|c| c.b), Some(1.0));
    }

    #[test]
    fn hover_rules_form_a_separate_layer() {
        let nodes = parse(
            "<style>.b { color: #ffffff } .b:hover { color: #ff0000 }</style>             <div class=\"b\">кнопка</div>",
            "",
        );
        let d = first_element(&nodes);
        assert_eq!(d.style.color.map(|c| c.r), Some(1.0), "базовый цвет белый");
        assert_eq!(
            d.style.color.map(|c| c.g),
            Some(1.0),
            "и не покрашен наведением"
        );
        let hover = d.hover.as_ref().expect("слой наведения собран");
        assert_eq!(hover.color.map(|c| c.g), Some(0.0), "в наведении — красный");
    }

    #[test]
    fn no_hover_rules_means_no_layer() {
        let nodes = parse("<div class=\"b\">без наведения</div>", "");
        assert!(first_element(&nodes).hover.is_none());
    }

    #[test]
    fn script_and_style_content_is_dropped() {
        let nodes = parse(
            "<script>alert(1)</script><style>.a{}</style><p>текст</p>",
            "",
        );
        let p = first_element(&nodes);
        assert_eq!(p.tag, "p");
        assert!(matches!(p.children.first(), Some(Node::Text(t)) if t == "текст"));
    }

    #[test]
    fn display_none_removes_the_subtree() {
        let nodes = parse(
            r#"<div style="display:none"><p>невидимо</p></div><p>видно</p>"#,
            "",
        );
        let first = first_element(&nodes);
        assert_eq!(first.tag, "p");
        assert!(matches!(first.children.first(), Some(Node::Text(t)) if t == "видно"));
    }

    #[test]
    fn unclosed_tags_are_recovered_by_the_parser() {
        let nodes = parse("<div><p>раз<p>два</div>", "");
        let div = first_element(&nodes);
        let ps = div
            .children
            .iter()
            .filter(|n| matches!(n, Node::Element(e) if e.tag == "p"))
            .count();
        assert_eq!(ps, 2, "html5ever закрывает <p> сам");
    }
}

#[cfg(test)]
mod presentational_tests {
    use super::*;

    /// `<img width=100>` — представленческая подсказка, и без неё картинка
    /// набирается по своему пикселю вместо заявленного размера.
    #[test]
    fn image_size_attributes_reach_the_style() {
        let nodes = parse(r#"<img src="x.png" width="100" height="40">"#, "");
        fn find<'a>(nodes: &'a [Node]) -> Option<&'a Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "img" {
                        return Some(e);
                    }
                    if let Some(found) = find(&e.children) {
                        return Some(found);
                    }
                }
            }
            None
        }
        let img = find(&nodes).expect("картинка в дереве");
        assert_eq!(img.style.width, Some(Len::Px(100.0)));
        assert_eq!(img.style.height, Some(Len::Px(40.0)));
    }
}

#[cfg(test)]
mod nth_child_tests {
    use super::*;

    fn spans(nodes: &[Node], out: &mut Vec<Computed>) {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "span" {
                    out.push(e.style.clone());
                }
                spans(&e.children, out);
            }
        }
    }

    /// `:nth-child(1)` адресует ПЕРВОГО ребёнка — на нём держатся эталоны
    /// целого набора тестов гибкой раскладки.
    #[test]
    fn first_child_is_addressable() {
        let html = r#"<style>
            span { background: white }
            span:nth-child(1) { background: yellow }
            span:first-child { color: red }
        </style><div style="display:flex"><span>a</span><span>b</span><span>c</span></div>"#;
        let mut found = vec![];
        spans(&parse(html, ""), &mut found);
        assert_eq!(found.len(), 3, "три куска");
        let first = &found[0];
        assert_ne!(
            first.background, found[1].background,
            "фон первого куска обязан отличаться от второго"
        );
        assert!(first.color.is_some(), ":first-child тоже обязан сработать");
    }
}

#[cfg(test)]
mod white_space_tests {
    use super::*;

    /// `white-space: break-spaces` обязан доехать до правил переноса: от него
    /// зависит, считается ли хвостовой пробел в ширину строки.
    #[test]
    fn break_spaces_reaches_the_wrap_rules() {
        let nodes = parse(r#"<div style="white-space: break-spaces">X XX X</div>"#, "");
        fn find(nodes: &[Node]) -> Option<&Element> {
            for n in nodes {
                if let Node::Element(e) = n {
                    if e.tag == "div" {
                        return Some(e);
                    }
                    if let Some(f) = find(&e.children) {
                        return Some(f);
                    }
                }
            }
            None
        }
        let div = find(&nodes).expect("блок в дереве");
        assert_eq!(div.style.break_after_spaces, Some(true), "разбор");
        let wrap = crate::lines::rules(&div.style).expect("правила");
        assert!(wrap.break_spaces, "правила переноса");
        assert!(wrap.keep_spaces, "сохранение пробелов");
    }
}

/// Первый СИЛЬНЫЙ знак содержимого: `true` — справа налево, `None` — сильных
/// знаков нет вовсе и сторона остаётся унаследованной.
fn first_strong(nodes: &[Node]) -> Option<bool> {
    use unicode_bidi::BidiClass::*;
    for node in nodes {
        match node {
            Node::Text(t) => {
                for ch in t.chars() {
                    match unicode_bidi::bidi_class(ch) {
                        L => return Some(false),
                        R | AL => return Some(true),
                        _ => {}
                    }
                }
            }
            Node::Element(e) => {
                // Внутри куска со СВОЕЙ стороной искать нечего: он сам себе
                // абзац для этого правила.
                if e.style.rtl.is_some() {
                    continue;
                }
                if let Some(found) = first_strong(&e.children) {
                    return Some(found);
                }
            }
        }
    }
    None
}

/// Сжать пробелы внутри скобок: `reversed( x )` — одна запись значения,
/// а разбор идёт по словам.
fn squeeze_parens(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth = 0i32;
    for ch in text.chars() {
        match ch {
            '(' => {
                depth += 1;
                out.push(ch);
            }
            ')' => {
                depth -= 1;
                out.push(ch);
            }
            c if c.is_whitespace() && depth > 0 => {}
            c => out.push(c),
        }
    }
    out
}
