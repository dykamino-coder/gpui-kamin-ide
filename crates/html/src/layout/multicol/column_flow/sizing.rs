//! Ограничения размеров текстового потока перед разбиением колонок.

use super::*;

/// Многоколоночный поток из сплошного текста.
///
/// Годится, когда всё содержимое — строчное: тогда режется сам текст. Если
/// внутри блоки, колонки набираются из них сеткой, как и раньше.
pub(crate) fn column_flow(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    outer: Option<Rc<Nested>>,
) -> Option<AnyElement> {
    let mut dimensions = MulticolDimensions::css(inherited);
    dimensions.preferred[1] = inherited.height.or(e.style.used().height);
    dimensions.border_box = e.style.used().border_box == Some(true);
    column_flow_with_dimensions(e, inherited, opts, count, col_w, outer, dimensions)
}

#[allow(clippy::too_many_arguments)]
pub(crate) fn column_flow_with_dimensions(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    count: Option<usize>,
    col_w: Option<f32>,
    outer: Option<Rc<Nested>>,
    dimensions: MulticolDimensions,
) -> Option<AnyElement> {
    // Линейки последней (единственной) линии колонок тянутся до низа
    // содержимого коробки заданной высоты (Blink `PaintColumnRules`: «Paint
    // column rules as tall as the entire multicol container, but only when at
    // the last row»; `multicol-rule-004`: две строки в коробке 5em — линейка
    // на все 5em). Высота содержимого — заданная `height` в точках.
    let px = |l: &Option<Len>| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    };
    let stretch = match dimensions.preferred[1] {
        Some(Len::Px(h)) if h > 0.0 => {
            if dimensions.border_box {
                let b = e.style.used().borders();
                (|| {
                    Some(
                        h - px(&e.style.used().padding.top)?
                            - px(&e.style.used().padding.bottom)?
                            - px(&b.top)?
                            - px(&b.bottom)?,
                    )
                })()
            } else {
                Some(h)
            }
        }
        _ => None,
    }
    .filter(|h| *h > 0.0);
    column_flow_in(
        e, inherited, opts, count, col_w, false, stretch, outer, dimensions,
    )
}
