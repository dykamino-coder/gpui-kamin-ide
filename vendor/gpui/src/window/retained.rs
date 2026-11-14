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
/// The frame's scope sink measures several scopes in turn (`begin_scope`,
/// `Window::with_ink_scope`): paint
/// deferred past a scope feeds the scope open at capture, whose ink is no
/// longer kept; the taint of an ended scope is kept as the range of ended
/// scopes tainted so far (`scope_tainted`), which a host whose scopes follow
/// its blocks in order sees as the run of blocks around the tainted ones. The
/// sink stays the size of one that measures a single scope.
#[derive(Default)]
pub struct InkSink {
    bounds: std::cell::Cell<Bounds<ScaledPixels>>,
    /// Bit 0: the open scope (or the sink without scopes) is tainted; bit 1:
    /// `bounds` holds ink; bit 2: an ended scope beyond the range of `late`
    /// was tainted; the bits above: the open scope, 0 if none.
    state: std::cell::Cell<u32>,
    /// The first (low half) and last (high half) ended scope tainted, 0 if none.
    late: std::cell::Cell<u32>,
}

/// KaminIDE patch: the ink sinks of one host's frame, opened by number
/// (`Window::with_ink_sink`). A host that records thousands of blocks a frame
/// keeps their sinks in one list it releases when the frame's records are
/// sealed, instead of a sink inside each kept record: the measure belongs to
/// the frame, the record keeps only its result (Blink accumulates the bounds
/// of a paint chunk in the paint controller's chunk list, not in the client).
/// A sink is added while no paint measures into the list (a build), so a
/// paint never finds the list borrowed.
#[derive(Default)]
pub struct InkSinkList(std::cell::RefCell<Vec<InkSink>>);

impl InkSinkList {
    /// A new, empty sink; its number in the list.
    pub fn open(&self) -> u32 {
        let mut sinks = self.0.borrow_mut();
        sinks.push(InkSink::new());
        (sinks.len() - 1) as u32
    }

    /// `f` of sink `at`; `None` once the list was released.
    pub fn with<R>(&self, at: u32, f: impl FnOnce(&InkSink) -> R) -> Option<R> {
        self.0.borrow().get(at as usize).map(f)
    }

    /// Drop every sink and the list's storage: the frame's measures are read.
    pub fn release(&self) {
        drop(self.0.take());
    }

    fn add_in(&self, at: u32, scope: u32, bounds: Bounds<ScaledPixels>) {
        self.with(at, |sink| sink.add_in(scope, bounds));
    }

    fn taint_in(&self, at: u32, scope: u32) {
        self.with(at, |sink| sink.taint_in(scope));
    }
}

/// The ink sinks open at a point of paint: rarely more than one, and copied
/// into every deferred paint environment, so one sink stays inline. One
/// pointer and a number keep the list as small as a `Vec` (24 bytes with
/// smallvec's `union`): the scene and every paint snapshot keep their size.
pub(crate) type InkSinks = smallvec::SmallVec<[(Rc<InkSinkList>, u32); 1]>;

/// Offer `bounds` painted under scope `scope` to the open sinks `sinks`.
pub(crate) fn ink_add(sinks: &InkSinks, scope: u32, bounds: Bounds<ScaledPixels>) {
    for (list, at) in sinks {
        list.add_in(*at, scope, bounds);
    }
}

/// Taint the open sinks `sinks` in scope `scope`.
pub(crate) fn ink_taint(sinks: &InkSinks, scope: u32) {
    for (list, at) in sinks {
        list.taint_in(*at, scope);
    }
}
const INK_TAINTED: u32 = 1;
const INK_PAINTED: u32 = 2;
const INK_LATE_ALL: u32 = 4;
const INK_SCOPE_SHIFT: u32 = 3;

impl InkSink {
    const fn new() -> Self {
        Self {
            bounds: std::cell::Cell::new(Bounds {
                origin: Point { x: ScaledPixels(0.0), y: ScaledPixels(0.0) },
                size: Size { width: ScaledPixels(0.0), height: ScaledPixels(0.0) },
            }),
            state: std::cell::Cell::new(0),
            late: std::cell::Cell::new(0),
        }
    }

    /// The scope being measured, 0 if the sink never began one.
    pub(crate) fn scope(&self) -> u32 {
        self.state.get() >> INK_SCOPE_SHIFT
    }

    /// Add ink painted under scope `scope` (`Scene::ink_scope`); a sink that
    /// never began a scope takes all of it.
    pub(crate) fn add_in(&self, scope: u32, bounds: Bounds<ScaledPixels>) {
        if self.scope() != 0 && scope != self.scope() {
            return;
        }
        let state = self.state.get();
        self.bounds.set(match state & INK_PAINTED {
            0 => bounds,
            _ => self.bounds.get().union(&bounds),
        });
        self.state.set(state | INK_PAINTED);
    }

