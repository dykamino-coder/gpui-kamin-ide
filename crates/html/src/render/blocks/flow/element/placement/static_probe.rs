//! Щуп статического места в источнике: процентная абсолютная коробка, чей размер
//! зависит от высоты содержащего блока, получает статическую позицию из потока
//! источника, как и остальные нативные продолжения.

use crate::layout::options::RenderOpts;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;

pub(super) fn static_source_probe(
    e: &crate::dom::Element,
    inherited: &BoxStyle,
    x_set: bool,
    y_set: bool,
    out: &mut Vec<AnyElement>,
    opts: &RenderOpts,
) {
    use crate::layout::positioned::containing_block::{Spot, SpotCell, spot_probe};
    let Some(recorder) = &opts.cx.position.geometry_record else {
        return;
    };
    if !crate::layout::positioned::static_position::static_place_in_flow_only(&e.style.used()) {
        return;
    }
    let spot = SpotCell::default();
    spot.set(Spot {
        fixed_axes: (x_set, y_set),
        rtl: inherited.wm().rtl(),
        vertical: inherited.wm().vertical(),
        vertical_rl: inherited.wm().block_rl(),
        own_vertical: e.style.cascaded().wm().vertical(),
        ..Default::default()
    });
    out.push(spot_probe(spot.clone(), true, None, &opts.cx));
    recorder.spot(
        e.node_id,
        &spot,
        opts.cx.position.line_box.clone(),
        &e.style.used(),
        opts.cx.vcx(),
    );
}
