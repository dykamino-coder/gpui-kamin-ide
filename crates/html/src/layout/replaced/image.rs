//! Картинки.
// owner: A

use crate::dom::Element;
use crate::layout::block::containing::{AVAIL_W, CB_WIDTH};
use crate::layout::replaced::limits::css2_replaced_limits;
use crate::layout::replaced::{replaced_content, replaced_holder_ratio, replaced_used_style};
use crate::render::styled_div;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, div, px};

pub(crate) fn image(e: &Element) -> AnyElement {
    image_with(e, None)
}

/// Доля ВЫСОТЫ замещаемого, сведённая к точкам по содержащему блоку.
///
/// Держатель картинки стоит внутри анонимного ряда строки
/// (`inline::as_wrapped_row`): у ряда высота `auto`, элемент прижат по базовой
/// линии и не растягивается, поэтому доля высоты бралась ОТ РЯДА и разрешалась
/// в ноль — картинка не рисовалась ни одной точкой
/// (`background-image-cover-002-ref`). Содержащий блок здесь известен точно —
/// это `inherited`, и доля сводится к точкам ещё на сборке.
///
/// Только обычный блочный контейнер: у гибкого, сеточного и лунок высота
/// приходит от раскладки, и подстановка ломает `row-auto-repeat-auto-023`.
pub(crate) fn pct_height_to_px(e: &Element, inherited: &Computed) -> Element {
    let e = &pct_limits_to_px(e, inherited);
    let (Some(Len::Pct(k)), Some(Len::Px(h))) = (e.style.height, inherited.height) else {
        return e.clone();
    };
    // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда и `inline-block` — 0 и 0 по обоим
    // сводам, `sizing-percentages-replaced-orthogonal-001` не сдвинулась.
    // Пущен снова (03.10): картинка СТРОКИ внутри `inline-block` с высотой
    // в точках рисовалась природным размером — доля высоты до раскладки
    // не доезжала (`intrinsic-percent-replaced-021`: `<button>` 100px,
    // картинка 200×200 вместо 100×100; проба: тот же `div` блоком — верно).
    if !matches!(
        inherited.display,
        None | Some(Display::Block) | Some(Display::InlineBlock)
    ) {
        return e.clone();
    }
    // `height` в разборе — высота СОДЕРЖИМОГО (§10.6.2, content-box): отступы
    // и рамку прибавляет уже раскладка. Вычитать их отсюда нельзя — картинка
    // выходила на 6 точек короче (0.91 вместо 0.00 на `cover-002`).
    if h <= 0.0 {
        return e.clone();
    }
    // …но при `box-sizing: border-box` у РОДИТЕЛЯ заданная высота включает
    // его отступы и рамку, а опора доли — содержимое (CSS 2.1 §10.5,
    // css-sizing-3 §box-sizing): `.inner {box-sizing: border-box; height: 50px;
    // padding-top: 25px}` даёт холсту 25, а не 50
    // (`intrinsic-percent-replaced-012/013`).
    let h = if inherited.border_box == Some(true) {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = inherited.borders();
        (h - px_of(inherited.padding.top)
            - px_of(inherited.padding.bottom)
            - px_of(b.top)
            - px_of(b.bottom))
        .max(0.0)
    } else {
        h
    };
    let mut copy = e.clone();
    copy.style.height = Some(Len::Px(k * h));
    copy
}

