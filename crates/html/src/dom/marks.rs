//! Внутренние метки layout и идентичность синтетической коробки между кадрами.
//!
//! Layout переписывает отрисовываемое дерево (строки float и band-хосты, дочерние
//! элементы multicol-стека, подъём spanner-ов, анонимные ячейки таблиц) и должен
//! сообщить следующему проходу, как получен узел. Эти метки лежат в типизированном
//! поле [`super::Element`], а не в `Element::attrs` и не в имени тега: авторская
//! разметка (`dom/walk`) заполняет только атрибуты и тег, поэтому документ не может
//! подделать метку, а метка не попадает в селекторы атрибутов и сериализацию SVG.

/// Типизированные флаги layout и ключ состояния коробки, принадлежащий документу.
///
/// The fields are crate-private: outside the engine an element can only carry
/// the default (no marks).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Marks {
    /// Общий для клонов ключ синтетической коробки; node_id остаётся layout-меткой.
    pub(crate) state_id: std::rc::Rc<std::cell::OnceCell<String>>,
    /// Document root/body eligible for the quirks-mode block-size fill rules.
    pub(crate) quirks_fill: bool,
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
    /// Исходный содержащий блок, удалённый из пути при разрыве inline.
    pub(crate) inline_cb: Option<InlineCb>,
    /// Block whose auto height is proven zero by its float host
    /// (`layout::block::margin_height`).
    pub(crate) auto_zero_height: bool,
    /// Anonymous table cell (CSS 2.1 §17.2.1), not an HTML `td`.
    pub(crate) anonymous_cell: bool,
    /// Synthetic measured float band host (`band_flow_host`).
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

/// Потерянные предки CB при разрыве строчной коробки вокруг блока.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct InlineCb {
    pub(crate) node: u64,
    pub(crate) depth: u16,
}
