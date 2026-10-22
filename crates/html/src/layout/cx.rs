//! Входы layout, которые бокс передаёт боксам, строящимся внутри него.
//!
//! `ConstraintSpace` в Blink / `ContainingBlock` в Servo: родитель решает, что
//! потомки вправе предполагать (ширина containing block, доступные размеры), и
//! передаёт это вместе с вызовом построения. Здесь контекст едет в
//! [`RenderOpts::cx`], который и так доходит до каждой функции построения и
//! клонируется в отложенные замыкания построения — замыкание видит контекст места,
//! которое его создало.
//!
//! [`RenderOpts::cx`]: crate::layout::options::RenderOpts::cx

use crate::document::state::ValueCx;
use crate::layout::options::RenderOpts;
use crate::layout::positioned::containing_block::Positioning;
use std::rc::Rc;

#[derive(Clone, Default)]
pub struct LayoutCx {
    /// Идентичности отложенного поддерева сохраняют порядок исходного дерева.
    pub(crate) oof_identities:
        Option<Rc<crate::layout::positioned::anchor::identities::Identities>>,
    /// Входы размера корня OOF из геометрии текущего кадра.
    pub(crate) oof_sizes: Option<(u64, crate::layout::positioned::anchor::resolve::SizeInputs)>,
    /// Неизменяемые планы clamp этого построения.
    pub(crate) clamp_plans: Rc<crate::text::clamp::ClampPlans>,
    /// Вход измерения или применения плана активного clamp-контейнера.
    pub(crate) clamp_input: Option<crate::text::clamp::ClampInput>,
    /// Active clamp count root and paragraph sequence of this build.
    pub(crate) clamp_build: crate::text::clamp::ClampBuild,
    /// Child-build requests of the render orchestrator (`ChildBuild`).
    pub(crate) build: crate::layout::build_contracts::ChildBuild,
    /// Paint-order and deferred-geometry boundaries of the box being built.
    pub(crate) paint_depth: crate::paint::effects::paint_scope::Depth,
    /// Scrollable overflow owner captured by probes and deferred text elements.
    pub(crate) scroll_frame: Option<crate::interactive::scroll_extent::Frame>,
    /// Native scroll handle offered to this CSS box by its scroll-area builder.
    pub(crate) scroll_target: Option<Rc<crate::interactive::scroll_target::Target>>,
    /// Prepaint frame shared only by elements of this render tree.
    /// Traversal state captured by this render tree's deferred painters.
    pub(crate) paint: Rc<crate::paint::context::PaintCx>,
    pub(crate) vertical_paint: Rc<crate::text::vertical::VerticalPaint>,
    /// Enclosing atomic inlines relative to this clamp count root.
    pub(crate) clamp_atom_depth: u32,
    /// Inputs offered only to the paragraph currently being built.
    pub(crate) clamp_paragraph: Option<(u64, u32)>,
    /// Build a physical row of atoms without turning its enclosing text line.
    pub(crate) pure_atom_paragraph: bool,
    /// Keep scrollbar hover and drag at rest for a static host capture.
    pub freeze_scrollbars: bool,
    /// Containing-block width in px when the parent's style gives it. A
    /// replaced element without intrinsic size but with a ratio takes its
    /// width from the block equation, i.e. from the containing block (CSS 2.1
    /// §10.3.2, last case); 300×150 applies only when there is none.
    pub(crate) cb_width: Option<f32>,
    /// Width a block-level child with `width: auto` gets in the current
    /// block container: its stretch-fit size (CSS 2.1 §10.3.3; css-sizing-3:
    /// the inline size of an in-flow block-level box is definite). Needed by
    /// `repeat(auto-fill | auto-fit, …)` (css-grid-1 §7.2.3.2) and by
    /// percentages of inline boxes, whose containing block is the paragraph's
    /// block. `None` — honestly unknown (flex, grid, table, inline-block,
    /// float, abspos, multicol, vertical writing).
    pub(crate) avail_w: Option<f32>,
    /// Definite content height of the current block container, for
    /// percentage heights of embedded viewports (CSS 2.1 §10.5).
    pub(crate) avail_h: Option<f32>,
    /// The block list being built is a table cell's (or caption's) own
    /// content: a BFC root whose floats it contains (§10.6.7).
    pub(crate) cell_bfc: bool,
    /// Containing-block layers of the current build pass (`Positioning`):
    /// a new document (`render`, a nested document) starts its own.
    pub(crate) positioning: Rc<Positioning>,
    /// Line-box hand-off for the static-position probe of the inline atom
    /// being built (`LineBoxSlot`); `None` outside such an atom.
    pub(crate) line_box: Option<crate::layout::positioned::containing_block::LineBoxSlot>,
    /// Chain of the sticky boxes and reversed scroll projections the box is
    /// built inside (`ShiftLink`): its scroll-extent probes subtract their
    /// paint-time shifts.
    pub(crate) sticky: Option<crate::interactive::scroll_extent::StickyShift>,
    /// Offer of a positioned inline ancestor as the containing block of the
    /// inline atom being built (`InlineCbOffer`).
    pub(crate) inline_cb: Option<std::rc::Rc<crate::text::inline::InlineCbOffer>>,
    /// Paint keys of CSS 2.1 Appendix E step 8 (`gpui::PaintLast`), in build
    /// order: an ancestor gets a smaller key than its descendants. Shared by
    /// everything that paints into one collector, nested documents included.
    pub(crate) paint_keys: Rc<std::cell::Cell<u64>>,
    /// Positions of the document's nodes in a forward walk and of the LAST
    /// positioned node that keeps no paint key (`unkeyed_positions`).
    pub(crate) unkeyed: Rc<crate::layout::order::Unkeyed>,
    /// How many documents (iframes, SVG document images) the box is nested
    /// in: a guard against cyclic embedding.
    pub(crate) frame_depth: u32,
    /// Built inside a `render` call: an inner `render` must not reset the
    /// step-8 paint collector of the outer one.
    pub(crate) in_render: bool,
    /// `text-box-trim` requests of the trimming containers above, addressed
    /// to the blocks holding their first/last formatted line
    /// (`text::line_trim`).
    pub(crate) trim_requests: Rc<Vec<crate::text::line_trim::TrimRequest>>,
    /// The block whose paragraph is being built and its trim flags.
    pub(crate) trim_para: Option<(u64, crate::text::line_trim::Trim)>,
    /// Base-width sink of the ruby column being built: the first paragraph
    /// built under it takes it and reports its max-content width (ruby
    /// overhang, css-ruby-1 §4.4).
    pub(crate) ruby_base: Option<crate::text::paragraph::RubyBaseOffer>,
    /// Annotation layers of the ruby base being built: nested ruby
    /// annotations stack on them.
    pub(crate) ruby_layers: Option<Rc<crate::text::ruby::layers::Layers>>,
    /// Collection of the annotation stacks of the inline atom being built
    /// (the line grows by them); `None` inside — collection closed.
    pub(crate) ruby_extents:
        Option<std::rc::Rc<std::cell::RefCell<Option<crate::text::paragraph::RubyExtents>>>>,
    /// The document being built: its quirks mode, `@position-try` rules,
    /// medium and the arenas of its `calc()`/`anchor()` values. Set by
    /// `Document::render` and for each nested document.
    pub(crate) doc: BuildDoc,
    /// Nearest gap-decorated container collecting child bounds.
    pub(crate) gap_items: Option<crate::paint::gap_rules::GapItems>,
    /// Intrinsic inline-size measurement offered to this build only.
    pub(crate) inline_measure: Option<crate::text::vertical::InlineMeasure>,
    /// CSS viewport retained by deferred effects and nested document trees.
    pub(crate) paint_viewport: (f32, f32),
    /// Immutable parent frames of this fragmentation measurement.
    pub(crate) lines: Option<Rc<crate::layout::fragment::LineCx>>,
}

