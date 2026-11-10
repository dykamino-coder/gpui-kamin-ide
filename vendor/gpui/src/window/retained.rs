//! KaminIDE patch: host-keyed retained subtrees across frames.
//!
//! Entity views are cached by `Context::notify`; a host engine that owns its
//! own invalidation (a document with dirty bits) needs the same reuse keyed by
//! its inputs instead. The host records the prepaint and paint index ranges of
//! a subtree, stores them in element state (which only survives a frame in
//! which it was accessed), and replays them while every input it keys on is
//! unchanged. GPUI reports which recorded ranges are replayable at all and the
//! window inputs (events, focus) the host cannot observe itself.

use super::*;

/// Prepaint output of a subtree in the frame that recorded it.
#[derive(Clone, Default)]
pub struct RetainedPrepaint(Range<PrepaintStateIndex>);

/// Paint output of a subtree in the frame that recorded it.
#[derive(Clone, Default)]
pub struct RetainedPaint {
    range: Range<PaintIndex>,
    replayable: bool,
}

impl RetainedPaint {
    /// Whether these commands can be copied into a later frame unchanged.
    /// Composition groups live outside the command list (`Scene::groups`) and
    /// isolated scenes are discarded by their owner, so neither replays.
    pub fn replayable(&self) -> bool {
        self.replayable
    }

    /// Extend this output by `next`, recorded right after it in the same
    /// frame: one replay of the joined range copies exactly the commands of
    /// both, in order. `false` leaves this output unchanged when `next` does
    /// not start where this one ends.
    pub fn append(&mut self, next: &RetainedPaint) -> bool {
        if self.range.end != next.range.start {
            return false;
        }
        self.range.end = next.range.end.clone();
        self.replayable &= next.replayable;
        true
    }
}

impl Window {
    /// Whether the current element draws in window coordinates without an
    /// enclosing transform, opacity, mask, 3D plane, composition group or
    /// segment capture: replayed commands carry their environment baked in.
    pub fn retained_root_environment(&self) -> bool {
        self.transformation_stack.is_empty()
            && self.element_opacity == 1.0
            && self.mask_map.is_none()
            && self.mask_scale.is_none()
            && self.plane_contexts.is_empty()
            && self.exact_element_offset_depth == 0
            && !self.next_frame.scene.in_group
            && self.next_frame.scene.segment_capture.is_none()
    }

    /// Counter of window inputs that can change element state without the
    /// host's knowledge: dispatched platform events and focus changes.
    pub fn interaction_epoch(&self) -> u64 {
        self.interaction_epoch
    }

    /// Callbacks scheduled for the next frame so far in this draw. A subtree
    /// that adds one (animation frames, deferred notifications) is not settled.
    pub fn next_frame_callback_count(&self) -> usize {
        self.next_frame_callbacks.borrow().len()
    }

    /// Number of frames this window has drawn. A record made while drawing
    /// frame `n` replays only while `frame_serial() == n + 1`, i.e. while the
    /// frame it was made in is the rendered one.
    pub fn frame_serial(&self) -> u64 {
        self.frame_serial
    }

    /// Run the prepaint of a subtree and return its range in this frame.
    pub fn record_retained_prepaint<R>(
        &mut self,
        f: impl FnOnce(&mut Self) -> R,
    ) -> (R, RetainedPrepaint) {
        let start = self.prepaint_index();
        let result = f(self);
        (result, RetainedPrepaint(start..self.prepaint_index()))
    }

    /// Copy the prepaint output recorded in the previous frame into this one.
    /// The caller guarantees the record comes from the rendered frame.
    pub fn replay_retained_prepaint(&mut self, record: &RetainedPrepaint) -> RetainedPrepaint {
        let start = self.prepaint_index();
        self.reuse_prepaint(record.0.clone());
        RetainedPrepaint(start..self.prepaint_index())
    }

