//! KaminIDE patch: scene identity separates empty paint ranges in different records.

/// Which scene a command index counts in: each scene has its own, kept by
/// a copy of the scene (a layer's content written while a frame holds it).
/// A scroll node painted while a layer records has its range in the layer's
/// scene, not the frame's (`Window::begin_scroll_node_paint`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SceneSpace(u32);

impl Default for SceneSpace {
    fn default() -> Self {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(1);
        Self(NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed))
    }
}
