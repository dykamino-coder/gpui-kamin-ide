//! Сборки OpenType MATH: повтор extender-ов и распределение перекрытия соединителей.
use ttf_parser::math::{GlyphAssembly, GlyphPart};

pub(super) fn parts(
    assembly: GlyphAssembly<'_>,
    minimum: u16,
    target: f32,
    vertical: bool,
) -> Vec<(u16, f32, f32)> {
    let recipe = assembly.parts.into_iter().collect::<Vec<_>>();
    if recipe.is_empty() {
        return vec![];
    }
    let has_extenders = recipe.iter().any(|p| p.part_flags.extender());
    let mut expanded = vec![];
    for repetitions in 0..=1024 {
        expanded.clear();
        for part in &recipe {
            let count = if part.part_flags.extender() {
                repetitions
            } else {
                1
            };
            expanded.extend(std::iter::repeat_n(*part, count));
        }
        let total = span(&expanded, f32::from(minimum));
        if total >= target || !has_extenders {
            break;
        }
    }
    if expanded.is_empty() {
        return vec![];
    }
    let joins = expanded
        .windows(2)
        .map(|p| f32::from(p[0].end_connector_length.min(p[1].start_connector_length)))
        .collect::<Vec<_>>();
    let advance = expanded
        .iter()
        .map(|p| f32::from(p.full_advance))
        .sum::<f32>();
    let available = joins
        .iter()
        .map(|v| (v - f32::from(minimum)).max(0.0))
        .sum::<f32>();
    let extra = (advance - f32::from(minimum) * joins.len() as f32 - target).max(0.0);
    let ratio = if available > 0.0 {
        (extra / available).min(1.0)
    } else {
        0.0
    };
    let mut offset = 0.0;
    expanded
        .iter()
        .enumerate()
        .map(|(i, p)| {
            let result = (
                p.glyph_id.0,
                if vertical { 0.0 } else { offset },
                if vertical { -offset } else { 0.0 },
            );
            offset += f32::from(p.full_advance);
            if let Some(maximum) = joins.get(i) {
                offset -= f32::from(minimum) + (maximum - f32::from(minimum)).max(0.0) * ratio;
            }
            result
        })
        .collect()
}
fn span(parts: &[GlyphPart], overlap: f32) -> f32 {
    parts.iter().map(|p| f32::from(p.full_advance)).sum::<f32>()
        - overlap * parts.len().saturating_sub(1) as f32
}
