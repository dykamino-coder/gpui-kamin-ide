//! Unit boxes for container; split out to keep the owning module within 250 lines.

use crate::dom::Node;
use crate::render::{RenderOpts, blocks, replaced_tag};
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::ruby::{ruby_role, ruby_transform};
use crate::text::text_box::blank_text;
use gpui::{AnyElement, IntoElement, ParentElement, div};

// Единица из ОДНОГО `<rb>`/`<rt>` (или элемента с ролью базы /
// аннотации по `display`) рисуется его собственной БЛОЧНОЙ
// коробкой: распорка строки — от его кегля и `line-height` (UA
// `rt { font-size: 50%; line-height: 1 }`), а не от контейнера
// руби. Прежде коробка аннотации носила строку контейнера, и при
// разном кегле контейнера в тесте и эталоне
// (`ruby-base-different-size`: 16px против 32px) аннотации
// вставали на разной высоте; в `nested-ruby-pairing-001` уровень
// `<rt>` (стиль контейнера) выходил выше уровня `<rtc>`. Поля,
// рамка и фон единицы действуют (§3.3: базы и аннотации —
// строчные коробки, все их свойства применяются). Анонимная
// единица (текст) — по-прежнему абзац со стилем уровня.
// Содержимое единицы не рвётся: разрыв внутри базы — только
// вынужденный (§3.5.2), а атом монолитен; без `nowrap` анонимная
// база `あい` рвалась внутри колонки (эталон `rbc-rtc-basic-001`).
// Единица из одних пробелов (межбазовая, межаннотационная,
// межсегментная — css-ruby-1 §2.2 п.6) — это ПРОБЕЛ строки, а не
// пустота: у нас единица — свой блок, и пробел в нём срезался как
// краевой, единица выходила нулевой, а колонка без строки ломала
// общую базовую линию ряда (`ruby-box-generation-*`). Пробел
// заменяется неразрывным: ширина пробела, строка и базовая на месте.
// Пробельной считается и единица, где пробел обёрнут строчным
// элементом: эталоны пишут межбазовый пробел как
// `<rb><span> </span></rb>`, и пустая колонка без строки рядом с
// колонкой «e» роняла базовую ряда (`ruby-box-generation-001-ref`).
// Остаётся ОДИН неразрывный пробел — прочие пробельные тексты
// единицы схлопнулись бы с ним (css-text-3 §4.1.1).
pub(super) fn only_space(nodes: &[Node]) -> bool {
    nodes.iter().all(|n| match n {
        Node::Text(t) => blank_text(t),
        Node::Element(k) => {
            let plain = ruby_role(k)
                .is_some_and(|r| r != crate::style::computed::RubyRole::Container)
                || (k.style.display.is_none()
                    && k.style.inline_display != Some(false)
                    && !replaced_tag(k));
            plain && only_space(&k.children)
        }
    })
}

pub(super) fn has_space(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !t.is_empty(),
        Node::Element(k) => has_space(&k.children),
    })
}

pub(super) fn spaced(nodes: &[Node], done: &mut bool) -> Vec<Node> {
    nodes
        .iter()
        .map(|n| match n {
            Node::Text(t) if !t.is_empty() && !*done => {
                *done = true;
                Node::Text("\u{a0}".into())
            }
            Node::Text(_) => Node::Text(String::new()),
            Node::Element(k) => {
                let mut k = k.clone();
                k.children = spaced(&k.children, done);
                Node::Element(k)
            }
        })
        .collect()
}

pub(super) fn unit_box(nodes: &[Node], style: &Computed, opts: &RenderOpts) -> AnyElement {
    let blank_space = !nodes.is_empty() && only_space(nodes) && has_space(nodes);
    let owned;
    let nodes = if blank_space {
        owned = spaced(nodes, &mut false);
        &owned[..]
    } else {
        nodes
    };
    let mut style = style.clone();
    style.nowrap = Some(true);
    style.ruby_unit = true;
    // CSS Ruby 1 §2.1.1: these units share an inline formatting
    // context, rather than starting indented block paragraphs.
    // Blink line_breaker.cc:846-848 excludes ruby sub-line breakers.
    style.text_indent = Some(Len::Px(0.0));
    style.text_indent_each_line = Some(false);
    style.text_indent_hanging = Some(false);
    if let [Node::Element(k)] = nodes
        && matches!(
            ruby_role(k),
            Some(crate::style::computed::RubyRole::Base)
                | Some(crate::style::computed::RubyRole::Text)
        )
    {
        let mut block = k.clone();
        ruby_transform::clear(&mut block.style);
        block.style.display = Some(Display::Block);
        block.style.inline_display = None;
        block.style.ruby_role = None;
        block.style.text_indent = Some(Len::Px(0.0));
        block.style.text_indent_each_line = Some(false);
        block.style.text_indent_hanging = Some(false);
        return div()
            .children(blocks(&[Node::Element(block)], &style, opts))
            .into_any_element();
    }
    div()
        .children(blocks(nodes, &style, opts))
        .into_any_element()
}
