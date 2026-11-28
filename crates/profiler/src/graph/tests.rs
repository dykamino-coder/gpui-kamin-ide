use super::*;
use kamin_heap::retain::{Block, Memory, Region};

struct Words(BTreeMap<usize, Vec<usize>>);
impl Memory for Words {
    fn words(&self, address: usize, _: usize) -> &[usize] {
        self.0.get(&address).map_or(&[], Vec::as_slice)
    }
}

#[test]
fn shared_diamond_cut_paths_nested_groups_and_v8_wire() {
    // root -> A,B; A,B -> shared C; C -> D. A and C have the same label.
    let words = Words(BTreeMap::from([
        (16, vec![64, 96]),
        (64, vec![128]),
        (96, vec![128]),
        (128, vec![160]),
    ]));
    let graph = Graph::build(
        [64, 96, 128, 160]
            .map(|address| Block { address, bytes: 16 })
            .to_vec(),
        vec![Region {
            name: "stack".into(),
            start: 16,
            bytes: 16,
        }],
        &words,
    );
    let labels: Vec<_> = ["shared", "B", "shared", "D"]
        .map(|site| Label {
            site: site.into(),
            ..Label::default()
        })
        .to_vec();
    let view = View::new(&graph, &labels).unwrap();
    assert_eq!(
        view.groups()["shared"],
        Group {
            objects: 2,
            self_bytes: 32,
            retained_bytes: 48
        }
    );
    assert_eq!(view.cut(&["B".into()]).unreachable_requested_bytes, 0);
    assert_eq!(
        view.cut(&["shared".into(), "B".into()])
            .unreachable_requested_bytes,
        32
    );
    assert_eq!(view.holders(&["D".into()])["shared"].edges, 1);
    assert_eq!(view.path(3, false).last().unwrap().node, graph.root());
    assert_eq!(view.path(3, true)[1].node, 2);
    let mut bytes = Vec::new();
    export::heapsnapshot(&view, &mut bytes).unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(value["snapshot"]["node_count"], graph.nodes());
    let nodes = value["nodes"].as_array().unwrap();
    let edges = value["edges"].as_array().unwrap();
    assert_eq!(nodes.len(), graph.nodes() * 7);
    assert_eq!(edges.len(), graph.edge_to.len() * 3);
    assert_eq!(nodes[0], 9);
    assert_eq!(
        nodes
            .chunks(7)
            .map(|node| node[4].as_u64().unwrap())
            .sum::<u64>(),
        graph.edge_to.len() as u64
    );
    for edge in edges.chunks(3) {
        let target = edge[2].as_u64().unwrap() as usize;
        assert_eq!(target % 7, 0);
        assert!(target < nodes.len());
    }
}

#[test]
fn same_group_nested_dominators_are_counted_once() {
    let words = Words(BTreeMap::from([(16, vec![64]), (64, vec![96])]));
    let graph = Graph::build(
        [64, 96]
            .map(|address| Block { address, bytes: 16 })
            .to_vec(),
        vec![Region {
            name: "static".into(),
            start: 16,
            bytes: 8,
        }],
        &words,
    );
    let labels = vec![
        Label {
            site: "same".into(),
            ..Label::default()
        };
        2
    ];
    assert_eq!(
        View::new(&graph, &labels).unwrap().groups()["same"],
        Group {
            objects: 2,
            self_bytes: 32,
            retained_bytes: 32
        }
    );
}
