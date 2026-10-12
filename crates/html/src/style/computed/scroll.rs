//! CSS Scrollbars 1 styling, owned by HTML.
use super::{Computed, split_outside_parens};
use crate::style::values::value::Color;

#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollStyle {
    pub forced: [bool; 2],
    pub width: Option<u8>,
    pub current: [bool; 2],
    pub colors: Option<Option<[Color; 2]>>,
    pub inherit_width: bool,
}

impl ScrollStyle {
    // Blink core/scroll/scrollbar_theme_aura.cc:53–57,123–130: 15px, thin 2/3.
    pub(crate) fn thickness(self) -> f32 {
        match self.width {
            Some(1) => 10.0,
            Some(2) => 0.0,
            _ => 15.0,
        }
    }
}

impl Computed {
    pub(crate) fn apply_scroll(&mut self, key: &str, _: &str, v: &str, hit: &mut bool) {
        match key {
            "scrollbar-width" => match v {
                "auto" => {
                    self.scroll.width = Some(0);
                    self.scroll.inherit_width = false;
                }
                "thin" => {
                    self.scroll.width = Some(1);
                    self.scroll.inherit_width = false;
                }
                "none" => {
                    self.scroll.width = Some(2);
                    self.scroll.inherit_width = false;
                }
                "inherit" => self.scroll.inherit_width = true,
                _ => (),
            },
            "scrollbar-color" => {
                if v == "inherit" {
                    self.scroll.colors = None;
                    self.scroll.current = [false; 2];
                } else if v == "auto" {
                    self.scroll.colors = Some(None);
                    self.scroll.current = [false; 2];
                } else {
                    let parts = split_outside_parens(v);
                    if parts.len() == 2 {
                        let current = [
                            parts[0].eq_ignore_ascii_case("currentcolor"),
                            parts[1].eq_ignore_ascii_case("currentcolor"),
                        ];
                        let color = |part: &str, current| {
                            if current {
                                Some(Color::default())
                            } else {
                                Color::parse(part)
                            }
                        };
                        if let (Some(a), Some(b)) =
                            (color(&parts[0], current[0]), color(&parts[1], current[1]))
                        {
                            self.scroll.colors = Some(Some([a, b]));
                            self.scroll.current = current;
                        }
                    }
                }
            }
            _ => self.apply_snap(key, v, hit),
        }
    }
}

pub(crate) fn inherit(parent: &Computed, c: &mut Computed) {
    super::snap::inherit(parent, c);
    // Only color inherits by default (CSS Scrollbars 1 §2). Width and gutter
    // remain local to the scroll container, including a nested one.
    if let Some(Some(mut colors)) = c.scroll.colors {
        let current = c.color.or(parent.color).unwrap_or(Color {
            a: 1.0,
            ..Color::default()
        });
        for (i, color) in colors.iter_mut().enumerate() {
            if c.scroll.current[i] {
                *color = current;
            }
        }
        c.scroll.colors = Some(Some(colors));
    }
    c.scroll.colors = c.scroll.colors.or(parent.scroll.colors);
    if c.scroll.inherit_width {
        c.scroll.width = parent.scroll.width;
    }
}

#[cfg(test)]
mod tests;
