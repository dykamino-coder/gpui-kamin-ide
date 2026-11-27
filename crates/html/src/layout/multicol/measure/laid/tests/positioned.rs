//! Positioned sources stay outside normal-flow measurement.

use super::*;

#[test]
fn direct_positioned_siblings_are_not_normal_measurements() {
    for position in ["absolute", "fixed"] {
        let tree = crate::document::parsed::parse(
            &format!(
                "<div><aside style='position:{position};height:200px'></aside><div style='height:20px'></div><aside style='position:{position}'></aside><div style='height:30px'></div></div>"
            ),
            "",
        );
        let host = root(&tree);
        let measured = RefCell::default();
        let kids = children(host, &BoxStyle::default(), frame(&tree), 100., &measured).unwrap();
        assert_eq!(kids.len(), 2);
        assert!(kids.iter().all(|(c, _)| c.tag == "div"));
        assert_eq!(measured.borrow().len(), 2);
        let pending = pending(host, &BoxStyle::default(), frame(&tree)).unwrap();
        assert_eq!(
            pending.iter().map(|(c, _)| c.node_id).collect::<Vec<_>>(),
            kids.iter().map(|(c, _)| c.node_id).collect::<Vec<_>>()
        );
    }
}
