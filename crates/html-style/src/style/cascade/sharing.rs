//! Ограниченный кэш чистого каскада: совпавшие блоки, окружение и неизменяемые группы.
//! Контекстные функции и экранированные значения идут обычным путём: подстановки и арены
//! имеют собственный бюджет и могут читать атрибуты, положение или состояние элемента.

use crate::style::cascade::{map::CascadeMap, vars::CascadeCx};
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, LinkMatch, Origin, Rule, VarMap};
use crate::style::values::value::{RootMetrics, Scheme};
use crate::values::ValueCx;
use std::collections::VecDeque;

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
}

/// Закрытое владение ускорителем: наружу не выдаётся mutable style/cache.
#[derive(Default)]
pub(crate) struct StyleSharing(std::cell::RefCell<Cache>);

impl StyleSharing {
    pub(crate) fn resolve(
        &self,
        rules: &[&Rule],
        inline: &DeclBlock,
        map: &CascadeMap<'_>,
        vars: &VarMap,
        cx: CascadeCx<'_>,
        vcx: ValueCx<'_>,
    ) -> Computed {
        self.0
            .borrow_mut()
            .resolve(rules, inline, map, vars, cx, vcx)
    }
}

struct RuleInput {
    decls: DeclBlock,
    origin: Origin,
    layer: Vec<u32>,
    order: usize,
    specificity: (u32, u32, u32),
    link: LinkMatch,
}

impl RuleInput {
    fn new(rule: &Rule) -> Self {
        Self {
            decls: rule.decls.clone(),
            origin: rule.origin,
            layer: rule.layer.clone(),
            order: rule.order,
            specificity: rule.sel.specificity(),
            link: rule.link,
        }
    }

    fn matches(&self, rule: &Rule) -> bool {
        self.order == rule.order
            && self.origin == rule.origin
            && self.layer == rule.layer
            && self.specificity == rule.sel.specificity()
            && self.link == rule.link
            && self.decls == rule.decls
    }
}

struct Environment {
    root: RootMetrics,
    font_generation: u64,
    viewport: [u32; 2],
    media: (u32, u32, bool, bool),
    scheme: Scheme,
    quirks: bool,
    clearance: bool,
    frame: bool,
}

impl Environment {
    fn new(vcx: ValueCx<'_>) -> Self {
        let media = vcx.doc.media.get();
        Self {
            root: vcx.doc.root.borrow().clone(),
            font_generation: vcx.doc.font_generation(),
            viewport: [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()],
            media: (
                media.width.to_bits(),
                media.height.to_bits(),
                media.dark,
                media.print,
            ),
            scheme: vcx.scheme,
            quirks: vcx.doc.quirks(),
            clearance: vcx.clearance_possible,
            frame: vcx.frame,
        }
    }

    fn matches(&self, vcx: ValueCx<'_>) -> bool {
        let media = vcx.doc.media.get();
        self.font_generation == vcx.doc.font_generation()
            && self.viewport == [vcx.viewport.0.to_bits(), vcx.viewport.1.to_bits()]
            && self.media
                == (
                    media.width.to_bits(),
                    media.height.to_bits(),
                    media.dark,
                    media.print,
                )
            && self.scheme == vcx.scheme
            && self.quirks == vcx.doc.quirks()
            && self.clearance == vcx.clearance_possible
            && self.frame == vcx.frame
            && self.root.same_inputs(&vcx.doc.root.borrow())
    }
}

struct Entry {
    rules: Vec<RuleInput>,
    inline: DeclBlock,
    environment: Environment,
    style: Computed,
    _lease: Option<crate::budget::LiveReservation>,
}

impl Cache {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve(
        &mut self,
        rules: &[&Rule],
        inline: &DeclBlock,
        map: &CascadeMap<'_>,
        vars: &VarMap,
        cx: CascadeCx<'_>,
        vcx: ValueCx<'_>,
    ) -> Computed {
        let bytes = cacheable(rules, inline, cx, vcx);
        if bytes.is_some()
            && let Some(hit) = self.entries.iter().rev().find(|entry| {
                entry.rules.len() == rules.len()
                    && entry.inline == *inline
                    && entry.environment.matches(vcx)
                    && entry.rules.iter().zip(rules).all(|(a, b)| a.matches(b))
            })
        {
            crate::work::add!(StyleSharingHits, 1);
            return hit.style.clone();
        }
        let mut style = vcx.doc.initial_style();
        style.apply_map(map, vars, cx, vcx);
        if let Some(bytes) = bytes {
            let admission = vcx
                .doc
                .budget
                .borrow()
                .clone()
                .map(crate::budget::admission::Admission::new)
                .unwrap_or_default();
            if let Ok(lease) = admission.storage(bytes) {
                if self.entries.len() == 32 {
                    self.entries.pop_front();
                }
                self.entries.push_back(Entry {
                    rules: rules.iter().map(|r| RuleInput::new(r)).collect(),
                    inline: inline.clone(),
                    environment: Environment::new(vcx),
                    style: style.clone(),
                    _lease: lease,
                });
            }
        }
        style
    }
}

/// Ограничения относятся только к памяти ускорителя; отказ не меняет каскад.
fn cacheable(
    rules: &[&Rule],
    inline: &DeclBlock,
    cx: CascadeCx<'_>,
    vcx: ValueCx<'_>,
) -> Option<usize> {
    if cx.root || vcx.frame {
        return None;
    }
    let mut bytes = size_of::<Entry>() + vcx.doc.root.borrow().family.len();
    for block in rules
        .iter()
        .map(|r| &r.decls)
        .chain(std::iter::once(inline))
    {
        for decl in block.iter() {
            if !pure_value(&decl.value) {
                return None;
            }
            bytes = bytes.checked_add(
                size_of::<crate::style::css::Declaration>() + decl.name.len() + decl.value.len(),
            )?;
            if bytes > 65536 {
                return None;
            }
        }
    }
    for rule in rules {
        bytes = bytes.checked_add(size_of::<RuleInput>() + rule.layer.len() * 4)?;
    }
    // Запись ограничена вместе с копиями входов; COW-группы остаются общими.
    (bytes <= 65536).then_some(bytes)
}

/// Простые цветовые функции чистые; неизвестная функция сохраняет обычный путь.
fn pure_value(value: &str) -> bool {
    if value.contains('\\') {
        return false;
    }
    for (at, _) in value.match_indices('(') {
        let name = value[..at]
            .rsplit(|c: char| !c.is_ascii_alphabetic())
            .next()
            .unwrap_or("");
        if ![
            "rgb", "rgba", "hsl", "hsla", "hwb", "lab", "lch", "oklab", "oklch",
        ]
        .iter()
        .any(|pure| name.eq_ignore_ascii_case(pure))
        {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests;
