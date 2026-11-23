//! Правки сетки: подсетка (дорожки родителя), fr в точки, лунки как сетка, абсолютные в сетке.

pub(crate) mod lanes;
mod subgrid_geometry;
pub(crate) mod subgrid_tracks;
pub(super) use crate::dom::fixup_grid::lanes::lanes_as_grid;
pub use crate::dom::fixup_grid::lanes::lanes_row_dir;
pub use crate::dom::fixup_grid::lanes::lanes_to_grid;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_gap_slice;
pub use crate::dom::fixup_grid::subgrid_geometry::subgrid_inhibited;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_slot;
use crate::dom::fixup_grid::subgrid_geometry::subgrid_span;
pub(super) use crate::dom::fixup_grid::subgrid_tracks::subgrid_takes_parent_tracks;

pub(crate) mod hoist;
pub(super) use hoist::hoist_grid_abspos;
