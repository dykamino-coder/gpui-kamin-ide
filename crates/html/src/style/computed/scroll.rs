//! CSS Scrollbars 1 styling, owned by HTML.

use super::{Computed, split_outside_parens};
use crate::document::state::ValueCx;
use crate::style::values::value::Color;

mod inherit;
pub(crate) use inherit::inherit;

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
    pub(crate) fn apply_scroll(&mut self, key: &str, v: &str, hit: &mut bool, vcx: ValueCx<'_>) {
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
                                Color::parse(part, vcx)
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
            _ => self.apply_snap(key, v, hit, vcx),
        }
    }
}

#[cfg(test)]
mod tests {
    //! Regression checks for the cascade boundary of HTML scrollbar policies.
    use crate::style::computed::Computed;

    #[test]
    fn scrollbar_color_inherits_but_geometry_does_not() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("scrollbar-color", "rgb(255, 0, 0) blue", vcx);
        parent.apply_one("scrollbar-width", "none", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &Computed::default(), vcx);
        assert_eq!(child.scroll.colors, parent.scroll.colors);
        assert_eq!(child.scroll.width, None);
        let mut reset = Computed::default();
        reset.apply_one("scrollbar-color", "initial", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &reset, vcx);
        assert_eq!(child.scroll.colors, Some(None));
    }

    #[test]
    fn inherited_scrollbar_current_color_keeps_the_parent_computed_color() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut declaration = Computed::default();
        declaration.apply_one("color", "red", vcx);
        declaration.apply_one("scrollbar-color", "currentColor blue", vcx);
        let parent =
            crate::style::cascade::inherit::inherit(&Computed::default(), &declaration, vcx);
        let mut own = Computed::default();
        own.apply_one("color", "green", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
        assert_eq!(child.scroll.colors, parent.scroll.colors);
        own.apply_one("scrollbar-color", "currentColor blue", vcx);
        let child = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
        assert_eq!(
            child.scroll.colors.flatten().unwrap()[0],
            child.color.unwrap()
        );
        assert_ne!(child.scroll.colors, parent.scroll.colors);
    }

    #[test]
    fn invalid_scrollbar_color_preserves_the_last_valid_declaration() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("scrollbar-color", "red blue", vcx);
        let valid = c.scroll.colors;
        for invalid in ["red", "red blue green", "red not-a-color"] {
            c.apply_one("scrollbar-color", invalid, vcx);
            assert_eq!(c.scroll.colors, valid);
        }
    }

    #[test]
    fn initial_opposite_overflow_axis_computes_to_auto_without_becoming_forced() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        use crate::style::computed::Overflow;
        let mut own = Computed::default();
        own.apply_one("overflow-x", "scroll", vcx);
        let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
        assert_eq!([c.overflow_x, c.overflow_y], [Some(Overflow::Scroll); 2]);
        assert_eq!(c.scroll.forced, [true, false]);
        let child = crate::style::cascade::inherit::inherit(&c, &Computed::default(), vcx);
        assert_eq!([child.overflow_x, child.overflow_y], [None; 2]);
    }

    #[test]
    fn clip_preserves_non_scroll_pairs_and_computes_to_hidden_beside_auto() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
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
            own.apply_one("overflow", declaration, vcx);
            let c = crate::style::cascade::inherit::inherit(&Computed::default(), &own, vcx);
            assert_eq!([c.overflow_x, c.overflow_y], expected);
            assert_eq!(c.scroll.forced, [false; 2]);
        }
    }

    #[test]
    fn root_used_gutter_suppression_does_not_change_explicit_computed_inheritance() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed::default();
        parent.apply_one("scrollbar-gutter", "both-edges  stable", vcx);
        parent.scroll.viewport_root = true;
        let mut own = Computed::default();
        own.apply_one("scrollbar-gutter", "inherit", vcx);
        own.apply_one("scrollbar-gutter", "both-edges", vcx); // invalid declaration retains inherit
        let child = crate::style::cascade::inherit::inherit(&parent, &own, vcx);
        assert_eq!(child.scroll.gutter, Some(2));
        assert!(!child.scroll.viewport_root);
        let local = crate::style::cascade::inherit::inherit(&parent, &Computed::default(), vcx);
        assert_eq!(local.scroll.gutter, None);
    }
}
