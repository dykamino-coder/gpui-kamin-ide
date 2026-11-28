//! Streaming V8 heap-snapshot serialization: graph arrays are not duplicated in
//! a JSON Value. The synthetic root is node zero, as required by DevTools.

use super::View;
use serde_json::json;
use std::{
    collections::BTreeMap,
    io::{self, Write},
};

fn string(
    text: String,
    strings: &mut Vec<String>,
    interned: &mut BTreeMap<String, usize>,
) -> usize {
    if let Some(&index) = interned.get(&text) {
        return index;
    }
    let index = strings.len();
    interned.insert(text.clone(), index);
    strings.push(text);
    index
}

pub fn heapsnapshot(view: &View<'_>, out: &mut impl Write) -> io::Result<()> {
    let graph = view.graph;
    let meta = json!({
        "node_fields": ["type", "name", "id", "self_size", "edge_count", "trace_node_id", "detachedness"],
        "node_types": [["hidden", "array", "string", "object", "code", "closure", "regexp", "number", "native", "synthetic", "concatenated string", "sliced string", "symbol", "bigint", "object shape"], "string", "number", "number", "number", "number", "number"],
        "edge_fields": ["type", "name_or_index", "to_node"],
        "edge_types": [["context", "element", "property", "internal", "hidden", "shortcut", "weak"], "string_or_number", "node"],
        "trace_function_info_fields": ["function_id", "name", "script_name", "script_id", "line", "column"],
        "trace_node_fields": ["id", "function_info_index", "count", "size", "children"],
        "sample_fields": ["timestamp_us", "last_assigned_id"],
        "location_fields": ["object_index", "script_id", "line", "column"],
    });
    let order: Vec<_> = (graph.root()..graph.nodes() as u32)
        .chain(0..graph.root())
        .collect();
    let mut position = vec![0; graph.nodes()];
    for (index, &node) in order.iter().enumerate() {
        position[node as usize] = index;
    }
    let mut strings = Vec::new();
    let mut interned = BTreeMap::new();
    write!(out, "{{\"snapshot\":")?;
    serde_json::to_writer(
        &mut *out,
        &json!({ "meta": meta, "node_count": graph.nodes(), "edge_count": graph.edge_to.len(), "trace_function_count": 0 }),
    )?;
    write!(out, ",\"nodes\":[")?;
    for (index, &node) in order.iter().enumerate() {
        let is_block = (node as usize) < graph.blocks.len();
        let name = string(
            if is_block {
                view.group(node).to_owned()
            } else {
                view.label(node)
            },
            &mut strings,
            &mut interned,
        );
        let edges = graph.edge_start[node as usize + 1] - graph.edge_start[node as usize];
        write!(
            out,
            "{}{},{},{},{},{},0,0",
            if index == 0 { "" } else { "," },
            if is_block { 3 } else { 9 },
            name,
            2 * index + 1,
            graph.bytes(node),
            edges
        )?;
    }
    write!(out, "],\"edges\":[")?;
    let mut first = true;
    for node in order {
        for (target, offset) in graph.edges(node) {
            let label = string(format!("+0x{offset:x}"), &mut strings, &mut interned);
            write!(
                out,
                "{}2,{},{}",
                if first { "" } else { "," },
                label,
                position[target as usize] * 7
            )?;
            first = false;
        }
    }
    write!(
        out,
        "],\"trace_function_infos\":[],\"trace_tree\":[],\"samples\":[],\"locations\":[],\"strings\":"
    )?;
    serde_json::to_writer(&mut *out, &strings)?;
    writeln!(out, "}}")
}
