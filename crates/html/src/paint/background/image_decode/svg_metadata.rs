//! Фон канвы рисунка-картинки для покраски; размер и вырожденный `viewBox` рисунка —
//! факты самой картинки (`resources::images::vector`).

use crate::document::state::ValueCx;
pub(super) use crate::resources::images::vector::degenerate_viewbox;
use crate::resources::images::vector::root_tag;
pub(crate) use crate::resources::images::vector::svg_size;

/// Фон КАНВЫ рисунка: `style="background: …"` на корневом `<svg>`.
///
/// Это CSS-свойство замещаемого корня, а не SVG-контент — растеризатор его
/// не рисует, и рисунок из одного фона выходил прозрачным (box-sizing-007).
pub(crate) fn svg_root_background(
    markup: &str,
    vcx: ValueCx<'_>,
) -> Option<crate::style::values::value::Color> {
    let head = root_tag(markup)?;
    let at = head.find("style=")?;
    let rest = head[at + 6..].trim_start();
    let quote = rest.chars().next()?;
    let style = rest[1..].split(quote).next()?;
    let decls = crate::style::css::parse_decls(style);
    // `background` раскрыт разбором: цвет — в `background-color`.
    let v = decls.last("background-color")?;
    crate::style::values::value::Color::parse(v.split_whitespace().next()?, vcx)
}
