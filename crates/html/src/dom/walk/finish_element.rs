//! Finish element for walk; split out to keep the owning module within 250 lines.

use crate::dom::*;
use crate::style::computed::{Computed, Display};

#[allow(clippy::too_many_arguments)]
#[allow(clippy::redundant_field_names)]
pub(super) fn finish_element(
    mut style: Computed,
    mut children: Vec<Node>,
    tag: String,
    attrs: Vec<(String, String)>,
    anim: Option<Vec<(f32, Computed)>>,
    list_item: Option<i32>,
    node_id: u64,
    hover: Option<Computed>,
    first_letter: Option<Computed>,
    first_line: Option<Computed>,
    scroll_pseudos: ScrollPseudos,
    out: &mut Vec<Node>,
) {
    // `dir="auto"` — сторону задаёт ПЕРВЫЙ СИЛЬНЫЙ знак содержимого
    // (HTML §3.2.6.4). Раньше здесь не ставилось ничего в расчёте на
    // разбор двунаправленности, но он берёт сторону абзаца, а не
    // куска: строка `1;234;56א;` внутри `<span dir=auto>` выходила
    // слева направо (`empty-span-001`).
    if attrs
        .iter()
        .any(|(k, v)| k == "dir" && v.eq_ignore_ascii_case("auto"))
    {
        // `dir="auto"` в HTML — это `unicode-bidi: plaintext`: сторона
        // решается для КАЖДОГО абзаца между жёсткими разрывами, а не
        // для элемента целиком (`text-align-end-016`).
        style.bidi_plaintext = Some(true);
        if style.rtl.is_none()
            && let Some(rtl) = first_strong(&children)
        {
            style.rtl = Some(rtl);
        }
    }
    // Псевдокоробки ВНЕ элемента (css-overflow-5; порядок Blink
    // `kBoxTreeOrder`): группа `before` — перед ним, кнопки
    // (block-start, inline-start, inline-end, block-end) и группа
    // `after` — за ним. У корня всё это — его первый/последние дети
    // (§scroll-marker-group: «first child of the originating
    // element»). Неявный якорь у них — сам элемент.
    let mut sp = scroll_pseudos;
    for el in sp
        .group_before
        .iter_mut()
        .chain(sp.buttons.iter_mut())
        .chain(sp.group_after.iter_mut())
    {
        el.style.implicit_anchor = Some(node_id);
    }
    if tag == "html" {
        if let Some(g) = sp.group_before.take() {
            children.insert(0, Node::Element(g));
        }
        children.extend(sp.buttons.drain(..).map(Node::Element));
        if let Some(g) = sp.group_after.take() {
            children.push(Node::Element(g));
        }
    } else if let Some(g) = sp.group_before.take() {
        out.push(Node::Element(g));
    }
    // css-ruby-1 §2.1.2 «Non-Inline Ruby»: `display: block ruby` даёт
    // ДВЕ коробки — главную блочную и строчный контейнер руби внутри
    // (Blink `LayoutRubyAsBlock::AddChild`: первый ребёнок — анонимный
    // `LayoutInline` с `display: ruby`, все дети идут в него). Свойства
    // элемента — на главной коробке; наследуемые доходят до контейнера
    // обычным `inline::inherit` (стиль контейнера пуст). Тег `ruby` у
    // синтетического узла — роль контейнера по тегу (`block-ruby-001`).
    let children = if style.display == Some(Display::Block)
        && style.ruby_role == Some(crate::style::computed::RubyRole::Container)
    {
        vec![Node::Element(Element {
            list_item: None,
            node_id: 0,
            anim: None,
            inline: true,
            tag: "ruby".to_string(),
            style: Computed::default(),
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: vec![],
        })]
    } else {
        children
    };
    let children = if ruby_box_role(&tag, &style).is_none() {
        wrap_misparented_ruby(children)
    } else {
        children
    };
    let inline = INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
    style.block_tag = !inline;
    // Замена элемента (css-content-3 §content-property: «a single
    // <image>» на самом элементе): коробка становится замещаемой
    // картинкой, содержимое не рисуется. Уровень коробки остаётся от
    // исходного тега и `display` — `<p>` замещается блоком. Корень не
    // трогается: замещаемого корня у нас нет. Ненайденная картинка
    // замены не делает (Servo `replaced.rs:348`: `None` при ошибке).
    let (tag, children, attrs) = match style.content.as_deref() {
        Some([crate::style::computed::ContentItem::Image(src)])
            if tag != "html" && content_image_src(src).is_some() =>
        {
            let mut attrs: Vec<(String, String)> = attrs
                .into_iter()
                .filter(|(k, _)| k != "src" && k != "srcset")
                .collect();
            attrs.push(("src".into(), content_image_src(src).unwrap_or_default()));
            ("img".to_string(), vec![], attrs)
        }
        _ => (tag, children, attrs),
    };
    out.push(Node::Element(Element {
        list_item,
        node_id: node_id,
        anim,
        inline,
        tag,
        style,
        hover,
        first_letter,
        first_line,
        children,
        attrs,
    }));
    out.extend(sp.buttons.into_iter().map(Node::Element));
    if let Some(g) = sp.group_after {
        out.push(Node::Element(g));
    }
}
