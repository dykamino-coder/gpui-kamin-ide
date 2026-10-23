//! Геометрия правил промежутков: вид контейнера, промежутки, дорожки, направление и отступы (вторая
//! половина gap_rule_spec).

use crate::layout::multicol::gap_rules::GapLayout;
use crate::style::computed::FlexDir;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;

pub(super) fn gap_rule_layout(
    merged: &Computed,
    size: f32,
    s: &Computed,
    col: Option<crate::layout::multicol::gap_rules::GapAxisRule>,
    row: Option<crate::layout::multicol::gap_rules::GapAxisRule>,
) -> Option<crate::layout::multicol::gap_rules::GapRuleSpec> {
    let vertical = merged.wm().vertical();
    // Лунки идут путём сетки (`dom::lanes_to_grid`: `display: grid` с
    // пометкой `lanes_taffy`), но промежутки у них — ленты: главные между
    // лентами через всё поле содержимого, поперечные — между элементами
    // ленты (Blink `GridLanesGapAccumulator`).
    let lanes = merged.lanes_taffy || merged.display == Some(Display::GridLanes);
    let kind = match merged.display {
        _ if merged.lanes_taffy => GapLayout::Lines {
            stacked_vertically: crate::dom::lanes_row_dir(merged) != vertical,
        },
        Some(Display::Grid) | Some(Display::InlineGrid) => GapLayout::Grid,
        Some(Display::GridLanes) => {
            // Направление лент — как в `lanes()`: явное или по той оси, где
            // объявлены дорожки.
            let row_tracks = merged.grid_rows.is_some()
                || merged.auto_repeat_rows.is_some()
                || merged.grid_auto_fill_row.is_some();
            let col_tracks = merged.grid_tracks.is_some()
                || merged.auto_repeat_cols.is_some()
                || merged.grid_auto_fill_min.is_some();
            GapLayout::Lines {
                stacked_vertically: merged.lanes_row.unwrap_or(row_tracks && !col_tracks),
            }
        }
        _ => {
            // Строки гибкого контейнера уложены поперёк главной оси; в
            // вертикальном письме `row` идёт по вертикали.
            let row_dir = !matches!(s.flex_dir, Some(FlexDir::Col) | Some(FlexDir::ColReverse));
            GapLayout::Lines {
                stacked_vertically: row_dir != vertical,
            }
        }
    };
    let gap_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Em(k)) => Some(k * size),
        _ => None,
    };
    let (row_gap, col_gap) = match s.gap {
        Some((r, c)) => (gap_px(r), gap_px(c)),
        None => (None, None),
    };
    let (gap_x, gap_y) = if vertical {
        (row_gap, col_gap)
    } else {
        (col_gap, row_gap)
    };
    // Дорожки шаблона в точках. Берётся ТОЛЬКО целиком точечный список: доли
    // `fr`, проценты и дорожки по содержимому разрешает раскладка, а здесь
    // использованного размера контейнера ещё нет. Строки и ленты
    // (`GapLayout::Lines`) шаблона не имеют вовсе: у каждой строки свои
    // промежутки между элементами (css-gaps-1 §gap-flex).
    let track_px = |list: Option<&Vec<crate::style::computed::TrackSize>>| -> Option<Vec<f32>> {
        use crate::style::computed::{Track, TrackSize};
        let list = list?;
        if list.len() < 2 {
            return None;
        }
        list.iter()
            .map(|t| match t {
                TrackSize::Single(Track::Px(v)) => Some(*v),
                _ => None,
            })
            .collect()
    };
    let (tpl_rows, tpl_cols) = match kind {
        GapLayout::Grid => (
            track_px(merged.grid_rows.as_ref()),
            track_px(merged.grid_tracks.as_ref()),
        ),
        GapLayout::Lines { .. } | GapLayout::Multicol => (None, None),
    };
    // В вертикальном письме колонки сетки идут по y — тем же поворотом, что и
    // `gap_x`/`gap_y` строкой выше.
    let (tracks_x, tracks_y) = if vertical {
        (tpl_rows, tpl_cols)
    } else {
        (tpl_cols, tpl_rows)
    };
    // Логическое начало осей: в горизонтальном письме колонки (x) идут
    // справа налево при `rtl`; в вертикальном ряды (x) — справа налево при
    // `*-rl`, колонки (y) — снизу вверх при `sideways-lr`, и `rtl` это
    // переворачивает (эталоны `grid-gap-decorations-multi-value-writing-mode`).
    let rtl = merged.wm().rtl();
    let sideways_lr = vertical && merged.wm().sideways_lr();
    let (mut rev_x, mut rev_y) = if vertical {
        (merged.wm().block_rl(), sideways_lr != rtl)
    } else {
        (rtl, false)
    };
    // Grid lanes count grid-axis gaps from the first track in track order
    // and stacking-axis gaps from the fill start (css-gaps-1 §lists, Blink
    // `GridLanesGapAccumulator`): `track-reverse` and `fill-reverse` flip the
    // logical start of their axis.
    if lanes && let GapLayout::Lines { stacked_vertically } = kind {
        let (grid_rev, stack_rev) = (merged.lanes_track_reverse, merged.lanes_fill_reverse);
        if stacked_vertically {
            rev_y ^= grid_rev;
            rev_x ^= stack_rev;
        } else {
            rev_x ^= grid_rev;
            rev_y ^= stack_rev;
        }
    }
    if !lanes && let GapLayout::Lines { stacked_vertically } = kind {
        let items = matches!(
            merged.flex_dir,
            Some(FlexDir::RowReverse | FlexDir::ColReverse)
        );
        let lines = merged.flex_wrap_reverse == Some(true);
        if stacked_vertically {
            rev_x ^= items;
            rev_y ^= lines;
        } else {
            rev_x ^= lines;
            rev_y ^= items;
        }
    }
    // Поля контейнера: художник занимает его паддинг-бокс, а протяжённость
    // главных промежутков строк и лент считается от поля содержимого.
    let pad_px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * size,
        _ => 0.0,
    };
    let pad = [
        pad_px(s.padding.top),
        pad_px(s.padding.right),
        pad_px(s.padding.bottom),
        pad_px(s.padding.left),
    ];
    let lines_extent = match merged.display {
        _ if lanes => 2,
        Some(Display::Grid) | Some(Display::InlineGrid) => 0,
        _ => 1,
    };
    Some(crate::layout::multicol::gap_rules::GapRuleSpec {
        pad,
        lines_extent,
        flex_line_stretch: !lanes
            && matches!(
                merged.align_content,
                None | Some(crate::style::computed::Justify::Stretch)
            )
            && matches!(
                merged.align_items,
                None | Some(
                    crate::style::computed::Align::Start | crate::style::computed::Align::Stretch
                )
            ),
        lanes_content_aligned: lanes
            && if crate::dom::lanes_row_dir(merged) {
                merged.justify_content.is_some()
            } else {
                merged.align_content.is_some()
            },
        rev_x,
        rev_y,
        col,
        row,
        kind,
        vertical,
        rtl: merged.wm().rtl(),
        column_over_row: s.rule_column_over_row == Some(true),
        gap_x,
        gap_y,
        tracks_x,
        tracks_y,
    })
}
