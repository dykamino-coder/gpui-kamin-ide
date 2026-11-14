//! Правила документа одной сборки: лист агента, тема, quirks и каждый
//! `<style>` отдельной таблицей, кадры анимаций и аргументы `:has()`.
//! Общий вход полной сборки (`convert_restyling`) и пробы перерасчёта
//! стиля (`restyle_probe`): одно место, где собираются правила.

use super::input::Admitted;
use super::style_sources::document_style_tags;
use super::user_agent_css;
use crate::budget::admission::Admission;
use crate::dom::outcome::DocumentFailure;
use crate::dom::shadow::Scope;
use crate::style::css::{LayerRegistry, Media, Rule, parse_document_sheet, parse_keyframes};
use crate::style::select::has::{HasArg, parse_has_arg, retain_valid_rules};
use crate::values::ValueCx;
use std::rc::Rc;

/// Правила сборки: область агента (её получает каждая тень), область
/// документа, аргументы `:has()` и реестр слоёв.
pub(super) struct DocumentRules {
    pub agent: Scope,
    pub doc: Rc<Scope>,
    pub has_args: Vec<HasArg>,
    pub layers: LayerRegistry,
}

pub(super) fn document_rules(
    input: &Admitted,
    extra_css: &str,
    media: Media,
    admission: &Admission,
    vcx: ValueCx<'_>,
) -> Result<DocumentRules, DocumentFailure> {
    // Правила: сначала умолчания тегов, затем тема, затем <style> документа.
    // Каскадные слои у всех таблиц документа общие (css-cascade-5 §6.4.3).
    let mut layers = LayerRegistry::default();
    let mut rules = parse_document_sheet(user_agent_css(), media, &mut layers, vcx)?;
    let base = rules.len();
    for (i, r) in parse_document_sheet(extra_css, media, &mut layers, vcx)?
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
    if vcx.doc.quirks() {
        let base = rules.len();
        for (i, r) in parse_document_sheet(super::ua_style::QUIRKS_CSS, media, &mut layers, vcx)?
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
    let sheets = document_style_tags(&input.dom.document, admission, &vcx.doc.traversal)?;
    for css in sheets.iter() {
        let base = rules.len();
        for (i, r) in parse_document_sheet(css, media, &mut layers, vcx)?
            .into_iter()
            .enumerate()
        {
            rules.push(Rule {
                order: base + i,
                // Таблица ДОКУМЕНТА: её происхождение старше нашего умолчания и
                // темы приложения, поэтому она перебивает их независимо от
                // специфичности (CSS Cascade §6.4.4).
                origin: crate::style::css::Origin::Author,
                ..r
            });
        }
    }

    // Переменные темы: `:root { --x: … }` и `--x` в инлайн-стиле корня.
    // Собираются до обхода, потому что нужны каждому узлу.
    // Пользовательские свойства НАСЛЕДУЮТСЯ и каскадируют, поэтому корень
    // стартует пустым: каждый узел добавляет свои и передаёт вниз.

    // Наборы кадров собираются из тех же источников, что и правила.
    let mut frames = parse_keyframes(user_agent_css(), vcx);
    frames.extend(parse_keyframes(extra_css, vcx));
    let agent = Scope {
        index: crate::style::select::rule_index::RuleIndex::new(&agent_rules, admission)?,
        rules: agent_rules,
        frames: frames.clone(),
        clone_cost: input.agent_css.get(),
    };
    for css in sheets.iter() {
        frames.extend(crate::style::css::parse_keyframes_in(css, Some(media), vcx));
    }
    // `:has()`: аргументы собираются со всех селекторов, правила с
    // вложенным `:has` выкидываются (спека: cannot be nested), отметки
    // считаются отдельным проходом до обхода.
    let mut has_args_raw: Vec<String> = vec![];
    retain_valid_rules(admission, &mut rules, &mut has_args_raw)?;
    let has_args: Vec<HasArg> = has_args_raw
        .iter()
        .filter_map(|a| parse_has_arg(a))
        .collect();
    // Тени (`<template shadowrootmode>`): области стилей и распределение
    // слотов считаются ДО обхода, как отметки `:has`; обход получает их
    // индексом по узлу. Документ без теней предпроход не платит.
    let doc = Rc::new(Scope {
        index: crate::style::select::rule_index::RuleIndex::new(&rules, admission)?,
        rules,
        frames,
        clone_cost: Default::default(),
    });

    Ok(DocumentRules {
        agent,
        doc,
        has_args,
        layers,
    })
}