    /// Whether a record made in the rendered frame can be replayed into this
    /// one. Deferred draws paint after the whole tree, outside the subtree's
    /// paint range, and nested deferred draws are appended after their owner;
    /// when the subtree registered any, every deferred draw from its first one
    /// on must be free of composition groups.
    pub fn retained_replayable(&self, prepaint: &RetainedPrepaint, paint: &RetainedPaint) -> bool {
        use crate::scene::{PaintOperation, Primitive};
        let frame = &self.rendered_frame;
        let first = prepaint.0.start.deferred_draws_index;
        let groups = |draw: &DeferredDraw| {
            let range = draw.paint_range.start.scene_index..draw.paint_range.end.scene_index;
            frame.scene.paint_operations[range].iter().any(|operation| {
                matches!(
                    operation,
                    PaintOperation::Primitive(Primitive::Surface(surface)) if surface.group > 0
                )
            })
        };
        paint.replayable
            && (first == prepaint.0.end.deferred_draws_index
                || !frame.deferred_draws[first..].iter().any(groups))
    }

    /// Run the paint of a subtree and return its range in this frame.
    pub fn record_retained_paint<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> (R, RetainedPaint) {
        let root = self.retained_root_environment();
        let groups = self.next_frame.scene.groups.len();
        let start = self.paint_index();
        let result = f(self);
        let replayable = root && groups == self.next_frame.scene.groups.len();
        (result, RetainedPaint { range: start..self.paint_index(), replayable })
    }

    /// Copy replayable paint output recorded in the previous frame into this one.
    /// The caller guarantees the record comes from the rendered frame.
    pub fn replay_retained_paint(&mut self, record: &RetainedPaint) -> RetainedPaint {
        debug_assert!(record.replayable, "non-replayable paint record");
        let start = self.paint_index();
        self.reuse_paint(record.range.clone());
        RetainedPaint { range: start..self.paint_index(), replayable: true }
    }
}

/// KaminIDE patch: placed bounds of everything a retained subtree offered to
/// the scene in one frame, culled primitives included, in device pixels.
///
/// A host that skips a settled subtree may do so only where every primitive
/// of the subtree would be culled; the sink measures that ink while the
/// subtree paints. Paint deferred past the subtree (`Window::paint_ctx`
/// snapshots) keeps feeding the sinks open at capture. A sink is tainted when
/// a scope it observes does more than offer primitives: input state, element
/// state, composition groups, a non-root paint environment, accessibility.
///
/// One sink may measure several scopes in turn (`begin_scope`): paint
/// deferred past a scope feeds the scope open at capture, whose ink is no
/// longer kept and whose taint is kept for `scope_tainted`.
#[derive(Default)]
pub struct InkSink {
    bounds: std::cell::Cell<Option<Bounds<ScaledPixels>>>,
    tainted: std::cell::Cell<bool>,
    scope: std::cell::Cell<u32>,
    /// Ended scopes tainted by deferred paint.
    late: std::cell::RefCell<Vec<u32>>,
}

impl InkSink {
    fn add_in(&self, scope: u32, bounds: Bounds<ScaledPixels>) {
        if scope != self.scope.get() {
            return;
        }
        self.bounds.set(Some(match self.bounds.get() {
            Some(ink) => ink.union(&bounds),
            None => bounds,
        }));
    }

    fn taint_in(&self, scope: u32) {
        if scope == self.scope.get() {
            self.tainted.set(true);
        } else if !self.late.borrow().contains(&scope) {
            self.late.borrow_mut().push(scope);
        }
    }

    /// End the scope being measured and begin a new, empty one; its id.
    pub fn begin_scope(&self) -> u32 {
        let scope = self.scope.get() + 1;
        self.scope.set(scope);
        self.bounds.set(None);
        self.tainted.set(false);
        scope
    }

    /// Whether scope `scope` was tainted, while open or by paint deferred
    /// past it.
    pub fn scope_tainted(&self, scope: u32) -> bool {
        (scope == self.scope.get() && self.tainted.get()) || self.late.borrow().contains(&scope)
    }

