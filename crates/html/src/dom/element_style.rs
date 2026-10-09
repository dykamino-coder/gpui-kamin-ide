//! Доводка стиля элемента: направление, руби, строчный display, авто-пропорция, размеры из атрибутов.

use crate::dom::*;

/// Направление письма, заданное АТРИБУТОМ: `<div dir="rtl">`.
///
/// В разметке направление задают именно атрибутом, а не стилем: он и есть
/// обычный способ написать страницу справа налево. Тег `<bdo>` вдобавок
/// ОТМЕНЯЕТ разбор двунаправленности — знаки идут ровно в заданную сторону.
pub(crate) fn apply_direction(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
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
pub(crate) fn inlinify_in_ruby<'a>(
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

pub(crate) fn finish_inline_display(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
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
pub(crate) fn promote_auto_ratio(style: &mut Computed, tag: &str) {
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

pub(crate) fn apply_presentational_size(style: &mut Computed, tag: &str, attrs: &[(String, String)]) {
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
    // Таблица замещаемой не является: её height уже прошёл каскад намёков
    // (HTML §15.3.8), а `attr_*` держит соотношение сторон замещаемого и
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
