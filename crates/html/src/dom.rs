//! HTML → дерево узлов с вычисленным стилем.
//!
//! Разбор отдан `html5ever` — тому же парсеру, что стоит в браузерах на Rust:
//! писать свой означало бы повторять правила восстановления после ошибок
//! (незакрытые теги, неявные `<tbody>`), которые модель нарушает регулярно.
//! Наша часть — превратить его дерево в своё: с каскадом и без узлов, которые
//! ничего не рисуют.

mod subgrid_axes;
mod grid_static_position;
mod replaced_display;
#[path = "dom_display_inheritance.rs"]
mod display_inheritance;
use display_inheritance::resolve_display_inherit;
#[path = "dom_initial_pseudos.rs"]
mod initial_pseudos;
#[path = "dom_containment.rs"]
mod containment;
#[path = "dom_language.rs"]
mod language;
#[path = "dom_counter_decls.rs"]
mod counter_decls;
mod presentational_hints;
pub(crate) use counter_decls::{
    apply_counter_decls, apply_value_hint, counter_snapshot, inherit_counter_decls,
};
#[path = "dom_content.rs"]
mod content;
pub(crate) use content::{content_text, host_content, resolve_content_attributes};

use crate::computed::{Computed, Display, Position};
use crate::css::{
    Decls, Keyframes, Media, Rule, Selector, parse_decls, parse_keyframes, parse_stylesheet_media,
};
use crate::value::Len;
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;
use std::rc::Rc;

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
    // Все руби-теги — строчные (css-ruby-1 §2.1.1: контейнер и внутренние
    // коробки руби неатомарны и строчного уровня).
    "q", "rb", "rbc", "rp", "rt", "rtc", "ruby", "s", "samp", "small", "span", "strong", "sub",
    "sup", "time", "u", "var", "wbr", "img", "svg",
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
head, title, meta, link, template { display: none }
    slot { display: contents }
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
    i, em { font-style: italic }
    u { text-decoration: underline }
    s, del { text-decoration: line-through }
    small { font-size: 11px }
    a[href] { color: #8ab4f8; text-decoration: underline }
    code, kbd, samp { font-family: monospace; font-size: 12px }
    pre { font-family: monospace; margin: 6px 0; padding: 8px; overflow-x: auto }
    /* Заранее размеченный текст зазоров `text-autospace` не получает: правка
       ширины ломает выравнивание в столбик, ради которого его и пишут
       (css-text-4 §7, таблица стилей агента). */
    pre, code, kbd, samp, tt, textarea, input { text-autospace: no-autospace }
    ul, ol { margin: 6px 0; padding-inline-start: 18px }
    li { margin: 2px 0 }
    blockquote { margin: 6px 0; padding-left: 10px; border-left: 3px solid #4a4a5a }
    hr { height: 1px; margin: 8px 0; background: #4a4a5a }
    table { margin: 6px 0 }
    th { font-weight: 700; padding: 4px 8px; text-align: left }
    td { padding: 4px 8px }
    button { padding: 4px 10px; border-radius: 4px }
    /* Руби (css-ruby-1, Appendix A.1): скобки `rp` — только для движков без
       руби; аннотация вполовину кегля, одной строкой, без знака акцента.
       Пара правил равносильна спековому `rtc, :not(rtc) > rt { font-size: 50% }`.
       `unicode-bidi: isolate` пока не ставится — мерить отдельно (`ruby-bidi-001`). */
    /* css-content-3 §4.2, HTML §15.3.3: `q` берёт кавычки из `quotes`. */
    q::before { content: open-quote }
    q::after { content: close-quote }
    rp { display: none }
    rb, rt, rtc { white-space: nowrap }
    rt, rtc { font-size: 50%; line-height: 1; text-emphasis: none; text-justify: ruby }
    rtc > rt { font-size: 100% }
    /* Языковые правила A.1: чжуинь (zh-TW) — 30% кегля, у китайского
       аннотация по центру. Без них строка под аннотацию росла на кегль
       50% (`ruby-lang-specific-style-001`). */
    rt:lang(zh-TW), rtc:lang(zh-TW) { font-size: 30% }
    rtc:lang(zh-TW) > rt { font-size: 100% }
    rt:lang(zh), rtc:lang(zh) { ruby-align: center }
    "#
}

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn parse(html: &str, extra_css: &str) -> Vec<Node> {
    parse_media(html, extra_css, Media::default())
}

/// XHTML (`application/xhtml+xml`: `<?xml` или `xmlns` XHTML в шапке) у нас
/// разбирает HTML-парсер, а тот на НЕ-void теге признак самозакрытия
/// игнорирует: `<div class="a"/>` открывал элемент, и всё дальнейшее
/// вкладывалось в него (46 пар css-flexbox `.xhtml`, семьи с
/// `<div/>`-распорками). Такие теги разворачиваются в пару `<tag …></tag>`
/// до разбора; void-элементы и содержимое `svg`/`math` (там парсер
/// самозакрытие понимает) не трогаются.
fn expand_xhtml_self_closing(html: &str) -> std::borrow::Cow<'_, str> {
    let xhtml = content::is_xhtml(html);
    // `<pre>`/`<listing>`/`<textarea>` в XHTML тоже требуют правки (см. ниже),
    // даже если самозакрытых тегов в документе нет.
    let lf_tags = ["<pre", "<listing", "<textarea"].iter().any(|t| html.contains(t));
    if !xhtml
        || (!html.contains("/>") && !lf_tags && !html.contains("<!--") && !html.contains('&'))
    {
        return std::borrow::Cow::Borrowed(html);
    }
    const VOID: &[&str] = &[
        "area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param",
        "source", "track", "wbr", "basefont", "frame", "keygen",
    ];
    let mut out = String::with_capacity(html.len() + 256);
    let mut rest = html;
    let mut foreign = 0usize;
    while let Some(at) = rest.find('<') {
        out.push_str(&rest[..at]);
        let tag = &rest[at..];
        // Раздел CDATA — дословный текст до `]]>` (XML 1.0 §2.7): `<!--`
        // внутри него — не комментарий, а символы таблицы стилей
        // (`sgml-comments-000`: CSS видит CDO и правило за ним).
        if tag.starts_with("<![CDATA[") {
            let end = tag.find("]]>").map(|e| e + 3).unwrap_or(tag.len());
            out.push_str(&tag[..end]);
            rest = &tag[end..];
            continue;
        }
        // Комментарий XML (XML 1.0 §2.5) — разметка, а не текст, и в том
        // числе внутри `<style>`: HTML-разбор держит его в сыром тексте
        // таблицы, и слова комментария становились мусорным селектором,
        // который глотал СЛЕДУЮЩЕЕ правило (эталоны
        // `flexbox-align-self-vert-*-ref.xhtml`: `.centerParent
        // { text-align: center }` за комментарием терялось, середина
        // уезжала к левому краю). Вне сырого текста комментарий ничего не
        // рисует — выбрасывается целиком.
        if tag.starts_with("<!--") {
            let end = tag.find("-->").map(|e| e + 3).unwrap_or(tag.len());
            rest = &tag[end..];
            continue;
        }
        if tag.starts_with("<!") || tag.starts_with("<?") {
            let end = tag.find('>').map(|e| e + 1).unwrap_or(tag.len());
            out.push_str(&tag[..end]);
            rest = &tag[end..];
            continue;
        }
        // Конец тега — с учётом кавычек в значениях атрибутов.
        let mut end = None;
        let mut quote: Option<u8> = None;
        for (i, b) in tag.bytes().enumerate().skip(1) {
            match (quote, b) {
                (Some(q), _) if b == q => quote = None,
                (Some(_), _) => {}
                (None, b'"') | (None, b'\'') => quote = Some(b),
                (None, b'>') => {
                    end = Some(i);
                    break;
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            out.push_str(tag);
            rest = "";
            break;
        };
        let inner = &tag[1..end];
        let name_end = inner
            .find(|c: char| c.is_whitespace() || c == '/' || c == '>')
            .unwrap_or(inner.len());
        let name = inner[..name_end].to_ascii_lowercase();
        if let Some(open) = name.strip_prefix('/') {
            if open == "svg" || open == "math" {
                foreign = foreign.saturating_sub(1);
            }
            out.push_str(&tag[..=end]);
        } else if inner.trim_end().ends_with('/') {
            if foreign > 0 || VOID.contains(&name.as_str()) || name.is_empty() {
                out.push_str(&tag[..=end]);
            } else {
                let attrs = inner.trim_end().trim_end_matches('/');
                out.push('<');
                out.push_str(attrs);
                out.push_str("></");
                out.push_str(&name);
                out.push('>');
            }
        } else {
            if name == "svg" || name == "math" {
                foreign += 1;
            }
            out.push_str(&tag[..=end]);
            // Первый перевод строки после `<pre>` выбрасывает только HTML-разбор
            // (`ignore_lf`, `vendor/html5ever/src/tree_builder/mod.rs:536`); в XML
            // такого правила нет, и XHTML-документ держит его строкой. Лишний
            // `\n` отдаётся разборщику на съедение, исходный остаётся:
            // `c548-ln-ht-000` (`pre.control` — 5 строк, у нас было 4),
            // `white-space-pre-001` (эталон ждёт 7 строк).
            let after = &tag[end + 1..];
            // В XML у `<style>`/`<script>` нет «сырого текста»: ссылки на
            // символы в нём раскрываются (XML 1.0 §4.4.2, §4.6), и правило
            // `div#test &gt; span` значит `div#test > span`. HTML-разбор
            // оставил бы `&gt;` буквами, и селектор пропадал целиком
            // (`inline-table-zorder-003…005`). Раскрываем до разбора —
            // кроме разделов CDATA (дословный текст) и комментариев (их
            // выбрасывает общий проход выше).
            if foreign == 0 && matches!(name.as_str(), "style" | "script") {
                let close = format!("</{name}");
                let body_end = after
                    .char_indices()
                    .find(|(i, _)| {
                        after[*i..]
                            .get(..close.len())
                            .is_some_and(|t| t.eq_ignore_ascii_case(&close))
                    })
                    .map(|(i, _)| i)
                    .unwrap_or(after.len());
                let mut body = &after[..body_end];
                while !body.is_empty() {
                    if body.starts_with("<![CDATA[") {
                        let e = body.find("]]>").map(|e| e + 3).unwrap_or(body.len());
                        out.push_str(&body[..e]);
                        body = &body[e..];
                    } else if body.starts_with("<!--") {
                        let e = body.find("-->").map(|e| e + 3).unwrap_or(body.len());
                        body = &body[e..];
                    } else if body.starts_with('&') {
                        let semi = body.bytes().take(12).position(|b| b == b';');
                        let ch = semi.and_then(|e| match &body[1..e] {
                            "lt" => Some('<'),
                            "gt" => Some('>'),
                            "amp" => Some('&'),
                            "quot" => Some('"'),
                            "apos" => Some('\''),
                            r if r.starts_with("#x") || r.starts_with("#X") => {
                                u32::from_str_radix(&r[2..], 16).ok().and_then(char::from_u32)
                            }
                            r if r.starts_with('#') => {
                                r[1..].parse::<u32>().ok().and_then(char::from_u32)
                            }
                            _ => None,
                        });
                        match (ch, semi) {
                            (Some(c), Some(e)) => {
                                out.push(c);
                                body = &body[e + 1..];
                            }
                            _ => {
                                out.push('&');
                                body = &body[1..];
                            }
                        }
                    } else {
                        let e = body
                            .find(|c| c == '&' || c == '<')
                            .map(|e| if e == 0 { 1 } else { e })
                            .unwrap_or(body.len());
                        out.push_str(&body[..e]);
                        body = &body[e..];
                    }
                }
                rest = &after[body_end..];
                continue;
            }
            if foreign == 0
                && matches!(name.as_str(), "pre" | "listing" | "textarea")
                && (after.starts_with('\n') || after.starts_with("\r\n"))
            {
                out.push('\n');
            }
        }
        rest = &tag[end + 1..];
    }
    out.push_str(rest);
    std::borrow::Cow::Owned(out)
}

/// То же, но с известными условиями окружения для `@media`.
pub fn parse_media(html: &str, extra_css: &str, media: Media) -> Vec<Node> {
    let _content_document = content::document(html);
    // Правила `@page` — от последнего РАЗОБРАННОГО документа: почистить,
    // чтобы прошлый лист не красил страницу нового. `@position-try` — тот же
    // пул и та же чистка.
    let _ = crate::css::take_page_decls();
    let _ = crate::css::take_try_rules();
    let _ = crate::css::take_property_rules();
    crate::counter_style_rules::reset();
    crate::css::reset_layers();
    crate::value::set_dark_scheme(false);
    // Корневые метрики (`rem`, `rlh`) — тоже от прошлого документа: у рамки
    // и у страницы свой корень, и чужие четыре точки на кегль испортили бы
    // весь разбор. Пишет их `walk` ниже, на элементе `html`.
    crate::value::reset_root_metrics();
    let html = expand_xhtml_self_closing(html);
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });
    // Режим quirks (HTML §13.2.6.4.1, «initial» insertion mode: документ без
    // DOCTYPE). Документ XHTML в quirks не бывает никогда (HTML §2.1, «XML
    // documents … always in no-quirks mode»), а мы разбираем и его HTML-
    // разборщиком, поэтому пространство имён корня его исключает.
    QUIRKS.with(|q| {
        q.set(
            dom.quirks_mode.get() == html5ever::tree_builder::QuirksMode::Quirks
                && !html.contains("http://www.w3.org/1999/xhtml"),
        )
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
    // Лист агента и тема — общие для ВСЕХ областей дерева: каждая тень
    // получает их копию, а правила документа в тень не попадают
    // (css-shadow-1 §3.2: селекторы сопоставляются в своей области).
    let agent_rules = rules.clone();
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
    let agent = Scope {
        rules: agent_rules,
        frames: frames.clone(),
    };
    for css in &sheets {
        frames.extend(crate::css::parse_keyframes_in(css, Some(media)));
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
    // Тени (`<template shadowrootmode>`): области стилей и распределение
    // слотов считаются ДО обхода, как отметки `:has`; обход читает их по
    // адресу узла. Документ без теней предпроход не платит.
    SHADOWS.with(|m| m.borrow_mut().clear());
    SLOTS.with(|m| m.borrow_mut().clear());
    let doc = Rc::new(Scope { rules, frames });
    if html.contains("shadowrootmode") {
        let mut drafts: HashMap<usize, SlotInfo> = HashMap::new();
        let top: Vec<Handle> = dom.document.children.borrow().clone();
        scan_shadows(&top, &mut vec![], &doc, &agent, media, &mut drafts);
        finish_slots(drafts);
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::counters::Counters::default();
    walk_children(
        &dom.document,
        &doc.rules,
        &vars,
        &doc.frames,
        &mut counter,
        &mut counters,
        &[],
        false,
        &mut out,
    );
    // ПЕРВЫМ проходом: табличная починка и подъёмы ниже читают `display`.
    resolve_display_inherit(&mut out, (None, None, None, None, None));
    resolve_rule_color_inherit(&mut out, &RuleColors::default());
    // Anonymous inline-table around orphan table boxes inside inline boxes
    // (CSS 2.1 §17.2.1 step 3); block parents are fixed up by `blocks()`.
    crate::render::inline_anon_tables(&mut out);
    // Лунки, которые умеет taffy, — на путь сетки ДО подъёмов и среза
    // подсетки: дальше они идут тем же кодом, что и сетка.
    lanes_as_grid(&mut out);
    hoist_grid_abspos(&mut out);
    grid_static_position::adjust(&mut out);
    flex_items_lose_float(&mut out);
    align_self_from_dom_parent(&mut out, None);
    grid_table_items_keep_stretch(&mut out);
    subgrid_takes_parent_tracks(&mut out);
    filter_ref_only_empty(&mut out);
    fold_run_ins(&mut out, None);
    if quirks() {
        quirks_percent_heights(&mut out, None);
    }
    out
}

/// Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме quirks
/// доля высоты элемента в потоке ищет опору через предков-блоков с
/// `height: auto` до ближайшего с заданной высотой. Сводится к точкам ЗДЕСЬ, в
/// стиле узла: флоаты и картинки строятся из сырого стиля (`wrap_floats`,
/// `image_with`), и пересчёт только в слитом (`inline::inherit`) до них не
/// доходил (`float-percentage-resolution-quirks-mode`,
/// `intrinsic-percent-replaced-003`). `base` — высота содержимого опоры для
/// детей; гибкий/сеточный/табличный предок с `auto` и абсолют цепочку рвут.
fn quirks_percent_heights(nodes: &mut [Node], base: Option<f32>) {
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

type RuleColors = (
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
fn resolve_rule_color_inherit(nodes: &mut [Node], parent: &RuleColors) {
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
fn filter_ref_only_empty(nodes: &mut [Node]) {
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
fn fold_run_ins(nodes: &mut Vec<Node>, parent: Option<&Computed>) {
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
            // Разница зазоров — «extra layer of (potentially negative)
            // margin» (css-grid-2 §subgrid-item-gaps): дорожка бывает и
            // ОТРИЦАТЕЛЬНОЙ (`grid-gap-011-ref`: 25 / −50 / 25); Blink
            // `accumulated_gutter_size_delta_` пола не имеет.
            *w += d / 2.0 * f32::from(sides);
        }
    }
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (08.09, v164/v165, `scout-grid-2026-09g.md`, 6 хунков):
/// поосевые признаки `subgrid_rows`/`subgrid_cols` в `Computed`/`dom.rs`/
/// `render.rs` + мост `TaffyLayoutEngine::grid_track_sizes` (KaminIDE patch).
/// Обещание +2…+4. Срез из 63 пар (v165, только этот патч и `<canvas>`):
/// +2 (`column-line-names-014`, `row-line-names-014`) / −3
/// (`column-auto-placed-subgrid-inherited-tracks-001` и
/// `-nested-subgrid-inherited-tracks-001` → «красное видно»,
/// `-inherited-tracks-003` 0.00 → 2.08). Поосевой признак без второго прохода
/// по разрешённым дорожкам ломает наследование дорожек у авто-размещённой
/// подсетки. Возвращать только вместе со вторым проходом (GRID-SUBGRID-TRACKS).
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
/// Подсеточность ЗАПРЕЩЕНА независимым контекстом форматирования.
///
/// css-grid-2 §subgrid-listing: «If there is no parent grid, or if the grid
/// container is otherwise forced to establish an independent formatting
/// context (for example, due to layout containment [CSS-CONTAIN-2] or
/// absolute positioning [CSS-POSITION-3]), the used value is the initial
/// value, `none`, and the grid container is not a subgrid.»
///
/// Список ровно тот же, что у Blink (`chromium-blink/third_party/blink/
/// renderer/core/layout/grid/grid_item.cc:189-191`): обособление РАСКЛАДКИ,
/// обособление ОТРИСОВКИ и контейнер запросов размера — плюс внепоточность.
/// `contain: strict` и `contain: content` сюда попадают сами: разбор
/// `computed.rs` раскрывает их в `layout`+`paint`.
///
/// Чего в списке НЕТ и быть не должно:
/// * `overflow: hidden|scroll` — css-grid-2 §subgrid-overflow прямо разрешает
///   прокручиваемую подсетку (`overflow-hidden-does-not-prohibit-subgrid`,
///   две пары корпуса);
/// * `contain: size` и `contain: style` в одиночку — случаи 8 и 9
///   `independent-formatting-context.html` требуют, чтобы подсетка ОСТАЛАСЬ;
/// * `<fieldset>` и `<button>` — `independent-formatting-context-fieldset`
///   (0.00) проверяет, что они ГОДНЫЕ подсетки.
pub(crate) fn subgrid_inhibited(style: &Computed) -> bool {
    style.contain_layout == Some(true)
        || style.contain_paint == Some(true)
        || style.container_size_query
        || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed))
}

pub(crate) fn subgrid_takes_parent_tracks(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        if matches!(
            el.style.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) {
            for row_dir in [false, true] {
                let raw = if row_dir {
                    el.style.grid_rows.clone()
                } else {
                    el.style.grid_tracks.clone()
                }
                .unwrap_or_default();
                // Доли `fr` при точечном размере родителя — в точки ДО нарезки
                // (`fr_tracks_to_px`). Такой срез годен только оси, где ребёнок
                // вправду подсеточный (проверка ниже, у ребёнка).
                let fr_px = fr_tracks_to_px(&el.style, &raw, row_dir);
                let from_fr = fr_px.is_some();
                let tracks = fr_px.unwrap_or(raw);
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (06.09, v125/v126,
                // `scout-subgrid-2026-09.md` шаг 1): расширить гейт с «все
                // дорожки `Px`» до «все нарезаемы» (симметрично здесь и в
                // `render.rs`). Срез css-grid+css-gaps+css-contain 2123 общих:
                // +11/−8 с `fr` и +9/−7 без `fr`, причём потери грубые
                // (`subgrid-gap-decorations-003` 0.00 → 99.00 с `fr`,
                // `auto-track-sizing-001` → 12.78,
                // `row-subgrid-orthogonal-writing-mode-002` → 11.38). Сначала
                // нужен шаг 2 отчёта — снять фазовый разрыв между лунками и
                // обычной сеткой: в `grid-subgridded-to-grid-lanes/**` тест и
                // эталон отличаются одним словом разметки и идут разными
                // путями, поэтому односторонняя правка разводит пару.
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
                    // css-grid-2 §subgrid-listing: использованное значение у
                    // такого элемента — НАЧАЛЬНОЕ `none`, то есть не «срез не
                    // выдали», а «явных дорожек нет вовсе». Иначе слово
                    // `subgrid` доживает до раскладки счётной дорожкой:
                    // `count_tracks` считает его за одну.
                    if subgrid_inhibited(&child.style) {
                        if row_dir {
                            child.style.grid_rows = None;
                        } else {
                            child.style.grid_tracks = None;
                            child.style.grid_cols = None;
                        }
                        continue;
                    }
                    // Переведённые доли режутся только в ПАРАЛЛЕЛЬНУЮ ось, где
                    // написано `subgrid`: своя ось подсетки остаётся своей
                    // (`subgrid-gap-decorations-003`: ряды `subgrid`, колонки
                    // `repeat(2, 1fr)` — прежний откат с сырой долей давал 99.00).
                    let parallel = child.style.vertical.unwrap_or(false)
                        == el.style.vertical.unwrap_or(false);
                    if !subgrid_axes::linked(&el.style, &child.style, row_dir)
                        || (from_fr && !parallel)
                    {
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
                    // Проверяется СВОЯ ось: первый проход (колонки) уже записал
                    // зазор колонок в `child.style.gap`, и прежнее условие «обе
                    // оси пусты» во втором проходе ложно — ряды подсетки
                    // оставались без зазора (`subgrid-gap-decorations-007`: ряды
                    // 0/100/200 вместо 0/110/220 при эталоне `grid-010-ref`).
                    let unset = own.is_none();
                    let own = own.or(par);
                    if own != Some(Len::Px(0.0)) && unset {
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
                    // css-grid-2 §subgrid-box-alignment: «The subgrid is
                    // always stretched in its subgridded dimension(s): the
                    // align-self/justify-self properties on it are ignored,
                    // as are any specified width/height constraints.»
                    //
                    // Гейт ПООСЕВОЙ (`subgrid_rows`/`subgrid_cols`), потому
                    // что срез приходит в ОБЕ оси, а гасить размер положено
                    // только в той, где вправду написано `subgrid`: иначе
                    // уходят пять зелёных `standalone-axis-size-*`.
                    //
                    // Ортогональную подсетку правило пропускает: `grid-template-
                    // rows` у неё — ось СВОЯ, и физическое свойство другое.
                    // Это отдельный корень (`scout-subgrid-orthogonal-2026-09`),
                    // трогать его здесь нельзя — три зелёных
                    // `row-subgrid-orthogonal-writing-mode-001/002/003`.
                    // ОРТОГОНАЛЬНАЯ подсетка: оси родителя и подсетки
                    // скрещены. `Computed` хранит дорожки ЛОГИЧЕСКИ
                    // (`apply::grid_style` переставляет их через `flip`), а
                    // подсеточная ось называется по шаблону САМОЙ подсетки
                    // (css-grid-2 §subgrid-listing): колонки родителя у
                    // подсетки с другим письмом — это её РЯДЫ, ряды родителя
                    // — её колонки. Blink пишет то же (`grid/grid_item.cc`:
                    // `has_subgridded_columns = is_parallel_with_root_grid ?
                    // GridTemplateColumns() : GridTemplateRows()`). Прежде
                    // срез колонок ложился в колонки подсетки (после `flip` —
                    // в ФИЗИЧЕСКИЕ ряды), а §subgrid-box-alignment
                    // («always stretched … any specified width/height
                    // constraints» игнорируются) у ортогональной подсетки не
                    // делался вовсе: вторая половина `subgrid/subgrid-stretch`
                    // (восемь коробок `vrl`, 16.23) держала свои 50/150 вместо
                    // дорожки 100. Размер гасится ФИЗИЧЕСКИЙ: ряды
                    // горизонтального родителя — высота, колонки — ширина.
                    // Разница зазоров по-прежнему пишется в оси родителя —
                    // отдельный шаг.
                    if !parallel {
                        let own = if row_dir {
                            child.style.subgrid_cols
                        } else {
                            child.style.subgrid_rows
                        };
                        let vertical_axis = row_dir != el.style.vertical.unwrap_or(false);
                        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (b47ecf2): класть СРЕЗ в скрещенную
                        // ось (колонки родителя → `grid_rows` подсетки): +1/−2,
                        // ушли `grid-subgridded-to-grid-lanes/track-sizing/
                        // {column,row}-subgrid-auto-fill-007` — тест там на
                        // ЛУНКАХ (срез режет `render.rs`, оси не скрещивает),
                        // эталон — та же разметка на `inline grid` (режет этот
                        // проход). Скрещиваются только признак и растяжка ниже;
                        // `subgrid-stretch` срезу безразличен (обе оси по 100).
                        // Возвращать вместе со скрещиванием в `render.rs` (Blink
                        // `grid_item.cc:192-207`) и замером лунковых пар.
                        if row_dir {
                            child.style.grid_rows = Some(slice);
                            child.style.align_self = None;
                        } else {
                            child.style.grid_tracks = Some(slice);
                            child.style.grid_cols = Some(span as u16);
                            child.style.justify_self = None;
                        }
                        if own {
                            if vertical_axis {
                                child.style.height = None;
                                child.style.max_height = None;
                                child.style.min_height = Some(Len::Px(0.0));
                            } else {
                                child.style.width = None;
                                child.style.max_width = None;
                                child.style.min_width = Some(Len::Px(0.0));
                            }
                            let stretch = Some(crate::computed::Align::Stretch);
                            if row_dir {
                                child.style.align_self = stretch;
                            } else {
                                child.style.justify_self = stretch;
                            }
                        }
                    } else if row_dir {
                        child.style.grid_rows = Some(slice);
                        child.style.align_self = None;
                        if parallel && child.style.subgrid_rows {
                            child.style.height = None;
                            child.style.max_height = None;
                            // Не `None`, а НОЛЬ: `None` вернул бы автоминимум
                            // элемента сетки, и подсетка раздулась бы шире
                            // своей области. Спека требует «размер
                            // игнорируется», а не «минимум по содержимому».
                            child.style.min_height = Some(Len::Px(0.0));
                            child.style.align_self = Some(crate::computed::Align::Stretch);
                        }
                    } else {
                        child.style.grid_tracks = Some(slice);
                        child.style.grid_cols = Some(span as u16);
                        child.style.justify_self = None;
                        if parallel && child.style.subgrid_cols {
                            child.style.width = None;
                            child.style.max_width = None;
                            child.style.min_width = Some(Len::Px(0.0));
                            child.style.justify_self = Some(crate::computed::Align::Stretch);
                        }
                    }
                }
            }
        }
        subgrid_takes_parent_tracks(&mut el.children);
    }
}

/// Доли `fr` родительской сетки в точках — для среза в ПОДСЕТКУ.
///
/// css-grid-2 §subgrids: подсетка получает ИСПОЛЬЗОВАННЫЕ размеры дорожек
/// родителя. Сырую долю резать нельзя: у подсетки она разрешается заново
/// против её собственного неопределённого размера (откат v125/v126 в
/// `subgrid_takes_parent_tracks`). Здесь доля переводится в точки по размеру
/// САМОГО родителя — css-grid-1 §12.7.1 «Find the Size of an fr»: остаток
/// после точечных дорожек и зазоров делится на сумму долей, но не меньше
/// единицы. Гейт: размер оси и зазор — точки (или зазор не задан), все
/// дорожки — точки или доли, хотя бы одна доля; иначе `None`. Рост доли под
/// содержимое (`minmax(auto, 1fr)`) здесь не виден — у пар семьи элементы пустые.
pub(crate) fn fr_tracks_to_px(
    style: &Computed,
    tracks: &[crate::computed::TrackSize],
    row_dir: bool,
) -> Option<Vec<crate::computed::TrackSize>> {
    use crate::computed::{Track, TrackSize};
    let mut fr_sum = 0.0f32;
    let mut px_sum = 0.0f32;
    for t in tracks {
        match t {
            TrackSize::Single(Track::Fr(f)) => fr_sum += *f,
            TrackSize::Single(Track::Px(v)) => px_sum += *v,
            _ => return None,
        }
    }
    if fr_sum <= 0.0 {
        return None;
    }
    let size = match if row_dir { style.height } else { style.width } {
        Some(Len::Px(v)) => v,
        _ => return None,
    };
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        None => Some(0.0),
        _ => None,
    };
    // `box-sizing: border-box` — заданный размер включает поля и рамку.
    let inner = if style.border_box == Some(true) {
        let b = style.borders();
        let (p0, p1, b0, b1) = if row_dir {
            (style.padding.top, style.padding.bottom, b.top, b.bottom)
        } else {
            (style.padding.left, style.padding.right, b.left, b.right)
        };
        size - px(p0)? - px(p1)? - px(b0)? - px(b1)?
    } else {
        size
    };
    let (grow, gcol) = style.gap.unwrap_or((None, None));
    let gap = px(if row_dir { grow } else { gcol })?;
    let n = tracks.len() as f32;
    let leftover = (inner - px_sum - gap * (n - 1.0)).max(0.0);
    let per = leftover / fr_sum.max(1.0);
    Some(
        tracks
            .iter()
            .map(|t| match t {
                TrackSize::Single(Track::Fr(f)) => TrackSize::Single(Track::Px(f * per)),
                other => other.clone(),
            })
            .collect(),
    )
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
fn align_self_from_dom_parent(nodes: &mut [Node], parent: Option<&ParentAlign>) {
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
struct ParentAlign {
    value: Option<crate::computed::Align>,
    /// safe, normal, own_axis, flex_kw, last.
    flags: (bool, bool, bool, bool, bool),
    /// Родитель — блочный контейнер (не flex/grid/contents/таблица).
    block: bool,
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
fn grid_table_items_keep_stretch(nodes: &mut [Node]) {
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
fn ruby_box_role(tag: &str, style: &Computed) -> Option<crate::computed::RubyRole> {
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
fn wrap_misparented_ruby(children: Vec<Node>) -> Vec<Node> {
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

/// Абсолютный ПОТОМОК сетки размещается по её линиям, а не по статической
/// позиции: если содержащий блок такого элемента — сама сетка, то `grid-row`
/// и `grid-column` задают ему прямоугольник области (css-grid-2 §9). Раскладка
/// знает только ПРЯМЫХ детей сетки, поэтому потомок поднимается к ней. Стиль к
/// этому моменту уже вычислен, и переезд по дереву его не меняет.
/// Ось лунок контейнера: `true` — лунки РЯДАМИ (ось решётки — ряды).
///
/// Без явного `grid-lanes-direction` направление выдаёт ТА ОСЬ, по которой
/// объявлены дорожки — то же правило, что у `render::lanes`.
pub(crate) fn lanes_row_dir(s: &Computed) -> bool {
    let row_tracks = s.grid_rows.is_some() || s.auto_repeat_rows.is_some() || s.grid_auto_fill_row.is_some();
    let col_tracks = s.grid_tracks.is_some() || s.auto_repeat_cols.is_some() || s.grid_auto_fill_min.is_some();
    s.lanes_row.unwrap_or(row_tracks && !col_tracks)
}

/// Контейнер лунок — на путь СЕТКИ, раскладку лунками делает taffy
/// (`vendor/taffy/src/compute/grid/lanes.rs`, css-grid-3).
///
/// css-grid-3 §grid-lanes-track-templates (Overview.bs:414-433): по оси
/// решётки «the full power of grid layout is available» — шаблоны, линии,
/// области, явная и неявная сетка «formed in the same way as for a regular
/// grid container», а дорожки размеряются алгоритмом css-grid-2 §12
/// (Overview.bs:619-669). Поэтому контейнер становится обычной сеткой с
/// пометкой `lanes_taffy`: шаблоны, зазоры, выравнивание и дети идут ТЕМ ЖЕ
/// путём, что у сетки-эталона (`grid-subgridded-to-grid-lanes/**` — та же
/// разметка на `inline-grid`), а не рукописной оценкой `render::lanes`.
///
/// Все контейнеры лунок идут сюда: вертикальное письмо — осями из
/// `apply.rs` (`grid_style`, `placement_flip`), `rtl` — как у сетки-эталона
/// (зеркала строчной оси у сетки taffy нет, и эталоны `inline-grid` с `rtl`
/// рисуются тем же путём; ★ ЗАМЕРЕНО: 57 пар лунок с `rtl` +3/−0). Подсетки среди
/// детей идут тем же путём: срез им режет `subgrid_takes_parent_tracks`
/// ровно как у сетки-эталона; интрин-дорожки в `repeat(auto-*)` считает
/// taffy (css-grid-3 §7.2.1).
fn lanes_as_grid(nodes: &mut [Node]) {
    for node in nodes.iter_mut() {
        let Node::Element(el) = node else { continue };
        lanes_as_grid(&mut el.children);
        if el.style.display != Some(Display::GridLanes) {
            continue;
        }
        lanes_to_grid(&mut el.style);
    }
}

/// Перевод контейнера лунок на путь сетки (см. `lanes_as_grid`).
pub(crate) fn lanes_to_grid(style: &mut Computed) {
    style.display = Some(if style.lanes_inline {
        Display::InlineGrid
    } else {
        Display::Grid
    });
    style.lanes_taffy = true;
}

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
        && &*name.local == "style"
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
                // Preserve child text content: an invented LF makes EOF strings invalid.
                sheet.push_str(body);
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
    /// Computed counter directives follow DOM inheritance, even without a box.
    pub(crate) counter_style: [Option<String>; 3],
    tag: String,
    /// Only HTML documents use ASCII case-insensitive names on HTML elements.
    html_attrs: bool,
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
    /// Хост, увиденный ИЗНУТРИ своей тени: безликий (css-shadow-1 §3.1 —
    /// «the shadow host is featureless»), с ним совпадает только компаунд из
    /// `:host`/`:host()`. `Some` несёт цепочку предков хоста в его СВЕТЛОМ
    /// контексте — ею проверяется аргумент `:host(S)`.
    featureless: Option<Rc<Vec<Ancestor>>>,
    /// Слот дерева теней: его распределение (см. `SLOTS`).
    slot: Option<Rc<SlotInfo>>,
    /// Братья-элементы узла и его место среди них — у предка в цепочке
    /// `path`: нужны компаунду предка с соседним комбинатором (`div + div
    /// span`, Selectors-4 §16.3). У переписи братьев (`census_of`) пусто —
    /// там соседи приходят через `Sibs`.
    peers: Option<(Rc<Vec<Ancestor>>, usize)>,
}

/// Отметки `:has()` текущего документа: адрес узла - хеши аргументов.
///
/// Поток разбирает документ целиком, поэтому склад потоко-локальный:
/// заполняется перед обходом, чистится по его окончании. Протаскивать его
/// параметром через всю цепочку обхода - шесть сигнатур ради одной ветки.
thread_local! {
    /// Документ в режиме quirks (ставит `parse_media`; рамка сохраняет и
    /// возвращает признак внешнего документа сама — `render::iframe`).
    pub(crate) static QUIRKS: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Режим quirks текущего документа.
pub(crate) fn quirks() -> bool {
    QUIRKS.with(|q| q.get())
}

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

/// Таблицы одной области дерева — документа или тени: правила и кадры.
pub(crate) struct Scope {
    rules: Vec<Rule>,
    frames: HashMap<String, Keyframes>,
}

/// Дерево теней хоста (HTML §4.12.3, `<template shadowrootmode>`).
///
/// html5ever тень к хосту не крепит (`attach_declarative_shadow` у `RcDom`
/// возвращает false) и по HTML §13.2.6.4.4 шаг 8.1.1 оставляет обычный
/// `<template>` ребёнком хоста, а разметку тени — в его `template_contents`.
/// Здесь это и есть корень тени.
struct Shadow {
    root: Handle,
    /// Таблицы тени: лист агента + `<style>` тени. Правила документа сюда
    /// не попадают, правила тени — наружу (css-shadow-1 §3.2).
    scope: Rc<Scope>,
    /// Паспорт хоста глазами тени — безликий, с цепочкой светлых предков.
    marker: Ancestor,
}

/// Узел, распределённый в слот, и его место среди СВЕТЛЫХ детей хоста.
struct Slotted {
    node: Handle,
    /// Номер в списке светлых детей (без шаблона тени).
    light_idx: usize,
    /// Сколько элементов стоит ДО узла; для элемента `light_all[elem_pos]` —
    /// он сам (соглашение `Sibs`).
    elem_pos: usize,
    /// Паспорт элемента; None — текст.
    anc: Option<Ancestor>,
}

/// Плоский распределённый элемент со светлым контекстом сопоставления —
/// для аргумента `:has-slotted(S)`.
struct FlatCtx {
    anc: Ancestor,
    path: Rc<Vec<Ancestor>>,
    all: Rc<Vec<Ancestor>>,
    pos: usize,
}

/// Слот дерева теней: что в него распределено и чем это стилизовать.
struct SlotInfo {
    slot: Handle,
    /// Распределённые (DOM §4.2.2.4 «find slottables»); пусто — рисуется
    /// fallback, то есть собственные дети слота.
    assigned: Vec<Slotted>,
    /// Область ХОСТА: распределённые дети стилизуются её таблицами.
    outer: Rc<Scope>,
    /// Цепочка предков распределённого: предки хоста + сам хост.
    host_path: Rc<Vec<Ancestor>>,
    /// Светлые дети хоста без шаблона тени, их места и паспорта.
    light: Vec<Handle>,
    light_spots: Vec<Spot>,
    light_all: Rc<Vec<Ancestor>>,
    /// Плоские распределённые (DOM «find flattened slottables»): текст —
    /// None, но в счёте участвует (`has-slotted-001` зелёная от пробелов).
    flattened: Vec<Option<FlatCtx>>,
}

thread_local! {
    /// Тени документа: адрес хоста → тень.
    static SHADOWS: std::cell::RefCell<HashMap<usize, Rc<Shadow>>> =
        std::cell::RefCell::new(HashMap::new());
    /// Слоты теней документа: адрес слота → распределение.
    static SLOTS: std::cell::RefCell<HashMap<usize, Rc<SlotInfo>>> =
        std::cell::RefCell::new(HashMap::new());
}

fn node_key(handle: &Handle) -> usize {
    Rc::as_ptr(handle) as usize
}

fn shadow_of(handle: &Handle) -> Option<Rc<Shadow>> {
    SHADOWS.with(|m| m.borrow().get(&node_key(handle)).cloned())
}

fn slot_of(handle: &Handle) -> Option<Rc<SlotInfo>> {
    SLOTS.with(|m| m.borrow().get(&node_key(handle)).cloned())
}

fn is_slot(handle: &Handle) -> bool {
    matches!(&handle.data, NodeData::Element { name, .. } if local_name(&name.local) == "slot")
}

fn attr_of(handle: &Handle, key: &str) -> Option<String> {
    let NodeData::Element { attrs, .. } = &handle.data else {
        return None;
    };
    attrs
        .borrow()
        .iter()
        .find(|a| &*a.name.local == key)
        .map(|a| a.value.to_string())
}

/// Шаблон объявленной тени среди детей хоста: ПЕРВЫЙ `<template
/// shadowrootmode="open|closed">` (HTML §13.2.6.4.4: второй такой шаблон к
/// хосту не крепится и остаётся обычным `<template>`). Возвращает сам
/// шаблон (его надо вычесть из светлых детей) и содержимое — корень тени.
fn declarative_shadow(host: &Handle) -> Option<(Handle, Handle)> {
    host.children.borrow().iter().find_map(|child| {
        let NodeData::Element {
            name,
            template_contents,
            ..
        } = &child.data
        else {
            return None;
        };
        if local_name(&name.local) != "template" {
            return None;
        }
        let mode = attr_of(child, "shadowrootmode")?.to_ascii_lowercase();
        if mode != "open" && mode != "closed" {
            return None;
        }
        let root = template_contents.borrow().clone()?;
        Some((child.clone(), root))
    })
}

/// Таблицы тени: копия листа агента + каждый `<style>` тени отдельной
/// таблицей происхождения документа. Возвращает область и аргументы её
/// `:has()` — отметки для них ставятся по дереву тени.
fn shadow_scope(root: &Handle, agent: &Scope, media: Media) -> (Rc<Scope>, Vec<HasArg>) {
    let mut rules = agent.rules.clone();
    let mut frames = agent.frames.clone();
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(root, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_stylesheet_media(css, media).into_iter().enumerate() {
            rules.push(Rule {
                order: base + i,
                origin: 1,
                ..r
            });
        }
        frames.extend(crate::css::parse_keyframes_in(css, Some(media)));
    }
    let mut raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut raw));
    let args = raw.iter().filter_map(|a| parse_has_arg(a)).collect();
    (Rc::new(Scope { rules, frames }), args)
}

/// Все `<slot>` дерева тени в порядке дерева. Вложенные тени лежат в
/// `template_contents`, а не в `children`, поэтому сюда не попадают; светлые
/// дети вложенных хостов — попадают, они в этом же дереве.
fn collect_slots(handle: &Handle, out: &mut Vec<Handle>) {
    for child in handle.children.borrow().iter() {
        if is_slot(child) {
            out.push(child.clone());
        }
        collect_slots(child, out);
    }
}

/// Распределение по слотам (DOM §4.2.2.4 «find a slot»): каждый светлый
/// ребёнок хоста — элемент или ТЕКСТ — уходит в первый слот тени с его
/// именем (`slot=""` элемента; у текста и без атрибута — пустое). Ребёнок без
/// подходящего слота не рисуется вовсе.
fn assign_slots(
    root: &Handle,
    light: &[Handle],
    outer: &Rc<Scope>,
    host_path: &Rc<Vec<Ancestor>>,
    drafts: &mut HashMap<usize, SlotInfo>,
) {
    let mut slots: Vec<Handle> = vec![];
    collect_slots(root, &mut slots);
    if slots.is_empty() {
        return;
    }
    let (spots, all) = census_of(light);
    let light_all = Rc::new(all);
    let mut assigned: HashMap<usize, Vec<Slotted>> = HashMap::new();
    let mut pos = 0usize;
    for (idx, node) in light.iter().enumerate() {
        let (name, anc, elem_pos) = match &node.data {
            NodeData::Element { .. } => {
                let anc = light_all[pos].clone();
                pos += 1;
                let name = anc
                    .attrs
                    .iter()
                    .find(|(k, _)| k == "slot")
                    .map(|(_, v)| v.clone())
                    .unwrap_or_default();
                (name, Some(anc), pos - 1)
            }
            NodeData::Text { .. } => (String::new(), None, pos),
            _ => continue,
        };
        let Some(slot) = slots
            .iter()
            .find(|s| attr_of(s, "name").unwrap_or_default() == name)
        else {
            continue;
        };
        assigned.entry(node_key(slot)).or_default().push(Slotted {
            node: node.clone(),
            light_idx: idx,
            elem_pos,
            anc,
        });
    }
    for slot in slots {
        let key = node_key(&slot);
        drafts.insert(
            key,
            SlotInfo {
                slot,
                assigned: assigned.remove(&key).unwrap_or_default(),
                outer: outer.clone(),
                host_path: host_path.clone(),
                light: light.to_vec(),
                light_spots: spots.clone(),
                light_all: light_all.clone(),
                flattened: vec![],
            },
        );
    }
}

/// Предпроход по теням в порядке дерева. На хосте: область стилей тени,
/// отметки `:has` по тени, распределение слотов; дальше — в тень с цепочкой
/// из одного безликого хоста (css-shadow-1 §3.1: «the selector match list is
/// initially the shadow host, followed by all children of the shadow root»)
/// и в светлых детей — в текущей области.
fn scan_shadows(
    children: &[Handle],
    path: &mut Vec<Ancestor>,
    scope: &Rc<Scope>,
    agent: &Scope,
    media: Media,
    drafts: &mut HashMap<usize, SlotInfo>,
) {
    let (_, all) = census_of(children);
    let mut pos = 0usize;
    for child in children {
        if !matches!(&child.data, NodeData::Element { .. }) {
            continue;
        }
        let me = all[pos].clone();
        pos += 1;
        if let Some((template, root)) = declarative_shadow(child) {
            let (inner, args) = shadow_scope(&root, agent, media);
            if !args.is_empty() {
                mark_has(&root, &args, &mut vec![]);
            }
            let host_path: Rc<Vec<Ancestor>> = Rc::new(
                path.iter()
                    .cloned()
                    .chain(std::iter::once(me.clone()))
                    .collect(),
            );
            let light: Vec<Handle> = child
                .children
                .borrow()
                .iter()
                .filter(|c| !Rc::ptr_eq(c, &template))
                .cloned()
                .collect();
            assign_slots(&root, &light, scope, &host_path, drafts);
            let marker = Ancestor {
                featureless: Some(Rc::new(path.clone())),
                ..me.clone()
            };
            SHADOWS.with(|m| {
                m.borrow_mut().insert(
                    node_key(child),
                    Rc::new(Shadow {
                        root: root.clone(),
                        scope: inner.clone(),
                        marker: marker.clone(),
                    }),
                )
            });
            let shadow_kids: Vec<Handle> = root.children.borrow().clone();
            scan_shadows(&shadow_kids, &mut vec![marker], &inner, agent, media, drafts);
            path.push(me);
            scan_shadows(&light, path, scope, agent, media, drafts);
            path.pop();
        } else {
            let kids: Vec<Handle> = child.children.borrow().clone();
            path.push(me);
            scan_shadows(&kids, path, scope, agent, media, drafts);
            path.pop();
        }
    }
}

/// Плоские распределённые слота (DOM «find flattened slottables»):
/// распределённые, иначе fallback-дети; слот среди них раскрывается
/// рекурсивно. Глубина ограничена: цикла распределений в дереве быть не
/// может, но стража дешевле доказательства.
fn flatten_slot(
    key: usize,
    drafts: &HashMap<usize, SlotInfo>,
    depth: usize,
    out: &mut Vec<Option<FlatCtx>>,
) {
    let Some(info) = drafts.get(&key) else { return };
    if depth > 32 {
        return;
    }
    if !info.assigned.is_empty() {
        for s in &info.assigned {
            if is_slot(&s.node) && drafts.contains_key(&node_key(&s.node)) {
                flatten_slot(node_key(&s.node), drafts, depth + 1, out);
            } else {
                out.push(s.anc.clone().map(|anc| FlatCtx {
                    anc,
                    path: info.host_path.clone(),
                    all: info.light_all.clone(),
                    pos: s.elem_pos,
                }));
            }
        }
        return;
    }
    let kids: Vec<Handle> = info.slot.children.borrow().clone();
    let (_, all) = census_of(&kids);
    let all = Rc::new(all);
    let mut pos = 0usize;
    for kid in &kids {
        match &kid.data {
            NodeData::Element { .. } => {
                if is_slot(kid) && drafts.contains_key(&node_key(kid)) {
                    flatten_slot(node_key(kid), drafts, depth + 1, out);
                } else {
                    out.push(Some(FlatCtx {
                        anc: all[pos].clone(),
                        path: Rc::new(vec![]),
                        all: all.clone(),
                        pos,
                    }));
                }
                pos += 1;
            }
            NodeData::Text { .. } => out.push(None),
            _ => {}
        }
    }
}

/// Досчитать плоские списки и выложить слоты на склад.
fn finish_slots(mut drafts: HashMap<usize, SlotInfo>) {
    let keys: Vec<usize> = drafts.keys().copied().collect();
    let mut flats: HashMap<usize, Vec<Option<FlatCtx>>> = HashMap::new();
    for key in &keys {
        let mut flat = vec![];
        flatten_slot(*key, &drafts, 0, &mut flat);
        flats.insert(*key, flat);
    }
    SLOTS.with(|m| {
        let mut m = m.borrow_mut();
        for key in keys {
            if let Some(mut info) = drafts.remove(&key) {
                info.flattened = flats.remove(&key).unwrap_or_default();
                m.insert(key, Rc::new(info));
            }
        }
    });
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
    // Атрибут `dir` (и `auto`: сторону потом решает первый сильный знак,
    // `render.rs`) — встраивание (прежний ход: RLE/LRE … PDF), пока стиль
    // не задал `unicode-bidi` сам. HTML UA-лист даёт `[dir] { unicode-bidi:
    // isolate }`; здесь сохранено прежнее встраивание — переход на
    // изоляцию отдельный шаг с замером.
    // HTML §15.3.4 (Bidirectional text): `[dir=ltr i], [dir=rtl i] {
    // unicode-bidi: isolate }` — изоляция, а не встраивание: строчный
    // `<span dir=rtl>` в rtl-абзаце не перемешивается с соседним ltr-текстом
    // (`text-overflow-string-007/008-ref`). `auto` пока остаётся прежним
    // встраиванием: сторону ему выбирает отрисовка.
    let explicit = style.bidi_embed.is_some() || style.bidi_isolate.is_some();
    match value.to_ascii_lowercase().as_str() {
        "rtl" | "ltr" if !explicit && tag != "bdo" => style.bidi_isolate = Some(true),
        "rtl" | "ltr" | "auto" if style.bidi_embed.is_none() => style.bidi_embed = Some(true),
        _ => {}
    }
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

/// css-ruby-1 §2.2 п.1 «Inlinify block-level boxes».
///
/// Коробка блочного УРОВНЯ в потоке, лежащая внутри руби-коробки (контейнер,
/// `rb`, `rt`, `rbc`, `rtc`), получает строчный аналог: `block`/`list-item`
/// -> `inline-block`, `table` -> `inline-table`, `flex` -> `inline-flex`,
/// `grid` -> `inline-grid`; блочный ПО ТЕГУ элемент без своего `display`
/// (`<div>`, `<p>`, `<li>`) — `inline-block`. Внутренние табличные виды не
/// трогаются: их по спеке заворачивает АНОНИМНАЯ строчная таблица, которой у
/// нас нет (`ruby-inlinize-blocks-003` этим рукавом не берётся). Правило
/// проходит сквозь неатомарные строчные звенья (`<b>` внутри `<ruby>`), но не
/// сквозь блок или атом: те начинают свой поток.
///
/// `ancestors` — цепочка предков от БЛИЖАЙШЕГО. Руби узнаётся по имени тега:
/// `display: ruby*` пока не разбирается, а разметка набора пишется тегами.
fn inlinify_in_ruby<'a>(
    style: &mut Computed,
    tag: &str,
    ancestors: impl Iterator<Item = &'a Ancestor>,
) {
    // Вне потока коробка блокифицируется (§9.7) и инлайнизации не подлежит.
    if style.float.is_some_and(|f| f != 0)
        || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed))
    {
        return;
    }
    let mut inside = false;
    for a in ancestors {
        if matches!(a.tag.as_str(), "ruby" | "rb" | "rt" | "rbc" | "rtc") {
            inside = true;
            break;
        }
        if !INLINE_TAGS.contains(&a.tag.as_str()) {
            break;
        }
    }
    if !inside {
        return;
    }
    style.display = match style.display {
        Some(Display::Block) | Some(Display::ListItem) => Some(Display::InlineBlock),
        Some(Display::Table) => Some(Display::InlineTable),
        Some(Display::Flex) => Some(Display::InlineFlex),
        Some(Display::Grid) => Some(Display::InlineGrid),
        None if tag == "table" => Some(Display::InlineTable),
        None if BLOCK_TAGS.contains(&tag) => Some(Display::InlineBlock),
        other => other,
    };
}

fn finish_inline_display(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
    use crate::computed::Display;
    replaced_display::normalize(style, tag, attrs);
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
        // Блокифицированная коробка строчного выравнивания не имеет
        // (`vertical-align` «applies to inline-level and table-cell
        // elements», CSS 2.1 §10.8.1): статическая позиция абсолюта — та же,
        // что без `sub` (`vertical-align-sub-001`: зелёный уезжал вниз и
        // открывал красный).
        style.apply_one("vertical-align", "baseline");
    }
    if out_of_flow {
        if matches!(
            style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) && matches!(
            style.display,
            Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
                | Some(Display::InlineBlock)
        ) {
            style.abs_inline_level = true;
        }
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
    containment::normalize(style, tag, out_of_flow);
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

/// `aspect-ratio: auto && <ratio>` у НЕзамещаемой коробки (css-sizing-4
/// §5.1): «the preferred aspect ratio is the specified ratio … unless it is
/// a replaced element with a natural aspect ratio … size calculations
/// involving the aspect ratio work with the content box dimensions always».
/// У замещаемых запасное соотношение читает отрисовка (`image_with::ratio_of`),
/// здесь их не трогаем. Раскладка движка считает соотношение по
/// border-box при `box-sizing: border-box`, поэтому соотношение контента
/// переводится в соотношение border-box по оси, заданной в точках
/// (`block-aspect-ratio-004/006`, `flex-aspect-ratio-025/026`).
fn promote_auto_ratio(style: &mut Computed, tag: &str) {
    if matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input" | "select"
            | "textarea" | "button"
    ) || style.aspect_ratio.is_some()
    {
        return;
    }
    let Some(r) = style.aspect_ratio_auto.filter(|r| r.is_finite() && *r > 0.0) else {
        return;
    };
    if style.border_box != Some(true) {
        style.aspect_ratio = Some(r);
        return;
    }
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = style.borders();
    let pad_x = px(style.padding.left) + px(style.padding.right) + px(b.left) + px(b.right);
    let pad_y = px(style.padding.top) + px(style.padding.bottom) + px(b.top) + px(b.bottom);
    // Ось, заданная в точках (с зажимом своими пределами), либо её предел.
    let axis = |v: Option<Len>, lo: Option<Len>, hi: Option<Len>| -> Option<f32> {
        let clamp = |x: f32| {
            let x = match lo {
                Some(Len::Px(l)) => x.max(l),
                _ => x,
            };
            match hi {
                Some(Len::Px(h)) => x.min(h),
                _ => x,
            }
        };
        match (v, lo) {
            (Some(Len::Px(x)), _) => Some(clamp(x)),
            (_, Some(Len::Px(l))) => Some(l),
            _ => None,
        }
    };
    let w = axis(style.width, style.min_width, style.max_width);
    let h = axis(style.height, style.min_height, style.max_height);
    let border_ratio = match (w, h) {
        (Some(wb), None) => {
            let hb = (wb - pad_x).max(0.0) / r + pad_y;
            (hb > 0.0).then(|| wb / hb)
        }
        (None, Some(hb)) if hb > 0.0 => {
            let wb = (hb - pad_y).max(0.0) * r + pad_x;
            Some(wb / hb)
        }
        _ => None,
    };
    style.aspect_ratio = Some(border_ratio.unwrap_or(r));
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
    // Оба атрибута объявлены разметкой — только тогда природное соотношение
    // сторон холста известно точно. При одном объявленном вторая сторона
    // берётся из умолчания 300/150 ниже, и «соотношением» она быть не может:
    // на заданной атрибутом стороне стоит `flex-basis: content`
    // (`flexbox-flex-basis-content-001a`: `<canvas width="20"
    // style="height: 8px">`).
    let natural_pair =
        tag == "canvas" && style.attr_width.is_some() && style.attr_height.is_some();
    if tag == "canvas" {
        style.attr_width = style.attr_width.or(Some(Len::Px(300.0)));
        style.attr_height = style.attr_height.or(Some(Len::Px(150.0)));
    }
    if style.width.is_none() {
        style.width = style.attr_width;
        style.attr_sized.0 = tag == "canvas" && style.attr_width.is_some();
    }
    if style.height.is_none() {
        style.height = style.attr_height;
        style.attr_sized.1 = tag == "canvas" && style.attr_height.is_some();
    }
    // Атрибуты холста — ПРИРОДНЫЙ размер, а не заданный автором: HTML §4.12.5
    // («the intrinsic dimensions of the canvas element equal the size of the
    // coordinate space»), и в списке «dimension attributes» HTML Rendering
    // §15.3.10 холста нет. Значит, как только автор назвал в CSS хоть одну
    // ось, оставшаяся обязана прийти из соотношения, а не из атрибута —
    // css-sizing-4 §4.1 «Min/Max Size Transfers» и пример там же: у
    // `<div style="height:100px;float:left"><canvas style="height:100%">`
    // ширина холста и ВКЛАД во внутренний размер равны 100 точкам. Пока
    // атрибут занимал `style.width`, вклад был равен атрибуту, и флоат
    // выходил 10 точек вместо 100 (`intrinsic-percent-replaced-001`).
    // Когда обе оси пришли от атрибутов, это и есть природный размер — там
    // ничего не меняется, и `flex-basis: content`, `contain: size` и спаннер
    // многоколоночника, читающие `attr_width`/`attr_height` отдельно
    // (`render.rs:3140`, `:16979`), работают как прежде.
    if natural_pair && !(style.attr_sized.0 && style.attr_sized.1) {
        if let (Some(Len::Px(w)), Some(Len::Px(h))) = (style.attr_width, style.attr_height)
            && w > 0.0
            && h > 0.0
            && style.aspect_ratio.is_none()
        {
            style.aspect_ratio = Some(w / h);
        }
        if style.attr_sized.0 {
            style.width = None;
        }
        if style.attr_sized.1 {
            style.height = None;
        }
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
    /// Тот же список под `Rc`, если он есть: его забирает паспорт узла в
    /// цепочку предков (`Ancestor::peers`).
    pub(crate) rc: Option<&'a Rc<Vec<Ancestor>>>,
}

impl<'a> Sibs<'a> {
    pub(crate) const EMPTY: Sibs<'static> = Sibs {
        all: &[],
        pos: 0,
        is_elem: false,
        rc: None,
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
            rc: self.rc,
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
            .find(|a| &*a.name.local == key)
            .map(|a| a.value.to_string())
    };
    Some(Ancestor {
        counter_style: Default::default(),
        tag: local_name(&name.local),
        html_attrs: content::html_attributes(&name.ns),
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
        featureless: None,
        slot: slot_of(child),
        peers: None,
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
        universal: false,
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
            rc: None,
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
                            rc: None,
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
    let all = Rc::new(all);
    let mut pos = 0usize;
    for (idx, (child, spot)) in children.iter().zip(&spots).enumerate() {
        let is_elem = spot.index != 0;
        let sibs = Sibs {
            all: &all,
            pos,
            is_elem,
            rc: Some(&all),
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
            // Пробельный узел без пробела (`</span>\n\t<span>`) между двумя
            // строчными соседями — это тоже схлопываемый пробел строки
            // (css-text-3 §4.1.1: перевод строки и табуляция превращаются в
            // пробел), а не отбивка разметки между блоками. Выброшенный, он
            // склеивал соседние слова: `multicol-basic-001…004` — «XXXX» двух
            // span сливались в одно слово, колонки резались не там.
            let between_inline = !preserve
                && text.chars().all(collapsible)
                && matches!(out.last(), Some(Node::Element(prev))
                    if prev.inline && prev.style.display.is_none() && prev.tag != "br")
                && level[level_pos + 1..]
                    .iter()
                    .find(|h| !matches!(h.data, NodeData::Comment { .. }))
                    .is_some_and(|h| match &h.data {
                        NodeData::Text { contents } => {
                            !contents.borrow().chars().all(collapsible)
                        }
                        NodeData::Element { name, .. } => {
                            let tag = local_name(&name.local);
                            tag != "br" && INLINE_TAGS.contains(&tag.as_str())
                        }
                        _ => false,
                    });
            let text = if between_inline { " ".to_string() } else { text };
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
            let mut me = Ancestor {
                counter_style: Default::default(),
                tag: tag.clone(),
                html_attrs: content::html_attributes(&name.ns),
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
                featureless: None,
                slot: slot_of(handle),
                peers: sibs.is_elem.then_some(sibs.rc).flatten().map(|r| (r.clone(), sibs.pos)),
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
            // Правила `:host`/`:host(S)` из ТЕНИ хоста ложатся на сам хост
            // (css-shadow-1 §3.1; Blink `MatchHostRules`). Хост для них
            // безлик: совпадает только `:host`-компаунд без предков и братьев.
            let shadow = shadow_of(handle);
            if let Some(shadow) = &shadow {
                matched.extend(
                    shadow
                        .scope
                        .rules
                        .iter()
                        .filter(|r| matches(&r.sel, &shadow.marker, &[], Sibs::EMPTY)),
                );
            }
            // Свои переменные: родительские, поверх них объявления
            // совпавших правил в порядке каскада, поверх — свои же в
            // атрибуте. Пока словарь был один на документ, `:root{--c:red}`
            // и `.dark{--c:blue}` складывались в него подряд, и последнее
            // объявление красило ВЕСЬ документ — переключение темы классом
            // не работало в принципе.
            let registered = crate::css::property_rules();
            let cascaded = crate::css::custom_properties::cascade(
                &matched, &inline_decls, vars, &registered, syntax_accepts,
            );
            let own_vars = crate::css::variable_values::compute(
                &cascaded, vars, &registered, syntax_accepts,
            );
            let vars = &own_vars;
            // Используемая схема цвета (css-color-adjust-1 §color-scheme-prop):
            // своё `color-scheme` — последнее по каскаду, иначе родительская
            // (свойство наследуемое). Тёмная — когда названа только `dark`:
            // при `light dark` берётся предпочтение пользователя, у стенда
            // светлое. Держится на время узла и его потомков.
            let scheme = {
                let mut by_cascade: Vec<&&Rule> = matched.iter().collect();
                by_cascade.sort_by(|a, b| {
                    (a.origin, &a.layer, a.sel.specificity(), a.order)
                        .cmp(&(b.origin, &b.layer, b.sel.specificity(), b.order))
                });
                let mut last: Option<String> = None;
                for rule in by_cascade {
                    if let Some(v) = rule.decls.get("color-scheme") {
                        last = Some(v.clone());
                    }
                }
                if let Some(v) = inline_decls.get("color-scheme") {
                    last = Some(v.clone());
                }
                last
            };
            struct SchemeGuard(bool);
            impl Drop for SchemeGuard {
                fn drop(&mut self) {
                    crate::value::set_dark_scheme(self.0);
                }
            }
            let parent_dark = crate::value::dark_scheme();
            let _scheme = SchemeGuard(parent_dark);
            if let Some(v) = scheme {
                let low = v.to_ascii_lowercase();
                let words: Vec<&str> = low.split_whitespace().collect();
                let dark = words.contains(&"dark") && !words.contains(&"light");
                if !low.contains("inherit") {
                    crate::value::set_dark_scheme(dark);
                }
            }
            // Типизированный `attr()` читает атрибуты ЭТОГО элемента
            // (css-values-5 §7.7): слот ставится только на время его каскада.
            crate::computed::set_current_attrs(&attrs);
            crate::computed::set_current_sibling((spot.index > 0).then_some((spot.index, spot.total)));
            let nowrap_hint = presentational_hints::nowrap(&tag, &attrs);
            if let Some(rule) = &nowrap_hint {
                matched.push(rule);
            }
            let mut style = Computed::resolve_with_vars(&mut matched, &inline_decls, vars);
            inherit_counter_decls(&mut style, path.last().map(|p| &p.counter_style));
            apply_value_hint(&mut style, &me);
            me.counter_style = counter_snapshot(&style);
            crate::computed::clear_current_attrs();
            crate::computed::set_current_sibling(None);
            // Корневые метрики для `rem`/`rlh` (css-values-4 §6.1.4).
            // Записываются ЗДЕСЬ, а не в наследовании: `Len::parse` работает
            // на разборе объявлений, а `walk` идёт в порядке документа —
            // корень разбирается раньше любого потомка, и его `25rem` уже
            // читается верно. Собственные объявления корня успевают
            // разобраться по прежней базе; на самом корне `rem` по спеке и
            // так меряется РОДИТЕЛЬСКИМИ (начальными) метриками.
            if tag == "html" {
                let font = match style.font_size {
                    Some(crate::value::Len::Px(v)) => v,
                    Some(crate::value::Len::Em(k)) | Some(crate::value::Len::Pct(k)) => k * 16.0,
                    _ => 16.0,
                };
                let family = style.font_family.clone().unwrap_or_default();
                let line = match style.line_height {
                    Some(crate::value::Len::Px(v)) => v,
                    Some(crate::value::Len::Em(k)) | Some(crate::value::Len::Pct(k)) => k * font,
                    _ => {
                        let f = crate::metrics::normal_line(&family);
                        if f > 0.0 { f * font } else { 1.2 * font }
                    }
                };
                crate::value::set_root_metrics(font, line);
                crate::value::set_root_font_view(match style.font_size {
                    Some(crate::value::Len::Vh(k)) => Some((true, k)),
                    Some(crate::value::Len::Vw(k)) => Some((false, k)),
                    _ => None,
                });
            }
            apply_presentational_size(&mut style, &tag, &attrs);
            promote_auto_ratio(&mut style, &tag);
            presentational_hints::colors(&mut style, &tag, &attrs);
            finish_inline_display(&mut style, &tag, &attrs);
            style.plain_block_box = {
                use crate::computed::Display;
                let special = matches!(
                    tag.as_str(),
                    "table" | "caption" | "colgroup" | "col" | "thead" | "tbody" | "tfoot"
                        | "tr" | "td" | "th" | "hr" | "fieldset" | "legend" | "details"
                        | "summary" | "dialog" | "option" | "optgroup" | "html" | "body"
                        | "input" | "textarea" | "select" | "button" | "img" | "video"
                        | "canvas" | "iframe" | "embed" | "object" | "svg" | "meter"
                        | "progress"
                );
                let block = match style.display {
                    Some(Display::Block)
                    | Some(Display::GridLanes)
                    | Some(Display::ListItem)
                    | Some(Display::Flex)
                    | Some(Display::Grid) => true,
                    None => style.inline_display != Some(true) && BLOCK_TAGS.contains(&tag.as_str()),
                    _ => false,
                };
                block && !special && style.float.is_none_or(|f| f == 0)
            };
            // css-will-change-1: обещанный `transform`/`contain` делает коробку
            // содержащим блоком и контекстом наложения лишь там, где само
            // свойство применимо. У строчной НЕатомарной коробки его нет
            // (`will-change-transform-inline`: `fixed` внутри `<span>` стоит от
            // окна); замещаемые и вынесенные из потока — атомарны. Вид коробки
            // известен только здесь, после `finish_inline_display`.
            if style.will_change & crate::computed::wc::BOX != 0 {
                let out_of_flow = style.float.is_some_and(|f| f != 0)
                    || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed));
                let replaced = matches!(
                    tag.as_str(),
                    "img" | "svg" | "input" | "select" | "textarea" | "button" | "video"
                        | "canvas" | "iframe" | "object" | "embed" | "meter" | "progress"
                );
                let inline_tag =
                    INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
                let non_atomic = !out_of_flow
                    && !replaced
                    && (style.inline_display == Some(true)
                        || (style.display.is_none() && inline_tag));
                if !non_atomic {
                    use crate::computed::wc;
                    style.will_change |= wc::CB_ABS | wc::CB_FIXED | wc::STACK;
                }
            }
            // `transform-style` applies only to transformable elements
            // (css-transforms-2): a non-atomic inline with `preserve-3d` is no
            // 3D context, stacking context or containing block
            // (`preserve-3d-flat-grouping-properties-containing-block-inline`).
            if style.preserve_3d == Some(true) {
                let out_of_flow = style.float.is_some_and(|f| f != 0)
                    || matches!(style.position, Some(Position::Absolute) | Some(Position::Fixed));
                let inline_tag =
                    INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
                let replaced = matches!(
                    tag.as_str(),
                    "img" | "svg" | "input" | "select" | "textarea" | "button" | "video"
                        | "canvas" | "iframe" | "object" | "embed" | "meter" | "progress"
                );
                if !out_of_flow
                    && !replaced
                    && (style.inline_display == Some(true)
                        || (style.display.is_none() && inline_tag))
                {
                    style.preserve_3d = None;
                    style.frame_3d = None;
                }
            }
            inlinify_in_ruby(&mut style, &tag, path.iter().rev());
            // css-ruby-1 §3.3: «Neither the margin, padding, and border
            // properties … apply to base containers or annotation containers»
            // (`ruby-box-model-001`: `.rbc.pv { padding: 100px }` не должен
            // отодвигать аннотацию от базы). Контейнер — по тегу или по роли.
            if matches!(tag.as_str(), "rbc" | "rtc")
                || matches!(
                    style.ruby_role,
                    Some(crate::computed::RubyRole::BaseContainer)
                        | Some(crate::computed::RubyRole::TextContainer)
                )
            {
                style.margin = crate::computed::Sides::default();
                style.padding = crate::computed::Sides::default();
                style.border_width = crate::computed::Sides::default();
            }
            // motion-1: offset-трансформ считается НЕ здесь, а вторым проходом
            // по дереву коробок (`motion::settle`, зовётся из `doc.rs`).
            // §offset-path: «In CSS contexts, the boxes being referenced are
            // from the element that establishes the containing block for this
            // element» — опорная коробка `<coord-box>`, длина `ray()` и начало
            // `at <position>` берутся у СОДЕРЖАЩЕГО БЛОКА, а на разборе стиля
            // родителя нет вовсе: сюда доезжает только собственный каскад.
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
            let layer = |name, base| initial_pseudos::resolve(name, rules, vars, &me, path, sibs, base);
            let first_letter = layer("first-letter", Some(&style));
            let first_line = layer("first-line", Some(&style));
            let first_line_own = layer("first-line", None).map(Box::new);
            style.first_letter_own = layer("first-letter", None).map(Box::new);
            style.first_line_own = first_line_own;
            // `::marker` — НЕ копией стиля хозяина, как первая буква, а
            // ТОЛЬКО своими объявлениями поверх таблицы агента: копия
            // протащила бы в маркер рамку, поля и размеры самого `<li>`.
            // CSS Lists 3 §3.1.1 gives marker text its own transform default,
            // including when no author ::marker rule matches.
            // Сворачивается ниже, ПОСЛЕ снятия номера пункта:
            // `counter(list-item)` в его `content` обязан видеть своё
            // значение.
            let marker_layer = {
                let mut found: Vec<&Rule> = rules
                    .iter()
                    .filter(|r| r.sel.pseudo.as_deref() == Some("marker"))
                    .filter(|r| matches_ignoring_pseudo(&r.sel, &me, path, sibs))
                    .collect();
                found.sort_by_key(|r| (r.sel.specificity(), r.order));
                (!found.is_empty() || tag == "li" || style.display == Some(Display::ListItem)).then(|| {
                    let mut m = Computed::default();
                    m.text_transform = Some(crate::computed::TextTransform::None);
                    if !found.is_empty() {
                        m.bidi_isolate = Some(true);
                    }
                    for rule in found.iter() {
                        m.apply_decls_with_vars(&rule.decls, vars);
                    }
                    m
                })
            };

            // Обратный счётчик без числа: начальное значение — итог
            // предварительного обхода области (css-lists-3
            // §instantiating-counters). Считается ЗДЕСЬ, до применения
            // директив: запись создаётся уже готовым числом.
            let reversed_start = |nm: &str, counters: &mut crate::counters::Counters| {
                crate::counters_scan::reversed_initial(
                    rules, vars, nm, handle, &me, path, sibs, level, spots, level_pos,
                )
            };

            if style.display == Some(Display::None) {
                // Table columns use an internal non-flow display, but still
                // generate boxes (CSS 2.1 §17.2). Their counter directives
                // apply; actual display:none nodes have no counters (§12.4.3).
                // CSS 2.1 §14.2: the root background paints the canvas even
                // without a root box. Keep its style for canvas propagation.
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
                counters.enter();
                apply_counter_decls(
                    &style, counters, &tag, &attrs, &mut false, &reversed_start,
                );
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
                counters.leave();
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

            // Counters apply before children; display:contents has no box level.
            let box_level = style.display != Some(Display::Contents);
            if box_level {
                counters.enter();
                counters.set_quote_language(language::parent(&me, path).unwrap_or(""));
                if let Some(q) = &style.quotes {
                    counters.set_quotes(q.clone());
                }
            }
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
            let style_scope = (box_level && style.contain_style == Some(true))
                .then(|| counters.enter_style_scope());
            // Содержимое маркера — по первому верному условию css-lists-3
            // §content-property: `content` на `::marker` не `normal` →
            // «exactly as for ::before»; `none` → коробки нет; иначе
            // прежний путь `list-style-*`. Blink делает ту же отсечку
            // первой строкой `ListMarker::MarkerText`:
            // `if (!marker.StyleRef().ContentBehavesAsNormal()) return
            // kNotText;` (`list_marker.cc:159`).
            //
            // Своей коробки у маркера в дереве нет — его рисует
            // `render::list` по `marker_text`/`no_marker`, поэтому
            // содержимое сворачивается в эти поля, а прочие свойства слоя
            // (цвет, шрифт, разрядка) едут в `marker_layer`.
            //
            // CSS Lists 3 §marker-pseudo: the marker is the item's FIRST child,
            // before ::before; counter-* declared on ::marker (with DOM
            // inheritance of counter directives) apply right here, before the
            // marker's own content is evaluated (`marker-counter`).
            if let Some(mut m) = marker_layer {
                inherit_counter_decls(&mut m, Some(&me.counter_style));
                counters.enter_marker();
                language::pseudo(counters, &m, &me, path);
                apply_counter_decls(&m, counters, "", &[], &mut false, &|_, _| 0);
                if let Some(items) = host_content(&m, &me) {
                    let quotes = m.quotes.as_ref();
                    style.marker_text = Some(content_text(
                        &items,
                        counters,
                        &attrs,
                        quotes,
                        me.html_attrs,
                    ));
                    style.no_marker = Some(false);
                } else if m.content_none == Some(true) {
                    style.no_marker = Some(true);
                }
                counters.leave();
                style.marker_layer = Some(Box::new(m));
            }

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
            let keep = style.preserve_newlines.unwrap_or(preserve);
            if let Some(shadow) = &shadow {
                // Плоское дерево (css-shadow-1 §3.3): хост наполняется детьми
                // корня тени вместо светлых — в области стилей тени и с
                // цепочкой предков из одного безликого хоста. Светлые дети
                // попадут в вывод только через `<slot>`.
                walk_children(
                    &shadow.root,
                    &shadow.scope.rules,
                    vars,
                    &shadow.scope.frames,
                    counter,
                    counters,
                    std::slice::from_ref(&shadow.marker),
                    keep,
                    &mut children,
                );
            } else if let Some(slot) = slot_of(handle).filter(|s| !s.assigned.is_empty()) {
                // Слот показывает распределённые узлы, fallback — только без
                // них (HTML §4.12.4). Распределённый ребёнок стилизуется
                // таблицами ОБЛАСТИ ХОСТА и сопоставляется в светлом контексте
                // (предки хоста + хост, светлые братья), а наследует — от
                // слота, своего родителя в плоском дереве (`vars`, `keep`).
                for s in &slot.assigned {
                    let sibs = Sibs {
                        all: &slot.light_all[..],
                        pos: s.elem_pos,
                        is_elem: s.anc.is_some(),
                        rc: Some(&slot.light_all),
                    };
                    walk(
                        &s.node,
                        &slot.outer.rules,
                        vars,
                        &slot.outer.frames,
                        counter,
                        counters,
                        &slot.host_path[..],
                        slot.light_spots[s.light_idx],
                        keep,
                        sibs,
                        &slot.light,
                        &slot.light_spots,
                        s.light_idx,
                        &mut children,
                    );
                }
            } else {
                walk_children(
                    handle, rules, vars, frames, counter, counters, &path2, keep, &mut children,
                );
            }
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
            // css-overflow-5: скроллер со `scroll-marker-group` собирает
            // `::scroll-marker` потомков в группу-соседа (у корня — ребёнка),
            // скроллер без группы их гасит; кнопки `::scroll-button()` — по
            // `content`. Свой маркер элемента кладётся ПОСЛЕ сбора: он идёт
            // в группу ВНЕШНЕГО скроллера, а не в собственную.
            let scroll_pseudos = scroll_marker_pass(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                &tag,
                &style,
                &attrs,
                &mut children,
            );
            if let Some(el) = pseudo_box(
                rules,
                vars,
                counters,
                &me,
                path,
                sibs,
                "scroll-marker",
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
            // CSS Lists §12.4.1: ordinary counters survive into following siblings.
            // Restore only the isolated subtree; remove_stale handles ordinary scopes.
            if let Some(scope) = style_scope {
                counters.leave_style_scope(scope);
            }
            if box_level {
                counters.leave();
            }

            *counter += 1;
            // Неявный якорь псевдоэлемента — порождающий элемент
            // (css-anchor-position-1 §implicit); его `node_id` известен
            // только здесь, после обхода детей: `::before` собран до них,
            // `::after` — после, а номер хозяину даёт этот же счётчик.
            for kid in children.iter_mut() {
                if let Node::Element(c) = kid
                    && c.tag.starts_with("::")
                {
                    c.style.implicit_anchor = Some(*counter);
                }
            }
            // Кадры разрешаются здесь же: к моменту отрисовки таблицы стилей
            // уже нет, а интерполировать нужно готовые стили, а не текст.
            let anim = style.animation.as_ref().and_then(|a| {
                // Набор `name` поверх стиля `base`.
                let resolve = |name: &str, base: &Computed| -> Option<Vec<(f32, Computed)>> {
                    let track = frames.get(name)?;
                    let mut resolved: Vec<(f32, Computed)> = track
                        .iter()
                        .map(|(at, decls)| {
                            let mut c = base.clone();
                            // Кадр ЗАМЕНЯЕТ `transform`, а разбор свойства
                            // дописывает функции к уже стоящим (ветка
                            // `"transform"`: `self.transform.unwrap_or_default()`).
                            if decls.contains_key("transform") {
                                c.transform = None;
                            }
                            c.apply_decls_with_vars(decls, vars);
                            (*at, c)
                        })
                        .collect();
                    // Недостающие `0%`/`100%` строятся из вычисленного стиля
                    // (css-animations-1 §keyframes). Только у остановленной:
                    // живая обёртка с таким кадром сделала бы reftest
                    // недетерминированным (`individual-transform-combine`: пять
                    // наборов из одного `to`).
                    if a.frozen() {
                        if resolved.first().is_some_and(|f| f.0 > 0.0) {
                            resolved.insert(0, (0.0, base.clone()));
                        }
                        if resolved.last().is_some_and(|f| f.0 < 1.0) {
                            resolved.push((1.0, base.clone()));
                        }
                    }
                    (resolved.len() >= 2).then_some(resolved)
                };
                // Несколько остановленных анимаций — слоями по порядку списка
                // (css-animations-1 §3: при общем свойстве побеждает имя,
                // стоящее позже); итог — постоянный набор из двух одинаковых
                // кадров (`individual-transform-ordering`: `anim-7, anim-8`).
                if a.frozen() && a.names.len() > 1 {
                    let t = a.frozen_t();
                    let mut base = style.clone();
                    for name in &a.names {
                        if let Some(track) = resolve(name, &base) {
                            base = crate::render::frame_at(&track, t);
                        }
                    }
                    return Some(vec![(0.0, base.clone()), (1.0, base)]);
                }
                resolve(&a.name, &style)
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
            // Псевдокоробки ВНЕ элемента (css-overflow-5; порядок Blink
            // `kBoxTreeOrder`): группа `before` — перед ним, кнопки
            // (block-start, inline-start, inline-end, block-end) и группа
            // `after` — за ним. У корня всё это — его первый/последние дети
            // (§scroll-marker-group: «first child of the originating
            // element»). Неявный якорь у них — сам элемент.
            let mut sp = scroll_pseudos;
            for el in sp
                .group_before
                .iter_mut()
                .chain(sp.buttons.iter_mut())
                .chain(sp.group_after.iter_mut())
            {
                el.style.implicit_anchor = Some(*counter);
            }
            if tag == "html" {
                if let Some(g) = sp.group_before.take() {
                    children.insert(0, Node::Element(g));
                }
                children.extend(sp.buttons.drain(..).map(Node::Element));
                if let Some(g) = sp.group_after.take() {
                    children.push(Node::Element(g));
                }
            } else if let Some(g) = sp.group_before.take() {
                out.push(Node::Element(g));
            }
            // css-ruby-1 §2.1.2 «Non-Inline Ruby»: `display: block ruby` даёт
            // ДВЕ коробки — главную блочную и строчный контейнер руби внутри
            // (Blink `LayoutRubyAsBlock::AddChild`: первый ребёнок — анонимный
            // `LayoutInline` с `display: ruby`, все дети идут в него). Свойства
            // элемента — на главной коробке; наследуемые доходят до контейнера
            // обычным `inline::inherit` (стиль контейнера пуст). Тег `ruby` у
            // синтетического узла — роль контейнера по тегу (`block-ruby-001`).
            let children = if style.display == Some(Display::Block)
                && style.ruby_role == Some(crate::computed::RubyRole::Container)
            {
                vec![Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    inline: true,
                    tag: "ruby".to_string(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children,
                    attrs: vec![],
                })]
            } else {
                children
            };
            let children = if ruby_box_role(&tag, &style).is_none() {
                wrap_misparented_ruby(children)
            } else {
                children
            };
            let inline = INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
            style.block_tag = !inline;
            // Замена элемента (css-content-3 §content-property: «a single
            // <image>» на самом элементе): коробка становится замещаемой
            // картинкой, содержимое не рисуется. Уровень коробки остаётся от
            // исходного тега и `display` — `<p>` замещается блоком. Корень не
            // трогается: замещаемого корня у нас нет. Ненайденная картинка
            // замены не делает (Servo `replaced.rs:348`: `None` при ошибке).
            let (tag, children, attrs) = match style.content.as_deref() {
                Some([crate::computed::ContentItem::Image(src)])
                    if tag != "html" && content_image_src(src).is_some() =>
                {
                    let mut attrs: Vec<(String, String)> =
                        attrs.into_iter().filter(|(k, _)| k != "src" && k != "srcset").collect();
                    attrs.push(("src".into(), content_image_src(src).unwrap_or_default()));
                    ("img".to_string(), vec![], attrs)
                }
                _ => (tag, children, attrs),
            };
            out.push(Node::Element(Element {
                list_item,
                node_id: *counter,
                anim,
                inline,
                tag,
                style,
                hover,
                first_letter,
                first_line,
                children,
                attrs,
            }));
            out.extend(sp.buttons.into_iter().map(Node::Element));
            if let Some(g) = sp.group_after {
                out.push(Node::Element(g));
            }
        }
        _ => walk_children(
            handle, rules, vars, frames, counter, counters, path, preserve, out,
        ),
    }
}

/// Псевдокоробки скроллера ВНЕ его коробки (css-overflow-5): группа маркеров
/// и кнопки прокрутки. Порядок в дереве — Blink `kBoxTreeOrder`.
struct ScrollPseudos {
    group_before: Option<Element>,
    group_after: Option<Element>,
    /// block-start, inline-start, inline-end, block-end — те, у кого есть
    /// `content`.
    buttons: Vec<Element>,
}

/// Собрать группу маркеров и кнопки элемента; `children` — уже построенные
/// дети (с их `::scroll-marker`), из них маркеры ВЫНИМАЮТСЯ.
///
/// Скроллер (overflow scroll/auto/hidden — Blink `IsScrollContainer`) или
/// корень: со свойством `scroll-marker-group` собирает маркеры потомков, без
/// него — гасит их (§scroll-markers: «nearest ancestor scroll container …
/// not none»). Группа создаётся по одному свойству, даже без правил
/// `::scroll-marker-group` (`scroll-marker-group-015`), блокифицируется и
/// получает `contain: layout` (+ `size` в потоке) поверх авторского
/// (`style_adjuster.cc` 827, 1219–1230; `scroll-marker-007/008`).
#[allow(clippy::too_many_arguments)]
fn scroll_marker_pass(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    tag: &str,
    style: &Computed,
    attrs: &[(String, String)],
    children: &mut Vec<Node>,
) -> ScrollPseudos {
    use crate::computed::Overflow;
    let mut out = ScrollPseudos {
        group_before: None,
        group_after: None,
        buttons: vec![],
    };
    // Кнопки — по `content`, скроллер не обязателен (Blink
    // `CanGeneratePseudoElement`; `scroll-buttons-001` — `div` без overflow).
    // Физическая сторона переводится в логическую по письму элемента
    // (`scroll_button_pseudo_element.cc` PhysicalToLogical).
    if rules.iter().any(|r| {
        r.sel
            .pseudo
            .as_deref()
            .is_some_and(|p| p.starts_with("scroll-button("))
    }) {
        let vertical = style.vertical == Some(true);
        let rl = style.vertical_rl == Some(true);
        let rtl = style.rtl == Some(true);
        for logical in ["block-start", "inline-start", "inline-end", "block-end"] {
            let physical = match (logical, vertical) {
                ("block-start", false) => "up",
                ("block-end", false) => "down",
                ("inline-start", false) => if rtl { "right" } else { "left" },
                ("inline-end", false) => if rtl { "left" } else { "right" },
                ("block-start", true) => if rl { "right" } else { "left" },
                ("block-end", true) => if rl { "left" } else { "right" },
                ("inline-start", true) => if rtl { "down" } else { "up" },
                _ => if rtl { "up" } else { "down" },
            };
            let l = format!("scroll-button({logical})");
            let p = format!("scroll-button({physical})");
            if let Some(el) = pseudo_box_named(
                rules,
                vars,
                counters,
                me,
                path,
                sibs,
                &[l.as_str(), p.as_str(), "scroll-button(*)"],
                &l,
                false,
                attrs,
            ) {
                out.buttons.push(el);
            }
        }
    }
    let scrolls = |o: Option<Overflow>| {
        matches!(o, Some(Overflow::Scroll) | Some(Overflow::Hidden))
    };
    let scroller = tag == "html" || scrolls(style.overflow_x) || scrolls(style.overflow_y);
    let Some(before) = style.scroll_marker_group else {
        if scroller {
            purge_scroll_markers(children);
        }
        return out;
    };
    // Группа — только у скролл-контейнера (§scroll-marker-group-property:
    // «on a scroll container … generates a ::scroll-marker-group»; Blink
    // `CanGeneratePseudoElement`: `IsScrollContainer()`); у `div` без
    // overflow свойство молчит (`scroll-marker-group-010`).
    if !scroller {
        return out;
    }
    let mut markers = vec![];
    {
        let abs_ok = crate::inline::establishes_cb(style);
        let fixed_ok = style.transform.is_some()
            || style.contain_layout == Some(true)
            || style.contain_paint == Some(true);
        collect_scroll_markers(children, abs_ok, fixed_ok, &mut markers);
    }
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref() == Some("scroll-marker-group"))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    let mut gstyle = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    if gstyle.display == Some(Display::None) {
        return out;
    }
    gstyle.display = match gstyle.display {
        Some(Display::InlineBlock) => Some(Display::Block),
        Some(Display::InlineFlex) => Some(Display::Flex),
        Some(Display::InlineGrid) => Some(Display::Grid),
        Some(Display::InlineTable) => Some(Display::Table),
        other => other,
    };
    gstyle.contain_layout = Some(true);
    if !matches!(gstyle.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        gstyle.contain_size = Some(true);
    }
    let group = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        inline: false,
        tag: "::scroll-marker-group".to_string(),
        style: gstyle,
        hover: None,
        first_letter: None,
        first_line: None,
        children: markers.into_iter().map(Node::Element).collect(),
        attrs: vec![],
    };
    if before {
        out.group_before = Some(group);
    } else {
        out.group_after = Some(group);
    }
    out
}

/// Убрать все `::scroll-marker` поддерева (их скроллер без группы), не
/// заходя в уже собранные группы.
fn purge_scroll_markers(nodes: &mut Vec<Node>) {
    nodes.retain(|n| !matches!(n, Node::Element(e) if e.tag == "::scroll-marker"));
    for n in nodes.iter_mut() {
        if let Node::Element(e) = n
            && e.tag != "::scroll-marker-group"
        {
            purge_scroll_markers(&mut e.children);
        }
    }
}

/// Вынуть `::scroll-marker` потомков в порядке документа (§scroll-markers:
/// «tree order of their originating element»): сперва свой маркер хозяина
/// (он лежит последним среди его детей), потом маркеры его потомков. Во
/// вложенные скроллеры не заходим — их маркеры уже собраны или погашены
/// ими самими; в готовые группы и псевдокоробки — тоже. Хозяин с
/// `position: absolute|fixed` считается, только если его содержащий блок
/// внутри скроллера (`abs_ok`/`fixed_ok`): иначе его коробка раскладки
/// лежит снаружи (`scroll-marker-005/006`), и его поддерево гасится.
fn collect_scroll_markers(
    nodes: &mut Vec<Node>,
    abs_ok: bool,
    fixed_ok: bool,
    out: &mut Vec<Element>,
) {
    use crate::computed::Overflow;
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        if e.tag.starts_with("::") {
            continue;
        }
        let inside = match e.style.position {
            Some(Position::Absolute) => abs_ok,
            Some(Position::Fixed) => fixed_ok,
            _ => true,
        };
        let mut kept = Vec::with_capacity(e.children.len());
        for c in e.children.drain(..) {
            match c {
                Node::Element(m) if m.tag == "::scroll-marker" => {
                    if inside {
                        out.push(m);
                    }
                }
                other => kept.push(other),
            }
        }
        e.children = kept;
        if !inside {
            purge_scroll_markers(&mut e.children);
            continue;
        }
        let nested = matches!(
            e.style.overflow_x,
            Some(Overflow::Scroll) | Some(Overflow::Hidden)
        ) || matches!(
            e.style.overflow_y,
            Some(Overflow::Scroll) | Some(Overflow::Hidden)
        );
        if nested || e.style.display == Some(Display::None) {
            continue;
        }
        let abs2 = abs_ok || crate::inline::establishes_cb(&e.style);
        let fixed2 = fixed_ok
            || e.style.transform.is_some()
            || e.style.contain_layout == Some(true)
            || e.style.contain_paint == Some(true);
        collect_scroll_markers(&mut e.children, abs2, fixed2, out);
    }
}

/// Подходит ли значение под синтаксис `@property` (css-properties-values-api-1
/// §5). Проверяются однозначные типы; значение с `var()` решается позже и
/// принимается; незнакомый синтаксис — тоже (лучше принять, чем потерять).
fn syntax_accepts(syntax: &str, value: &str) -> bool {
    let v = value.trim();
    if v.contains("var(") || syntax.trim() == "*" {
        return true;
    }
    let one = |ty: &str| -> bool {
        match ty.trim() {
            "<color>" => {
                crate::value::Color::parse(v).is_some()
                    || v.eq_ignore_ascii_case("currentcolor")
                    || v.to_ascii_lowercase().starts_with("light-dark(")
            }
            "<length>" => {
                !v.ends_with('%')
                    && (v == "0" || matches!(crate::value::Len::parse_mixed(v), Some(l) if !matches!(l, crate::value::Len::Pct(_) | crate::value::Len::Auto)))
            }
            "<length-percentage>" => crate::value::Len::parse_mixed(v)
                .is_some_and(|l| l != crate::value::Len::Auto),
            "<percentage>" => v.ends_with('%') && v[..v.len() - 1].trim().parse::<f32>().is_ok(),
            "<number>" => v.parse::<f32>().is_ok(),
            "<integer>" => v.parse::<i64>().is_ok(),
            _ => true,
        }
    };
    syntax.split('|').any(one)
}

/// Адрес картинки из `content: url()` в форме `src` для `<img>`: загрузчик
/// ждёт `file:///` с прямыми косыми (как пишет стенд для `<img src>`), а
/// разбор стиля отдаёт голый путь. `None` — файла нет: такая картинка коробки
/// не даёт (Servo `components/layout/replaced.rs:348`).
pub(crate) fn content_image_src(src: &str) -> Option<String> {
    if src.starts_with("data:") {
        return Some(src.to_string());
    }
    let path = src.trim_start_matches("file:///");
    if !std::path::Path::new(path).is_file() {
        return None;
    }
    Some(format!("file:///{path}").replace('\\', "/"))
}

/// Коробка псевдоэлемента `::before`/`::after`, если правила её создают.
///
/// В CSS это настоящий потомок с собственным стилем; так его и собираем —
/// обычным инлайновым элементом с текстовым содержимым. `attr(имя)`
/// подставляется значением атрибута хозяина.
// ★ Прошлый заход (06.09) на слой `::marker` был откачен «848 -> 847,
// +3/-4»: терялись `disclosure-styles`, `marker-counter`,
// `marker-content-020`, `marker-text-transform-default`. Три корня потерь
// названы и закрыты здесь же: таблица агента маркера (`text-transform:
// none`, `unicode-bidi: isolate` — css-lists-3 §marker-properties),
// исполнение `counter-*` слоя в собственном сегменте маркера и снятие
// маркера у пункта с чужим `display`. Разбор — `target/scout-markers-
// 2026-09b.md` §1.
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
    pseudo_box_named(
        rules,
        vars,
        counters,
        me,
        path,
        sibs,
        &[which],
        which,
        which == "before",
        attrs,
    )
}

/// То же, но правила отбираются по ЛЮБОМУ из имён `names`: у кнопки
/// прокрутки `::scroll-button(right)`, `::scroll-button(inline-end)` и
/// `::scroll-button(*)` — одна коробка (css-overflow-5 §scroll-buttons,
/// `scroll-buttons-003`). `tag` — имя коробки (`::{tag}`), `before` — вести
/// счётчики как у `::before`.
#[allow(clippy::too_many_arguments)]
fn pseudo_box_named(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    names: &[&str],
    tag: &str,
    before: bool,
    attrs: &[(String, String)],
) -> Option<Element> {
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref().is_some_and(|p| names.contains(&p)))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let mut style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    inherit_counter_decls(&mut style, Some(&me.counter_style));
    // Псевдоэлемент — ребёнок хозяина: блочный `::before` внутри руби
    // инлайнизируется так же, как элемент (css-ruby-1 §2.2 п.1,
    // `ruby-inlinize-blocks-005`). Ближайший предок — сам хозяин.
    inlinify_in_ruby(&mut style, "", std::iter::once(me).chain(path.iter().rev()));
    // Нет содержимого или коробки — нет и псевдоэлемента: его директивы
    // счётчиков тогда не действуют вовсе (у него нет объекта раскладки).
    let list = resolve_content_attributes(style.content.as_ref()?, attrs, me.html_attrs)?;
    if style.display == Some(Display::None) {
        return None;
    }
    // Pseudo counters occupy their own level among the host children.
    counters.enter_pseudo(before);
    language::pseudo(counters, &style, me, path);
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
    // Составляющие идут по порядку: подряд идущие текстовые склеиваются в
    // один текстовый узел, `url()` становится строчным `<img>` между ними.
    // Ненайденная картинка коробки НЕ даёт вовсе — как в Servo
    // (`components/layout/dom_traversal.rs:398`: `from_image` → `None` при
    // ошибке загрузки, и элемент пропускается). Прошлые заходы давали ей
    // коробку и теряли `before-after-images-001` и `-table-whitespace-001`.
    let mut children: Vec<Node> = vec![];
    let mut run: Vec<crate::computed::ContentItem> = vec![];
    let flush = |run: &mut Vec<crate::computed::ContentItem>,
                 children: &mut Vec<Node>,
                 counters: &mut crate::counters::Counters| {
        if !run.is_empty() {
            let t = content_text(
                run,
                counters,
                attrs,
                style.quotes.as_ref(),
                me.html_attrs,
            );
            children.push(Node::Text(t));
            run.clear();
        }
    };
    for item in &list {
        if let crate::computed::ContentItem::Image(src) = item {
            flush(&mut run, &mut children, counters);
            if let Some(src) = content_image_src(src) {
                children.push(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "img".into(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: vec![],
                    attrs: vec![("src".into(), src)],
                    inline: true,
                }));
            }
        } else {
            run.push(item.clone());
        }
    }
    flush(&mut run, &mut children, counters);
    if children.is_empty() {
        children.push(Node::Text(String::new()));
    }
    let list_item =
        (style.display == Some(Display::ListItem)).then(|| counters.value_of("list-item"));
    counters.leave();
    Some(Element {
        list_item,
        // Псевдоэлемент своей анимации не несёт: правило `::before` задаёт
        // содержимое, а не движение.
        node_id: 0,
        anim: None,
        inline: !matches!(
            style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ),
        tag: format!("::{tag}"),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: vec![],
    })
}

/// Сопоставление селектора с узлом и его цепочкой предков.
///
/// `sibs` — предыдущие соседи-элементы узла в порядке разметки: по ним
/// решаются соседние комбинаторы `+` и `~`.
pub(crate) fn matches(sel: &Selector, me: &Ancestor, path: &[Ancestor], sibs: Sibs) -> bool {
    // Безликий хост (изнутри своей тени): предков и братьев у него в этой
    // области нет, совпадает лишь `:host`-компаунд (см. `matches_compound`).
    if me.featureless.is_some() {
        return sel.ancestor.is_none() && sel.prev.is_none() && matches_compound(sel, me);
    }
    if let Some(pseudo) = &sel.pseudo {
        // `:host` вне тени «matches nothing» (css-shadow-1 §3.1).
        if is_host_pseudo(pseudo) {
            return false;
        }
        if pseudo.starts_with("has-slotted") {
            if !has_slotted_holds(pseudo, me) {
                return false;
            }
            return matches_ignoring_pseudo(sel, me, path, sibs);
        }
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
    let Some(lang) = language::effective(me, path) else {
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
    if pseudo.starts_with("has-slotted") {
        return has_slotted_holds(pseudo, me);
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

fn is_host_pseudo(pseudo: &str) -> bool {
    pseudo == "host" || pseudo.starts_with("host(")
}

/// `:host` / `:host(S)` на безликом хосте: голый совпадает всегда, с
/// аргументом — если хост В СВОЁМ СВЕТЛОМ КОНТЕКСТЕ совпадает с S
/// (css-shadow-1 §3.1 «in its normal context»; Blink `CheckPseudoHost`
/// сопоставляет в `element->GetTreeScope()`). Светлых братьев здесь нет:
/// `:first-child` в аргументе решается по `spot`, of-форма и `+`/`~` — нет.
fn host_holds(pseudo: &str, node: &Ancestor) -> bool {
    if pseudo == "host" {
        return true;
    }
    let (Some(arg), Some(light)) = (
        pseudo.strip_prefix("host(").and_then(|r| r.strip_suffix(')')),
        &node.featureless,
    ) else {
        return false;
    };
    let Some(arg) = Selector::parse(arg) else {
        return false;
    };
    let real = Ancestor {
        featureless: None,
        ..node.clone()
    };
    matches(&arg, &real, &light[..], Sibs::EMPTY)
}

/// `:has-slotted` — у слота непуст список ПЛОСКИХ распределённых, включая
/// текст (`has-slotted-001` зелёная от одних пробелов); `:has-slotted(S)` —
/// среди них есть ЭЛЕМЕНТ, совпадающий с S в своём светлом контексте
/// (`functional-007`: `div + div` смотрит на светлых братьев). Не слот или
/// слот вне тени — не совпадает.
fn has_slotted_holds(pseudo: &str, me: &Ancestor) -> bool {
    let Some(slot) = &me.slot else { return false };
    let Some(arg) = pseudo
        .strip_prefix("has-slotted(")
        .and_then(|r| r.strip_suffix(')'))
    else {
        return !slot.flattened.is_empty();
    };
    let list: Vec<Selector> = crate::css::split_selector_list(arg)
        .into_iter()
        .filter_map(Selector::parse)
        .collect();
    slot.flattened.iter().flatten().any(|c| {
        let sibs = Sibs {
            all: &c.all[..],
            pos: c.pos,
            is_elem: true,
            rc: None,
        };
        list.iter().any(|s| matches(s, &c.anc, &c.path[..], sibs))
    })
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
    if direct {
        return !path.is_empty() && ancestor_holds(parent_sel, path, path.len() - 1);
    }
    (0..path.len()).rev().any(|i| ancestor_holds(parent_sel, path, i))
}

/// Предок `path[at]` — предмет компаунда `sel` вместе с его соседним
/// комбинатором и цепочкой выше.
///
/// Сосед ПРЕДКА (`div + div span`, Selectors-4 §16.3/§16.4) проверяется по
/// братьям предка, сохранённым в его паспорте (`Ancestor::peers`): у соседа
/// те же предки — `path[..at]`. Прежде такое правило не совпадало никогда
/// (`ch-unit-001`: ширина `div + div span` терялась). Без сохранённых братьев
/// (обход вне `walk`) — честно не совпадает, как раньше.
fn ancestor_holds(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    if !matches_ancestor_compound(sel, &path[at]) {
        return false;
    }
    if sel.prev.is_some() {
        let Some((all, pos)) = &path[at].peers else {
            return false;
        };
        // Предок сопоставляется ПОЛНОСТЬЮ, как предмет: псевдоклассы его
        // компаунда (`* ~ :root div` — у корня братьев нет), соседи и цепочка
        // выше; братья предка делят с ним предков `path[..at]`.
        let sibs = Sibs {
            all: &all[..],
            pos: *pos,
            is_elem: true,
            rc: Some(all),
        };
        return matches(sel, &path[at], &path[..at], sibs);
    }
    matches_chain(sel, path, at)
}

/// Продолжение цепочки вверх для `.a .b .c`.
fn matches_chain(sel: &Selector, path: &[Ancestor], at: usize) -> bool {
    let Some(anc) = &sel.ancestor else {
        return true;
    };
    let (parent_sel, direct) = (&anc.0, anc.1);
    if direct {
        return at > 0 && ancestor_holds(parent_sel, path, at - 1);
    }
    (0..at).rev().any(|i| ancestor_holds(parent_sel, path, i))
}

/// Компаунд ПРЕДКА: как `matches_compound`, но псевдоклассы действия
/// пользователя на нём не выполняются.
fn matches_ancestor_compound(sel: &Selector, node: &Ancestor) -> bool {
    // Псевдоклассы действия пользователя у НЕ-предметного компаунда
    // (`grid:hover item[style]`): в неподвижном кадре ни наведения, ни
    // нажатия нет (selectors-4 §user-action: «matches while the user
    // designates an element»), а пропуск делал предка всегда наведённым —
    // правило красило всех потомков. Слой наведения строится только для
    // предметного `:hover` (`dom.rs`, `pseudo == "hover"`).
    let user_action = |p: &str| matches!(p, "hover" | "active");
    if sel.pseudo.as_deref().is_some_and(user_action) || sel.also.iter().any(|p| user_action(p)) {
        return false;
    }
    matches_compound(sel, node)
}

fn matches_compound(sel: &Selector, node: &Ancestor) -> bool {
    // Безликий хост: ни тег, ни `*`, ни класс, ни атрибут его не берут —
    // только `:host`/`:host(S)`, и все псевдоклассы компаунда обязаны быть
    // такими (`:host:host` — да, `div:host`, `:host.host` — нет;
    // `selectors/featureless-002`).
    if node.featureless.is_some() {
        let bare = sel.tag.is_none()
            && !sel.universal
            && sel.id.is_none()
            && sel.classes.is_empty()
            && sel.attrs.is_empty();
        return bare
            && sel.pseudo.as_deref().is_some_and(|p| host_holds(p, node))
            && sel.also.iter().all(|p| host_holds(p, node));
    }
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
    fn declarative_shadow_scopes_styles_and_slots() {
        let red = crate::value::Color::parse("red");
        let green = crate::value::Color::parse("green");
        // Плоские дети хоста: `b` тени (её правило, не документное) и слот
        // (правило тени). Документное `b { red }` в тень не протекает.
        let colors = child_colors(
            "<style>b { color: red } slot { color: green }</style>             <div id=\"box\"><template shadowrootmode=\"open\"><style>b { color: green } slot { color: red }</style>             <b></b><slot></slot></template><i></i></div>",
        );
        assert_eq!(colors, vec![green, red]);
        // Распределённый ребёнок стилизуется таблицей ДОКУМЕНТА, не тени.
        let colors = child_colors(
            "<style>i { color: green }</style>             <div><template shadowrootmode=\"open\"><style>i { color: red }</style>             <slot id=\"box\"></slot></template><i></i></div>",
        );
        assert_eq!(colors, vec![green]);
        // `:host` красит хост, `:has-slotted` — слот с распределёнными.
        let colors = child_colors(
            "<div id=\"box\"><div><template shadowrootmode=\"open\"><style>:host { color: green } slot { color: red } :has-slotted { color: green }</style>             <slot></slot></template><i></i></div></div>",
        );
        assert_eq!(colors, vec![green]);
        // Обычный `<template>` как прежде не рисуется.
        let colors = child_colors("<div id=\"box\"><template><b></b></template><i></i></div>");
        assert_eq!(colors, vec![None]);
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
