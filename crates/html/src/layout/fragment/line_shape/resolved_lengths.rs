//! Длины коробки в точках для клона поддерева вложенных рядов.

use crate::document::state::ValueCx;
use crate::dom::{Element, Node};
use crate::style::computed::BoxStyle;
use crate::style::values::value::Len;

/// Клон поддерева с длинами коробки в ТОЧКАХ: `em`/`ch`/`ex`/`rem` в
/// размерах, полях, отбивках, рамках и вставках разрешены по кеглю своего
/// элемента (тем же `style::cascade::inherit` → `Computed::resolve_em`, что и при
/// отрисовке). `shape_full` читает голый стиль, и `margin-top: 2em` мерился
/// нулём (нестрогий `px_or`), а `height: 4em` — отказом всей стопки, хотя
/// рисунок кладёт их в точках. Наследуемые (`font-size`, `line-height`) не
/// трогаются: число в `line-height` наследуется множителем.
pub(crate) fn resolved_lengths(c: &Element, parent: &BoxStyle, vcx: ValueCx<'_>) -> Element {
    let m = crate::style::cascade::materialize::box_style(parent, &c.style, vcx);
    let mut t = c.clone();
    // Подменяются ТОЛЬКО шрифтовые единицы, разрешённые в точки: прочее
    // (`None`, `auto`, доли, точки) остаётся как было — `inherit` дописывает
    // и умолчания, а на `min-height.is_some()` стоят гейты строк flex
    // (`multi-line-row-flex-fragmentation-018/037`).
    fn fix(own: &mut Option<Len>, res: Option<Len>) {
        if own.is_some_and(|l| !matches!(l, Len::Px(_) | Len::Auto | Len::Pct(_)))
            && matches!(res, Some(Len::Px(_)))
        {
            *own = res;
        }
    }
    fn fix_sides(own: &mut crate::style::computed::Sides, res: &crate::style::computed::Sides) {
        fix(&mut own.top, res.top);
        fix(&mut own.right, res.right);
        fix(&mut own.bottom, res.bottom);
        fix(&mut own.left, res.left);
    }
    fix(&mut t.style.used_mut().width, m.width);
    fix(&mut t.style.used_mut().height, m.height);
    fix(&mut t.style.used_mut().min_width, m.min_width);
    fix(&mut t.style.used_mut().min_height, m.min_height);
    fix(&mut t.style.used_mut().max_width, m.max_width);
    fix(&mut t.style.used_mut().max_height, m.max_height);
    fix_sides(&mut t.style.used_mut().margin, &m.margin);
    fix_sides(&mut t.style.used_mut().padding, &m.padding);
    fix_sides(&mut t.style.used_mut().border_width, &m.border_width);
    fix_sides(&mut t.style.used_mut().inset, &m.inset);
    t.children = c
        .children
        .iter()
        .map(|n| match n {
            Node::Element(k) => Node::Element(resolved_lengths(k, &m, vcx)),
            other => other.clone(),
        })
        .collect();
    t
}
