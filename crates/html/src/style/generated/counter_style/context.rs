//! Direction-dependent predefined symbols, shared by generated content and markers.

use super::{builtin_repr, normalize_name, suffix};
use crate::style::computed::Computed;
use crate::style::generated::counter_style_rules::{
    custom_marker_in_context, custom_repr_in_context,
};

#[derive(Clone, Copy, Debug, Default)]
pub struct Context {
    pub rtl: bool,
    pub vertical: bool,
    pub vertical_rl: bool,
}

impl Context {
    pub fn from_style(style: &Computed) -> Self {
        Self {
            rtl: style.rtl == Some(true),
            vertical: style
                .vertical
                .or(style.rotated_line)
                .unwrap_or(style.cb_vertical),
            vertical_rl: style.vertical_rl == Some(true),
        }
    }

    pub fn inherit(style: &Computed, parent: Self) -> Self {
        Self {
            rtl: style.rtl.unwrap_or(parent.rtl),
            vertical: style.vertical.unwrap_or(parent.vertical),
            vertical_rl: style.vertical_rl.unwrap_or(parent.vertical_rl),
        }
    }

    pub(in crate::style::generated) fn symbol(self, name: &str) -> Option<&'static str> {
        Some(match name {
            "disc" => "•",
            "circle" => "◦",
            "square" => "▪",
            "disclosure-open" if self.vertical && self.vertical_rl => "◂",
            "disclosure-open" if self.vertical => "▸",
            "disclosure-open" => "▾",
            "disclosure-closed" if self.vertical && self.rtl => "▴",
            "disclosure-closed" if self.vertical => "▾",
            "disclosure-closed" if self.rtl => "◂",
            "disclosure-closed" => "▸",
            _ => return None,
        })
    }
}

pub fn repr_in_context(value: i32, style: &str, context: Context) -> String {
    let style = &*normalize_name(style);
    custom_repr_in_context(value as i64, style, context).unwrap_or_else(|| {
        context
            .symbol(style)
            .map_or_else(|| builtin_repr(value, style), str::to_string)
    })
}

pub fn marker_in_context(value: i32, style: &str, context: Context) -> String {
    if style == "none" {
        return String::new();
    }
    let style = &*normalize_name(style);
    custom_marker_in_context(value as i64, style, context).unwrap_or_else(|| {
        format!(
            "{}{}",
            repr_in_context(value, style, context),
            suffix(style)
        )
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherited_context_and_rotated_coordinates() {
        let parent = Context {
            rtl: true,
            vertical: true,
            vertical_rl: true,
        };
        let mut style = Computed {
            rtl: Some(false),
            ..Computed::default()
        };
        let context = Context::inherit(&style, parent);
        assert!(!context.rtl);
        assert!(context.vertical && context.vertical_rl);
        style.rotated_line = Some(true);
        assert!(Context::from_style(&style).vertical);
        style.vertical = Some(false);
        assert!(!Context::from_style(&style).vertical);
    }

    #[test]
    fn disclosure_orientation_and_cyclic_values() {
        for vertical in [false, true] {
            for vertical_rl in [false, true] {
                for rtl in [false, true] {
                    let context = Context {
                        rtl,
                        vertical,
                        vertical_rl,
                    };
                    let closed = match (vertical, rtl) {
                        (false, false) => "▸",
                        (false, true) => "◂",
                        (true, false) => "▾",
                        (true, true) => "▴",
                    };
                    let open = if !vertical {
                        "▾"
                    } else if vertical_rl {
                        "◂"
                    } else {
                        "▸"
                    };
                    for value in [i32::MIN, -1, 0, 1, i32::MAX] {
                        assert_eq!(repr_in_context(value, "disclosure-closed", context), closed);
                        assert_eq!(
                            marker_in_context(value, "disclosure-open", context),
                            format!("{open} ")
                        );
                    }
                }
            }
        }
    }
}
