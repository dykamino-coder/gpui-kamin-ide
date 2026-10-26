//! Базовый лист UA и добавка quirks; объявления сохраняют исходный порядок каскада.

/// Стиль по умолчанию для тега — то, что браузер берёт из своей таблицы.
/// Без него `<b>` не жирный, а `<h1>` неотличим от абзаца.
/// Проверено: браузерные значения вместо чатовых (`pre { padding: 8px }`,
/// `td { padding: 4px 8px }`) здесь ничего не меняют — стенд подаёт
/// `BROWSER_CSS` вторым листом того же происхождения, и по порядку каскада
/// выигрывает он; красное в `white-space-pre-*`, `text-indent-intrinsic-*`,
/// `border-style-applies-to-*`, `table-visual-layout-*` держит другое.
pub(crate) fn user_agent_css() -> &'static str {
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
    button { background: ButtonFace; border: 1px solid ButtonBorder; color: ButtonText }
    *::scroll-button(*) { background: ButtonFace; border: 1px solid ButtonBorder; color: ButtonText }
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
    *::scroll-marker { color: #8ab4f8; text-decoration: underline }
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
    input, textarea, select { color: FieldText }
    textarea { font: 13.3333px monospace; overflow: auto }
    button { padding: 4px 10px; border-radius: 4px }
    /* Руби (css-ruby-1, Appendix A.1): скобки `rp` — только для движков без
       руби; аннотация вполовину кегля, одной строкой, без знака акцента.
       Пара правил равносильна спековому `rtc, :not(rtc) > rt { font-size: 50% }`.
       `unicode-bidi: isolate` пока не ставится — мерить отдельно (`ruby-bidi-001`). */
    /* css-content-3 §4.2, HTML §15.3.3: `q` берёт кавычки из `quotes`. */
    q::before { content: open-quote }
    q::after { content: close-quote }
    rp { display: none }
    ruby, rt { text-indent: 0 }
    rb, rt, rtc { white-space: nowrap }
    rt, rtc { font-size: 50%; line-height: 1; text-emphasis: none; text-justify: ruby }
    rtc > rt { font-size: 100% }
    /* Языковые правила A.1: чжуинь (zh-TW) — 30% кегля, у китайского
       аннотация по центру. Без них строка под аннотацию росла на кегль
       50% (`ruby-lang-specific-style-001`). */
    rt:lang(zh-TW), rtc:lang(zh-TW) { font-size: 30% }
    rtc:lang(zh-TW) > rt { font-size: 100% }
    rt:lang(zh), rtc:lang(zh) { ruby-align: center }
    /* HTML §15.5.14 (rendering, the marquee element): строчно-блочная
       коробка со срезом; без правила `marquee` оставался строчным и терял
       `width/height` (`positioned-grid-items-023`: красный квадрат 50%). */
    marquee { display: inline-block; text-align: initial; overflow: hidden }
    "#
}

/// HTML rendering, lists: orphan list items use inside markers in quirks mode.
pub(crate) const QUIRKS_CSS: &str = r#"
/* Quirks Mode §3.12, HTML rendering §tables. */
table { color: initial }
li { list-style-position: inside }
li :is(dir, menu, ol, ul) { list-style-position: outside }
:is(dir, menu, ol, ul) :is(dir, menu, ol, ul, li) { list-style-position: unset }
"#;
