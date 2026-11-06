//! Проверка полей заместителя идёт по краям дерева без роста native-стека.

use super::*;

fn node(engine: &mut TaffyLayoutEngine, style: Style, children: &[NodeId]) -> NodeId {
    engine.taffy.new_with_children(style, children).unwrap()
}

fn block() -> Style {
    Style {
        display: Display::Block,
        size: Size {
            width: Dimension::length(10.0),
            height: Dimension::length(10.0),
        },
        ..Style::default()
    }
}

fn layout(engine: &mut TaffyLayoutEngine, root: NodeId) {
    engine
        .taffy
        .compute_layout(root, Size::MAX_CONTENT)
        .unwrap();
}

#[test]
fn deep_margin_edges_use_a_constant_native_stack() {
    std::thread::Builder::new()
        .stack_size(512 * 1024)
        .spawn(|| {
            let mut engine = TaffyLayoutEngine::new();
            let mut root = node(&mut engine, block(), &[]);
            for _ in 0..20_000 {
                root = node(&mut engine, block(), &[root]);
            }
            layout(&mut engine, root);
            assert!(engine.own_margins(root, true));
            assert!(engine.own_margins(root, false));
        })
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn only_the_first_and_last_in_flow_edges_are_followed() {
    let mut engine = TaffyLayoutEngine::new();
    let mut margin = block();
    margin.margin.bottom = LengthPercentageAuto::length(2.0);
    let first = node(&mut engine, block(), &[]);
    let last = node(&mut engine, margin.clone(), &[]);
    margin.position = Position::Absolute;
    margin.margin.top = LengthPercentageAuto::length(2.0);
    let absolute = node(&mut engine, margin.clone(), &[]);
    margin.display = Display::None;
    let hidden = node(&mut engine, margin, &[]);
    let root = node(&mut engine, block(), &[hidden, first, last, absolute]);
    layout(&mut engine, root);
    assert!(engine.own_margins(root, true));
    assert!(!engine.own_margins(root, false));
}

#[test]
fn separating_edges_stop_before_a_descendant_margin() {
    let mut engine = TaffyLayoutEngine::new();
    let mut margin = block();
    margin.margin.top = LengthPercentageAuto::length(2.0);
    margin.margin.bottom = LengthPercentageAuto::length(2.0);
    let child = node(&mut engine, margin, &[]);
    let mut separated = block();
    separated.padding.top = LengthPercentage::length(1.0);
    separated.border.bottom = LengthPercentage::length(1.0);
    let root = node(&mut engine, separated, &[child]);
    layout(&mut engine, root);
    assert!(engine.own_margins(root, true));
    assert!(engine.own_margins(root, false));
    let mut empty = block();
    empty.size.height = Dimension::length(0.0);
    let empty = node(&mut engine, empty, &[]);
    let root = node(&mut engine, block(), &[empty]);
    layout(&mut engine, root);
    assert!(!engine.own_margins(root, true));
    assert!(!engine.own_margins(root, false));
}