    /// Union of the placed bounds offered so far; `None` when nothing painted.
    pub fn bounds(&self) -> Option<Bounds<ScaledPixels>> {
        self.bounds.get()
    }

    /// Whether an observed scope had effects beyond offering primitives.
    pub fn tainted(&self) -> bool {
        self.tainted.get()
    }
}

/// KaminIDE patch: an open ink sink with the scope it measures.
#[derive(Clone)]
pub(crate) struct InkScope {
    sink: Rc<InkSink>,
    scope: u32,
}

impl InkScope {
    pub(crate) fn add(&self, bounds: Bounds<ScaledPixels>) {
        self.sink.add_in(self.scope, bounds);
    }

    fn taint(&self) {
        self.sink.taint_in(self.scope);
    }

    /// The same sink measuring the same scope.
    pub(crate) fn same(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.sink, &other.sink) && self.scope == other.scope
    }
}

/// Paint effects of a scope observed by open ink sinks.
pub(crate) struct InkProbe {
    start: PaintIndex,
    groups: usize,
    callbacks: usize,
    allowed: (usize, usize),
    clean: bool,
}

impl InkProbe {
    pub(crate) fn open(window: &Window) -> Option<Self> {
        (!window.next_frame.scene.ink.is_empty()).then(|| Self {
            start: window.paint_index(),
            groups: window.next_frame.scene.groups.len(),
            callbacks: window.next_frame_callback_count(),
            allowed: window.next_frame.scene.ink_allowed,
            clean: window.retained_root_environment() && !window.a11y.is_active(),
        })
    }

    pub(crate) fn close(self, window: &Window) {
        let end = window.paint_index();
        let start = &self.start;
        let allowed = window.next_frame.scene.ink_allowed;
        let quiet = self.clean
            && end.mouse_listeners_index - start.mouse_listeners_index == allowed.0 - self.allowed.0
            && start.input_handlers_index == end.input_handlers_index
            && start.cursor_styles_index == end.cursor_styles_index
            && end.accessed_element_states_index - start.accessed_element_states_index
                == allowed.1 - self.allowed.1
            && start.tab_handle_index == end.tab_handle_index
            && self.groups == window.next_frame.scene.groups.len()
            && self.callbacks == window.next_frame_callback_count();
        if !quiet {
            for scope in &window.next_frame.scene.ink {
                scope.taint();
            }
        }
    }
}

impl RetainedPrepaint {
    /// Whether the recorded prepaint registered nothing a later frame could
    /// miss: no tooltips, deferred draws or element state, and only plain
    /// hitboxes. Returns the union of those hitboxes (`Some(None)` for none):
    /// a frame that skips the subtree misses them only where they lie outside
    /// the content mask, where they could not be hit.
    pub fn quiet(&self, window: &Window) -> Option<Option<Bounds<Pixels>>> {
        let (start, end) = (&self.0.start, &self.0.end);
        let frame = &window.next_frame;
        let hitboxes = frame.hitboxes.get(start.hitboxes_index..end.hitboxes_index)?;
        let plain = hitboxes.iter().zip(start.hitboxes_index..).all(|(hitbox, index)| {
            hitbox.behavior == crate::HitboxBehavior::Normal
                && !frame.hitbox_scopes.contains_key(&hitbox.id)
                && !frame.hitbox_entry_scopes.contains_key(&index)
        });
        let quiet = plain
            && start.tooltips_index == end.tooltips_index
            && start.deferred_draws_index == end.deferred_draws_index
            && start.accessed_element_states_index == end.accessed_element_states_index;
        quiet.then(|| {
            hitboxes
                .iter()
                .map(|hitbox| hitbox.bounds)
                .reduce(|a, b| a.union(&b))
        })
    }
}

/// KaminIDE patch: the layout nodes one subtree requested in a pass
/// (`Window::capture_layout`). A later pass that keeps the root's stamp can
/// claim them whole (`Window::request_retained_layout`) instead of requesting
/// every node again: the subtree's elements need not be built.
#[derive(Clone)]
pub struct RetainedSubtree {
    stamp: u64,
    nodes: Rc<[::taffy::tree::NodeId]>,
}

