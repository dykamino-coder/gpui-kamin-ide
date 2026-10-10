//! Inspect constructed native styles and layout only under an explicit diagnostic flag.
use super::{EXPECT_MESSAGE, LayoutId, TaffyLayoutEngine};
use taffy::TraversePartialTree as _;

pub(super) fn dump(engine: &TaffyLayoutEngine, root: LayoutId, scale: f32) {
    static ENABLED: std::sync::LazyLock<bool> =
        std::sync::LazyLock::new(|| std::env::var_os("GPUI_NATIVE_TREE").is_some());
    if !*ENABLED {
        return;
    }
    eprintln!("NATIVE_TREE root={root:?} scale={scale}");
    let mut pending = vec![root];
    while let Some(id) = pending.pop() {
        let children = engine.taffy.children(id.into()).expect(EXPECT_MESSAGE);
        let style = engine.taffy.style(id.into()).expect(EXPECT_MESSAGE);
        let layout = engine.taffy.layout(id.into()).expect(EXPECT_MESSAGE);
        eprintln!("NATIVE_NODE id={id:?} children={children:?} style={style:?} layout={layout:?}");
        pending.extend(children.into_iter().rev().map(LayoutId::from));
    }
}

impl TaffyLayoutEngine {
    // Used to understand performance
    #[allow(dead_code)]
    fn count_all_children(&self, parent: LayoutId) -> anyhow::Result<u32> {
        let mut count = 0;

        for child in self.taffy.children(parent.0)? {
            // Count this child.
            count += 1;

            // Count all of this child's children.
            count += self.count_all_children(LayoutId(child))?
        }

        Ok(count)
    }

    // Used to understand performance
    #[allow(dead_code)]
    fn max_depth(&self, depth: u32, parent: LayoutId) -> anyhow::Result<u32> {
        println!(
            "{parent:?} at depth {depth} has {} children",
            self.taffy.child_count(parent.0)
        );

        let mut max_child_depth = 0;

        for child in self.taffy.children(parent.0)? {
            max_child_depth = std::cmp::max(max_child_depth, self.max_depth(0, LayoutId(child))?);
        }

        Ok(depth + 1 + max_child_depth)
    }

    // Used to understand performance
    #[allow(dead_code)]
    fn get_edges(&self, parent: LayoutId) -> anyhow::Result<Vec<(LayoutId, LayoutId)>> {
        let mut edges = Vec::new();

        for child in self.taffy.children(parent.0)? {
            edges.push((parent, LayoutId(child)));

            edges.extend(self.get_edges(LayoutId(child))?);
        }

        Ok(edges)
    }
}