    pub(crate) fn taint_in(&self, scope: u32) {
        let open = self.scope();
        if open == 0 || scope == open {
            self.state.set(self.state.get() | INK_TAINTED);
        } else if scope > 0xffff {
            self.state.set(self.state.get() | INK_LATE_ALL);
        } else {
            let late = self.late.get();
            let (first, last) = (late & 0xffff, late >> 16);
            let first = if first == 0 { scope } else { first.min(scope) };
            self.late.set(last.max(scope) << 16 | first);
        }
    }

    /// End the scope being measured and begin a new, empty one; its id.
    pub(crate) fn begin_scope(&self) -> u32 {
        let scope = self.scope() + 1;
        self.state
            .set(scope << INK_SCOPE_SHIFT | self.state.get() & INK_LATE_ALL);
        scope
    }

    /// Whether scope `scope` may have been tainted: while open, or by paint
    /// deferred past it (the range of tainted ended scopes holds it).
    pub(crate) fn scope_tainted(&self, scope: u32) -> bool {
        let (state, late) = (self.state.get(), self.late.get());
        state & INK_LATE_ALL != 0
            || scope == self.scope() && state & INK_TAINTED != 0
            || late != 0 && (late & 0xffff..=late >> 16).contains(&scope)
    }

    /// Forget all scopes and ink: the sink measures as if new.
    pub fn clear(&self) {
        self.state.set(0);
        self.late.set(0);
    }

    /// Union of the placed bounds offered so far; `None` when nothing painted.
    pub fn bounds(&self) -> Option<Bounds<ScaledPixels>> {
        (self.state.get() & INK_PAINTED != 0).then(|| self.bounds.get())
    }

    /// Whether an observed scope had effects beyond offering primitives.
    pub fn tainted(&self) -> bool {
        self.state.get() & INK_TAINTED != 0
    }
}
thread_local! {
    /// The frame's scope sink (`Window::with_ink_scope`), kept outside
    /// `Scene::ink`: a paint snapshot holds only the scope word
    /// (`PaintCtx::ink_scope`), by which deferred paint still feeds it, and no
    /// host allocates a sink for its scopes. Reset with the scene.
    static SCOPES: InkSink = const { InkSink::new() };
}

/// `f` of the scope sink while scope `scope` (`Scene::ink_scope`) is not 0.
pub(crate) fn scoped_ink(scope: u32, f: impl FnOnce(&InkSink)) {
    if scope != 0 {
        SCOPES.with(f);
    }
}

/// A new frame: no scope of the last one is measured any more.
pub(crate) fn reset_ink_scopes() {
    SCOPES.with(InkSink::clear);
}

/// Whether ink scope `scope` of this frame (`Window::with_ink_scope`) was
/// tainted, also by paint deferred past its end.
pub fn ink_scope_tainted(scope: u32) -> bool {
    SCOPES.with(|sink| sink.scope_tainted(scope))
}

/// What one ink scope measured (`Window::with_ink_scope`).
#[derive(Clone, Copy)]
pub struct InkScope {
    /// The scope's id in this frame, for `ink_scope_tainted` after deferred paint.
    pub id: u32,
    /// Union of the placed bounds painted in the scope; `None` when nothing.
    pub bounds: Option<Bounds<ScaledPixels>>,
    /// The scope had effects beyond offering primitives (`InkSink::tainted`).
    pub tainted: bool,
}

/// Paint effects of a scope observed by open ink sinks.
pub(crate) struct InkProbe {
    start: PaintIndex,
    groups: usize,
    callbacks: usize,
    allowed: (u32, u32),
    clean: bool,
}

