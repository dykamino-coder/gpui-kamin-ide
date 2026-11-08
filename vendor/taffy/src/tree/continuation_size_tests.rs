//! Leaf-only suspended state stays outside container continuations.
use super::*;
use core::{hint::black_box, mem::size_of_val};

struct PayloadMeasure<const N: usize>;
impl<const N: usize> crate::tree::LayoutMeasurer<()> for PayloadMeasure<N> {
    async fn measure<'a>(
        &'a mut self,
        _: LayoutInput,
        _: NodeId,
        _: Option<&'a mut ()>,
        _: &'a Style,
        _: &'a mut reblessive::Stk,
    ) -> LayoutOutput {
        let payload = black_box([0u8; N]);
        core::future::pending::<()>().await;
        black_box(payload);
        LayoutOutput::HIDDEN
    }
}

fn ancestor_bytes<const N: usize>() -> usize {
    let mut tree = TaffyTree::<()>::new();
    let node = tree.new_leaf(Style::DEFAULT).unwrap();
    let mut view = TaffyView {
        taffy: &mut tree,
        measure_function: PayloadMeasure::<N>,
    };
    reblessive::Stack::new()
        .enter(|stack| async {
            let future = view.compute_child_layout_on_stack(
                node,
                LayoutInput::HIDDEN,
                #[cfg(feature = "block_layout")]
                None,
                stack,
            );
            size_of_val(&future)
        })
        .finish()
}

#[test]
fn leaf_measurement_payload_does_not_widen_ancestor_continuations() {
    let ordinary = ancestor_bytes::<0>();
    let large_leaf = ancestor_bytes::<4096>();
    assert_eq!(
        ordinary, large_leaf,
        "leaf-only state leaked into ancestor: {ordinary} -> {large_leaf} bytes"
    );
}
