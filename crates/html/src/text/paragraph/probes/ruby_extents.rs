//! Ruby extents for probes; split out to keep the owning module within 250 lines.

use super::LayoutTap;
use gpui::{AnyElement, IntoElement, LayoutId};

/// Узлы стопок аннотаций одного руби: (под базой?, полулидинг базы, узел).
/// Полулидинг вычитается: стопка стоит на краю коробки строки базы, а
/// аннотация в браузере — на краю её СОДЕРЖИМОГО.
#[derive(Default)]
pub struct RubyExtents {
    pub levels: Vec<(bool, f32, std::rc::Rc<std::cell::Cell<Option<LayoutId>>>)>,
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

thread_local! {
    /// Sink of the base paragraph that is being built for a ruby column
    /// (`ruby_base_width_sink`): the paragraph records its max-content width
    /// there when it is measured.
    pub(super) static RUBY_BASE_SINK: std::cell::RefCell<Option<std::rc::Rc<std::cell::Cell<Option<f32>>>>> =
        const { std::cell::RefCell::new(None) };
}

/// Build the ruby BASE unit while a width sink is active: the paragraph
/// created inside takes the sink (`Paragraph::new`) and reports its content
/// width into it. Also registers the overhang info for the atom being
/// collected (`collect_ruby_extents`).
pub fn ruby_base_with_overhang<T>(
    mode: crate::style::computed::RubyOverhang,
    half_annotation_font: f32,
    align_start: bool,
    base_font: f32,
    build: impl FnOnce() -> T,
) -> T {
    let sink = std::rc::Rc::new(std::cell::Cell::new(None));
    let saved = RUBY_BASE_SINK.with(|s| s.replace(Some(sink.clone())));
    let out = build();
    RUBY_BASE_SINK.with(|s| s.replace(saved));
    RUBY_EXTENTS.with(|r| {
        if let Some(v) = r.borrow_mut().as_mut()
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
    });
    out
}

/// Take the active base-width sink (for the first paragraph built under it).
pub(crate) fn take_ruby_base_sink() -> Option<std::rc::Rc<std::cell::Cell<Option<f32>>>> {
    RUBY_BASE_SINK.with(|s| s.borrow_mut().take())
}

thread_local! {
    /// Сбор узлов аннотаций для атома, который сейчас строится
    /// (`collect_ruby_extents`). `None` — сбора нет: руби вне строки абзаца
    /// своих аннотаций никому не отдаёт.
    pub(super) static RUBY_EXTENTS: std::cell::RefCell<Option<RubyExtents>> =
        const { std::cell::RefCell::new(None) };
}

/// Построить атом, собрав узлы аннотаций руби, которые он заведёт.
/// Вложенный сбор (атом внутри атома) своё забирает сам: прежний список
/// восстанавливается после вызова.
pub fn collect_ruby_extents<T>(build: impl FnOnce() -> T) -> (T, RubyExtents) {
    let saved = RUBY_EXTENTS.with(|r| r.replace(Some(RubyExtents::default())));
    let out = build();
    let mine = RUBY_EXTENTS.with(|r| r.replace(saved)).unwrap_or_default();
    (out, mine)
}

/// Стопка аннотаций руби, чью высоту строка должна знать (css-ruby-1 §3.4):
/// аннотации в высоту строки не входят, но «the UA must increase the line's
/// spacing … so that the ruby annotation fits» — строка растёт ровно на то,
/// чем аннотация выходит за её коробку (Blink `ruby_utils.cc`
/// `ComputeAnnotationOverflow`). Без сбора — сам элемент как есть.
pub fn ruby_extent(el: AnyElement, under: bool, inset: f32) -> AnyElement {
    let slot = std::rc::Rc::new(std::cell::Cell::new(None));
    let collecting = RUBY_EXTENTS.with(|r| {
        r.borrow_mut()
            .as_mut()
            .map(|v| v.levels.push((under, inset, slot.clone())))
            .is_some()
    });
    if !collecting {
        return el;
    }
    LayoutTap { child: el, slot }.into_any_element()
}
