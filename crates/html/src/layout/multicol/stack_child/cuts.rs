//! Разрезы ребёнка по строкам и высота копии; заполнение полей StackChild.

use crate::dom::{Element, Node};
use crate::layout::fragment::breaks::{edge_avoid, edge_break};
use crate::layout::fragment::clone::solid_box;
use crate::layout::fragment::fragment_size::shape_full;
use crate::layout::fragment::grid_bands::grid_stack;
use crate::layout::fragment::line_shape::{side_margin_wrap, slack_fill, transpose_tree};
use crate::layout::fragment::probe::{plain_block_tree, stacked_flex_tree};
use crate::layout::fragment::table_bands::{repeat_bands, table_box};
use crate::layout::fragment::{ShapeCx, with_lines};
use crate::layout::multicol::spanner::{multicol_inside, parallel_items_inside};
use crate::layout::positioned::predicates::OOF_OWN;
use crate::render::RenderOpts;
use crate::style::computed::Computed;

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_cuts_and_height(
    ix: usize,
    h: f32,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    opts: &RenderOpts,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    col_rl: bool,
    line_col_w: Option<f32>,
    fixed: Option<f32>,
    merged: &Computed,
    copy: &Element,
) -> (Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>, f32, f32) {
    let copy_m = if col_vert {
        transpose_tree(copy, col_rl)
    } else {
        None
    };
    let (over, cuts, forced, solid) = line_cuts(
        h, cuts, forced, solid, opts, line_col_w, fixed, merged, copy, copy_m,
    );
    // Мера с дотягом внепоточных — поток, а не
    // коробка (`OOF_OWN`): сосед встаёт под концом
    // коробки, абсолют продолжается в колонках.
    let (h, over) = match OOF_OWN.with(|m| m.borrow().get(&copy.node_id).copied()) {
        // Только когда за коробкой в стопке есть
        // сосед: последней коробке её дотяг —
        // мера колонок многоколоночника.
        Some((own, full))
            if (full - h).abs() < 0.01 && !solid_box(copy) && ix + 1 < kid_par.len() =>
        {
            (own, over.max(full))
        }
        _ => (h, over),
    };
    (cuts, forced, solid, h, over)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn line_cuts(
    h: f32,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    opts: &RenderOpts,
    line_col_w: Option<f32>,
    fixed: Option<f32>,
    merged: &Computed,
    copy: &Element,
    copy_m: Option<Element>,
) -> (f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>) {
    let (over, cuts, forced, solid) = match with_lines(merged, line_col_w, opts, || {
        shape_full(
            copy_m.as_ref().unwrap_or(copy),
            4,
            ShapeCx {
                unclamped: true,
                ..ShapeCx::COLUMNS
            },
        )
    })
    .filter(|_| {
        // Сетка-стопка с обычными блочными элементами
        // меряется так же точно, как блок (`grid_stack`:
        // ряд = элемент), а `overflow-x: clip` блочное
        // переполнение не прячет (css-overflow-3:
        // `clip` парой к `visible` не делает коробку
        // прокручиваемой). `grid-container-
        // fragmentation-006`: сетка 200 с содержимым
        // 400 в четырёх колонках.
        let plain = plain_block_tree(copy, 4)
            || stacked_flex_tree(copy, 4)
            || (grid_stack(copy)
                && copy.children.iter().all(|n| match n {
                    Node::Element(k) => k.inline || plain_block_tree(k, 3),
                    _ => true,
                }));
        let ov = copy_m.as_ref().unwrap_or(copy);
        let block_visible = matches!(
            ov.style.overflow_y,
            None | Some(crate::style::computed::Overflow::Visible)
        ) && matches!(
            ov.style.overflow_x,
            None | Some(crate::style::computed::Overflow::Visible)
                | Some(crate::style::computed::Overflow::Clip)
        );
        fixed.is_some() && plain && block_visible
    })
    .filter(|s| s.0 > h + 0.01)
    {
        Some(s) => (s.0, s.3, s.4, s.5),
        None => (h, cuts, forced, solid),
    };
    (over, cuts, forced, solid)
}

#[allow(clippy::too_many_arguments)]
pub(super) fn stack_child_of(
    ix: usize,
    mt: f32,
    mb: f32,
    kid_par: &[crate::layout::fragment::types::Par],
    col_vert: bool,
    rows: Option<crate::layout::fragment::types::Rows>,
    fixed: Option<f32>,
    dec: Option<(f32, f32)>,
    copy: &Element,
    cuts: Vec<(f32, f32)>,
    forced: Vec<f32>,
    solid: Vec<(f32, f32)>,
    h: f32,
    over: f32,
    rel: (f32, f32),
    build: impl Fn(bool, usize) -> gpui::AnyElement,
    measure: Option<f32>,
    monolith: bool,
    span: bool,
    kid_copies: usize,
) -> crate::layout::fragment::types::StackChild {
    crate::layout::fragment::types::StackChild {
        measure,
        el: side_margin_wrap(build(true, 0), copy, col_vert),
        frags: if span {
            Vec::new()
        } else {
            (1..kid_copies)
                .map(|i| side_margin_wrap(build(false, i), copy, col_vert))
                .collect()
        },
        monolith,
        fit_whole: false,
        cuts,
        // Тот же подъём, что в мере (Х5): иначе
        // укладка колонок не увидит разрыва,
        // который мера уже посчитала.
        force_before: edge_break(copy, false),
        force_after: edge_break(copy, true),
        avoid_before: edge_avoid(copy, false),
        avoid_after: edge_avoid(copy, true),
        forced,
        solid,
        h,
        mt,
        mb,
        span,
        over,
        rel,
        clone_dec: dec,
        // Монолит с верха колонки переполняет её
        // (`flow.rs` `fill_at`, `overflow_to`) —
        // только при `column-fill: auto` без рядов
        // и без элементов ряда в поддереве.
        overflow_top: fixed.is_some() && rows.is_none() && !parallel_items_inside(copy, 4),
        // Вложенный многоколоночник — маска режет вбок
        // (`flow.rs` `StackChild::nested_cols`).
        nested_cols: multicol_inside(copy, 4),
        par: kid_par[ix],
        positioned: !span
            && (matches!(
                copy.style.position,
                Some(crate::style::computed::Position::Relative)
                    | Some(crate::style::computed::Position::Sticky)
            ) || copy.style.transform.is_some())
            && copy.style.z_index.unwrap_or(0) == 0,
        // Хвост непоследнего фрагмента таблицы — её фоном
        // (`flow.rs` `StackChild::slack`).
        slack: if col_vert {
            None
        } else if table_box(copy) {
            copy.style.background.map(|c| c.to_hsla())
        } else if dec.is_none() && over <= h + 0.01 {
            // Продолжение одного лишь параллельного
            // потока (`over`) — не продолжение коробки:
            // она кончилась, хвоста у неё нет.
            slack_fill(copy)
        } else {
            None
        },
        laid_w: Default::default(),
        // Повтор шапки/подвала таблицы — полосы своими
        // копиями (`flow::Repeat`); та же мера, что у
        // щупов (`repeat_leads`).
        repeat: repeat_bands(copy, fixed, rows)
            .filter(|_| !span && !col_vert)
            .map(
                |(head, foot, geom)| crate::layout::fragment::types::Repeat {
                    head,
                    foot,
                    geom,
                    head_els: match head {
                        Some(_) => (1..kid_copies).map(|i| build(false, i)).collect(),
                        None => Vec::new(),
                    },
                    foot_els: match foot {
                        Some(_) => (0..kid_copies).map(|i| build(false, i)).collect(),
                        None => Vec::new(),
                    },
                },
            ),
    }
}
