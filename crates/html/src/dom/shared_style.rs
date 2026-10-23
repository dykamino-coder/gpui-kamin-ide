//! Стиль элемента по стадиям значений css-cascade-5 §4: каскадный (выход каскада, его правит только
//! подготовка документа), вычисленный (материализуется из каскадного,
//! `style::cascade::materialize`) и слой used-значений раскладки поверх вычисленного. Раскладка
//! читает вычисленный стиль и пишет только в свой слой: каскадный и вычисленный стили остаются
//! общими для всех копий элемента (Blink: `LayoutObject::Style()` — неизменяемый вычисленный стиль;
//! Servo: `Arc<ComputedValues>`).

use crate::style::cascade::materialize::ComputedSlot;
use crate::style::computed::Computed;
use std::rc::Rc;

/// The style of an element by value stage. Copies of an element share all
/// three stages; an edit copies only the stage it edits.
#[derive(Clone, Debug, Default)]
pub struct SharedStyle {
    /// The cascaded style: cascade output, edited by document preparation only.
    cascaded: Rc<Computed>,
    /// The computed style materialized from `cascaded`.
    computed: ComputedSlot,
    /// Layout's used values: a copy of the computed style that layout edited.
    used: Option<Rc<Computed>>,
}

impl From<Computed> for SharedStyle {
    fn from(style: Computed) -> Self {
        Self {
            cascaded: Rc::new(style),
            computed: ComputedSlot::default(),
            used: None,
        }
    }
}

impl SharedStyle {
    /// The cascaded style (css-cascade-5 §4.2): input of the computed-value stage.
    pub(crate) fn cascaded(&self) -> &Computed {
        &self.cascaded
    }

    /// Edit the cascaded style (cascade and document preparation, before
    /// materialization): drops the computed style and layout's values.
    pub(crate) fn cascaded_mut(&mut self) -> &mut Computed {
        self.computed = ComputedSlot::default();
        self.used = None;
        Rc::make_mut(&mut self.cascaded)
    }

    /// The materialization slot of the cascaded style.
    pub(crate) fn slot(&self) -> &ComputedSlot {
        &self.computed
    }

    /// The element's computed style with layout's used values. A style that
    /// was never materialized has one stage (document preparation, a box
    /// layout made): the style itself is returned.
    pub fn computed(&self) -> Rc<Computed> {
        match &self.used {
            Some(used) => used.clone(),
            None => self.base(),
        }
    }

    /// Layout's used-value layer, copy-on-write over the computed style. A
    /// style that was never materialized has one stage, and that stage is
    /// edited.
    pub(crate) fn used_mut(&mut self) -> &mut Computed {
        if self.used.is_none() && self.computed.any().is_none() {
            return Rc::make_mut(&mut self.cascaded);
        }
        let base = match self.used.take() {
            Some(used) => used,
            None => self.base(),
        };
        Rc::make_mut(self.used.insert(base))
    }

    /// Layout's used-value layer, if layout edited this style.
    pub(crate) fn used(&self) -> Option<&Computed> {
        self.used.as_deref()
    }

    fn base(&self) -> Rc<Computed> {
        self.computed.any().unwrap_or_else(|| self.cascaded.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::style::values::value::Len;

    #[test]
    fn layout_edits_do_not_change_a_shared_cascade() {
        let parsed = crate::dom::parse(r#"<div style="width: 50%">a</div>"#, "");
        let vcx = parsed.values();
        crate::style::cascade::materialize::materialize(
            &parsed.nodes,
            &Rc::new(Computed::default()),
            vcx,
        );
        fn find<'a>(nodes: &'a [crate::dom::Node]) -> Option<&'a crate::dom::Element> {
            nodes.iter().find_map(|n| match n {
                crate::dom::Node::Element(e) if e.tag == "div" => Some(e),
                crate::dom::Node::Element(e) => find(&e.children),
                _ => None,
            })
        }
        let original = &find(&parsed.nodes).unwrap().style;
        let mut copy = original.clone();
        assert!(Rc::ptr_eq(&original.cascaded, &copy.cascaded));
        copy.used_mut().width = Some(Len::Px(100.0));
        assert_eq!(original.computed().width, Some(Len::Pct(0.5)));
        assert_eq!(copy.computed().width, Some(Len::Px(100.0)));
        // Neither the cascaded nor the materialized computed style changed.
        assert!(Rc::ptr_eq(&original.cascaded, &copy.cascaded));
        assert!(Rc::ptr_eq(
            &original.slot().any().unwrap(),
            &copy.slot().any().unwrap()
        ));
        assert_eq!(copy.cascaded().width, Some(Len::Pct(0.5)));
    }

    #[test]
    fn a_style_without_materialization_has_one_stage() {
        let mut style = SharedStyle::from(Computed::default());
        style.used_mut().width = Some(Len::Px(10.0));
        assert_eq!(style.cascaded().width, Some(Len::Px(10.0)));
        assert!(style.used().is_none());
    }

    #[test]
    fn element_copies_share_pseudo_layers_and_keyframes() {
        let mut node = crate::dom::table_boxes::anon_element("div", vec![]);
        node.hover = Some(Computed::default().into());
        node.first_letter = node.hover.clone();
        node.first_line = node.hover.clone();
        node.anim = Some(Rc::new(vec![(0.0, Computed::default())]));
        node.style.cascaded_mut().width = Some(Len::Pct(0.5));
        let mut copy = node.clone();
        copy.style.used_mut().width = Some(Len::Px(100.0));
        assert_eq!(node.style.computed().width, Some(Len::Pct(0.5)));
        assert!(Rc::ptr_eq(
            node.anim.as_ref().unwrap(),
            copy.anim.as_ref().unwrap()
        ));
        copy.hover.as_mut().unwrap().cascaded_mut().font_size = Some(Len::Px(32.0));
        assert!(node.hover.as_ref().unwrap().cascaded().font_size.is_none());
        assert!(
            copy.first_letter
                .as_ref()
                .unwrap()
                .cascaded()
                .font_size
                .is_none()
        );
        assert!(std::mem::size_of::<crate::dom::Element>() < 512);
    }
}
