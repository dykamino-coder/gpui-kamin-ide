//! Порождённое содержимое: наследование `content`, подстановка `attr()` и текст
//! псевдоэлемента (CSS 2.1 §12.2, css-values-5 §attr-notation, css-content-3 §2).
//!
//! `inherit_generated`/`snapshot_generated` переносят `content` и директивы
//! счётчиков по DOM-цепочке; `resolve_content_attributes` подставляет
//! атрибуты хозяина (регистр имени — по языку документа: HTML без учёта,
//! XHTML точно; `document()` держит этот признак на время разбора);
//! `content_text` собирает строку из строк, кавычек, `counter()`/`counters()`.

use crate::document::state::ValueCx;

/// CSS2 sections 6.2.1 and 12.2: explicit inheritance follows the DOM,
/// including ordinary elements on which `content` does not generate a box.
pub(crate) fn inherit_generated(
    style: &mut crate::style::computed::Computed,
    parent: Option<&crate::style::select::Ancestor>,
) {
    super::inherit_counter_decls(style, parent.map(|p| &p.counter_style));
    if style.content_inherit {
        style.content = parent.and_then(|p| p.content_style.clone());
        style.content_none = parent.and_then(|p| p.content_none);
        style.content_inherit = false;
    }
}

pub(crate) fn snapshot_generated(
    node: &mut crate::style::select::Ancestor,
    style: &crate::style::computed::Computed,
    parent: Option<&crate::style::select::Ancestor>,
) {
    node.counter_context = crate::style::generated::counter_style::Context::new(
        style,
        parent.map(|p| p.counter_context).unwrap_or_default(),
    );
    node.counter_style = super::counter_snapshot(style);
    node.content_style = style.content.clone();
    node.content_none = style.content_none;
    node.target_group = (style.scroll_target_group == Some(true))
        .then(|| std::rc::Rc::new(std::cell::Cell::new(false)));
}

/// HTML-адаптер XHTML включается явным xmlns разобранного корня html.
/// Настоящий XML задаёт язык хоста независимо от наличия пространства имён.
pub(super) fn is_xhtml(
    document: &markup5ever_rcdom::Handle,
    budget: &crate::budget::RevisionBudget,
) -> Result<bool, crate::document::FailureReason> {
    use crate::{budget::BudgetKind, document::FailureReason};
    for node in document.children.borrow().iter() {
        budget
            .consume(BudgetKind::TraversalVisits, 1)
            .map_err(FailureReason::Budget)?;
        if let markup5ever_rcdom::NodeData::Element { name, attrs, .. } = &node.data
            && &*name.local == "html"
            && &*name.ns == "http://www.w3.org/1999/xhtml"
        {
            for attr in attrs.borrow().iter() {
                budget
                    .consume(BudgetKind::TraversalVisits, 32)
                    .map_err(FailureReason::Budget)?;
                if attr.name.ns.is_empty()
                    && &*attr.name.local == "xmlns"
                    && &*attr.value == "http://www.w3.org/1999/xhtml"
                {
                    return Ok(true);
                }
            }
        }
    }
    Ok(false)
}

pub(crate) fn html_attributes(namespace: &str, xhtml: bool) -> bool {
    namespace == "http://www.w3.org/1999/xhtml" && !xhtml
}

pub(crate) fn host_content(
    style: &crate::style::computed::Computed,
    host: &crate::style::select::Ancestor,
    vcx: ValueCx<'_>,
) -> Option<Vec<crate::style::computed::ContentItem>> {
    resolve_content_attributes(style.content.as_ref()?, &host.attrs, host.html_attrs, vcx)
}

/// CSS Values 5 §8.7.1 substitutes attributes before checking content's
/// grammar. An invalid fallback must suppress the whole pseudo-element,
/// while a present empty attribute is still a valid string, not fallback.
pub(crate) fn resolve_content_attributes(
    items: &[crate::style::computed::ContentItem],
    attrs: &[(String, String)],
    html_attrs: bool,
    vcx: ValueCx<'_>,
) -> Option<Vec<crate::style::computed::ContentItem>> {
    let out = substitute_attributes(items, attrs, html_attrs, vcx)?;
    (!out.is_empty()).then_some(out)
}

fn substitute_attributes(
    items: &[crate::style::computed::ContentItem],
    attrs: &[(String, String)],
    html_attrs: bool,
    vcx: ValueCx<'_>,
) -> Option<Vec<crate::style::computed::ContentItem>> {
    use crate::style::computed::ContentItem;
    let mut out = Vec::new();
    for item in items {
        if let ContentItem::Attr(name, fallback) = item {
            if let Some((_, value)) = attrs
                .iter()
                .find(|(key, _)| key == name || (html_attrs && key.eq_ignore_ascii_case(name)))
            {
                out.push(ContentItem::Str(value.clone()));
            } else {
                let fallback = fallback.as_deref()?;
                if fallback.is_empty() {
                    continue;
                }
                let parsed = crate::style::computed::parse_content(fallback, vcx)?;
                let resolved = substitute_attributes(&parsed, attrs, html_attrs, vcx)?;
                // §8.7.2: attr-tainted values cannot be used as URLs.
                if resolved
                    .iter()
                    .any(|item| matches!(item, ContentItem::Image(_)))
                {
                    return None;
                }
                out.extend(resolved);
            }
        } else {
            out.push(item.clone());
        }
    }
    Some(out)
}

/// Текст из составляющих `content` (css-content-3 §2): строки как есть,
/// счётчики — знаками своего стиля, `attr()` — значением атрибута хозяина.
///
/// Общий для `::before`/`::after` и для `::marker { content }`: по
/// css-lists-3 §content-property содержимое маркера строится «exactly as for
/// ::before».
pub(crate) fn content_text(
    items: &[crate::style::computed::ContentItem],
    counters: &mut crate::style::generated::counters::Counters,
    attrs: &[(String, String)],
    own_quotes: Option<&Option<Vec<(String, String)>>>,
    html_attrs: bool,
    context: crate::style::generated::counter_style::Context,
    vcx: ValueCx<'_>,
) -> String {
    let Some(items) = resolve_content_attributes(items, attrs, html_attrs, vcx) else {
        return String::new();
    };
    let mut text = String::new();
    for item in &items {
        match item {
            crate::style::computed::ContentItem::Quote { open, emit } => {
                text.push_str(&counters.quote(*open, *emit, own_quotes));
            }
            crate::style::computed::ContentItem::Str(sv) => text.push_str(sv),
            crate::style::computed::ContentItem::Image(_) => {}
            crate::style::computed::ContentItem::Counter(name, style_name) => {
                let value = counters.value_of(name);
                text.push_str(&crate::style::generated::counter_style::repr_in(
                    value, style_name, context, vcx,
                ));
            }
            crate::style::computed::ContentItem::Counters(name, sep, style_name) => {
                // Вся цепочка области — от внешнего счётчика к внутреннему,
                // склеенная разделителем (css-lists-3 §counters).
                let chain: Vec<String> = counters
                    .chain_of(name)
                    .into_iter()
                    .map(|v| {
                        crate::style::generated::counter_style::repr_in(v, style_name, context, vcx)
                    })
                    .collect();
                text.push_str(&chain.join(sep));
            }
            crate::style::computed::ContentItem::Attr(..) => {
                // resolve_content_attributes выше заменяет все Attr или возвращает None.
                unreachable!("attributes were substituted")
            }
        }
    }
    text
}
