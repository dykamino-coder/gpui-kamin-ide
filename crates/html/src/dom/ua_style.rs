//! Ua style for dom; split out to keep the owning module within 250 lines.

/// Теги, которым таблица агента даёт блочный вид (HTML §15.3.2-15.3.12).
///
/// Правило для НЕИЗВЕСТНОГО тега — строчный: своей записи в листе агента у
/// него нет, а начальное значение `display` — `inline`. Прежде блочным
/// становилось всё, чего нет в `INLINE_TAGS`, и `<foo>` внутри абзаца рвал
/// строку (`line-breaking-font-size-zero-001`).
pub(super) const BLOCK_TAGS: &[&str] = &[
    "html",
    "body",
    "address",
    "blockquote",
    "center",
    "div",
    "figure",
    "figcaption",
    "footer",
    "form",
    "header",
    "hr",
    "legend",
    "listing",
    "main",
    "p",
    "plaintext",
    "pre",
    "xmp",
    "article",
    "aside",
    "h1",
    "h2",
    "h3",
    "h4",
    "h5",
    "h6",
    "hgroup",
    "nav",
    "section",
    "search",
    "dir",
    "dd",
    "dl",
    "dt",
    "ol",
    "ul",
    "menu",
    "li",
    "table",
    "caption",
    "colgroup",
    "col",
    "thead",
    "tbody",
    "tfoot",
    "tr",
    "td",
    "th",
    "fieldset",
    "details",
    "summary",
    "dialog",
    "optgroup",
    "option",
    "frameset",
    "frame",
    "noframes",
    "head",
    "title",
    "meta",
    "link",
    "base",
    "script",
    "style",
    "noscript",
    "template",
    "slot",
    "map",
    "area",
    "source",
    "track",
    "param",
];

/// Стиль по умолчанию для тега — то, что браузер берёт из своей таблицы.
/// Без него `<b>` не жирный, а `<h1>` неотличим от абзаца.
/// ПРОВЕРЕНО ДВАЖДЫ: этот лист чатовый (`pre { padding: 8px }`,
/// `td { padding: 4px 8px }`), но на своде он не сказывается НИ НА ЧЁМ —
/// стенд подаёт `BROWSER_CSS` вторым листом того же происхождения, и по
/// порядку каскада выигрывает он. Проба с браузерными значениями здесь:
/// 71 пара семей `white-space-pre-*`, `text-indent-intrinsic-*`,
/// `border-style-applies-to-*`, `table-visual-layout-*` — ни одного сдвига.
/// Красное в этих семьях держит что-то другое.
pub(super) fn user_agent_css() -> &'static str {
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
