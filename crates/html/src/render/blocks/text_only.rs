//! Прямой путь блочного контейнера, у которого все дети — текст.
//!
//! Каждый проход подготовки детей (анонимные таблицы, вынос абсолютов,
//! разрыв строчного блоком, схлопывание полей, флоаты, слои краски) работает
//! только с элементами и текстовый узел возвращает как есть. Такой список
//! сразу идёт в цикл потока: ни копии детей, ни запросов фактов поддеревьев.

use super::flow::blocks_flow;
use crate::dom::Node;
use crate::layout::options::RenderOpts;
use crate::render::first_letter_scope::Scope;
use crate::style::computed::BoxStyle;
use gpui::AnyElement;

/// Подготовка детей вернула бы `nodes` без изменений.
///
/// Исключения: гибкий контейнер и сетка (свои анонимные элементы) и буквица
/// `initial-letter`, которая вырезает первую букву из текста в плавающий узел.
pub(super) fn applies(nodes: &[Node], inherited: &BoxStyle, ordered_context: bool) -> bool {
    !ordered_context
        && nodes.iter().all(|n| matches!(n, Node::Text(_)))
        && inherited
            .reset_text_box
            .first_letter
            .as_deref()
            .is_none_or(|first| first.reset_text_box.initial_letter.is_none())
}

pub(super) async fn flow(
    stk: &mut reblessive::Stk,
    nodes: &[Node],
    inherited: &BoxStyle,
    opts: &RenderOpts,
    under_tf: bool,
) -> Vec<AnyElement> {
    blocks_flow(
        stk,
        nodes,
        vec![],
        vec![],
        Scope::new(nodes, inherited),
        inherited,
        opts,
        false,
        under_tf,
        Default::default(),
        usize::MAX,
        0,
        vec![],
    )
    .await
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::Element;
    use crate::style::computed::{Computed, ResetTextBox, StyleGroup};

    fn text() -> Vec<Node> {
        vec![Node::Text("Row 1 alpha".into()), Node::Text(" ".into())]
    }

    fn with_first_letter(initial_letter: Option<(f32, u32)>) -> BoxStyle {
        let letter = Computed {
            reset_text_box: StyleGroup::new(ResetTextBox {
                initial_letter,
                ..Default::default()
            }),
            ..Default::default()
        };
        BoxStyle::from(Computed {
            reset_text_box: StyleGroup::new(ResetTextBox {
                first_letter: Some(Box::new(letter)),
                ..Default::default()
            }),
            ..Default::default()
        })
    }

    #[test]
    fn text_children_of_a_block_take_the_direct_path() {
        let plain = BoxStyle::from(Computed::default());
        assert!(applies(&text(), &plain, false));
        assert!(applies(&[], &plain, false));
        assert!(applies(&text(), &with_first_letter(None), false));
    }

    #[test]
    fn elements_ordered_contexts_and_initial_letters_keep_the_full_path() {
        let plain = BoxStyle::from(Computed::default());
        let mut mixed = text();
        mixed.push(Node::Element(Element::synthetic(
            "span",
            Computed::default(),
            Default::default(),
            vec![],
        )));
        assert!(!applies(&mixed, &plain, false));
        assert!(!applies(&text(), &plain, true));
        assert!(!applies(&text(), &with_first_letter(Some((2.0, 2))), false));
    }
}
