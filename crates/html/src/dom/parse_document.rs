//! Вход конвейера DOM: `parse`/`parse_media` — от разметки к дереву `Node` с
//! вычисленными стилями. Правила, метрики и арены значений пишутся в состояние
//! документа (`document::state`): своё у каждого разбора, не глобальное.
//! Порядок: разбор html5ever, схема цвета и quirks, сборка правил (лист агента →
//! тема → quirks → каждый `<style>` отдельной таблицей), кадры анимаций, предпроходы
//! `:has()` и теней, обход `walk_children`, затем проходы по собранному дереву
//! в фиксированном порядке: `display: inherit`, quirks-цвета, анонимные таблицы,
//! лунки, подсетки, подъёмы, run-in, флоаты.

use super::Parsed;
use super::style_sources::collect_style_tags;
use super::{Node, user_agent_css};
use super::{content, float_tail, grid_static_position, subgrid_used};
use crate::dom::display_inheritance::resolve_display_inherit;
use crate::dom::fixup_grid::*;
use crate::dom::fixup_tree::*;
use crate::dom::shadow::*;
use crate::dom::tree_index::{NodeKey, TreeIndex};
use crate::dom::walk::*;
use crate::dom::xhtml::*;
use crate::style::css::{Decls, LayerRegistry, Media, Rule, parse_document_sheet, parse_keyframes};
use crate::style::select::has::{HasArg, collect_has_args, mark_has, parse_has_arg};
use markup5ever_rcdom::Handle;
use std::collections::HashMap;
use std::rc::Rc;

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn parse(html: &str, extra_css: &str) -> Parsed {
    parse_media(html, extra_css, Media::default())
}

/// То же, но с известными условиями окружения для `@media`.
///
/// Разбор без `Document` (тесты, инструменты): правила, метрики и арены
/// значений ложатся в свежий документ, который возвращается вместе с узлами.
pub fn parse_media(html: &str, extra_css: &str, media: Media) -> Parsed {
    let doc = crate::document::state::DocState::new(media);
    let nodes = parse_current(html, extra_css, media, &doc);
    Parsed { nodes, doc }
}

/// Разобрать в СВЕЖЕЕ состояние текущего документа: область открыта
/// вызывающим (`Document`, вложенный документ рамки).
pub(crate) fn parse_current(
    html: &str,
    extra_css: &str,
    media: Media,
    doc: &Rc<crate::document::state::DocState>,
) -> Vec<Node> {
    parse_input(html, extra_css, media, doc, false)
}

