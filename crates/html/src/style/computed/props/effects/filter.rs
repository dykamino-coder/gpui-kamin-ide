//! Каноническая грамматика filter и типизированные операнды длин фильтров изображений.

use crate::document::state::ValueCx;
use crate::style::computed::parse_util::shadow_values;
use crate::style::computed::{Computed, Filter, ImageFilter, Len, Shadow};

pub(super) struct ParsedFilter {
    pub(super) program: ImageFilter,
    pub(super) reference: Option<String>,
    operations: Vec<Filter>,
}

impl ParsedFilter {
    pub(super) fn parse(mut raw: &str, seed: Option<Filter>, vcx: ValueCx<'_>) -> Option<Self> {
        let mut program = ImageFilter::default();
        let mut f = seed.unwrap_or_else(Filter::neutral);
        let mut reference = None;
        let mut operations = Vec::new();
        if raw == "none" {
            f = Filter::neutral();
            raw = "";
        } else if raw.is_empty() {
            return None;
        }
        while !raw.trim().is_empty() {
            let open = raw.find('(')?;
            let end = crate::style::css::block_end(raw, open, '(')?;
            let (name, arg) = (raw[..open].trim(), raw[open + 1..end].trim());
            raw = raw.get(end + 1..)?.trim_start();
            program.calls += 1;
            let amount = || -> Option<f32> {
                let n = if arg.is_empty() {
                    1.0
                } else if let Some(n) = arg.strip_suffix('%') {
                    n.trim().parse::<f32>().ok()? / 100.0
                } else {
                    arg.parse::<f32>().ok()?
                };
                (n.is_finite() && n >= 0.0).then_some(n)
            };
            match name {
                "url" => {
                    let id = arg.trim_matches(['"', '\'']).trim();
                    if id.is_empty() {
                        return None;
                    }
                    reference = id.strip_prefix('#').map(str::to_string);
                }
                "grayscale" => f.grayscale = amount()?,
                "brightness" => f.brightness = amount()?,
                "saturate" => f.saturate = amount()?,
                "invert" => f.invert = amount()?,
                "sepia" => f.sepia = amount()?,
                "opacity" => f.opacity = amount()?,
                "contrast" => f.contrast = amount()?,
                "hue-rotate" => {
                    f.hue_rotate = if arg.is_empty() {
                        0.0
                    } else if let Some(n) = arg.strip_suffix("deg") {
                        n.parse().ok()?
                    } else if let Some(n) = arg.strip_suffix("turn") {
                        n.parse::<f32>().ok()? * 360.0
                    } else if let Some(n) = arg.strip_suffix("grad") {
                        n.parse::<f32>().ok()? * 0.9
                    } else if let Some(n) = arg.strip_suffix("rad") {
                        n.parse::<f32>().ok()?.to_degrees()
                    } else if arg == "0" {
                        0.0
                    } else {
                        return None;
                    };
                }
                "blur" => {
                    let len = if arg.is_empty() {
                        Len::Px(0.0)
                    } else {
                        Len::parse(arg, vcx)?
                    };
                    if matches!(
                        len,
                        Len::Auto
                            | Len::Pct(_)
                            | Len::MinContent
                            | Len::MaxContent
                            | Len::FitContent
                    ) || matches!(len, Len::Px(n) | Len::Em(n) | Len::Ex(n) | Len::Ch(n) if n < 0.0)
                    {
                        return None;
                    }
                    program.blur_length = Some(len);
                    f.blur = if let Len::Px(px) = len { px } else { 0.0 };
                }
                "drop-shadow" => {
                    let shadows = shadow_values(arg, vcx);
                    if !crate::style::computed::box_shadow_valid(arg, vcx)
                        || shadows.len() != 1
                        || shadows[0].inset
                        || shadows[0].lengths[3].is_some()
                    {
                        return None;
                    }
                    program.shadow_value = shadows.into_iter().next();
                    program.shadow = program
                        .shadow_value
                        .as_ref()
                        .and_then(|s| s.numeric(vcx))
                        .map(|sh| Shadow {
                            blur: sh.blur * 2.0,
                            ..sh
                        });
                }
                _ => return None,
            }
            if let Some(one) = f.color_operation(name) {
                operations.push(one);
            }
        }
        program.filter = Some(f);
        Some(Self {
            program,
            reference,
            operations,
        })
    }
}

impl Computed {
    pub(in crate::style::computed) fn parse_filter(
        &mut self,
        raw: &str,
        vcx: ValueCx<'_>,
    ) -> ImageFilter {
        let Some(parsed) = ParsedFilter::parse(raw, self.filter, vcx) else {
            return ImageFilter::default();
        };
        self.accept_declaration();
        self.filter = parsed.program.filter;
        self.drop_shadow = parsed.program.shadow.clone();
        self.filter_ref = parsed.reference;
        self.scrollbar_filter_ops = parsed.operations;
        let mut program = parsed.program;
        // `currentcolor` is resolved per element by `resolve_colors`.
        program.resolve_colors(crate::style::values::value::Color::BLACK);
        program
    }
}
