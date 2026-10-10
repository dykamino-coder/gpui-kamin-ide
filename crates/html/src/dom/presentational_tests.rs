//! Presentational tests for dom; split out to keep the owning module within 250 lines.

use super::*;
use crate::style::values::value::Len;

/// `<img width=100>` — представленческая подсказка, и без неё картинка
/// набирается по своему пикселю вместо заявленного размера.
#[test]
fn image_size_attributes_reach_the_style() {
    let nodes = parse(r#"<img src="x.png" width="100" height="40">"#, "");
    fn find(nodes: &[Node]) -> Option<&Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.tag == "img" {
                    return Some(e);
                }
                if let Some(found) = find(&e.children) {
                    return Some(found);
                }
            }
        }
        None
    }
    let img = find(&nodes).expect("картинка в дереве");
    assert_eq!(img.style.width, Some(Len::Px(100.0)));
    assert_eq!(img.style.height, Some(Len::Px(40.0)));
}
