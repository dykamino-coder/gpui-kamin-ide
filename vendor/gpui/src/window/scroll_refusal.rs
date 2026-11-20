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
    /// Commands a moved copy cannot place (`Scene::scrollable`).
    Commands,
}
