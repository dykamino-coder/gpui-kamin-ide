//! Строчный контейнер руби (`<ruby>` атомом строки): сегменты, базы и аннотации.
mod stacks;
mod unit_boxes;
use crate::text::ruby::container::stacks::level_wrap;
use crate::text::ruby::container::stacks::over_stack;
use crate::text::ruby::container::unit_boxes::unit_box;

// owner: A

use crate::dom::{Element, Node};
use crate::render::RenderOpts;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use crate::text::ruby::{RubyUnit, ruby_hiding, ruby_segments, ruby_unit_blank};
use crate::text::text_box::normal_fraction;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div};

pub(crate) fn ruby_container_atom(
    inherited: &Computed,
    e: &Element,
    opts: &RenderOpts,
) -> Option<AnyElement> {
    let mut merged = inherit(inherited, &e.style);
    // Внутри руби знак акцента не разворачивается (как прежде).
    merged.text_emphasis = None;
    let segments = ruby_segments(&e.children);
    // Без хотя бы одной непустой аннотации руби — обычный строчный
    // (`ruby-line-breaking-001`: `<rtc><rt>` пустой; `ruby-intrinsic-isize-*`).
    if !segments.iter().any(|s| {
        s.levels
            .iter()
            .any(|l| l.units.iter().any(|u| !ruby_unit_blank(u)))
    }) {
        return None;
    }
    stacks::align_units(&mut merged);
    // `ruby-position` уровня k (css-ruby-1 §4.1): явное `over`/`under`
    // — для всех уровней; начальное `alternate` (`None`) — первый
    // уровень над базой, следующий под, и так далее. `alternate
    // under` пока не различается (первый уровень над).
    let level_under = |k: usize| match merged.ruby_under {
        Some(under) => under,
        None => k % 2 == 1,
    };
    // Стопка «база, уровни наружу»: под базой — прямая гибкая колонка.
    let under_stack = || div().flex().flex_col().flex_shrink_0();
    let empty: RubyUnit = Vec::new();
    // Стопка уровней одной стороны — узел, чью высоту знает строка
    // (`lines::ruby_extent`, css-ruby-1 §3.4).
    // Полулидинг базы: стопка стоит на краю её коробки строки.
    let base_half = {
        let size = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        let family = merged.font_family.clone().unwrap_or_default();
        let (asc, desc, _) = crate::text::metrics::vmetrics_px(&family, size);
        let line = match merged.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
            _ => size * normal_fraction(&merged, opts),
        };
        (line - (asc + desc)) / 2.0
    };
    let extent = |d: gpui::Div, under: bool| {
        crate::text::paragraph::ruby_extent(d.into_any_element(), under, base_half)
    };
    let mut row = div().flex().flex_row().items_baseline().flex_shrink_0();
    for seg in &segments {
        // Стиль уровня: аннотации внутри `<rtc>` наследуют от него.
        let level_style: Vec<Computed> = seg
            .levels
            .iter()
            .map(|l| match &l.container {
                Some(c) => inherit(&merged, c),
                None => merged.clone(),
            })
            .collect();
        // Колонок столько, сколько баз или аннотаций самого длинного
        // нераспорного уровня; нехватка — пустые анонимные (§2.3.2).
        let columns = seg.bases.len().max(
            seg.levels
                .iter()
                .filter(|l| !l.spanning)
                .map(|l| l.units.len())
                .max()
                .unwrap_or(0),
        );
        // Колонки не сжимаются: ширина колонки — по самому широкому
        // из базы и аннотаций (§3.1.1), а не по остатку строки.
        let mut cols = div().flex().flex_row().items_baseline().flex_shrink_0();
        for i in 0..columns {
            // Уровни над базой — в обратную стопку вместе с базой,
            // уровни под ней — прямой стопкой снаружи.
            let mut over_anns: Vec<AnyElement> = Vec::new();
            let mut under: Vec<AnyElement> = Vec::new();
            for (k, (l, style)) in seg.levels.iter().zip(&level_style).enumerate() {
                if l.spanning {
                    continue;
                }
                let nodes = l.units.get(i).unwrap_or(&empty);
                let base = ruby_hiding::text(seg.bases.get(i).unwrap_or(&empty));
                let nodes = if ruby_hiding::hidden(nodes, &base, style) {
                    &empty
                } else {
                    nodes
                };
                let ann = unit_box(nodes, style, opts);
                if level_under(k) {
                    under.push(ann);
                } else {
                    over_anns.push(ann);
                }
            }
            // База — ПЕРВЫЙ элемент ячейки `over_stack`, обёртка уровней
            // идёт после неё в той же ячейке. Внутри обёртки уровни
            // в обратном порядке: нулевой (ближний к базе) — внизу.
            // css-ruby-1 §4.4 ruby-overhang: a single-column ruby lets
            // its line know the base content width, so an annotation
            // wider than the base may overhang the neighbours
            // (`lines::lay_atoms`, Blink `ruby_utils.cc` GetOverhang).
            let base_nodes = seg.bases.get(i).unwrap_or(&empty);
            let base_el = if segments.len() == 1 && columns == 1 && !seg.levels.is_empty() {
                let base_font = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                // The annotation's own font size: UA `rt { font-size: 50% }`.
                let ann_font = seg
                    .levels
                    .first()
                    .and_then(|l| l.units.first())
                    .and_then(|u| match u.as_slice() {
                        [Node::Element(k)] => match k.style.font_size {
                            Some(Len::Px(v)) => Some(v),
                            Some(Len::Pct(p)) | Some(Len::Em(p)) => Some(p * base_font),
                            _ => None,
                        },
                        _ => None,
                    })
                    .unwrap_or(base_font * 0.5);
                crate::text::paragraph::ruby_base_with_overhang(
                    merged
                        .ruby_overhang
                        .unwrap_or(crate::style::computed::RubyOverhang::Auto),
                    ann_font / 2.0,
                    merged.ruby_align == Some(crate::style::computed::RubyAlign::Start),
                    base_font,
                    || unit_box(base_nodes, &merged, opts),
                )
            } else {
                unit_box(base_nodes, &merged, opts)
            };
            let mut over = over_stack(base_el);
            if !over_anns.is_empty() {
                over = over.child(
                    level_wrap(false).child(extent(
                        div()
                            .flex()
                            .flex_col()
                            .flex_shrink_0()
                            .children(over_anns.into_iter().rev()),
                        false,
                    )),
                );
            }
            let col = if under.is_empty() {
                over.into_any_element()
            } else {
                under_stack()
                    .child(over)
                    .child(level_wrap(true).child(extent(
                        div().flex().flex_col().flex_shrink_0().children(under),
                        true,
                    )))
                    .into_any_element()
            };
            cols = cols.child(col);
        }
        let mut seg_el = cols.into_any_element();
        for (k, (l, style)) in seg.levels.iter().zip(&level_style).enumerate() {
            if l.spanning {
                let nodes = l.units.first().unwrap_or(&empty);
                let base: String = seg.bases.iter().map(|b| ruby_hiding::text(b)).collect();
                let nodes = if ruby_hiding::hidden(nodes, &base, style) {
                    &empty
                } else {
                    nodes
                };
                let host = if level_under(k) {
                    under_stack().child(seg_el)
                } else {
                    over_stack(seg_el)
                };
                seg_el = host
                    .child(
                        level_wrap(level_under(k)).child(extent(
                            div()
                                .flex()
                                .flex_col()
                                .flex_shrink_0()
                                .child(unit_box(nodes, style, opts)),
                            level_under(k),
                        )),
                    )
                    .into_any_element();
            }
        }
        row = row.child(seg_el);
    }
    Some(row.into_any_element())
}