impl RetainedSubtree {
    /// Number of layout nodes in the subtree.
    pub fn len(&self) -> usize {
        self.nodes.len()
    }

    /// Whether the subtree has no nodes (never: the root is one).
    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// KaminIDE patch: an exact layout leaf standing in for a laid out subtree
/// (`Window::layout_snapshot`), valid at the device scale it was taken at.
#[derive(Clone, Copy)]
pub struct LayoutSnapshot {
    leaf: crate::taffy::ExactLeaf,
    scale: f32,
}

impl LayoutSnapshot {
    /// Device scale of the layout the snapshot was taken from.
    pub fn scale(&self) -> f32 {
        self.scale
    }

    /// Border-box width of the snapshot leaf, in logical pixels; for an
    /// unpainted box ([`Window::layout_snapshot_unpainted`]) at least the
    /// width of its content.
    pub fn width(&self) -> f32 {
        self.leaf.width() / self.scale
    }

    /// The same leaf with its width left to the parent: the stretch-fit
    /// width of a block-level box (CSS2 §10.3.3), for a host that knows the
    /// subtree's height and overflow do not depend on that width.
    pub fn stretched(self) -> Self {
        Self { leaf: self.leaf.stretched(), ..self }
    }
}

impl Window {
    /// Paint `f` with `sink` measuring its ink (see [`InkSink`]).
    pub fn with_ink_sink<R>(&mut self, sink: Rc<InkSink>, f: impl FnOnce(&mut Self) -> R) -> R {
        let scope = sink.scope.get();
        self.next_frame.scene.ink.push(InkScope { sink, scope });
        let probe = InkProbe::open(self);
        let result = f(self);
        if let Some(probe) = probe {
            probe.close(self);
        }
        self.next_frame.scene.ink.pop();
        result
    }

    /// Run paint `f`, which reports whether its mouse listeners and element
    /// state reads are inert while nothing it guards is visible: listeners
    /// that act only on a hit of a hitbox under the mouse or on a gesture the
    /// state records, and state that is at its default. Open ink sinks then
    /// allow those effects; otherwise they are tainted.
    pub fn offscreen_inert<R>(&mut self, f: impl FnOnce(&mut Self) -> (R, bool)) -> R {
        if self.next_frame.scene.ink.is_empty() {
            return f(self).0;
        }
        let start = self.paint_index();
        let (result, inert) = f(self);
        let end = self.paint_index();
        let scene = &mut self.next_frame.scene;
        if inert {
            scene.ink_allowed.0 += end.mouse_listeners_index - start.mouse_listeners_index;
            scene.ink_allowed.1 +=
                end.accessed_element_states_index - start.accessed_element_states_index;
        } else {
            for scope in &scene.ink {
                scope.taint();
            }
        }
        result
    }

    /// The exact leaf of the laid out node `id`: an inflexible normal-flow item
    /// of a single-line column flex container, or a block-level box of a block
    /// container whose descendant margins do not adjoin its edges, whose
    /// overflow stays inside its border box. `None` when the node does not
    /// qualify. The snapshot is a few numbers (`Copy`).
    pub fn layout_snapshot(&self, id: LayoutId) -> Option<LayoutSnapshot> {
        let leaf = self.layout_engine.as_ref()?.exact_leaf(id)?;
        Some(LayoutSnapshot { leaf, scale: self.scale_factor() })
    }

    /// The exact leaf of a laid out node that paints nothing of its own and
    /// holds its content at its inline-start edge whatever its width (an
    /// anonymous wrapper box; the caller guarantees both). Unlike
    /// [`Self::layout_snapshot`] its content may overflow its inline-end edge:
    /// the leaf takes the parent's stretch fit, at least the content's width
    /// ([`LayoutSnapshot::width`]), which the parent places and unions into
    /// its scrollable overflow exactly like the node with its content.
    pub fn layout_snapshot_unpainted(&self, id: LayoutId) -> Option<LayoutSnapshot> {
        let leaf = self.layout_engine.as_ref()?.unpainted_leaf(id)?;
        Some(LayoutSnapshot { leaf, scale: self.scale_factor() })
    }