impl InkProbe {
    pub(crate) fn open(window: &Window) -> Option<Self> {
        let scene = &window.next_frame.scene;
        (!scene.ink.is_empty() || scene.ink_scope != 0).then(|| Self {
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
            && end.mouse_listeners_index - start.mouse_listeners_index == (allowed.0 - self.allowed.0) as usize
            && start.input_handlers_index == end.input_handlers_index
            && start.cursor_styles_index == end.cursor_styles_index
            && end.accessed_element_states_index - start.accessed_element_states_index
                == (allowed.1 - self.allowed.1) as usize
            && start.tab_handle_index == end.tab_handle_index
            && self.groups == window.next_frame.scene.groups.len()
            && self.callbacks == window.next_frame_callback_count();
        if !quiet {
            let scope = window.next_frame.scene.ink_scope;
            ink_taint(&window.next_frame.scene.ink, scope);
            scoped_ink(scope, |sink| sink.taint_in(scope));
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
/// every node again: the subtree's elements need not be built. It names the
/// range of the subtree's requests in its pass, so it is a few numbers
/// (`Copy`) and holds for the next pass only; the claim renews it for the
/// pass after.
#[derive(Clone, Copy)]
pub struct RetainedSubtree {
    pub(crate) stamp: u64,
    /// The pass whose request sequence holds the nodes.
    pub(crate) pass: std::num::NonZeroU64,
    pub(crate) start: u32,
    pub(crate) len: u32,
}

impl RetainedSubtree {
    /// Number of layout nodes in the subtree.
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Whether the subtree has no nodes (never: the root is one).
    pub fn is_empty(&self) -> bool {
        self.len == 0
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
    /// Paint `f` with the line layouts it looks up released from the text
    /// system's frame caches: the next frame does not find them. For a
    /// subtree whose layout the host keeps while later frames do not paint
    /// it, so its shaped lines go with this paint instead of a frame later.
    pub fn with_line_layouts_released<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> R {
        crate::text_system::with_layouts_released(|| f(self))
    }

    /// Paint `f` with sink `at` of `list` measuring its ink (see [`InkSink`],
    /// [`InkSinkList`]).
    pub fn with_ink_sink<R>(
        &mut self,
        (list, at): (&Rc<InkSinkList>, u32),
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.next_frame.scene.ink.push((list.clone(), at));
        let probe = InkProbe::open(self);
        let result = f(self);
        if let Some(probe) = probe {
            probe.close(self);
        }
        self.next_frame.scene.ink.pop();
        result
    }

    /// KaminIDE patch: offer `bounds` (window coordinates of the root paint
    /// environment) to the open ink sinks as the ink of paint the caller
    /// skipped because the content mask would cull all of it.
    pub fn offer_ink(&mut self, bounds: Bounds<Pixels>) {
        let scene = &self.next_frame.scene;
        if scene.ink.is_empty() && scene.ink_scope == 0 {
            return;
        }
        let scaled = bounds.scale(self.scale_factor());
        let scene = &self.next_frame.scene;
        ink_add(&scene.ink, scene.ink_scope, scaled);
        scoped_ink(scene.ink_scope, |sink| sink.add_in(scene.ink_scope, scaled));
    }

    /// Paint `f` measuring its ink in a new scope of the frame's scope sink:
    /// a host that measures many subtrees in turn (one per block) needs no
    /// sink of its own, and paint deferred past the scope taints it by id
    /// (`ink_scope_tainted`). A scope opened inside another ends the outer
    /// one, which is then tainted: its later ink would go unmeasured.
    pub fn with_ink_scope<R>(&mut self, f: impl FnOnce(&mut Self) -> R) -> (R, InkScope) {
        let outer = self.next_frame.scene.ink_scope;
        let id = SCOPES.with(|sink| {
            let id = sink.begin_scope();
            if outer != 0 {
                sink.taint_in(outer);
            }
            id
        });
        self.next_frame.scene.ink_scope = id;
        let probe = InkProbe::open(self);
        let result = f(self);
        if let Some(probe) = probe {
            probe.close(self);
        }
        self.next_frame.scene.ink_scope = outer;
        let scope = SCOPES.with(|sink| match sink.scope() == id {
            true => InkScope { id, bounds: sink.bounds(), tainted: sink.tainted() },
            // A scope opened inside ended this one before its paint did.
            false => InkScope { id, bounds: None, tainted: true },
        });
        (result, scope)
    }

    /// Run paint `f`, which reports whether its mouse listeners and element
    /// state reads are inert while nothing it guards is visible: listeners
    /// that act only on a hit of a hitbox under the mouse or on a gesture the
    /// state records, and state that is at its default. Open ink sinks then
    /// allow those effects; otherwise they are tainted.
    pub fn offscreen_inert<R>(&mut self, f: impl FnOnce(&mut Self) -> (R, bool)) -> R {
        if self.next_frame.scene.ink.is_empty() && self.next_frame.scene.ink_scope == 0 {
            return f(self).0;
        }
        let start = self.paint_index();
        let (result, inert) = f(self);
        let end = self.paint_index();
        let scene = &mut self.next_frame.scene;
        if inert {
            scene.ink_allowed.0 += (end.mouse_listeners_index - start.mouse_listeners_index) as u32;
            scene.ink_allowed.1 +=
                (end.accessed_element_states_index - start.accessed_element_states_index) as u32;
        } else {
            ink_taint(&scene.ink, scene.ink_scope);
            scoped_ink(scene.ink_scope, |sink| sink.taint_in(scene.ink_scope));
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
        self.layout_engine.as_mut()?.capture_subtree(start, root)
    }

    /// Claim a subtree captured in the previous pass whole. `None` when a node
    /// of it changed or was dropped. A claimed `layout` becomes the capture of
    /// this pass: the caller keeps it to claim the subtree in the next one.
    /// Its leaves are not measured again: if the
    /// layout needs a new measurement, `layout_stale` reports it after
    /// `compute_layout`, and the caller must discard this layout (the frame
    /// tree drops itself at the end of such a pass).
    pub fn request_retained_layout(&mut self, layout: &mut RetainedSubtree) -> Option<LayoutId> {
        self.invalidator.debug_assert_prepaint();
        self.layout_engine.as_mut()?.claim_subtree(layout)
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
            engine.keep_measures(layout, compact);
        }
    }

    /// Let the kept measure functions of the leaves of `layout`, claimed in
    /// this pass, prepare at request time what their measures will read
    /// (`Measure::prepare`), so a host builds its measure state where a
    /// request would have built it.
    pub fn prepare_retained_measures(&mut self, layout: &RetainedSubtree) {
        if let Some(engine) = self.layout_engine.as_mut() {
            engine.prepare_measures(layout);
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
            .nodes_layout_bounds(layout, scale_factor)?;
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