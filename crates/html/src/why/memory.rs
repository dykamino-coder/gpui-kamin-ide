//! Memory owners of the document, window, process caches and GPU storage.

use crate::Document;
use gpui::{App, Window};
use serde_json::{Value, json};
use std::collections::BTreeMap;

#[cfg(feature = "heap-report")]
pub(super) fn owners(
    doc: &Document,
    window: &Window,
    cx: &App,
    previous: &mut BTreeMap<String, u64>,
) -> Value {
    let mut ops = crate::heap::Ops::default();
    doc.heap_report(&mut ops);
    window.heap_report(cx, &mut |path, pointer, bytes| {
        if pointer.is_null() || ops.once(pointer) {
            ops.at(path, |ops| ops.block(pointer, bytes));
        }
    });
    let mut now = ops.report().entries().clone();
    for (name, bytes) in super::caches_held() {
        now.insert(format!("process/cache/{name}"), bytes as u64);
    }
    window.gpu_memory(&mut |path, bytes, _used| {
        *now.entry(format!("gpu/{path}")).or_default() += bytes;
    });
    let grew = super::text::changes(previous, &now);
    let gpu: u64 = now
        .iter()
        .filter(|(k, _)| k.starts_with("gpu/"))
        .map(|(_, v)| v)
        .sum();
    let heap = now.values().sum::<u64>() - gpu;
    *previous = now.clone();
    json!({"heap_bytes": heap, "gpu_bytes": gpu, "owners": now, "changed_bytes": grew})
}

#[cfg(not(feature = "heap-report"))]
pub(super) fn owners(_: &Document, _: &Window, _: &App, _: &mut BTreeMap<String, u64>) -> Value {
    json!({"heap_bytes": null, "gpu_bytes": null, "owners": null, "changed_bytes": null})
}

pub(super) fn process(memory: &mut Value, private: Option<u64>) {
    if let Some(private) = private {
        memory["process_private_bytes"] = json!(private);
        memory["outside_owners_bytes"] = memory["heap_bytes"]
            .as_u64()
            .map(|heap| json!(private as i64 - heap as i64))
            .unwrap_or(Value::Null);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_memory_belongs_to_the_explicit_owner_checkpoint() {
        let mut sample = json!({"heap_bytes": 100});
        process(&mut sample, None);
        assert!(sample.get("process_private_bytes").is_none());
        process(&mut sample, Some(150));
        assert_eq!(sample["process_private_bytes"], 150);
        assert_eq!(sample["outside_owners_bytes"], 50);
        process(&mut sample, Some(90));
        assert_eq!(sample["outside_owners_bytes"], -10);
        sample["heap_bytes"] = Value::Null;
        process(&mut sample, Some(200));
        assert_eq!(sample["process_private_bytes"], 200);
        assert!(sample["outside_owners_bytes"].is_null());
    }
}
