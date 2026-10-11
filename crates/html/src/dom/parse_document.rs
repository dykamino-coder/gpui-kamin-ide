//! Parse document for dom; split out to keep the owning module within 250 lines.

use super::{Node, user_agent_css};
use super::{content, float_tail, grid_static_position};
use crate::dom::display_inheritance::resolve_display_inherit;
use crate::dom::fixup_grid::*;
use crate::dom::fixup_tree::*;
use crate::dom::shadow::*;
use crate::dom::walk::*;
use crate::dom::xhtml::*;
use crate::style::css::{Decls, Media, Rule, parse_keyframes, parse_stylesheet_media};
use crate::style::select::has::{HAS_MARKS, HasArg, collect_has_args, mark_has, parse_has_arg};
use crate::style::select::{QUIRKS, quirks};
use html5ever::tendril::TendrilSink;
use markup5ever_rcdom::{Handle, NodeData, RcDom};
use std::collections::HashMap;
use std::rc::Rc;

/// Разобрать фрагмент и вернуть корневые узлы.
///
/// `extra_css` — таблица уровня приложения (тема чата), применяется до
/// `<style>` документа и до `style=""`.
pub fn parse(html: &str, extra_css: &str) -> Vec<Node> {
    parse_media(html, extra_css, Media::default())
}

