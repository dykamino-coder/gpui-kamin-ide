use super::*;

struct Empty;
impl super::super::Memory for Empty {
    fn words(&self, _: usize, _: usize) -> &[usize] {
        &[]
    }
}

#[test]
fn wire_roundtrip_and_malformed_counts_are_bounded() {
    let graph = Graph::build(
        vec![Block {
            address: 64,
            bytes: 32,
        }],
        vec![],
        &Empty,
    );
    let mut data = Vec::new();
    graph.write(&mut data, "{}").unwrap();
    let (copy, header) = Graph::read(&mut data.as_slice(), ReadLimits::default()).unwrap();
    assert_eq!(header, "{}");
    assert_eq!(copy.blocks, graph.blocks);
    assert_eq!(copy.retained, graph.retained);
    assert_eq!(copy.edge_to, graph.edge_to);
    assert!(
        Graph::read(
            &mut data.as_slice(),
            ReadLimits {
                file_bytes: data.len() - 1,
                ..ReadLimits::default()
            }
        )
        .is_err()
    );
    for length in [0, MAGIC.len(), data.len() - 1] {
        assert!(Graph::read(&mut &data[..length], ReadLimits::default()).is_err());
    }
    let counts = MAGIC.len() + 4 + 2;
    data[counts..counts + 8].copy_from_slice(&u64::MAX.to_le_bytes());
    assert!(Graph::read(&mut data.as_slice(), ReadLimits::default()).is_err());
}
