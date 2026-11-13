//! Вход конвейера DOM: `parse`/`parse_media` — от разметки к дереву `Node` с
//! вычисленными стилями. Правила, метрики и арены значений пишутся в состояние
//! документа (`values`): своё у каждого разбора, не глобальное.
//! Порядок: разбор html5ever, схема цвета и quirks, сборка правил (лист агента →
//! тема → quirks → каждый `<style>` отдельной таблицей), кадры анимаций, предпроходы
//! `:has()` и теней, обход `walk_children`, затем проходы по собранному дереву
//! в фиксированном порядке: `display: inherit`, quirks-цвета, анонимные таблицы,
//! лунки, подсетки, подъёмы, run-in, флоаты.

use super::Node;
use super::input::Admitted;
use crate::dom::outcome::DocumentFailure;
use crate::dom::shadow::*;
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::dom::walk::*;
use crate::style::css::{Media, VarMap};
use crate::style::select::has::mark_has;
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

/// Разобрать разметку в нижние данные документа `doc` и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
/// Глубина ограничивает только представление; исходный DOM сохраняется целиком.
pub fn parse_into(
    html: &str,
    extra_css: &str,
    media: Media,
    xml: bool,
    doc: &Rc<crate::values::DocumentValues>,
) -> Result<Vec<Node>, DocumentFailure> {
    let budget = crate::budget::DocumentBudget::new(Default::default(), Default::default())
        .map_err(|error| DocumentFailure::new(crate::dom::outcome::FailureReason::Budget(error)))?;
    let revision = budget.revision();
    revision
        .source(html, extra_css)
        .map_err(|error| DocumentFailure::new(crate::dom::outcome::FailureReason::Budget(error)))?;
    let input = Admitted::with_budget(html, xml, revision.clone())?;
    super::early_admission::admit_sources(&input, extra_css)?;
    doc.admit_revision(revision, None);
    let nodes = convert(&input, extra_css, media, doc)?;
    doc.traversal.retain(input);
    Ok(nodes)
}

/// Конвертация единственного исходного дерева в свежее состояние документа.
pub fn convert(
    input: &Admitted,
    extra_css: &str,
    media: Media,
    doc: &Rc<crate::values::DocumentValues>,
) -> Result<Vec<Node>, DocumentFailure> {
    convert_restyling(input, extra_css, media, doc, None)
}

/// [`convert`] of a rebuild for a batch of attribute mutations: the walk
/// collects in `invalidation` the elements the batch restyles. Like Blink's
/// pending invalidations, which live for one style recalc and not with the
/// document, it is the caller's for this one build: a document pays nothing
/// for it.
pub fn convert_restyling(
    input: &Admitted,
    extra_css: &str,
    mut media: Media,
    doc: &Rc<crate::values::DocumentValues>,
    mut invalidation: Option<&mut super::invalidation::Invalidation>,
) -> Result<Vec<Node>, DocumentFailure> {
    let vcx = doc.values();
    vcx.doc.reset_font_alternates();
    let dom = &input.dom;
    let xhtml = input.xhtml;
    let admission = crate::budget::admission::Admission::new(input.context().budget().clone());
    if let Some(dark) = super::page_scheme::preference(&dom.document, media.dark, &admission)? {
        media.dark = dark;
    }
    let vcx = vcx.with_scheme(crate::style::values::value::Scheme::new(media.dark));
    // Режим quirks (HTML §13.2.6.4.1, «initial» insertion mode: документ без
    // DOCTYPE). XML и HTML-адаптер XHTML всегда работают без quirks.
    doc.quirks
        .set(dom.quirks_mode.get() == html5ever::tree_builder::QuirksMode::Quirks && !xhtml);

    let values = doc;
    let super::document_rules::DocumentRules {
        agent,
        doc,
        mut has_args,
        mut layers,
    } = super::document_rules::document_rules(input, extra_css, media, &admission, vcx)?;
    let vars = VarMap::new();
    let mut out = vec![];
    let mut tree = TreeIndex::new(values.clone(), xhtml);
    let shadows = has_declarative_shadow(&dom.document, &admission)?;
    if let Some(invalidation) = invalidation.as_deref_mut() {
        invalidation.prepare(&doc.rules, shadows);
    }
    if shadows {
        let mut drafts: HashMap<NodeKey, SlotInfo> = HashMap::new();
        let top: Vec<Handle> = dom.document.children.borrow().clone();
        let prepared = prepare(
            &dom.document,
            &agent,
            media,
            &mut layers,
            &mut has_args,
            &mut tree,
        )?;
        scan_shadows(
            &top,
            &mut vec![],
            &doc,
            &prepared,
            &has_args,
            &mut drafts,
            &mut tree,
        )?;
        finish_slots(drafts, &mut tree)?;
    } else if !has_args.is_empty() {
        mark_has(&dom.document, &has_args, &mut vec![], &mut tree)?;
    }
    if !shadows {
        doc.index
            .prepare_bulk(&dom.document, &doc.rules, &tree, invalidation.is_some())?;
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::style::generated::counters::Counters::default();
    walk_children(
        &dom.document,
        &tree,
        &doc,
        &vars,
        &mut counter,
        &mut counters,
        &mut Vec::new(),
        false,
        &mut out,
        vcx,
        invalidation,
    )?;
    super::finish_document::finish(&tree, &admission, out, vcx)
}