/// То же, но с известными условиями окружения для `@media`.
pub fn parse_media(html: &str, extra_css: &str, media: Media) -> Vec<Node> {
    let _content_document = content::document(html);
    // Правила `@page` — от последнего РАЗОБРАННОГО документа: почистить,
    // чтобы прошлый лист не красил страницу нового. `@position-try` — тот же
    // пул и та же чистка.
    let _ = crate::style::css::take_page_decls();
    let _ = crate::style::css::take_try_rules();
    let _ = crate::style::css::take_property_rules();
    crate::style::generated::counter_style_rules::reset();
    crate::style::css::reset_layers();
    crate::style::values::value::set_dark_scheme(false);
    // Корневые метрики (`rem`, `rlh`) — тоже от прошлого документа: у рамки
    // и у страницы свой корень, и чужие четыре точки на кегль испортили бы
    // весь разбор. Пишет их `walk` ниже, на элементе `html`.
    crate::style::values::value::reset_root_metrics();
    let html = expand_xhtml_self_closing(html);
    let dom = html5ever::parse_document(RcDom::default(), Default::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| {
            html5ever::parse_document(RcDom::default(), Default::default()).one("")
        });
    // Режим quirks (HTML §13.2.6.4.1, «initial» insertion mode: документ без
    // DOCTYPE). Документ XHTML в quirks не бывает никогда (HTML §2.1, «XML
    // documents … always in no-quirks mode»), а мы разбираем и его HTML-
    // разборщиком, поэтому пространство имён корня его исключает.
    QUIRKS.with(|q| {
        q.set(
            dom.quirks_mode.get() == html5ever::tree_builder::QuirksMode::Quirks
                && !html.contains("http://www.w3.org/1999/xhtml"),
        )
    });

    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    let mut rules = parse_stylesheet_media(user_agent_css(), media);
    let base = rules.len();
    for (i, r) in parse_stylesheet_media(extra_css, media)
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
    let agent_rules = rules.clone();
    // Каждый `<style>` — ОТДЕЛЬНАЯ таблица: конец каждой закрывает свои
    // незакрытые конструкции (CSS 2.1 §4.2, `uri-017`). В склейке незакрытая
    // запись первой таблицы съедала правила второй.
    let mut sheets: Vec<String> = vec![];
    collect_style_tags(&dom.document, &mut sheets);
    for css in &sheets {
        let base = rules.len();
        for (i, r) in parse_stylesheet_media(css, media).into_iter().enumerate() {
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
    let mut frames = parse_keyframes(user_agent_css());
    frames.extend(parse_keyframes(extra_css));
    let agent = Scope {
        rules: agent_rules,
        frames: frames.clone(),
    };
    for css in &sheets {
        frames.extend(crate::style::css::parse_keyframes_in(css, Some(media)));
    }
    // `:has()`: аргументы собираются со всех селекторов, правила с
    // вложенным `:has` выкидываются (спека: cannot be nested), отметки
    // считаются отдельным проходом до обхода.
    let mut has_args_raw: Vec<String> = vec![];
    rules.retain(|r| collect_has_args(&r.sel, &mut has_args_raw));
    let has_args: Vec<HasArg> = has_args_raw
        .iter()
        .filter_map(|a| parse_has_arg(a))
        .collect();
    HAS_MARKS.with(|m| m.borrow_mut().clear());
    if !has_args.is_empty() {
        mark_has(&dom.document, &has_args, &mut vec![]);
    }
    // Тени (`<template shadowrootmode>`): области стилей и распределение
    // слотов считаются ДО обхода, как отметки `:has`; обход читает их по
    // адресу узла. Документ без теней предпроход не платит.
    SHADOWS.with(|m| m.borrow_mut().clear());
    SLOTS.with(|m| m.borrow_mut().clear());
    let doc = Rc::new(Scope { rules, frames });
    if html.contains("shadowrootmode") {
        let mut drafts: HashMap<usize, SlotInfo> = HashMap::new();
        let top: Vec<Handle> = dom.document.children.borrow().clone();
        scan_shadows(&top, &mut vec![], &doc, &agent, media, &mut drafts);
        finish_slots(drafts);
    }
    let mut counter = 0u64;
    // Счётчики документа: имя → текущее значение. Обход идёт в порядке
    // разметки, поэтому значение на узле — это то же, что видит браузер.
    let mut counters = crate::style::generated::counters::Counters::default();
    walk_children(
        &dom.document,
        &doc.rules,
        &vars,
        &doc.frames,
        &mut counter,
        &mut counters,
        &[],
        false,
        &mut out,
    );
    // ПЕРВЫМ проходом: табличная починка и подъёмы ниже читают `display`.
    resolve_display_inherit(&mut out, (None, None, None, None, None));
    resolve_rule_color_inherit(&mut out, &RuleColors::default());
    // Anonymous inline-table around orphan table boxes inside inline boxes
    // (CSS 2.1 §17.2.1 step 3); block parents are fixed up by `blocks()`.
    crate::layout::table::anon::inline_anon_tables(&mut out);
    // Лунки, которые умеет taffy, — на путь сетки ДО подъёмов и среза
    // подсетки: дальше они идут тем же кодом, что и сетка.
    lanes_as_grid(&mut out);
    hoist_grid_abspos(&mut out);
    grid_static_position::adjust(&mut out);
    flex_items_lose_float(&mut out);
    align_self_from_dom_parent(&mut out, None);
    grid_table_items_keep_stretch(&mut out);
    subgrid_takes_parent_tracks(&mut out);
    filter_ref_only_empty(&mut out);
    fold_run_ins(&mut out, None);
    float_tail::mark(&mut out);
    if quirks() {
        quirks_percent_heights(&mut out, None);
    }
    out
}

/// Содержимое `<style>`-тегов документа, каждый — отдельной таблицей;
/// html5ever кладёт его текстом внутрь.
/// ПРОБОВАЛИ И ОТКАТИЛИ: раскрывать ссылки на знаки (`&gt;`, `&amp;`) в
/// содержимом `<style>`, когда документ XHTML. По разбору XML это верно —
/// построитель HTML держит `<style>` сырым текстом, и селектор `body &gt; div`
/// не совпадает ни с чем; в наборе таких файлов 103. Замерено по всему CSS2:
/// 5233 -> 5230, приобретено 0, потеряно 3. Раскрытые правила доезжают до
/// раскладки и вскрывают ошибки НИЖЕ (`text-indent-intrinsic-001` 0.00 -> 1.10,
/// `-003` 0.00 -> 0.68), а `content-177` 0.42 -> «красное видно»: раскрытая
/// `&quot;` закрывает строку CSS раньше времени. Возвращаться сюда после
/// разбора этих трёх корней.
pub(super) fn collect_style_tags(handle: &Handle, out: &mut Vec<String>) {
    if let NodeData::Element { name, .. } = &handle.data
        && &*name.local == "style"
    {
        let mut sheet = String::new();
        for child in handle.children.borrow().iter() {
            if let NodeData::Text { contents } = &child.data {
                // Обёртка `<![CDATA[ … ]]>` встречается в эталонах XHTML: там
                // она прячет стиль от разбора XML. Разбору CSS она мусор, и
                // правила до первой закрывающей скобки пропадали вместе с ней.
                let text = contents.borrow();
                let trimmed = text.trim();
                let body = trimmed
                    .strip_prefix("<![CDATA[")
                    .and_then(|rest| rest.strip_suffix("]]>"))
                    .unwrap_or(&text);
                // Preserve child text content: an invented LF makes EOF strings invalid.
                sheet.push_str(body);
            }
        }
        out.push(sheet);
    }
    for child in handle.children.borrow().iter() {
        collect_style_tags(child, out);
    }
}
