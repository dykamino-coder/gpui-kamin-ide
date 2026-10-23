//! Картинка с заданным стилем (image_with): нормализация, обособление размера, локальный файл.

use super::{contained_image, fit_local_image, local_image_source, view_boxed};
use crate::dom::Element;
use crate::layout::cx::LayoutCx;
use crate::layout::replaced::image::PictureSizing;
use crate::layout::replaced::replaced_used_style;
use crate::paint::box_tree::styled::styled_div;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled};

pub(crate) fn image_with(e: &Element, base_font: Option<f32>, cx: &LayoutCx) -> AnyElement {
    let vcx = cx.vcx();
    // Ключевое слово содержимого в оси замещаемого — его природный (или
    // перенесённый через соотношение) размер, то есть `auto` (css-sizing-3
    // §5.1: «When the box has a preferred aspect ratio, size constraints in
    // the opposite dimension will transfer through»; Blink
    // `ComputeReplacedSizeInternal`): `width: min-content; height: 100px` —
    // ширина из соотношения (`intrinsic-size-017…025`).
    let normalized = replaced_used_style::normalize(e, cx.avail_w);
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
        copy.style.cascaded_mut().resolve_em(base, vcx);
        resolved = copy;
        &resolved
    } else {
        e
    };
    if let Some(vb) = e.style.cascaded().object_view_box
        && let Some(el) = view_boxed(e, vb, cx)
    {
        return el;
    }
    let d = styled_div(e, cx);
    // Замещаемый элемент в СТРОКЕ не сжимается: браузер даёт строке
    // переполниться или перенести коробку целиком (CSS 2.1 §10.3.2;
    // `wm-propagation-body-040`: картинка 340px ужималась на 6-7%). А вот
    // элемент ГИБКОГО РЯДА сжимается как всякий другой — `flex-shrink` ему
    // уже посчитан выше по файлу (единица в гибком окружении, ноль вне его),
    // и глушить его здесь значило запрещать картинке ужиматься даже при
    // явных `min-width: 0` и `flex-shrink: 1` (css-flexbox-1 §7.2).
    let mut d = d;
    match e.style.cascaded().flex_shrink {
        Some(k) => d.style().flex_shrink = Some(k),
        None => d = d.flex_shrink_0(),
    }
    let d = d;
    // Обособление размера меряет замещаемый элемент КАК ПУСТОЙ (css-contain-2
    // §size containment): своих размеров у картинки нет вовсе — ни сторон, ни
    // соотношения, — их задаёт `contain-intrinsic-size`. Врезка ранняя: ниже
    // по ветке `intrinsic()` вернул бы настоящие 100×100, и всё посчиталось бы
    // по ним.
    let d = match contained_image(base_font, src, e, d, cx) {
        Ok(value) => return value,
        Err(d) => d,
    };
    if !src.is_empty() || e.style.cascaded().content_image().is_some() {
        // Единственный синхронный источник — сервис документа.
        let (local, d, image) = match local_image_source(src, e, d, vcx) {
            Ok(value) => value,
            Err(value) => return value,
        };
        // Заданный размер коробки картинке надо ОТДАТЬ: сама она берёт свой
        // пиксель и рисуется им, сколько бы ни стояло в разметке:
        // `<img width=100 height=100>` и `img { width: 300px }` давали
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
            crate::resources::css_images::element_source(local.unwrap_or(src), e.style.cascaded(), vcx)
                .map(crate::paint::background::Source::from_cpu)
                .and_then(|s| match s {
                    crate::paint::background::Source::Vector { markup, .. } => Some(markup),
                    _ => None,
                });
        let vectorize = |old: gpui::Img, w: f32, h: f32| -> gpui::Img {
            let Some(m) = &vector else { return old };
            let viewport = crate::resources::images::viewport::with_viewport(m, (w, h));
            let mut out = match crate::svg::raster::rasterize_image(&viewport, w, h, vcx) {
                Some(r) => gpui::img(r),
                None => old,
            };
            if let Some(c) = crate::paint::background::svg_root_background(m, vcx) {
                out = out.bg(gpui::Rgba {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                });
            }
            out
        };
        // Контентная поправка, пределы и соотношение рисунка — `PictureSizing`
        // по природным размерам источника.
        // Проверено: подсказка, ПЕРЕНЕСЁННАЯ через отношение сторон, в
        // автоматическом минимуме гибкого элемента (css-flexbox §4.5;
        // `FlexItem::aspect_ratio` и зажим `min_content` в
        // `vendor/taffy/src/compute/flexbox.rs`) `flexbox-min-*-auto-*` не
        // меняет — ни с отношением только на ВНУТРЕННЕЙ картинке
        // (`image.style().aspect_ratio` в `local::draw`), ни с отношением и на
        // ВНЕШНЕЙ коробке замещённого (`image_with`, после `flex_shrink_0`).
        // Автоматический минимум этим парам не корень; корень искать заново.
        let natural =
            crate::resources::css_images::element_source(local.unwrap_or(src), e.style.cascaded(), vcx)
                .map(crate::paint::background::Source::from_cpu)
                .map(|s| s.intrinsic());
        let sizing = PictureSizing::new(e.style.cascaded(), natural, cx.cb_width);
        return fit_local_image(src, e, local, d, image, vectorize, &sizing, vcx);
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
