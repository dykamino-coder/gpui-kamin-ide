//! Shared cascades with gradient values: purity of gradient text and equal
//! computed gradients with and without sharing.
use super::*;
use std::rc::Rc;

#[test]
fn gradients_are_pure_unless_a_nested_function_is_contextual() {
    for value in [
        "linear-gradient(90deg, #def, #fff)",
        "REPEATING-radial-gradient(circle, rgb(1 2 3) 2em, currentcolor 3em)",
        "conic-gradient(from 45deg, red, oklch(.5 .2 30))",
    ] {
        assert!(pure_value(value), "{value}");
    }
    for value in [
        "linear-gradient(var(--a), red)",
        "linear-gradient(red calc(10% + 1px), blue)",
        "radial-gradient(red, blue), url(a.png)",
        "-webkit-linear-gradient(red, blue)",
    ] {
        assert!(!pure_value(value), "{value}");
    }
}

/// A shared gradient computes exactly what the per-element cascade computes:
/// `currentcolor` and font-relative stops are resolved after the shared step.
#[test]
fn shared_gradients_compute_like_unshared_ones() {
    fn gradients(html: &str) -> Vec<(Computed, Rc<Computed>)> {
        fn cells(nodes: &[crate::dom::Node], out: &mut Vec<(Computed, Rc<Computed>)>) {
            for node in nodes {
                if let crate::dom::Node::Element(el) = node {
                    if &*el.tag == "i" {
                        out.push((el.style.cascaded().clone(), el.style.computed()));
                    }
                    cells(&el.children, out);
                }
            }
        }
        let mut out = Vec::new();
        cells(&crate::dom::parse(html, "").nodes, &mut out);
        out
    }
    let style = "<style>.g{background-image:linear-gradient(currentcolor, red 2em)}\
                 p{font-size:10px}</style>";
    let shared = gradients(&format!(
        "{style}<p style='color:#00f'><i class=g></i></p>\
         <p style='color:#0f0;font-size:20px'><i class=g></i></p>"
    ));
    let alone = gradients(&format!(
        "{style}<p style='color:#00f'><i class=g style='--a:1'></i></p>\
         <p style='color:#0f0;font-size:20px'><i class=g style='--a:2'></i></p>"
    ));
    assert_eq!((shared.len(), alone.len()), (2, 2));
    assert!(shared[0].0.shares_storage(&shared[1].0));
    for (s, a) in shared.iter().zip(&alone) {
        assert_eq!(s.1.reset_background.gradient, a.1.reset_background.gradient);
        assert_eq!(
            s.1.reset_background.gradient_current,
            a.1.reset_background.gradient_current
        );
    }
}
