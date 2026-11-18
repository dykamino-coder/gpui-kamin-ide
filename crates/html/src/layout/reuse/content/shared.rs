//! Equal key inputs shared by the content records of a document (`KeyInputs`).
//!
//! Containers of one look (the cells of a table) record equal styles and
//! text styles from outside their content; each record kept its own copy, so
//! a key cost a kilobyte. The store keeps the latest distinct ones and hands
//! out an equal one, as a style sharing cache offers a recent sibling's style
//! (Gecko `StyleSharingCache`, Blink `StyleSharingList`).

use crate::style::computed::BoxStyle;
use std::cell::Cell;
use std::rc::Rc;

/// Candidates kept per input.
const KEPT: usize = 8;

/// The latest distinct values of one input, the found one first.
struct Latest<T>(Cell<Vec<Rc<T>>>);

impl<T> Default for Latest<T> {
    fn default() -> Self {
        Self(Cell::default())
    }
}

impl<T: Clone> Latest<T> {
    fn share(&self, value: &T, same: impl Fn(&T, &T) -> bool) -> Rc<T> {
        let mut latest = self.0.take();
        let shared = match latest.iter().position(|v| same(v, value)) {
            Some(i) => latest.remove(i),
            None => {
                latest.truncate(KEPT - 1);
                Rc::new(value.clone())
            }
        };
        latest.insert(0, shared.clone());
        self.0.set(latest);
        shared
    }
}

/// Shared inputs of content keys from outside the containers.
#[derive(Default)]
pub(crate) struct KeyInputs {
    styles: Latest<BoxStyle>,
    roots: Latest<BoxStyle>,
    texts: Latest<gpui::TextStyle>,
    envs: Latest<super::key::KeyEnv>,
}

impl KeyInputs {
    /// A container style with the box values of `style` (`same_box_values`,
    /// which leaves out the box's own identity: the key keeps that apart).
    pub(in crate::layout::reuse) fn style(&self, style: &BoxStyle) -> Rc<BoxStyle> {
        self.styles.share(style, BoxStyle::same_box_values)
    }

    pub(super) fn root(&self, root: &BoxStyle) -> Rc<BoxStyle> {
        self.roots.share(root, |a, b| **a == **b)
    }

    pub(in crate::layout::reuse) fn text(&self, text: &gpui::TextStyle) -> Rc<gpui::TextStyle> {
        self.texts.share(text, |a, b| a == b)
    }

    /// One environment for the content keys of equal containers: a key
    /// holds its container's identity and this pointer.
    pub(super) fn env(&self, env: &super::key::KeyEnv) -> Rc<super::key::KeyEnv> {
        self.envs.share(env, super::key::KeyEnv::same)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn equal_inputs_are_one_and_only_the_latest_are_kept() {
        let inputs = KeyInputs::default();
        let a = inputs.text(&gpui::TextStyle::default());
        assert!(Rc::ptr_eq(&a, &inputs.text(&gpui::TextStyle::default())));
        for n in 0..KEPT + 2 {
            let text = gpui::TextStyle {
                line_clamp: Some(n + 1),
                ..Default::default()
            };
            inputs.text(&text);
        }
        let texts = inputs.texts.0.take();
        assert_eq!(texts.len(), KEPT);
        inputs.texts.0.set(texts);
        assert!(!Rc::ptr_eq(&a, &inputs.text(&gpui::TextStyle::default())));
    }
}
