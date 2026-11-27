//! Boxes of a sample's layout passes by whether their layout inputs are those
//! of the previous pass (gpui `LayoutInputCounts`): the bound of laying out
//! only boxes whose inputs changed, and of skipping unchanged subtrees whole.

use gpui::LayoutInputCounts;
use serde_json::{Value, json};
use std::fmt::Write;

pub(super) fn to_json(counts: LayoutInputCounts) -> Value {
    if counts.passes == 0 {
        return Value::Null;
    }
    json!({
        "passes": counts.passes, "boxes": counts.boxes,
        "same_as_last_pass": counts.same, "in_kept_subtrees": counts.whole,
        "kept_subtrees": counts.skips, "performed": counts.performed,
    })
}

pub(super) fn render(out: &mut String, value: &Value) {
    let Some(boxes) = value["boxes"].as_u64() else {
        return;
    };
    let share = |n: &Value| 100.0 * n.as_u64().unwrap_or(0) as f64 / boxes.max(1) as f64;
    let _ = writeln!(
        out,
        "  inputs boxes={boxes} same={} ({:.1} %) in kept subtrees={} ({:.1} %) subtrees={} performed={} passes={}",
        value["same_as_last_pass"],
        share(&value["same_as_last_pass"]),
        value["in_kept_subtrees"],
        share(&value["in_kept_subtrees"]),
        value["kept_subtrees"],
        value["performed"],
        value["passes"],
    );
}

#[cfg(test)]
mod tests {
    use gpui::LayoutInputCounts;
    use serde_json::Value;

    #[test]
    fn counts_render_with_their_share_of_the_boxes() {
        assert!(super::to_json(LayoutInputCounts::default()).is_null());
        let value = super::to_json(LayoutInputCounts {
            passes: 2,
            boxes: 8,
            same: 6,
            whole: 4,
            skips: 1,
            performed: 3,
        });
        assert_eq!(value["same_as_last_pass"], 6);
        let mut out = String::new();
        super::render(&mut out, &value);
        assert!(out.contains(
            "boxes=8 same=6 (75.0 %) in kept subtrees=4 (50.0 %) subtrees=1 performed=3"
        ));
        let mut none = String::new();
        super::render(&mut none, &Value::Null);
        assert!(none.is_empty());
    }
}
