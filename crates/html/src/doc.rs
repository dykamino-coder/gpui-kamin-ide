//! Разобранный документ: то, что живёт между кадрами.
//!
//! Ключевое требование к переводчику — не быть дороже нативной вёрстки. В
//! GPUI элементы пересоздаются каждый кадр, и это нормально: сборка `div` с
//! готовыми числами дёшева. Дорого другое — разбор разметки, каскад и
//! растеризация рисунков. Всё это обязано случиться ОДИН раз на документ, а
//! не на кадр.
//!
//! Поэтому вызывающий держит у себя `Document`, а на кадре зовёт только
//! `render`. Пересборка происходит лишь когда сменилась сама разметка — что
//! проверяется по хэшу, а не по строке целиком.

use crate::dom::Node;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

/// Документ, разобранный один раз.
pub struct Document {
    nodes: Vec<Node>,
    /// Хэш разметки и темы: по нему видно, нужен ли повторный разбор.
    key: u64,
}

impl Document {
    pub fn new(html: &str, theme_css: &str) -> Self {
        Document {
            nodes: unwrap_document(crate::dom::parse(html, theme_css)),
            key: hash_of(html, theme_css),
        }
    }

    /// Разобрать заново, только если разметка действительно изменилась.
    ///
    /// Для стриминга (текст дописывается по кусочку) это и есть главный
    /// рубеж: пока пришедший кусок не изменил разметку, дерево остаётся тем
    /// же, и кадр стоит ровно столько же, сколько нативный.
    pub fn update(&mut self, html: &str, theme_css: &str) -> bool {
        let key = hash_of(html, theme_css);
        if key == self.key {
            return false;
        }
        self.nodes = unwrap_document(crate::dom::parse(html, theme_css));
        self.key = key;
        true
    }

    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    /// Сколько узлов в документе — для решения о виртуализации: раскладка в
    /// GPUI считается заново каждый кадр, поэтому длинный документ обязан
    /// рисоваться по видимым блокам, а не целиком.
    pub fn node_count(&self) -> usize {
        fn walk(nodes: &[Node]) -> usize {
            nodes
                .iter()
                .map(|n| match n {
                    Node::Text(_) => 1,
                    Node::Element(e) => 1 + walk(&e.children),
                })
                .sum()
        }
        walk(&self.nodes)
    }

    /// Блоки верхнего уровня — единица виртуализации: их можно раздать в
    /// список GPUI, который раскладывает только видимое.
    pub fn top_level_blocks(&self) -> usize {
        self.nodes.len()
    }
}

/// Снять обёртки `<html>`/`<body>`, которые парсер добавляет всегда.
///
/// Без этого «блоков верхнего уровня» ровно один — весь документ — и
/// виртуализация теряет смысл: список спрашивает единственный элемент и
/// раскладывает вместе с ним всё содержимое.
fn unwrap_document(mut nodes: Vec<Node>) -> Vec<Node> {
    loop {
        let single_wrapper = match nodes.as_slice() {
            [Node::Element(e)] if matches!(e.tag.as_str(), "html" | "body") => true,
            _ => false,
        };
        if !single_wrapper {
            return nodes;
        }
        let Some(Node::Element(e)) = nodes.pop() else {
            return nodes;
        };
        nodes = e.children;
    }
}

fn hash_of(html: &str, theme: &str) -> u64 {
    let mut h = DefaultHasher::new();
    html.hash(&mut h);
    theme.hash(&mut h);
    h.finish()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn same_markup_is_not_reparsed() {
        let mut doc = Document::new("<p>раз</p>", "");
        assert!(
            !doc.update("<p>раз</p>", ""),
            "разметка та же — разбора нет"
        );
        assert!(
            doc.update("<p>два</p>", ""),
            "разметка иная — разобрали заново"
        );
    }

    #[test]
    fn theme_change_also_triggers_a_reparse() {
        let mut doc = Document::new("<p>раз</p>", "");
        assert!(
            doc.update("<p>раз</p>", "p { color: red }"),
            "сменилась тема"
        );
    }

    #[test]
    fn document_wrappers_are_unwrapped() {
        // Парсер всегда добавляет <html><body>; для виртуализации нужны
        // настоящие блоки документа, а не одна обёртка.
        let doc = Document::new("<div>раз</div><div>два</div>", "");
        assert_eq!(
            doc.top_level_blocks(),
            2,
            "получено {}",
            doc.top_level_blocks()
        );
    }

    #[test]
    fn node_count_walks_the_whole_tree() {
        let doc = Document::new("<div><p>раз</p><p>два</p></div>", "");
        // html + body + div + два абзаца + два текста — важна не точная цифра,
        // а то, что счёт идёт вглубь.
        assert!(doc.node_count() >= 5, "получено {}", doc.node_count());
    }
}
