//! Ruby extents for probes; split out to keep the owning module within 250 lines.

use super::LayoutTap;
use crate::hooks::IntoHooked;
use gpui::{AnyElement, LayoutId};

/// Узлы стопок аннотаций одного руби: (под базой?, полулидинг базы, узел).
/// Полулидинг вычитается: стопка стоит на краю коробки строки базы, а
/// аннотация в браузере — на краю её СОДЕРЖИМОГО.
#[derive(Default)]
pub struct RubyExtents {
    pub levels: Vec<(bool, f32, std::rc::Rc<std::cell::Cell<Option<LayoutId>>>)>,
    pub(crate) nested: Vec<Vec<crate::text::ruby::layers::LayerAttachment>>,
    /// What the line needs to let the annotation overhang its neighbours
    /// (`ruby_overhang_probe`, css-ruby-1 §4.4); `None` for non-ruby atoms.
    pub overhang: Option<RubyOverhangInfo>,
}

impl RubyExtents {
    pub fn is_empty(&self) -> bool {
        self.levels.is_empty()
    }
}

/// Inputs of the ruby overhang computation (Blink `ruby_utils.cc`
/// `GetOverhang`): the overhang mode, half the annotation font size (the
/// `auto` limit), whether `ruby-align: start` (overhang only at the end), the
/// base content width reported by the base paragraph during layout, and the
/// base's font size (no end overhang over larger following text).
#[derive(Clone)]
pub struct RubyOverhangInfo {
    pub mode: crate::style::computed::RubyOverhang,
    pub half_annotation_font: f32,
    pub align_start: bool,
    pub base_font: f32,
    pub base_width: std::rc::Rc<std::cell::Cell<Option<f32>>>,
}

/// Take-once offer of a ruby column's base-width sink (`LayoutCx::ruby_base`).
pub(crate) type RubyBaseOffer =
    std::rc::Rc<std::cell::Cell<Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>>>;

/// Build the ruby BASE unit with a width sink offered to it: the first
/// paragraph built inside takes the sink (`Paragraph::ruby_context`) and
/// reports its content width into it. Also registers the overhang info for
/// the atom being collected (`LayoutCx::ruby_extents`).
pub fn ruby_base_with_overhang<T>(
    opts: &crate::render::RenderOpts,
    mode: crate::style::computed::RubyOverhang,
    half_annotation_font: f32,
    align_start: bool,
    base_font: f32,
    build: impl FnOnce(&crate::render::RenderOpts) -> T,
) -> T {
    let sink = std::rc::Rc::new(std::cell::Cell::new(None));
    let offer: RubyBaseOffer = std::rc::Rc::new(std::cell::Cell::new(Some(sink.clone())));
    let out = build(&opts.inner(|cx| cx.ruby_base = Some(offer)));
    if let Some(collect) = &opts.cx.ruby_extents
        && let Some(v) = collect.borrow_mut().as_mut()
        && v.overhang.is_none()
    {
        v.overhang = Some(RubyOverhangInfo {
            mode,
            half_annotation_font,
            align_start,
            base_font,
            base_width: sink,
        });
    }
    out
}

/// Стопка аннотаций руби, чью высоту строка должна знать (css-ruby-1 §3.4):
/// аннотации в высоту строки не входят, но «the UA must increase the line's
/// spacing … so that the ruby annotation fits» — строка растёт ровно на то,
/// чем аннотация выходит за её коробку (Blink `ruby_utils.cc`
/// `ComputeAnnotationOverflow`). Без сбора — сам элемент как есть.
pub fn ruby_extent(el: AnyElement, under: bool, inset: f32) -> AnyElement {
    ruby_layer(el, under, inset, &[], None)
}

pub(crate) fn ruby_layer(
    el: AnyElement,
    under: bool,
    inset: f32,
    layers: &[std::rc::Rc<crate::text::ruby::layers::Layers>],
    collect: Option<&std::rc::Rc<std::cell::RefCell<Option<crate::text::paragraph::RubyExtents>>>>,
) -> AnyElement {
    let slot = std::rc::Rc::new(std::cell::Cell::new(None));
    let attached: Vec<_> = layers
        .iter()
        .map(|layer| (layer.clone(), layer.add(under, slot.clone())))
        .collect();
    let collecting = collect
        .and_then(|c| {
            c.borrow_mut().as_mut().map(|v| {
                v.levels.push((under, inset, slot.clone()));
                v.nested.push(attached);
            })
        })
        .is_some();
    if !collecting && layers.is_empty() {
        return el;
    }
    LayoutTap { child: el, slot }.hooked()
}
