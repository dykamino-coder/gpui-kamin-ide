//! KaminIDE patch: why a retained record is built instead of replayed scrolled.

/// Why a record cannot be replayed scrolled (`Window::scroll_replay_plan`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScrollRefusal {
    /// Not replayable here: a scene layer or capture, an open layer or depth context.
    Environment,
    /// The record registered deferred draws.
    DeferredDraws,
    /// A node was not painted in the record's scene, or lost a command.
    Node,
    /// A node painted outside the record's range.
    NodeRange,
    /// A node's new clip shows more than its interest rect recorded.
    Interest,
    /// Paint that follows a moved node outside it.
    Dependent,
    /// Composition groups that cannot be moved with their nodes.
    Groups,
    // Commands a moved copy cannot place (`Scene::scroll_check`):
    /// A bottom layer in the record's range.
    BottomLayer,
    /// A depth context or plane (sorted, not painted in order).
    DepthSort,
    /// A layer that ends without its start, or stays open.
    UnbalancedLayer,
    /// A primitive or layer of another node inside a node's layer.
    ForeignMark,
    /// A primitive in a node's range without a scroll mark.
    Unmarked,
    /// A layer in a node's range without a scroll mark.
    UnmarkedLayer,
    /// A retained scene layer in a node's range.
    RetainedLayer,
    /// A reservoir inside a layer, or one that draws past its node's range.
    LayerReservoir,
}

impl ScrollRefusal {
    /// Every refusal, in counter order.
    pub const ALL: [ScrollRefusal; 15] = [
        Self::Environment,
        Self::DeferredDraws,
        Self::Node,
        Self::NodeRange,
        Self::Interest,
        Self::Dependent,
        Self::Groups,
        Self::BottomLayer,
        Self::DepthSort,
        Self::UnbalancedLayer,
        Self::ForeignMark,
        Self::Unmarked,
        Self::UnmarkedLayer,
        Self::RetainedLayer,
        Self::LayerReservoir,
    ];
}