/// Пределы замещаемого в ДОЛЯХ — в точки от содержащего блока.
///
/// Размер рисунка считается на сборке (`image_with`), и доля там уже не с чем
/// сравнивать: `max-width: 100%` у картинки отбрасывался целиком, и она
/// вылезала за родителя (css-sizing-3 §5: пределы решаются от содержащего
/// блока, как и сам размер). Ширина берётся от ширины содержащего блока,
/// высота — от его высоты и только у обычного блочного контейнера: у гибкого
/// и сеточного высота приходит от раскладки (та же оговорка, что у
/// `pct_height_to_px`).
pub(crate) fn pct_limits_to_px(e: &Element, inherited: &Computed) -> Element {
    let of = |l: Option<Len>, base: Option<Len>| match (l, base) {
        (Some(Len::Pct(k)), Some(Len::Px(b))) if b > 0.0 => Some(Len::Px(k * b)),
        _ => l,
    };
    let block_cb = matches!(inherited.display, None | Some(Display::Block));
    let h_base = block_cb.then_some(inherited.height).flatten();
    let mut copy = e.clone();
    copy.style.max_width = of(e.style.max_width, inherited.width);
    copy.style.min_width = of(e.style.min_width, inherited.width);
    copy.style.max_height = of(e.style.max_height, h_base);
    copy.style.min_height = of(e.style.min_height, h_base);
    copy
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (11.09, `scout-flexreplaced-2026-09.md`):
// «использованный размер замещаемого достаётся КОРОБКЕ, а не только
// рисунку» — предпочтительное соотношение на коробку элемента,
// упругий рисунок в гибком контейнере, выведенная сторона в коробку.
// Разбор скаута верен и подтверждён тремя снимками (фон коробки
// стоит верно, рисунок остаётся полоской; зелёный квадрат 200×200
// вместо 100×100), спека — css-flexbox-1 §9.2 п. B, §9.8 п.3, §9.4,
// Blink `ComputeReplacedSizeInternal` + `GetReplacedAspectRatio`.
// Широкий срез 3112 пар (гибкие коробки с картинками), база тем же
// списком: 2367 -> 2367. Вариант из четырёх блоков — **+0 / −0**,
// полный из шести — **+1 / −1**: взята
// `image-fractional-height-with-wide-aspect-ratio`, потеряна
// `box-sizing-007` 0.00 -> 2.50. Ожидание скаута было +6…+10.
// Возвращать только с разбором `box-sizing-007`: соотношение на
// коробку складывается с `border-box` в двойной учёт рамок.
/// То же, но с базовым кеглем для разрешения долей: атом строится от СЫРОГО
/// стиля, и `padding-right: 1em` без разрешения терялся вовсе
/// (wm-propagation-body-040: сосед вставал на 16 точек левее эталона).
/// `object-view-box` (css-images-4 §object-view-box, `csswg-drafts/css-images-4/
/// Overview.bs` «object-view-box»): видимая часть природного объекта
/// становится его НОВЫМ природным размером, а рисуется только она — вырез
/// растягивается на коробку (`object-fit: fill`). Вид записи сводится к
/// прямоугольнику в точках природного растра: `inset(t r b l)`, `rect(t r b l)`
/// (правый и нижний края от левого/верхнего края объекта), `xywh(x y w h)`.
/// Коробка: размер из обособления (`contain-intrinsic-size`) или заданный,
/// иначе из выреза — с соотношением выреза для одной заданной стороны.
/// Только растр (`background::Source::Raster`); иначе — прежний путь.
pub(crate) fn view_boxed(e: &Element, vb: (u8, [Len; 4])) -> Option<AnyElement> {
    let src = e.attr("src")?;
    let local = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))
        .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
    let source = crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))?;
    let nat = source.intrinsic();
    let (w0, h0) = (nat.w?, nat.h?);
    let crate::paint::background::Source::Raster(ready) = source else {
        return None;
    };
    let at = |l: Len, base: f32| match l {
        Len::Px(v) => v,
        Len::Pct(k) => k * base,
        _ => 0.0,
    };
    let [a, b, c, d] = vb.1;
    let (vx, vy, vw, vh) = match vb.0 {
        // inset(top right bottom left)
        0 => (at(d, w0), at(a, h0), w0 - at(d, w0) - at(b, w0), h0 - at(a, h0) - at(c, h0)),
        // rect(top right bottom left)
        1 => (at(d, w0), at(a, h0), at(b, w0) - at(d, w0), at(c, h0) - at(a, h0)),
        // xywh(x y w h)
        _ => (at(a, w0), at(b, h0), at(c, w0), at(d, h0)),
    };
    if !(vw > 0.0 && vh > 0.0) {
        return None;
    }
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (bw, bh) = if e.style.contains_width() || e.style.contains_height() {
        (
            px_of(e.style.width).or(e.style.contain_intrinsic.0).unwrap_or(0.0),
            px_of(e.style.height).or(e.style.contain_intrinsic.1).unwrap_or(0.0),
        )
    } else {
        match (px_of(e.style.width), px_of(e.style.height)) {
            (Some(w), Some(h)) => (w, h),
            (Some(w), None) => (w, w * vh / vw),
            (None, Some(h)) => (h * vw / vh, h),
            (None, None) => (vw, vh),
        }
    };
    let mut boxed = e.clone();
    boxed.style.object_view_box = None;
    boxed.style.width = Some(Len::Px(bw));
    boxed.style.height = Some(Len::Px(bh));
    boxed.style.contain_size = Some(false);
    boxed.style.contain_inline_size = Some(false);
    let (sx, sy) = (bw / vw, bh / vh);
    let picture = gpui::img(ready).preserve_natural_pixels(true)
        .absolute()
        .left(px(-vx * sx))
        .top(px(-vy * sy))
        .w(px(w0 * sx))
        .h(px(h0 * sy))
        .object_fit(gpui::ObjectFit::Fill);
    let window = div().size_full().relative().overflow_hidden().child(picture);
    Some(styled_div(&boxed).flex_shrink_0().child(window).into_any_element())
}