    /// Position in this pass's layout requests; pass it to `capture_layout`
    /// after requesting a subtree.
    pub fn layout_request_position(&self) -> usize {
        self.layout_engine
            .as_ref()
            .map_or(0, |engine| engine.request_position())
    }

    /// The nodes of the subtree `root` requested since `start`, when the frame
    /// tree keeps them (`None` otherwise).
    pub fn capture_layout(&mut self, start: usize, root: LayoutId) -> Option<RetainedSubtree> {
        let (stamp, nodes) = self.layout_engine.as_mut()?.capture_subtree(start, root)?;
        Some(RetainedSubtree { stamp, nodes })
    }

    /// Claim a subtree captured in the previous pass whole. `None` when a node
    /// of it changed or was dropped. Its leaves are not measured again: if the
    /// layout needs a new measurement, `layout_stale` reports it after
    /// `compute_layout`, and the caller must discard this layout (the frame
    /// tree drops itself at the end of such a pass).
    pub fn request_retained_layout(&mut self, layout: &RetainedSubtree) -> Option<LayoutId> {
        self.invalidator.debug_assert_prepaint();
        self.layout_engine
            .as_mut()?
            .claim_subtree(layout.stamp, &layout.nodes)
    }

    /// Keep the measure functions of the leaves of `layout` (requested or
    /// claimed in this pass) for the next pass: claimed whole there, the
    /// subtree is laid out at new constraints by those functions instead of
    /// going stale. The caller vouches that they read nothing else that
    /// changes between the passes, and keeps them pass by pass. With `compact`,
    /// a host measure with a smaller form (CompactMeasure) is replaced by it;
    /// without, it stays as its request made it, so the keep allocates nothing.
    pub fn keep_retained_measures(&mut self, layout: &RetainedSubtree, compact: bool) {
        if let Some(engine) = self.layout_engine.as_mut() {
            engine.keep_measures(&layout.nodes, compact);
        }
    }

    /// Let the kept measure functions of the leaves of `layout`, claimed in
    /// this pass, prepare at request time what their measures will read
    /// (`Measure::prepare`), so a host builds its measure state where a
    /// request would have built it.
    pub fn prepare_retained_measures(&mut self, layout: &RetainedSubtree) {
        if let Some(engine) = self.layout_engine.as_mut() {
            engine.prepare_measures(&layout.nodes);
        }
    }

    /// Union of the layout bounds of every node of `layout` in this pass,
    /// relative to the window like [`Self::layout_bounds`].
    pub fn retained_layout_bounds(&mut self, layout: &RetainedSubtree) -> Option<Bounds<Pixels>> {
        self.invalidator.debug_assert_prepaint();
        let scale_factor = self.scale_factor();
        let mut bounds = self
            .layout_engine
            .as_mut()?
            .nodes_layout_bounds(&layout.nodes, scale_factor)?;
        let offset = if self.exact_element_offset_depth > 0 {
            self.element_offset()
        } else {
            self.pixel_snap_point(self.element_offset())
        };
        bounds.origin += offset;
        Some(bounds)
    }

    /// Whether this pass measured a leaf of a subtree claimed whole.
    pub fn layout_stale(&self) -> bool {
        self.layout_engine
            .as_ref()
            .is_some_and(|engine| engine.layout_stale())
    }

    /// Request the leaf of `snapshot` in this frame's layout. The caller
    /// guarantees the device scale and the parent are those of the snapshot.
    pub fn request_snapshot_layout(&mut self, snapshot: &LayoutSnapshot) -> LayoutId {
        self.invalidator.debug_assert_prepaint();
        self.layout_engine
            .as_mut()
            .expect("layout engine during request_layout")
            .request_exact_leaf(&snapshot.leaf)
    }
}