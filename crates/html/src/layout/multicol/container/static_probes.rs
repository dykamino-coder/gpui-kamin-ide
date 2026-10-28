//! Щупы статической позиции прямых позиционированных детей многоколоночника: нулевые
//! дети стопки на их месте в потоке (CSS 2.1 §10.6.4, §10.3.7).

use crate::dom::Element;
use crate::layout::fragment::types::StackChild;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::SpotCell;
use crate::layout::positioned::predicates::edge_set;
use crate::style::computed::Computed;

pub(super) fn place_oof_statics(
    merged: &Computed,
    opts: &RenderOpts,
    oof_static: &[(usize, Element)],
    kid_starts: Vec<usize>,
    children: Vec<StackChild>,
) -> (Vec<StackChild>, Vec<SpotCell>) {
    let oof_spots: Vec<SpotCell> = oof_static.iter().map(|_| Default::default()).collect();
    let children =
        insert_static_probes(merged, opts, oof_static, &oof_spots, &kid_starts, children);
    (children, oof_spots)
}

/// Щупы статической позиции прямых позиционированных детей — нулевые дети стопки
/// на их месте в потоке; отложенная стопка ставит их после меры детей.
pub(in crate::layout::multicol) fn insert_static_probes(
    merged: &Computed,
    opts: &RenderOpts,
    oof_static: &[(usize, Element)],
    oof_spots: &[SpotCell],
    kid_starts: &[usize],
    children: Vec<StackChild>,
) -> Vec<StackChild> {
    let mut children = children;
    for (i, (at, oof)) in oof_static.iter().enumerate().rev() {
        // Щуп правит только пустую ось; заданную считает CB (CSS 2.1 §10.3.7), как fixed_axes.
        oof_spots[i].set(crate::layout::positioned::containing_block::Spot {
            fixed_axes: (
                edge_set(oof.style.used().inset.left) || edge_set(oof.style.used().inset.right),
                edge_set(oof.style.used().inset.top) || edge_set(oof.style.used().inset.bottom),
            ),
            rtl: merged.wm().rtl(),
            vertical: merged.wm().vertical(),
            vertical_rl: merged.wm().block_rl(),
            own_vertical: oof.style.cascaded().wm().vertical(),
            ..Default::default()
        });
        let probe = StackChild {
            #[cfg(feature = "wpt-harness")]
            probe_pending: false,
            #[cfg(feature = "wpt-harness")]
            probe: opts.frag.probe.clone(),
            fragment_builders: Vec::new(),
            source: None,
            watched: None,
            breaks_w: None,
            regrow: None,
            measure: None,
            el: crate::layout::positioned::containing_block::spot_probe(
                oof_spots[i].clone(),
                true,
                None,
                &opts.cx,
            ),
            frags: Vec::new(),
            monolith: false,
            fit_whole: false,
            cuts: Vec::new(),
            force_before: false,
            force_after: false,
            avoid_before: false,
            avoid_after: false,
            forced: Vec::new(),
            solid: Vec::new(),
            avoid: Vec::new(),
            keep: Vec::new(),
            line_keep: Vec::new(),
            table_rows: Vec::new(),
            h: 0.0,
            mt: 0.0,
            mb: 0.0,
            span: false,
            over: 0.0,
            rel: (0.0, 0.0),
            clone_dec: None,
            clone_fixed: false,
            overflow_top: false,
            nested_cols: false,
            repeat: None,
            par: crate::layout::fragment::types::Par::default(),
            slack: None,
            laid_w: Default::default(),
            paginate: None,
            clone_frags: None,
            copier: None,
            nest: None,
            nest_report: None,
            resume: None,
            positioned: false,
        };
        // Номер — среди ДЕТЕЙ ДО раскрытия строк flex (`split_flex_lines`).
        let at = kid_starts
            .get(*at)
            .copied()
            .unwrap_or(children.len())
            .min(children.len());
        children.insert(at, probe);
    }
    children
}
