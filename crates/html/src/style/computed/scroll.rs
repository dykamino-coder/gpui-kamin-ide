//! CSS Scrollbars 1 styling, owned by HTML.
use super::{Computed, split_outside_parens};
use crate::style::values::value::Color;

#[derive(Clone, Copy, Debug, Default)]
pub struct ScrollStyle {
    pub forced: [bool; 2],
    pub width: Option<u8>,
    pub gutter: Option<u8>,
    pub inherit_gutter: bool,
    pub(crate) viewport_root: bool,
    pub(crate) used: [f32; 4],
    pub(crate) native: [f32; 4],
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
            "scrollbar-gutter" => {
                let parts: Vec<_> = v.split_whitespace().collect();
                let gutter = match parts.as_slice() {
                    ["auto"] => Some(0),
                    ["stable"] => Some(1),
                    ["stable", "both-edges"] | ["both-edges", "stable"] => Some(2),
                    ["inherit"] => {
                        self.scroll.inherit_gutter = true;
                        None
                    }
                    _ => None,
                };
                if let Some(gutter) = gutter {
                    self.accept_declaration();
                    self.scroll.gutter = Some(gutter);
                    self.scroll.inherit_gutter = false;
                }
            }
            "scrollbar-width" => match v {
                "auto" => {
                    self.accept_declaration();
                    self.scroll.width = Some(0);
                    self.scroll.inherit_width = false;
                }
                "thin" => {
                    self.accept_declaration();
                    self.scroll.width = Some(1);
                    self.scroll.inherit_width = false;
                }
                "none" => {
                    self.accept_declaration();
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
                    self.accept_declaration();
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
                            self.accept_declaration();
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
    normalize_overflow(c);
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
    if c.scroll.inherit_gutter {
        c.scroll.gutter = parent.scroll.gutter;
    }
    if c.scroll.inherit_width {
        c.scroll.width = parent.scroll.width;
    }
}

/// CSS Overflow 3 §3: visible/clip compute to auto/hidden when the other axis
/// is neither visible nor clip. An absent declaration is initial `visible`.
fn normalize_overflow(c: &mut Computed) {
    use super::Overflow;
    let non_scroll = |o| matches!(o, None | Some(Overflow::Visible | Overflow::Clip));
    if non_scroll(c.overflow_x) && non_scroll(c.overflow_y) {
        return;
    }
    for axis in [&mut c.overflow_x, &mut c.overflow_y] {
        *axis = match *axis {
            None | Some(Overflow::Visible) => Some(Overflow::Scroll),
            Some(Overflow::Clip) => Some(Overflow::Hidden),
            other => other,
        };
    }
}

#[cfg(test)]
mod tests {
    //! Regression checks for the cascade boundary of HTML scrollbar policies.
    use crate::style::computed::Computed;

    #[test]
    fn scrollbar_color_inherits_but_geometry_does_not() {
        let mut parent = Computed::default();
        parent.apply_one("scrollbar-color", "rgb(255, 0, 0) blue");
        parent.apply_one("scrollbar-width", "none");
        let child = crate::style::cascade::inherit::inherit(&parent, &Computed::default());
        assert_eq!(child.scroll.colors, parent.scroll.colors);
        assert_eq!(child.scroll.width, None);
        let mut reset = Computed::default();
        reset.apply_one("scrollbar-color", "initial");
        let child = crate::style::cascade::inherit::inherit(&parent, &reset);
        assert_eq!(child.scroll.colors, Some(None));
    }

    #[test]
    fn inherited_scrollbar_current_color_keeps_the_parent_computed_color() {
        let mut declaration = Computed::default();
        declaration.apply_one("color", "red");
        declaration.apply_one("scrollbar-color", "currentColor blue");
        let parent = crate::style::cascade::inherit::inherit(&Computed::default(), &declaration);
        let mut own = Computed::default();
        own.apply_one("color", "green");
        let child = crate::style::cascade::inherit::inherit(&parent, &own);
        assert_eq!(child.scroll.colors, parent.scroll.colors);
        own.apply_one("scrollbar-color", "currentColor blue");
        let child = crate::style::cascade::inherit::inherit(&parent, &own);
        assert_eq!(
            child.scroll.colors.flatten().unwrap()[0],
            child.color.unwrap()
        );
        assert_ne!(child.scroll.colors, parent.scroll.colors);
    }

    #[test]
    fn invalid_scrollbar_color_preserves_the_last_valid_declaration() {
        let mut c = Computed::default();
        c.apply_one("scrollbar-color", "red blue");
        let valid = c.scroll.colors;
        for invalid in ["red", "red blue green", "red not-a-color"] {
            c.apply_one("scrollbar-color", invalid);
            assert_eq!(c.scroll.colors, valid);
        }
    }

    #[test]
    fn initial_opposite_overflow_axis_computes_to_auto_without_becoming_forced() {
        use crate::style::computed::Overflow;
        let mut own = Computed::default();
        own.apply_one("overflow-x", "scroll");
        let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
        assert_eq!([c.overflow_x, c.overflow_y], [Some(Overflow::Scroll); 2]);
        assert_eq!(c.scroll.forced, [true, false]);
        let child = crate::style::cascade::inherit::inherit(&c, &Computed::default());
        assert_eq!([child.overflow_x, child.overflow_y], [None; 2]);
    }

    #[test]
    fn clip_preserves_non_scroll_pairs_and_computes_to_hidden_beside_auto() {
        use crate::style::computed::Overflow;
        for (declaration, expected) in [
            (
                "visible clip",
                [Some(Overflow::Visible), Some(Overflow::Clip)],
            ),
            (
                "clip auto",
                [Some(Overflow::Hidden), Some(Overflow::Scroll)],
            ),
        ] {
            let mut own = Computed::default();
            own.apply_one("overflow", declaration);
            let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own);
            assert_eq!([c.overflow_x, c.overflow_y], expected);
            assert_eq!(c.scroll.forced, [false; 2]);
        }
    }

    #[test]
    fn root_used_gutter_suppression_does_not_change_explicit_computed_inheritance() {
        let mut parent = Computed::default();
        parent.apply_one("scrollbar-gutter", "both-edges  stable");
        parent.scroll.viewport_root = true;
        let mut own = Computed::default();
        own.apply_one("scrollbar-gutter", "inherit");
        own.apply_one("scrollbar-gutter", "both-edges"); // invalid declaration retains inherit
        let child = crate::style::cascade::inherit::inherit(&parent, &own);
        assert_eq!(child.scroll.gutter, Some(2));
        assert!(!child.scroll.viewport_root);
        let local = crate::style::cascade::inherit::inherit(&parent, &Computed::default());
        assert_eq!(local.scroll.gutter, None);
    }
}
