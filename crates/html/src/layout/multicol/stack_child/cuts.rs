//! Сборка StackChild из измеренной копии ребёнка многоколоночника.

use crate::dom::Element;
use crate::layout::cx::FrameCx;
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::line_shape::slack_fill;
use crate::layout::fragment::table_bands::{repeat_bands, table_box};
use crate::layout::fragment::types::{Par, Repeat, Rows, StackChild};
use crate::layout::multicol::spanner::{multicol_inside, parallel_items_inside};
use crate::tolerance::COORD_EPS;

#[allow(clippy::too_many_arguments)]
pub(super) fn stack_child_of(
    ix: usize,
    mt: f32,
    mb: f32,
    kid_par: &[Par],
    col_vert: bool,
    rows: Option<Rows>,
    fixed: Option<f32>,
    dec: Option<(f32, f32)>,
    copy: &Element,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    h: f32,
    over: f32,
    rel: (f32, f32),
    build: super::factory::Builder,
    measure: Option<f32>,
    monolith: bool,
    span: bool,
    kid_copies: usize,
    measured_table: Option<(bool, bool)>,
    vcx: FrameCx<'_>,
) -> StackChild {
    let repeat = repeat_bands(copy, fixed, rows, measured_table);
    let (el, frags, fragment_builders) = super::factory::fragments(
        build.clone(),
        copy,
        col_vert,
        if span { 1 } else { kid_copies },
    );
    StackChild {
        #[cfg(feature = "wpt-harness")]
        probe_pending: measure.is_none(),
        #[cfg(feature = "wpt-harness")]
        probe: Default::default(),
        source: Some(copy.node_id),
        watched: None,
        breaks_w: None,
        regrow: None,
        measure,
        el,
        frags,
        fragment_builders,
        monolith,
        fit_whole: false,
        cuts,
        // Подъём разрыва совпадает с мерой.
        force_before: edge_break(copy, false, vcx),
        force_after: edge_break(copy, true, vcx),
        avoid_before: edge_avoid(copy, false, vcx),
        avoid_after: edge_avoid(copy, true, vcx),
        forced,
        solid,
        avoid: Vec::new(),
        keep: Vec::new(),
        line_keep: Vec::new(),
        table_rows: Vec::new(),
        h,
        mt,
        mb,
        span,
        over,
        rel,
        clone_dec: dec,
        clone_fixed: dec.is_some()
            && crate::layout::block::sizing::BoxDimensions::for_element(copy, &copy.style.used())
                .border_box
            && matches!(
                copy.style.used().height,
                Some(crate::style::values::value::Len::Px(_))
            ),
        // Монолит с верха колонки переполняет её
        // (`layout::fragment::plan` `fill_at`, `overflow_to`) —
        // только при `column-fill: auto` без рядов
        // и без элементов ряда в поддереве, кроме элементов сетки: они
        // продолжаются каждый со своего расхода (column_regrow::items).
        overflow_top: fixed.is_some()
            && rows.is_none()
            && (!parallel_items_inside(copy, 4)
                || crate::layout::fragment::flow::blocks::grid_items_resume(copy)),
        // Вложенный многоколоночник — маска режет вбок
        // (`StackChild::nested_cols`).
        nested_cols: multicol_inside(copy, 4),
        par: kid_par[ix],
        positioned: !span
            && (matches!(
                copy.style.used().position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
            ) || copy.style.used().transform.is_some())
            && copy.style.used().z_index.unwrap_or(0) == 0,
        // Хвост непоследнего фрагмента таблицы — её фоном
        // (`StackChild::slack`).
        slack: if col_vert {
            None
        } else if table_box(copy) {
            copy.style.used().background.map(|c| c.to_hsla())
        } else if dec.is_none() && over <= h + COORD_EPS {
            // Продолжение одного лишь параллельного
            // потока (`over`) — не продолжение коробки:
            // она кончилась, хвоста у неё нет.
            slack_fill(copy)
        } else {
            None
        },
        laid_w: Default::default(),
        paginate: None,
        clone_frags: None,
        copier: None,
        nest: None,
        nest_report: None,
        resume: None,
        // Повтор шапки/подвала — копии flow::Repeat, полосы по разложенным секциям.
        repeat: repeat
            .filter(|_| !span && !col_vert)
            .map(|(head, foot, geom)| Repeat {
                head,
                foot,
                geom,
                row_ends: Vec::new(),
                head_els: match head {
                    Some(_) => (1..kid_copies).map(|i| build(false, i, true)).collect(),
                    None => Vec::new(),
                },
                foot_els: match foot {
                    Some(_) => (0..kid_copies).map(|i| build(false, i, true)).collect(),
                    None => Vec::new(),
                },
            }),
    }
}