/// Parse one input with its parser mode and document-local host-language rules.
pub(super) fn parse_input(
    html: &str,
    extra_css: &str,
    mut media: Media,
    doc: &Rc<crate::document::state::DocState>,
    xml: bool,
) -> Vec<Node> {
    let vcx = doc.values();
    let xhtml = content::is_xhtml(html);
    // `@font-feature-values` — от прошлого разбора (слои `@layer` — свои у
    // разбора, `LayerRegistry` ниже).
    crate::text::fonts::alternates::reset(vcx);
    // XML document (`super::xml`): XML parser, no HTML tree-builder fixups.
    let html = if xml {
        std::borrow::Cow::Borrowed(html)
    } else {
        expand_xhtml_self_closing(html)
    };
    let dom = if xml {
        super::xml::xml_dom(&html)
    } else {
        super::input::html(&html)
    };
    if let Some(dark) = super::page_scheme::preference(&dom.document, media.dark) {
        media.dark = dark;
    }
    let vcx = vcx.with_scheme(crate::style::values::value::Scheme::new(media.dark));
    // Режим quirks (HTML §13.2.6.4.1, «initial» insertion mode: документ без
    // DOCTYPE). Документ XHTML в quirks не бывает никогда (HTML §2.1, «XML
    // documents … always in no-quirks mode»), а мы разбираем и его HTML-
    // разборщиком, поэтому пространство имён корня его исключает.
    doc.quirks.set(
        dom.quirks_mode.get() == html5ever::tree_builder::QuirksMode::Quirks
            && !html.contains("http://www.w3.org/1999/xhtml"),
    );

    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    // Каскадные слои у всех таблиц документа общие (css-cascade-5 §6.4.3).
    let mut layers = LayerRegistry::default();
    let mut rules = parse_document_sheet(user_agent_css(), media, &mut layers, vcx);
    let base = rules.len();
    for (i, r) in parse_document_sheet(extra_css, media, &mut layers, vcx)
        .into_iter()
        .enumerate()
    {
        rules.push(Rule {
            order: base + i,
            ..r
        });
    }
    // Лист агента и тема — общие для ВСЕХ областей дерева: каждая тень
    // получает их копию, а правила документа в тень не попадают
    // (css-shadow-1 §3.2: селекторы сопоставляются в своей области).
    if doc.quirks() {
        let base = rules.len();
        for (i, r) in parse_document_sheet(super::ua_style::QUIRKS_CSS, media, &mut layers, vcx)
            .into_iter()
            .enumerate()
        {
            rules.push(Rule {
                order: base + i,
                ..r
            });
        }
    }
    let agent_rules = rules.clone();
    // Каждый `<style>` — ОТДЕЛЬНАЯ таблица: конец каждой закрывает свои
    // незакрытые конструкции (CSS 2.1 §4.2, `uri-017`). В склейке незакрытая
    // запись первой таблицы съедала правила второй.
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(&dom.document, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_document_sheet(css, media, &mut layers, vcx)
            .into_iter()
            .enumerate()
        {
            rules.push(Rule {
                order: base + i,
                // Таблица ДОКУМЕНТА: её происхождение старше нашего умолчания и
                // темы приложения, поэтому она перебивает их независимо от
                // специфичности (CSS Cascade §6.4.4).
                origin: 1,
                ..r
            });
        }
    }

    // Переменные темы: `:root { --x: … }` и `--x` в инлайн-стиле корня.
    // Собираются до обхода, потому что нужны каждому узлу.
    // Пользовательские свойства НАСЛЕДУЮТСЯ и каскадируют, поэтому корень
    // стартует пустым: каждый узел добавляет свои и передаёт вниз.
    let vars = Decls::new();

    let mut out = vec![];
    // Наборы кадров собираются из тех же источников, что и правила.
    let mut frames = parse_keyframes(user_agent_css(), vcx);
    frames.extend(parse_keyframes(extra_css, vcx));
    let agent = Scope {
        rules: agent_rules,
        frames: frames.clone(),
    };
    for css in &sheets {
        frames.extend(crate::style::css::parse_keyframes_in(css, Some(media), vcx));
    }
    // `:has()`: аргументы собираются со всех селекторов, правила с
    // вложенным `:has` выкидываются (спека: cannot be nested), отметки
    // считаются отдельным проходом до обхода.
    let mut has_args_raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut has_args_raw));
    let mut has_args: Vec<HasArg> = has_args_raw
        .iter()
        .filter_map(|a| parse_has_arg(a))
        .collect();
    // Тени (`<template shadowrootmode>`): области стилей и распределение
    // слотов считаются ДО обхода, как отметки `:has`; обход получает их
    // индексом по узлу. Документ без теней предпроход не платит.
    let mut tree = TreeIndex::new(doc.clone(), xhtml);
    let doc = Rc::new(Scope { rules, frames });
    if html.contains("shadowrootmode") {
        let mut drafts: HashMap<NodeKey, SlotInfo> = HashMap::new();
        let top: Vec<Handle> = dom.document.children.borrow().clone();
        let prepared = prepare(
            &dom.document,
            &agent,
            media,
            &mut layers,
            &mut has_args,
            &mut tree,
        );
        scan_shadows(
            &top,
            &mut vec![],
            &doc,
            &prepared,
            &has_args,
            &mut drafts,
            &mut tree,
        );
        finish_slots(drafts, &mut tree);
    } else if !has_args.is_empty() {
        mark_has(&dom.document, &has_args, &mut vec![], &mut tree);
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::style::generated::counters::Counters::default();
    walk_children(
        &dom.document,
        &tree,
        &doc.rules,
        &vars,
        &doc.frames,
        &mut counter,
        &mut counters,
        &mut Vec::new(),
        false,
        &mut out,
        vcx,
    );
    // ПЕРВЫМ проходом: табличная починка и подъёмы ниже читают `display`.
    resolve_display_inherit(&mut out, (None, None, None, None, None));
    if tree.doc.quirks() {
        super::quirks_color::resolve(&mut out, vcx);
    }
    resolve_rule_color_inherit(&mut out, &RuleColors::default());
    // Anonymous inline-table around orphan table boxes inside inline boxes
    // (CSS 2.1 §17.2.1 step 3); block parents are fixed up by `blocks()`.
    crate::dom::table_boxes::inline_anon_tables(&mut out);
    // Лунки, которые умеет taffy, — на путь сетки ДО подъёмов и среза
    // подсетки: дальше они идут тем же кодом, что и сетка.
    lanes_as_grid(&mut out);
    subgrid_used::adjust(&mut out);
    hoist_grid_abspos(&mut out);
    grid_static_position::adjust(&mut out);
    flex_items_lose_float(&mut out);
    align_self_from_dom_parent(&mut out, None);
    grid_table_items_keep_stretch(&mut out);
    subgrid_takes_parent_tracks(&mut out);
    filter_ref_only_empty(&mut out);
    fold_run_ins(&mut out, None);
    orthogonal_inlines_become_flow_roots(&mut out, (false, false, false), None);
    float_tail::mark(&mut out);
    if tree.doc.quirks() {
        quirks_percent_heights(&mut out, None);
    }
    super::state_identity::finalize(&out, vcx);
    out
}