pub(crate) fn image_with(e: &Element, base_font: Option<f32>) -> AnyElement {
    // Ключевое слово содержимого в оси замещаемого — его природный (или
    // перенесённый через соотношение) размер, то есть `auto` (css-sizing-3
    // §5.1: «When the box has a preferred aspect ratio, size constraints in
    // the opposite dimension will transfer through»; Blink
    // `ComputeReplacedSizeInternal`): `width: min-content; height: 100px` —
    // ширина из соотношения (`intrinsic-size-017…025`).
    let normalized = replaced_used_style::normalize(e, AVAIL_W.get());
    let e = normalized.as_ref().unwrap_or(e);
    let src = e.attr("src").unwrap_or_default();
    // Размеры коробки ставит общий разбор стиля (`apply`): он же добавляет к
    // заданной ширине отступы и рамку, потому что раскладка под нами считает
    // размер по внешнему краю, а CSS по умолчанию — по содержимому. Ставить
    // ширину ЕЩЁ РАЗ отсюда нельзя: она затирала эту поправку, и картинка с
    // `padding-left` вылезала за край на величину отступа.
    let resolved;
    let e = if let Some(base) = base_font {
        let mut copy = e.clone();
        copy.style.resolve_em(base);
        resolved = copy;
        &resolved
    } else {
        e
    };
    if let Some(vb) = e.style.object_view_box
        && let Some(el) = view_boxed(e, vb)
    {
        return el;
    }
    let d = styled_div(e);
    // Замещаемый элемент в СТРОКЕ не сжимается: браузер даёт строке
    // переполниться или перенести коробку целиком (CSS 2.1 §10.3.2, замер
    // wm-propagation-body-040: картинка 340px ужималась на 6-7%). А вот
    // элемент ГИБКОГО РЯДА сжимается как всякий другой — `flex-shrink` ему
    // уже посчитан выше по файлу (единица в гибком окружении, ноль вне его),
    // и глушить его здесь значило запрещать картинке ужиматься даже при
    // явных `min-width: 0` и `flex-shrink: 1` (css-flexbox-1 §7.2).
    let mut d = d;
    match e.style.flex_shrink {
        Some(k) => d.style().flex_shrink = Some(k),
        None => d = d.flex_shrink_0(),
    }
    let d = d;
    // Обособление размера меряет замещаемый элемент КАК ПУСТОЙ (css-contain-2
    // §size containment): своих размеров у картинки нет вовсе — ни сторон, ни
    // соотношения, — их задаёт `contain-intrinsic-size`. Врезка ранняя: ниже
    // по ветке `intrinsic()` вернул бы настоящие 100×100, и всё посчиталось бы
    // по ним.
    if e.style.contains_width() || e.style.contains_height() {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = e.style.borders();
        let pad_x =
            side(e.style.padding.left) + side(e.style.padding.right) + side(b.left) + side(b.right);
        let pad_y =
            side(e.style.padding.top) + side(e.style.padding.bottom) + side(b.top) + side(b.bottom);
        let used = |explicit: Option<Len>, ci: Option<f32>| match explicit {
            Some(Len::Px(v)) => v,
            _ => ci.unwrap_or(0.0),
        };
        // Обособление снимает ПРИРОДНОЕ соотношение, но не ЗАЯВЛЕННОЕ:
        // css-contain-2 §size containment, «Size containment only suppresses
        // the natural aspect ratio, so properties like 'aspect-ratio' which
        // affect that preferred aspect ratio directly are honored»
        // (Overview.bs:659-662), и пример там же (:737-752):
        // `img{width:100px;aspect-ratio:1/1;contain:size}` = 100×100, а без
        // объявленного соотношения — 100×0. Пара атрибутов `width`/`height`
        // разметки — это `aspect-ratio: auto <ratio>` (HTML Rendering
        // §attributes for embedded content): природная его половина снята,
        // заявленная осталась. Blink делает ровно так —
        // `BlockNode::GetReplacedAspectRatio` (`block_node.cc:1328`):
        // заявленное отдаётся ДО проверки обособления, гейт
        // `ShouldApplyAnySizeContainment` стоит только вокруг природного.
        let ratio = e
            .style
            .aspect_ratio
            .filter(|r| r.is_finite() && *r > 0.0)
            .or_else(|| match (e.style.attr_width, e.style.attr_height) {
                (Some(Len::Px(aw)), Some(Len::Px(ah))) if aw > 0.0 && ah > 0.0 => Some(aw / ah),
                _ => None,
            });
        let named = |l: Option<Len>| matches!(l, Some(Len::Px(_)));
        let (nw, nh) = (named(e.style.width), named(e.style.height));
        let cw = match ratio {
            Some(r) if !nw && nh => used(e.style.height, e.style.contain_intrinsic.1) * r,
            _ => used(e.style.width, e.style.contain_intrinsic.0),
        };
        let ch = match ratio {
            Some(r) if nw && !nh => used(e.style.width, e.style.contain_intrinsic.0) / r,
            _ => used(e.style.height, e.style.contain_intrinsic.1),
        };
        let mut d = d;
        if e.style.contains_width() && matches!(e.style.width, None | Some(Len::Auto)) {
            d = d.w(px(cw + pad_x));
        }
        if e.style.contains_height() && matches!(e.style.height, None | Some(Len::Auto)) {
            d = d.h(px(ch + pad_y));
        }
        // Источник берётся тем же путём, что и вне обособления: строку со
        // схемой `file:` система уходит скачивать, и картинка молча не
        // рисуется. Растр декодируется своим декодером, вектор растрируется
        // в уже посчитанный размер.
        let local = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))
            .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
        let mut image = match crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style)) {
            Some(crate::paint::background::Source::Vector { markup, .. }) if cw > 0.0 && ch > 0.0 => {
                match crate::svg::rasterize(&markup, cw, ch) {
                    Some(r) => gpui::img(r),
                    None => gpui::img(SharedString::from(src.to_string())),
                }
            }
            Some(crate::paint::background::Source::Raster(ready)) => gpui::img(ready).preserve_natural_pixels(true),
            _ => match local {
                Some(path) => gpui::img(std::path::PathBuf::from(path)),
                None => gpui::img(SharedString::from(src.to_string())),
            },
        };
        // Подгонка содержимого замещаемой коробки считается от ПРИРОДНОГО
        // размера картинки, который у самой картинки никуда не делся:
        // обособление меняет коробку, а не объект. У Blink это два разных
        // входа — раскладочный `LayoutReplaced::ComputeNaturalSizingInfo`
        // начинается с `DCHECK(!ShouldApplySizeContainment())`
        // (`layout_replaced.cc:509`), а рисовательный
        // `LayoutReplaced::ReplacedContentRectFrom` берёт
        // `GetNaturalDimensions()` БЕЗ всякого гейта (`:497`), и уже от него
        // `ComputeObjectFitAndPositionRect` (`:426`) считает `object-fit`.
        // Поэтому коробку домеряем здесь, а рисуем ОБЫЧНЫМ путём: клон с уже
        // посчитанными сторонами содержимого и снятым обособлением — это и
        // есть второй такт, «laying out in-place» (css-contain-2
        // Overview.bs:702-707).
        if e.style.object_fit.is_some() || e.style.object_position.is_some() {
            let mut fitted = e.clone();
            fitted.style.width = Some(Len::Px(cw));
            fitted.style.height = Some(Len::Px(ch));
            // `cw`/`ch` — размер СОДЕРЖИМОГО по построению, поэтому клон
            // меряется по содержимому независимо от `box-sizing` документа.
            fitted.style.border_box = Some(false);
            fitted.style.contain_size = Some(false);
            fitted.style.contain_inline_size = Some(false);
            return image_with(&fitted, base_font);
        }
        image = image.w(px(cw)).h(px(ch)).object_fit(gpui::ObjectFit::Fill);
        return d.child(image).into_any_element();
    }
    if src.starts_with("data:") || src.starts_with("file:") || src.starts_with('/') {
        // Локальный файл отдаётся ПУТЁМ, а не строкой адреса. Строку со схемой
        // `file:` система разбирает как сетевой адрес и уходит его скачивать —
        // ничего не приходит, и картинка молча не рисуется вовсе. Ровно на
        // этом эталоны из одних картинок выходили пустой страницей.
        let local = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))
            .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
        // Растровый файл декодируется СРАЗУ, своим декодером: штатный путь
        // грузит асинхронно (кадр успевал сняться до загрузки — стенд мигал),
        // и не применяет вшитый цветовой профиль (css-color-4 §12).
        // Ключ источника несёт `image-orientation`: развёрнутый и сырой растр
        // — РАЗНЫЕ картинки с разным природным размером, и кэш обязан их
        // различать (css-images-3 §5.4).
        let key = crate::paint::background::key(local.unwrap_or(src), &e.style);
        let own = crate::paint::background::source(&key).and_then(|s| match s {
            crate::paint::background::Source::Raster(image) => Some(image),
            crate::paint::background::Source::Vector { .. }
            | crate::paint::background::Source::Gradient { .. }
            | crate::paint::background::Source::Shape { .. } => None,
        });
        // `object-position` (и точные режимы `object-fit`) — фоновой трубой:
        // concrete object size = размер плитки, позиционирование = origin,
        // клип по content box (css-images-3 §5.1/5.2). Путь включается только
        // при заданной позиции и точной коробке — прочее живёт старым путём.
        // Труба включается при ЛЮБОМ из object-fit/object-position: тест и
        // эталон (background-*) обязаны сойтись одной механикой; позиция по
        // умолчанию — центр (css-images-3 §5.2). Раньше последняя картинка
        // ряда (без object-position) шла другой трубой и расходилась.
        let wants_pipe = e.style.object_position.is_some()
            || e.style
                .object_fit
                .as_deref()
                .is_some_and(|f| matches!(f, "fill" | "contain" | "cover" | "none" | "scale-down"));
        if let (true, Some(Len::Px(w)), Some(Len::Px(h))) =
            (wants_pipe, e.style.width, e.style.height)
        {
            let pos = e.style.object_position.unwrap_or(crate::style::computed::BgPos {
                x: Some(Len::Pct(0.5)),
                y: Some(Len::Pct(0.5)),
            });
            use crate::style::computed::BgSize;
            let mut bgc = crate::style::computed::Computed::default();
            bgc.bg_image = Some(local.unwrap_or(src).to_string());
            bgc.bg_repeat = Some(crate::style::computed::BgRepeat::NoRepeat);
            bgc.bg_pos = pos;
            // Стиль трубы собран с нуля, и отказ от EXIF-разворота в него надо
            // положить руками: иначе `image-orientation: none` вместе с
            // `object-fit`/`object-position` уходил бы мимо ключа источника.
            bgc.image_orient_none = e.style.image_orient_none;
            bgc.bg_size = match e.style.object_fit.as_deref() {
                Some("contain") => BgSize::Contain,
                Some("cover") => BgSize::Cover,
                Some("none") => BgSize::Auto,
                Some("scale-down") => {
                    // Меньшее из `none` и `contain`: влезает — своим
                    // размером, нет — вписать.
                    let fits = crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))
                        .map(|s| s.intrinsic())
                        .is_some_and(|i| {
                            i.w.is_some_and(|iw| iw <= w) && i.h.is_some_and(|ih| ih <= h)
                        });
                    if fits { BgSize::Auto } else { BgSize::Contain }
                }
                _ => BgSize::Fixed(Some(Len::Pct(1.0)), Some(Len::Pct(1.0))),
            };
            // `overflow: visible` на замещаемом (HTML §rendering: UA-правило
            // `img { overflow: clip; overflow-clip-margin: content-box }`,
            // css-overflow-3): ЯВНОЕ `visible` выпускает картинку за content
            // box — `object-fit: none` рисуется своим размером целиком,
            // скругление её тоже не режет (`overflow-img`, `-svg`,
            // `-border-radius`: эталон — та же картинка без обрезки).
            // Умолчание (`None`) — UA-шный `clip`, прежний путь ниже.
            let spills = e.style.overflow_x == Some(crate::style::computed::Overflow::Visible)
                && e.style.overflow_y == Some(crate::style::computed::Overflow::Visible);
            if spills {
                let style = bgc.clone();
                let layer = gpui::canvas(
                    |_, _, _| {},
                    move |bounds: gpui::Bounds<gpui::Pixels>, _, window, _| {
                        // Область ОТСЧЁТА — content box, область КРАСКИ — с
                        // запасом во все стороны: плитка одна (`no-repeat`),
                        // и рисуется она ровно своим размером.
                        let reach = px(4096.0);
                        let paint = gpui::Bounds {
                            origin: gpui::point(bounds.origin.x - reach, bounds.origin.y - reach),
                            size: gpui::size(
                                bounds.size.width + reach * 2.0,
                                bounds.size.height + reach * 2.0,
                            ),
                        };
                        crate::paint::background::paint_tiles(&style, bounds, Some(paint), window);
                    },
                )
                .absolute()
                .top_0()
                .left_0()
                .size_full();
                return d
                    .child(div().w(px(w)).h(px(h)).relative().child(layer))
                    .into_any_element();
            }
            if let Some(layer) = crate::paint::background::layer(&bgc) {
                // Внутренняя коробка = content box: поля и рамка остаются
                // на хосте, слой не должен их накрывать.
                return d
                    .child(
                        div()
                            .w(px(w))
                            .h(px(h))
                            .relative()
                            .overflow_hidden()
                            .child(layer),
                    )
                    .into_any_element();
            }
        }
        let mut image = match (own, local) {
            (Some(ready), _) => gpui::img(ready).preserve_natural_pixels(true),
            (None, Some(path)) => gpui::img(std::path::PathBuf::from(path)),
            (None, None) => gpui::img(SharedString::from(src.to_string())),
        };
        // Картинку арифметикой над своими цветами не поправить — но
        // обесцвечивание у неё своё, встроенное.
        if e.style.filter.is_some_and(|f| f.grayscale > 0.5) {
            image = image.grayscale(true);
        }
        // Заданный размер коробки картинке надо ОТДАТЬ: сама она берёт свой
        // пиксель и рисуется им, сколько бы ни стояло в разметке. Замерено на
        // пробе: `<img width=100 height=100>` и `img { width: 300px }` давали
        // ровно один и тот же рисунок в 15 точек — то есть размер не работал
        // никогда. Отдаётся он, только когда заданы ОБЕ стороны: с одной
        // вторая считается по соотношению сторон, а его коробка не знает.
        // Размер ставится САМОЙ картинке, а не через «во весь родитель»: в
        // ряду обтекания родитель своего размера не имеет, и доля от него
        // схлопывала рисунок в ничто.
        // Векторный источник штатный загрузчик не рисует вовсе — растрируем
        // сами в конечный размер; фон канвы (`style="background:…"` корня) —
        // CSS-слой, не SVG-контент, растеризатор его тоже не рисует.
        let vector: Option<String> =
            crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style)).and_then(|s| match s {
                crate::paint::background::Source::Vector { markup, .. } => Some(markup),
                _ => None,
            });
        let vectorize = |old: gpui::Img, w: f32, h: f32| -> gpui::Img {
            let Some(m) = &vector else { return old };
            let mut out = match crate::svg::rasterize(m, w, h) {
                Some(r) => gpui::img(r),
                None => old,
            };
            if let Some(c) = crate::paint::background::svg_root_background(m) {
                out = out.bg(gpui::Rgba {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                });
            }
            out
        };
        // Контентная поправка: при `box-sizing: border-box` названный размер
        // или предел включает паддинг и рамку — рисунку остаётся остальное.
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let (sub_w, sub_h) = if e.style.border_box == Some(true) {
            let b = e.style.borders();
            (
                side(e.style.padding.left)
                    + side(e.style.padding.right)
                    + side(b.left)
                    + side(b.right),
                side(e.style.padding.top)
                    + side(e.style.padding.bottom)
                    + side(b.top)
                    + side(b.bottom),
            )
        } else {
            (0.0, 0.0)
        };
        let clamp = |l: Option<Len>, sub: f32| match l {
            Some(Len::Px(v)) => Some((v - sub).max(0.0)),
            _ => None,
        };
        let max_w = clamp(e.style.max_width, sub_w);
        let max_h = clamp(e.style.max_height, sub_h);
        // Сперва потолок, затем пол (§10.4).
        let limit = |v: f32, min: Option<f32>, max: Option<f32>| {
            let v = match max {
                Some(m) => v.min(m),
                None => v,
            };
            match min {
                Some(m) => v.max(m),
                None => v,
            }
        };
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: подсказка, ПЕРЕНЕСЁННАЯ через отношение
        // сторон, в автоматическом минимуме гибкого элемента (css-flexbox
        // §4.5). Патч вендора написан (`FlexItem::aspect_ratio` +
        // зажим `min_content` перенесённым размером в
        // `vendor/taffy/src/compute/flexbox.rs`). Срез flex/aspect/ratio
        // (493 пары, 416 зелёных): 416, ни одной пары в любую сторону, и
        // целевые `flexbox-min-{width,height}-auto-002` остались 0.53/0.96/
        // 1.56/2.10. Причина: отношение сторон ставится на ВНУТРЕННЮЮ
        // картинку (`image.style().aspect_ratio` ниже), а гибкий элемент —
        // это ВНЕШНЯЯ коробка замещённого, и у её узла отношения нет вовсе.
        // ПЕРЕПРОВЕРЕНО ПОСЛЕ правок §10.4 (пределы замещённого): патч
        // вендора и отношение на внешней коробке (теперь — только когда хоть
        // одна сторона не задана) возвращены вместе. Срез из 12 пар
        // `flexbox-min-*-auto-*`: сдвинулась одна и та же пара 0.96 → 0.94,
        // флипов ноль. Корень этих пар лежит не в автоматическом минимуме.
        // Проверено и это: отношение сторон ПОСТАВЛЕНО и на внешнюю коробку
        // (`image_with`, сразу после `flex_shrink_0`), патч вендора вернули —
        // срез из тех же 493 пар опять 416, целевые пары 0.53/1.56/2.10 без
        // движения, и только две из шести шевельнулись 0.96 → 0.91. Значит
        // автоматический минимум этим парам не корень; корень искать заново.
        // Использованный размер замещённого после §10.4: если пределы его
        // изменили, коробка обязана ужаться вместе с рисунком.
        let mut узкая: Option<(f32, f32)> = None;
        let natural_ratio = || {
            crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))
                .map(|s| s.intrinsic())
                .and_then(|i| {
                    i.ratio.or(match (i.w, i.h) {
                        (Some(w), Some(h)) if h > 0.0 => Some(w / h),
                        _ => None,
                    })
                })
        };
        let ratio_of = || {
            // CSS Sizing 4 #aspect-ratio: authored ratio overrides the natural ratio.
            if let Some(r) = e.style.aspect_ratio.filter(|r| *r > 0.0) {
                return Some(r);
            }
            // `auto <ratio>`: природное сильнее, заявленное — запасное.
            natural_ratio()
                .filter(|r| *r > 0.0)
                .or(e.style.aspect_ratio_auto.filter(|r| *r > 0.0))
        };
        let transfer = |size, ratio, from_width| {
            replaced_used_style::transfer(&e.style, size, ratio, from_width, [sub_w, sub_h])
        };
        if let (Some(Len::Px(w)), Some(Len::Px(h))) = (e.style.width, e.style.height) {
            image = vectorize(image, (w - sub_w).max(1.0), (h - sub_h).max(1.0));
            // Элемент ГИБКОГО контейнера: коробку задаёт раскладка (рост,
            // сжатие — css-flexbox-1 §9.7), и картинка заполняет её
            // (`object-fit: fill`, css-images-3 §5.5), а не держит
            // объявленную ширину. Прежде коробка `flex: 5` росла до 122.5,
            // а картинка оставалась 10 точек (`flexbox-basic-img-horiz-001`,
            // `-vert-001`).
            image = if e.style.flex_item {
                image.size_full()
            } else {
                image.w(px(w)).h(px(h))
            };
        } else if let (Some(Len::Px(w)), None | Some(Len::Auto)) = (e.style.width, e.style.height) {
            // Заданная ширина + auto-высота: высота из соотношения (§10.6.2),
            // без соотношения — своя, резерв 150.
            // §10.7: пределы зажимают и НАЗВАННУЮ сторону тоже. Коробку
            // `apply` уже зажал, а картинка шла как написана и вылезала за неё.
            let cw = limit((w - sub_w).max(0.0), clamp(e.style.min_width, sub_w), max_w);
            let ch = match ratio_of() {
                Some(r) if r > 0.0 => transfer(cw, r, true),
                _ => crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))
                    .and_then(|s| s.intrinsic().h)
                    .unwrap_or(150.0),
            };
            // §10.4: пределы держат ВЫВЕДЕННУЮ сторону тоже — заданная
            // остаётся как написана, а высота из соотношения обязана влезть
            // в свой потолок и пол. Замерено отдельно: 0 и 0 — правка по
            // спеке, счёт на ней не держится.
            let ch_free = ch;
            let ch = limit(ch, clamp(e.style.min_height, sub_h), max_h);
            // §10.4: когда потолок или пол ИЗМЕНИЛИ выведенную сторону,
            // заданная пересчитывается по соотношению — коробка остаётся
            // пропорциональной, а не растягивается. `width: 200px` при
            // `max-height: 50px` у картинки 100×50 — это 100×50, а не
            // 200×50.
            let cw = match ratio_of() {
                Some(r) if r > 0.0 && (ch - ch_free).abs() > 0.01 => {
                    let w = limit(transfer(ch, r, false), clamp(e.style.min_width, sub_w), max_w);
                    // Коробка ужимается ТОЛЬКО когда предел и правда изменил
                    // выведенную сторону: иначе высота у неё остаётся `auto`,
                    // и подстановка ломала замещённые без пределов вовсе
                    // (`replaced-intrinsic-004`).
                    узкая = Some((w, ch));
                    w
                }
                _ => cw,
            };
            image = vectorize(image, cw.max(1.0), ch.max(1.0))
                .w(px(cw))
                .h(px(ch))
                .object_fit(gpui::ObjectFit::Fill);
        } else if let (None | Some(Len::Auto), Some(Len::Px(h))) = (e.style.width, e.style.height) {
            // Зеркально: заданная высота + auto-ширина (§10.3.2).
            let ch = limit(
                (h - sub_h).max(0.0),
                clamp(e.style.min_height, sub_h),
                max_h,
            );
            let cw = match ratio_of() {
                Some(r) if r > 0.0 => transfer(ch, r, false),
                _ => crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))
                    .and_then(|s| s.intrinsic().w)
                    .unwrap_or(300.0),
            };
            let cw_free = cw;
            let cw = limit(cw, clamp(e.style.min_width, sub_w), max_w);
            // Зеркально §10.4: изменённая ширина тянет за собой высоту.
            let ch = match ratio_of() {
                Some(r) if r > 0.0 && (cw - cw_free).abs() > 0.01 => {
                    let h = limit(transfer(cw, r, true), clamp(e.style.min_height, sub_h), max_h);
                    узкая = Some((cw, h));
                    h
                }
                _ => ch,
            };
            image = vectorize(image, cw.max(1.0), ch.max(1.0))
                .w(px(cw))
                .h(px(ch))
                .object_fit(gpui::ObjectFit::Fill);
        } else if let (Some(Len::Pct(_)), None | Some(Len::Auto)) = (e.style.width, e.style.height)
        {
            // Доля ширины при auto-высоте: ширину даёт содержащий блок, высоту
            // — собственное соотношение сторон (§10.3.2, §10.6.2). Раньше доля
            // не попадала ни в одну ветку, и замещаемый рисовался СВОИМ
            // пикселем (`absolute-replaced-width-006`: 15×15 вместо 96×96).
            // Долю по этой оси УЖЕ поставил хозяин (`styled_div(e)` несёт
            // стиль элемента целиком), поэтому картинке остаётся заполнить
            // его: `relative(kw)` внутри давал долю ОТ ДОЛИ — `width: 50%`
            // выходило четвертью содержащего блока.
            image = image.w(gpui::relative(1.0));
            if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
                image.style().aspect_ratio = Some(r);
            }
        } else if let (None | Some(Len::Auto), Some(Len::Pct(_))) = (e.style.width, e.style.height)
        {
            // Зеркально: доля высоты при auto-ширине.
            image = image.h(gpui::relative(1.0));
            if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
                image.style().aspect_ratio = Some(r);
            }
        } else if !matches!(e.style.width, None | Some(Len::Auto))
            && !matches!(e.style.height, None | Some(Len::Auto))
        {
            // Обе стороны заданы, но не обе в точках: каждая ось — своим
            // значением. `width:100%; height:100%` растягивается на коробку
            // (sizing-percentages-replaced-orthogonal-001), а смешанная
            // запись `width:50%; height:15px` раньше падала в size_full и
            // ТЕРЯЛА пиксельную сторону (inline-replaced-width-011..015).
            image = match (e.style.width, e.style.height) {
                (Some(Len::Pct(_)), Some(Len::Px(h))) => image.w(gpui::relative(1.0)).h(px(h)),
                (Some(Len::Px(w)), Some(Len::Pct(_))) => image.w(px(w)).h(gpui::relative(1.0)),
                _ => image.size_full(),
            };
        } else if !matches!(e.style.width, Some(Len::Px(_)))
            && !matches!(e.style.height, Some(Len::Px(_)))
        {
            let both_auto = matches!(e.style.width, None | Some(Len::Auto))
                && matches!(e.style.height, None | Some(Len::Auto));
            if both_auto && let Some(ready) = crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style)) {
                // Авто-размер замещаемого считается ЗДЕСЬ, а не отдаётся
                // загрузчику картинок: тот работает асинхронно, и в первом
                // кадре коробка выходила нулевой высоты (box-sizing-007 —
                // страница вовсе без квадратов). Недостающая сторона
                // достраивается соотношением, резерв — 300×150
                // (CSS 2.1 §10.3.2/§10.6.2, css-images-3 §default-sizing).
                let side = ready.intrinsic();
                let (w0, h0) = match (side.w, side.h, side.ratio) {
                    (Some(w), Some(h), _) => (w, h),
                    (Some(w), None, Some(r)) => (w, w / r),
                    (None, Some(h), Some(r)) => (h * r, h),
                    (Some(w), None, None) => (w, 150.0),
                    (None, Some(h), None) => (300.0, h),
                    (None, None, Some(r)) => {
                        // §10.3.2, последний пункт: при обеих `auto` и одном
                        // лишь соотношении ширина берётся из содержащего
                        // блока, а не из резерва. Резерв остаётся, когда
                        // ширину взять неоткуда (`ratio-2.svg` в
                        // `visudet/replaced-elements-*`: мы рисовали 300×150,
                        // эталон — 200×100 по `div { width: 200px }`).
                        let w = CB_WIDTH
                            .get()
                            .filter(|v| *v > 0.0)
                            .unwrap_or_else(|| (150.0 * r).min(300.0));
                        (w, w / r)
                    }
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: считать ДОЛЮ собственного
                    // размера рисунка (`<svg width="100%">` и SVG вовсе без
                    // атрибутов — у него подразумевается `100%`) и разрешать
                    // её от содержащего блока. Написано было и в `Intrinsic`
                    // (поля `w_pct`/`h_pct` в `background::svg_size`), и
                    // здесь. Срез из 943 пар (replaced/normal-flow/svg/
                    // background-size/image): 880 → 874. `replaced-intrinsic-
                    // 002` пошла 6.10 → 1.41, но вся семья `replaced-
                    // elements-*` рухнула (0.01 → 24.17 и родня).
                    // Причина в РАЗНИЦЕ ДВУХ СЛУЧАЕВ: у `<img>` доля значит
                    // «своего размера нет» и работает умолчальный размер
                    // 300×150 (css-images-3 §5.2), а у `<object>` SVG — это
                    // вложенный ДОКУМЕНТ, и его `100%` считается от коробки
                    // объекта, то есть от содержащего блока. Возвращать
                    // вместе с этим различением.
                    (None, None, None) => (300.0, 150.0),
                };
                if w0 > 0.0 && h0 > 0.0 {
                    // §10.4: пределы — по таблице (`css2_replaced_limits`).
                    let min_w = clamp(e.style.min_width, sub_w);
                    let min_h = clamp(e.style.min_height, sub_h);
                    // Без СОБСТВЕННОГО соотношения стороны независимы: потолок
                    // высоты режет только высоту, и ширина остаётся своей
                    // (§10.4, таблица «no intrinsic ratio»). Прежде общий
                    // множитель ужимал и её — картинка без соотношения под
                    // `max-height: 20px` выходила у́же в пятнадцать раз
                    // (`replaced-elements-max-height-20`, снимки `no-ratio` и
                    // `height-25-no-ratio`).
                    let без_соотношения = side.ratio.is_none()
                        && !(side.w.is_some() && side.h.is_some());
                    let (rw, rh) = if без_соотношения {
                        (
                            limit(w0, min_w, max_w),
                            limit(h0, min_h, max_h),
                        )
                    } else {
                        css2_replaced_limits(w0, h0, min_w, max_w, min_h, max_h)
                    };
                    image = vectorize(image, rw, rh)
                        .w(px(rw))
                        .h(px(rh))
                        .object_fit(gpui::ObjectFit::Fill);
                }
            } else if (max_w.is_some() || max_h.is_some())
                && let Some(ready) = crate::paint::background::source(&crate::paint::background::key(local.unwrap_or(src), &e.style))
            {
                let side = ready.intrinsic();
                if let (Some(w0), Some(h0)) = (side.w, side.h)
                    && w0 > 0.0
                    && h0 > 0.0
                {
                    let mut scale = 1.0f32;
                    if let Some(m) = max_w {
                        scale = scale.min(m / w0);
                    }
                    if let Some(m) = max_h {
                        scale = scale.min(m / h0);
                    }
                    if scale < 1.0 {
                        image = vectorize(image, w0 * scale, h0 * scale)
                            .w(px(w0 * scale))
                            .h(px(h0 * scale))
                            .object_fit(gpui::ObjectFit::Fill);
                    }
                }
            }
        }
        // CSS Images 3 §4.5: object-fit initially fills the content box.
        // Intrinsic sizing above already preserves the natural ratio. Applying
        // contain again to the snapped box introduces unintended letterboxing.
        image = match e.style.object_fit.as_deref() {
            Some("cover") => image.object_fit(gpui::ObjectFit::Cover),
            Some("contain") => image.object_fit(gpui::ObjectFit::Contain),
            Some("scale-down") => image.object_fit(gpui::ObjectFit::ScaleDown),
            Some("none") => image.object_fit(gpui::ObjectFit::None),
            _ => image.object_fit(gpui::ObjectFit::Fill),
        };
        let mut d = match узкая {
            Some((w, h)) => d.w(px(w + sub_w)).h(px(h + sub_h)),
            None => d,
        };
        if replaced_holder_ratio::apply(&mut d, &e.style, ratio_of(), узкая.is_some()) {
            image = image.size_full().object_fit(gpui::ObjectFit::Fill);
        }
        return d.child(replaced_content::position(image, &e.style)).into_any_element();
    }
    // Картинки БЕЗ АДРЕСА вовсе (`<img>` без `src`) не существует: коробки
    // она не порождает и в замер по содержимому не входит (HTML §4.8.4.4 —
    // «if the element has no src attribute … the element represents
    // nothing»). Подпись-заглушка тут вредна: она даёт ширину, и
    // `width: max-content` вокруг такой картинки выходил шире содержимого
    // (`white-space-intrinsic-size-024/025`).
    if e.attr("src").is_none_or(|s| s.trim().is_empty()) && e.attr("alt").is_none() {
        return d.into_any_element();
    }
    // Пустая рамка вместо чужой картинки: молча ничего не показать хуже —
    // в разметке останется дыра без объяснения.
    d.child(SharedString::from(
        e.attr("alt")
            .map(str::to_string)
            .unwrap_or_else(|| "[изображение]".into()),
    ))
    .into_any_element()
}
