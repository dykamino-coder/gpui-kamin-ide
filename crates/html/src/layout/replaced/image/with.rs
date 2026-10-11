//! Картинка с заданным стилем (image_with): нормализация, обособление размера, локальный файл.

use super::{contained_image, fit_local_image, local_image_source, view_boxed};
use crate::dom::Element;
use crate::layout::block::containing::AVAIL_W;
use crate::layout::replaced::replaced_used_style;
use crate::render::styled_div;
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled};

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
    let d = match contained_image(base_font, src, e, d) {
        Ok(value) => return value,
        Err(d) => d,
    };
    if src.starts_with("data:") || src.starts_with("file:") || src.starts_with('/') {
        // Локальный файл отдаётся ПУТЁМ, а не строкой адреса. Строку со схемой
        // `file:` система разбирает как сетевой адрес и уходит его скачивать —
        // ничего не приходит, и картинка молча не рисуется вовсе. Ровно на
        // этом эталоны из одних картинок выходили пустой страницей.
        let (local, d, image) = match local_image_source(src, e, d) {
            Ok(value) => value,
            Err(value) => return value,
        };
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
        let vector: Option<String> = crate::paint::background::source(
            &crate::paint::background::key(local.unwrap_or(src), &e.style),
        )
        .and_then(|s| match s {
            crate::paint::background::Source::Vector { markup, .. } => Some(markup),
            _ => None,
        });
        let vectorize = |old: gpui::Img, w: f32, h: f32| -> gpui::Img {
            let Some(m) = &vector else { return old };
            let mut out = match crate::svg::raster::rasterize(m, w, h) {
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
        let узкая: Option<(f32, f32)> = None;
        let natural_ratio = || {
            crate::paint::background::source(&crate::paint::background::key(
                local.unwrap_or(src),
                &e.style,
            ))
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
        return fit_local_image(
            src, e, local, d, image, vectorize, sub_w, sub_h, clamp, max_w, max_h, limit, узкая,
            ratio_of, transfer,
        );
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
