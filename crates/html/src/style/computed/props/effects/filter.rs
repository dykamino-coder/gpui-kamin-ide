//! Каноническая грамматика filter и типизированные операнды длин фильтров изображений.

use crate::document::state::ValueCx;
use crate::style::computed::parse_util::shadow_values;
use crate::style::computed::{Computed, Filter, ImageFilter, Len, Shadow};

impl Computed {
    pub(in crate::style::computed) fn parse_filter(
        &mut self,
        mut raw: &str,
        vcx: ValueCx<'_>,
    ) -> ImageFilter {
        let mut program = ImageFilter {
            filter: self.filter,
            ..ImageFilter::default()
        };
        let mut f = self.filter.unwrap_or_else(Filter::neutral);
        self.drop_shadow = None;
        self.scrollbar_filter_ops.clear();
        while let Some(open) = raw.find('(') {
            let Some(end) = crate::style::css::block_end(raw, open, '(') else {
                break;
            };
            let (name, arg) = (raw[..open].trim(), raw[open + 1..end].trim());
            raw = raw.get(end + 1..).unwrap_or_default();
            program.calls += 1;
            let amount = || match arg.strip_suffix('%') {
                Some(n) => n.trim().parse::<f32>().unwrap_or(100.0) / 100.0,
                None => arg.parse::<f32>().unwrap_or(1.0),
            };
            match name {
                "url" => {
                    let id = arg.trim_matches(|c| c == '\"' || c == '\'').trim();
                    if let Some(id) = id.strip_prefix('#') {
                        self.filter_ref = Some(id.to_string());
                    }
                }
                "grayscale" => f.grayscale = amount(),
                "brightness" => f.brightness = amount(),
                "saturate" => f.saturate = amount(),
                "invert" => f.invert = amount(),
                "sepia" => f.sepia = amount(),
                "opacity" => f.opacity = amount(),
                "hue-rotate" => f.hue_rotate = arg.trim_end_matches("deg").parse().unwrap_or(0.0),
                "contrast" => f.contrast = amount(),
                "blur" => {
                    program.blur_length = Len::parse(arg, vcx);
                    f.blur = match program.blur_length {
                        Some(Len::Px(px)) => px,
                        _ => 0.0,
                    };
                }
                "drop-shadow" => {
                    program.shadow_value = shadow_values(arg, vcx).into_iter().find(|s| !s.inset);
                    program.shadow_current = program
                        .shadow_value
                        .as_ref()
                        .is_some_and(|s| s.color.a < 0.0);
                    self.drop_shadow = program
                        .shadow_value
                        .as_ref()
                        .and_then(|s| s.numeric(vcx))
                        .map(|sh| Shadow {
                            blur: sh.blur * 2.0,
                            ..sh
                        });
                }
                _ => {}
            }
            if let Some(one) = f.color_operation(name) {
                self.scrollbar_filter_ops.push(one);
            }
        }
        self.filter = Some(f);
        program.filter = Some(f);
        program.shadow = self.drop_shadow;
        program
    }
}