/// The document of a build. `Default` is an empty document: a host builds a
/// `Document`'s nodes through `Document::render`, which sets its own.
#[derive(Clone)]
pub(crate) struct BuildDoc(pub(crate) Rc<crate::document::state::DocState>);

impl Default for BuildDoc {
    fn default() -> Self {
        BuildDoc(crate::document::state::DocState::new(
            crate::style::css::Media::default(),
        ))
    }
}

impl LayoutCx {
    pub(crate) fn clamp_plan(&self, key: u64) -> Option<&crate::text::clamp::ClampPlan> {
        if let Some(input) = &self.clamp_input
            && crate::text::clamp::clamp_context(self).is_some_and(|(own, _)| own == key)
        {
            return match input {
                crate::text::clamp::ClampInput::Apply(plan) => Some(plan),
                crate::text::clamp::ClampInput::Measure(_) => None,
            };
        }
        self.clamp_plans.plan(key)
    }

    pub(crate) fn clamp_box_height(&self, node: u64) -> Option<f32> {
        match &self.clamp_input {
            Some(crate::text::clamp::ClampInput::Measure(_)) => None,
            Some(crate::text::clamp::ClampInput::Apply(plan)) => plan
                .sizes
                .iter()
                .find(|(key, _)| *key == node)
                .map(|(_, h)| *h),
            None => self.clamp_plans.box_height(node),
        }
    }

    /// The document being built.
    pub(crate) fn doc(&self) -> &Rc<crate::document::state::DocState> {
        &self.doc.0
    }

    /// Values of the build: `calc()`/`anchor()` bodies parsed while building
    /// the frame go to the document's frame arena.
    pub(crate) fn vcx(&self) -> ValueCx<'_> {
        self.doc
            .0
            .frame_values()
            .with_lines(self.lines.as_ref())
            .with_viewport(self.paint_viewport)
            .with_paint(Some(&self.paint))
    }

    /// The next step-8 paint key: taken on entering an element, before its
    /// children are built.
    pub(crate) fn next_paint_key(&self) -> u64 {
        let v = self.paint_keys.get().wrapping_add(1);
        self.paint_keys.set(v);
        v
    }
}

impl RenderOpts {
    /// Options for the boxes inside the current one: same document options,
    /// context changed by `f`. Boxed — the build recursion is deep and its
    /// stack frames are tight.
    pub(crate) fn inner(&self, f: impl FnOnce(&mut LayoutCx)) -> Box<RenderOpts> {
        let mut opts = Box::new(self.clone());
        f(&mut opts.cx);
        opts
    }
}
