//! Internal layout marks of an element: protocol between layout passes.
//!
//! Layout rewrites the tree it renders (float rows and band hosts, multicol
//! stack children, spanner hoisting, anonymous table cells) and has to tell a
//! later pass how a node was produced. These marks live in a typed field of
//! [`super::Element`], never in `Element::attrs` or in the tag name: author
//! markup (`dom/walk`) fills only the attributes and the tag, so a document
//! cannot forge a mark, and a mark never leaks into attribute selectors or SVG
//! serialization.

/// Typed internal flags set by layout on the boxes it synthesizes or rewrites.
///
/// The fields are crate-private: outside the engine an element can only carry
/// the default (no marks).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Marks {
    /// Document root/body eligible for the quirks-mode block-size fill rules.
    pub(crate) quirks_fill: bool,
    /// A lone float in a vertical parent: its size does not contribute to the
    /// auto block size of the parent (CSS 2.1 §10.6.3); set by
    /// `layout::float::wrap::lone`, rendered by `layout::float::outside_block`.
    pub(crate) outside_block: bool,
    /// Synthetic float-wrap row (a float beside its flowing text); set by
    /// `layout::float::wrap::row`, rendered by `float_flow`.
    pub(crate) float_row: bool,
    /// A column-wide float lifted into the multicol stack as a block; set by
    /// the multicol float pass, read by the stack construction.
    pub(crate) float_block: bool,
    /// The multicol host trims its first formatted line here
    /// (`text-box-trim-start`, css-inline-3 §4.2).
    pub(crate) host_trim_start: bool,
    /// The multicol host trims its last formatted line here.
    pub(crate) host_trim_end: bool,
    /// A stack child fragmented by the paginate path of the multicol container.
    pub(crate) paginate: bool,
    /// `node_id` of the source-tree parent of a hoisted column spanner: margins
    /// of spanners from different parents do not collapse.
    pub(crate) span_parent: Option<u64>,
    /// Block whose auto height is proven zero by its float host
    /// (`layout::block::margin_height`).
    pub(crate) auto_zero_height: bool,
    /// Anonymous table cell (CSS 2.1 §17.2.1), not an HTML `td`.
    pub(crate) anonymous_cell: bool,
    /// Synthetic float band host (`shape_flow` render path).
    pub(crate) shape_flow: Option<ShapeFlow>,
    /// The band host's in-flow content alone sizes it (CSS 2.1 §10.6.3).
    pub(crate) inflow_height: bool,
    /// The band host's first child margin adjoins its container's top.
    pub(crate) adjoining_start: bool,
    /// Anonymous run of inline content inside a band host.
    pub(crate) anon: bool,
    /// The anonymous run holds only atomic inlines.
    pub(crate) atoms: bool,
    /// The anonymous run continues a paragraph (not its first line).
    pub(crate) cont: bool,
    /// The box is an initial letter placed by the measured band host.
    pub(crate) initial_letter: bool,
    /// Width probe of inline content before the floats of a measured band host.
    pub(crate) lead: Option<LeadProbe>,
}

/// Synthetic band host: floats of one run and the content they wrap.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct ShapeFlow {
    /// Side of the first float (fallback of the live `shape-outside` path).
    pub(crate) right: bool,
    /// Number of leading float children.
    pub(crate) count: usize,
    /// Font size for `em` margins of the measured host.
    pub(crate) em: Option<f32>,
    /// Containing block height in px of the measured host, when definite.
    pub(crate) cbh: Option<f32>,
    /// Containing block width in px of the measured host, when definite.
    pub(crate) cbw: Option<f32>,
    /// Writing mode of the containing block the host was built for.
    pub(crate) wm: crate::style::computed::writing_mode::WritingMode,
    /// The containing block is the initial containing block.
    pub(crate) icb: bool,
    /// Band layout path; `None` — the live `shape-outside` path.
    pub(crate) bands: Option<BandPath>,
}

/// Band layout path of a synthetic band host.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BandPath {
    /// Measured band host (`band_flow`).
    Measured,
}

/// Lead probe: floats `from..to` measure the inline content before them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct LeadProbe {
    /// The probe holds the line base before the last forced break
    /// (`lead-base`), not the float line itself (`lead-for`).
    pub(crate) base: bool,
    pub(crate) from: usize,
    pub(crate) to: usize,
}
