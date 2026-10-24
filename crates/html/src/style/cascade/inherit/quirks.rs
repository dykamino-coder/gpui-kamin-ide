//! Стадия коробки, режим quirks: опора доли высоты (Quirks Mode §3.5) ищется по предкам-коробкам.

use super::*;
use crate::document::state::ValueCx;
use crate::style::computed::BoxStyle;

/// Quirks Mode §3.5 «The percentage height calculation quirk»: в режиме
/// quirks содержащий блок для ДОЛИ высоты ищется циклом — предки с
/// `height: auto` пропускаются, пока не найдётся предок с заданной
/// высотой, абсолютный или табличный (тогда он и есть опора). Сама
/// коробка с долей и табличный `display` квирку не подлежат. Без него
/// `<canvas style="height:100%">` в `div` без высоты внутри флоата
/// высотой 100 схлопывался в ноль (`intrinsic-percent-replaced-002/006`,
/// `float-percentage-resolution-quirks-mode`). Blink:
/// `LayoutBox::ContainingBlockLogicalHeightForPercentageResolution`
/// (`SkipContainingBlockForPercentHeightCalculation`).
pub(super) fn percentage_height(
    parent: &BoxStyle,
    own: &Computed,
    c: &mut BoxStyle,
    vcx: ValueCx<'_>,
) {
    if !vcx.doc.quirks() {
        return;
    }
    use crate::style::computed::Display as D;
    use crate::style::computed::Position as P;
    use crate::style::values::value::Len as L;
    let out_of_flow = matches!(own.position, Some(P::Absolute) | Some(P::Fixed));
    let tabular = matches!(
        own.display,
        Some(D::Table | D::InlineTable | D::TableCell | D::TableRow | D::TableRowGroup)
    );
    // Опора считается по ЗАДАННОЙ высоте коробки (своя запись или доля,
    // решённая квирком), как её видит предок-коробка.
    let mut height = own.height;
    if let (Some(L::Pct(k)), false, false, Some(base)) =
        (own.height, c.cb_height_def, tabular, parent.quirk_pct_base)
    {
        height = Some(L::Px(k * base));
        set!(c.height, height);
    }
    // Only block containers and table wrappers transmit an indefinite
    // percentage basis. Flex/grid containers stop the quirks ancestor walk.
    let blockish = matches!(
        own.display,
        None | Some(D::Block | D::InlineBlock | D::ListItem)
    );
    c.quirk_pct_base = match height {
        Some(L::Px(h)) => Some(h),
        // Доля от определённого блока — тоже опора: флоат `height: 50%`
        // в контейнере 200 даёт потомкам 100 (`intrinsic-percent-
        // replaced-003/004`).
        Some(L::Pct(k)) if !tabular => match parent.height {
            Some(L::Px(h)) => Some(k * h),
            _ => parent.quirk_pct_base.map(|b| k * b),
        },
        None | Some(L::Auto) if blockish && !out_of_flow && !tabular && !c.root_box => {
            parent.quirk_pct_base
        }
        _ => None,
    };
}
