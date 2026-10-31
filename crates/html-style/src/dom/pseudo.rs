//! Псевдоэлементы ::before/::after/::marker и др.: коробка, content-картинка.

use crate::dom::*;
use crate::style::computed::{Computed, Display};
use crate::style::css::{DeclBlock, Rule, VarMap};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};
use crate::values::ValueCx;

mod input;
pub use input::content_image_src;
pub(super) use input::syntax_accepts;

/// Коробка псевдоэлемента `::before`/`::after`, если правила её создают.
///
/// В CSS это настоящий потомок с собственным стилем; так его и собираем —
/// обычным инлайновым элементом с текстовым содержимым. `attr(имя)`
/// подставляется значением атрибута хозяина.
// Слой `::marker` держится на трёх условиях (без них уходят
// `disclosure-styles`, `marker-counter`, `marker-content-020`,
// `marker-text-transform-default`): таблица агента маркера (`text-transform:
// none`, `unicode-bidi: isolate` — css-lists-3 §marker-properties),
// исполнение `counter-*` слоя в собственном сегменте маркера и снятие
// маркера у пункта с чужим `display`.
// Pseudo boxes need the originating cascade, counters and generated-content state.
#[allow(clippy::too_many_arguments)]
pub(super) fn pseudo_box(
    scope: &crate::dom::shadow::Scope,
    indices: &[usize],
    vars: &VarMap,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    which: &str,
    attrs: &[(String, String)],
    vcx: ValueCx<'_>,
) -> Option<Element> {
    pseudo_box_named(
        scope,
        indices,
        vars,
        counters,
        me,
        path,
        sibs,
        &[which],
        which,
        which == "before",
        attrs,
        vcx,
    )
}

/// То же, но правила отбираются по ЛЮБОМУ из имён `names`: у кнопки
/// прокрутки `::scroll-button(right)`, `::scroll-button(inline-end)` и
/// `::scroll-button(*)` — одна коробка (css-overflow-5 §scroll-buttons,
/// `scroll-buttons-003`). `tag` — имя коробки (`::{tag}`), `before` — вести
/// счётчики как у `::before`.
#[allow(clippy::too_many_arguments)]
pub(super) fn pseudo_box_named(
    scope: &crate::dom::shadow::Scope,
    indices: &[usize],
    vars: &VarMap,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    names: &[&str],
    tag: &str,
    before: bool,
    attrs: &[(String, String)],
    vcx: ValueCx<'_>,
) -> Option<Element> {
    let matched: Vec<&Rule> = indices
        .iter()
        .map(|&i| &scope.rules[i])
        .filter(|r| r.sel.pseudo.as_deref().is_some_and(|p| names.contains(&p)))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let mut style = Computed::resolve_with_vars(&matched, &DeclBlock::new(), vars, vcx);
    content::inherit_generated(&mut style, Some(me));
    // Псевдоэлемент — ребёнок хозяина: блочный `::before` внутри руби
    // инлайнизируется так же, как элемент (css-ruby-1 §2.2 п.1,
    // `ruby-inlinize-blocks-005`). Ближайший предок — сам хозяин.
    inlinify_in_ruby(&mut style, "", std::iter::once(me).chain(path.iter().rev()));
    // Нет содержимого или коробки — нет и псевдоэлемента: его директивы
    // счётчиков тогда не действуют вовсе (у него нет объекта раскладки).
    let list = resolve_content_attributes(style.content.as_ref()?, attrs, me.html_attrs, vcx)?;
    if style.display == Some(Display::None) {
        return None;
    }
    // Pseudo counters occupy their own level among the host children.
    counters.enter_pseudo(before);
    language::pseudo(counters, &style, me, path);
    // У псевдоэлемента-создателя предварительного обхода нет: своей области
    // в дереве коробок он не открывает, и таких пар в наборе не встречается.
    apply_counter_decls(&style, counters, "", &[], &mut false, &|_, _| 0);
    // Составляющие идут по порядку: подряд идущие текстовые склеиваются в
    // один текстовый узел, `url()` становится строчным `<img>` между ними.
    // Ненайденная картинка коробки НЕ даёт вовсе — как в Servo
    // (`components/layout/dom_traversal.rs:398`: `from_image` → `None` при
    // ошибке загрузки, и элемент пропускается). Коробка у ненайденной
    // картинки ломает `before-after-images-001` и `-table-whitespace-001`.
    let mut children: Vec<Node> = vec![];
    let mut run: Vec<crate::style::computed::ContentItem> = vec![];
    let flush = |run: &mut Vec<crate::style::computed::ContentItem>,
                 children: &mut Vec<Node>,
                 counters: &mut crate::style::generated::counters::Counters| {
        if !run.is_empty() {
            let t = content_text(
                run,
                counters,
                attrs,
                style.quotes.as_ref(),
                me.html_attrs,
                crate::style::generated::counter_style::Context::new(&style, me.counter_context),
                vcx,
            );
            children.push(Node::Text(t));
            run.clear();
        }
    };
    for item in &list {
        if let crate::style::computed::ContentItem::Image(src) = item {
            flush(&mut run, &mut children, counters);
            if let Some(src) = content_image_src(src) {
                children.push(Node::Element(Element {
                    marks: Default::default(),
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "img".into(),
                    style: Computed {
                        content: Some(vec![item.clone()]),
                        ..Computed::default()
                    }
                    .into(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: (vec![]).into(),
                    attrs: vec![("src".into(), src)],
                    inline: true,
                }));
            }
        } else {
            run.push(item.clone());
        }
    }
    flush(&mut run, &mut children, counters);
    if children.is_empty() {
        children.push(Node::Text(String::new()));
    }
    let list_item =
        (style.display == Some(Display::ListItem)).then(|| counters.value_of("list-item"));
    counters.leave();
    Some(Element {
        marks: Default::default(),
        list_item,
        // Псевдоэлемент своей анимации не несёт: правило `::before` задаёт
        // содержимое, а не движение.
        node_id: 0,
        anim: None,
        // A pseudo-element is inline unless `display` says otherwise; an
        // absolutely positioned one keeps that original inline display for
        // its static position (CSS 2.1 §10.3.7 hypothetical box), so `PASS`
        // of `span::after { position: absolute }` stays on the span's line.
        // An anchor-positioned one is placed from its anchor instead: the
        // anchor pass shifts the laid-out box, and an inline placeholder put
        // it one device row off (`position-anchor-*-pseudo-element-*`).
        inline: !anchor_positioned(&style),
        tag: format!("::{tag}"),
        style: style.into(),
        hover: None,
        first_letter: None,
        first_line: None,
        children: children.into(),
        attrs: vec![],
    })
}

/// Absolutely positioned and placed against an anchor: `position-area`, an `anchor()` inset,
/// `anchor-center` or a named `position-anchor` (css-anchor-position-1).
fn anchor_positioned(c: &crate::style::computed::Computed) -> bool {
    use crate::style::computed::{Align, Position, PositionAnchor};
    use crate::style::values::value::Len;
    let abspos = matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed));
    let i = &c.inset;
    abspos
        && (c.position_area.is_some()
            || [i.top, i.right, i.bottom, i.left]
                .iter()
                .any(|l| matches!(l, Some(Len::Anchor(_))))
            || c.align_self == Some(Align::AnchorCenter)
            || c.justify_self == Some(Align::AnchorCenter)
            || matches!(c.position_anchor, Some(PositionAnchor::Named(_))))
}
