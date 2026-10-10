//! Псевдоэлементы ::before/::after/::marker и др.: коробка, content-картинка.

use crate::dom::*;
use crate::style::computed::{Computed, Display};
use crate::style::css::{Decls, Rule};
use crate::style::select::matching::matches_ignoring_pseudo;
use crate::style::select::{Ancestor, Sibs};

/// Подходит ли значение под синтаксис `@property` (css-properties-values-api-1
/// §5). Проверяются однозначные типы; значение с `var()` решается позже и
/// принимается; незнакомый синтаксис — тоже (лучше принять, чем потерять).
pub(super) fn syntax_accepts(syntax: &str, value: &str) -> bool {
    let v = value.trim();
    if v.contains("var(") || syntax.trim() == "*" {
        return true;
    }
    let one = |ty: &str| -> bool {
        match ty.trim() {
            "<color>" => {
                crate::style::values::value::Color::parse(v).is_some()
                    || v.eq_ignore_ascii_case("currentcolor")
                    || v.to_ascii_lowercase().starts_with("light-dark(")
            }
            "<length>" => {
                !v.ends_with('%')
                    && (v == "0"
                        || matches!(crate::style::values::value::Len::parse_mixed(v), Some(l) if !matches!(l, crate::style::values::value::Len::Pct(_) | crate::style::values::value::Len::Auto)))
            }
            "<length-percentage>" => crate::style::values::value::Len::parse_mixed(v)
                .is_some_and(|l| l != crate::style::values::value::Len::Auto),
            "<percentage>" => v.ends_with('%') && v[..v.len() - 1].trim().parse::<f32>().is_ok(),
            "<number>" => v.parse::<f32>().is_ok(),
            "<integer>" => v.parse::<i64>().is_ok(),
            _ => true,
        }
    };
    syntax.split('|').any(one)
}

/// Адрес картинки из `content: url()` в форме `src` для `<img>`: загрузчик
/// ждёт `file:///` с прямыми косыми (как пишет стенд для `<img src>`), а
/// разбор стиля отдаёт голый путь. `None` — файла нет: такая картинка коробки
/// не даёт (Servo `components/layout/replaced.rs:348`).
pub(crate) fn content_image_src(src: &str) -> Option<String> {
    if src.starts_with("data:") {
        return Some(src.to_string());
    }
    let path = src.trim_start_matches("file:///");
    if !std::path::Path::new(path).is_file() {
        return None;
    }
    Some(format!("file:///{path}").replace('\\', "/"))
}

/// Коробка псевдоэлемента `::before`/`::after`, если правила её создают.
///
/// В CSS это настоящий потомок с собственным стилем; так его и собираем —
/// обычным инлайновым элементом с текстовым содержимым. `attr(имя)`
/// подставляется значением атрибута хозяина.
// ★ Прошлый заход (06.09) на слой `::marker` был откачен «848 -> 847,
// +3/-4»: терялись `disclosure-styles`, `marker-counter`,
// `marker-content-020`, `marker-text-transform-default`. Три корня потерь
// названы и закрыты здесь же: таблица агента маркера (`text-transform:
// none`, `unicode-bidi: isolate` — css-lists-3 §marker-properties),
// исполнение `counter-*` слоя в собственном сегменте маркера и снятие
// маркера у пункта с чужим `display`. Разбор — `target/scout-markers-
// 2026-09b.md` §1.
pub(super) fn pseudo_box(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    which: &str,
    attrs: &[(String, String)],
) -> Option<Element> {
    pseudo_box_named(
        rules,
        vars,
        counters,
        me,
        path,
        sibs,
        &[which],
        which,
        which == "before",
        attrs,
    )
}

/// То же, но правила отбираются по ЛЮБОМУ из имён `names`: у кнопки
/// прокрутки `::scroll-button(right)`, `::scroll-button(inline-end)` и
/// `::scroll-button(*)` — одна коробка (css-overflow-5 §scroll-buttons,
/// `scroll-buttons-003`). `tag` — имя коробки (`::{tag}`), `before` — вести
/// счётчики как у `::before`.
#[allow(clippy::too_many_arguments)]
pub(super) fn pseudo_box_named(
    rules: &[Rule],
    vars: &Decls,
    counters: &mut crate::style::generated::counters::Counters,
    me: &Ancestor,
    path: &[Ancestor],
    sibs: Sibs,
    names: &[&str],
    tag: &str,
    before: bool,
    attrs: &[(String, String)],
) -> Option<Element> {
    let mut matched: Vec<&Rule> = rules
        .iter()
        .filter(|r| r.sel.pseudo.as_deref().is_some_and(|p| names.contains(&p)))
        .filter(|r| matches_ignoring_pseudo(&r.sel, me, path, sibs))
        .collect();
    if matched.is_empty() {
        return None;
    }
    let mut style = Computed::resolve_with_vars(&mut matched, &Decls::new(), vars);
    inherit_counter_decls(&mut style, Some(&me.counter_style));
    // Псевдоэлемент — ребёнок хозяина: блочный `::before` внутри руби
    // инлайнизируется так же, как элемент (css-ruby-1 §2.2 п.1,
    // `ruby-inlinize-blocks-005`). Ближайший предок — сам хозяин.
    inlinify_in_ruby(&mut style, "", std::iter::once(me).chain(path.iter().rev()));
    // Нет содержимого или коробки — нет и псевдоэлемента: его директивы
    // счётчиков тогда не действуют вовсе (у него нет объекта раскладки).
    let list = resolve_content_attributes(style.content.as_ref()?, attrs, me.html_attrs)?;
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
    // ошибке загрузки, и элемент пропускается). Прошлые заходы давали ей
    // коробку и теряли `before-after-images-001` и `-table-whitespace-001`.
    let mut children: Vec<Node> = vec![];
    let mut run: Vec<crate::style::computed::ContentItem> = vec![];
    let flush = |run: &mut Vec<crate::style::computed::ContentItem>,
                 children: &mut Vec<Node>,
                 counters: &mut crate::style::generated::counters::Counters| {
        if !run.is_empty() {
            let t = content_text(run, counters, attrs, style.quotes.as_ref(), me.html_attrs);
            children.push(Node::Text(t));
            run.clear();
        }
    };
    for item in &list {
        if let crate::style::computed::ContentItem::Image(src) = item {
            flush(&mut run, &mut children, counters);
            if let Some(src) = content_image_src(src) {
                children.push(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "img".into(),
                    style: Computed::default(),
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: vec![],
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
        list_item,
        // Псевдоэлемент своей анимации не несёт: правило `::before` задаёт
        // содержимое, а не движение.
        node_id: 0,
        anim: None,
        // A pseudo-element is inline unless `display` says otherwise; an
        // absolutely positioned one keeps that original inline display for
        // its static position (CSS 2.1 §10.3.7 hypothetical box), so `PASS`
        // of `span::after { position: absolute }` stays on the span's line.
        inline: true,
        tag: format!("::{tag}"),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: vec![],
    })
}
