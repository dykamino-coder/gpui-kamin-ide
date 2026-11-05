//! Ограниченный кэш чистого каскада: совпавшие блоки, окружение и неизменяемые группы.
//! Контекстные функции и экранированные значения идут обычным путём: подстановки и арены
//! имеют собственный бюджет и могут читать атрибуты, положение или состояние элемента.

use crate::style::cascade::{map::CascadeMap, vars::CascadeCx};
use crate::style::computed::Computed;
use crate::style::css::{DeclBlock, LinkMatch, Origin, Rule, VarMap};
use crate::style::values::value::{RootMetrics, Scheme};
use crate::values::ValueCx;
use std::collections::VecDeque;
use std::hash::{DefaultHasher, Hash, Hasher};

/// Записей в кэше: чередование больше этого числа разных входов (91 разный
/// `style="flex:…"` по кругу) выбивало бы каждую запись до повторного входа.
const CAPACITY: usize = 128;

#[derive(Default)]
struct Cache {
    entries: VecDeque<Entry>,
}

/// Закрытое владение ускорителем: наружу не выдаётся mutable style/cache.
#[derive(Default)]
pub(crate) struct StyleSharing(std::cell::RefCell<Cache>);

impl StyleSharing {
    /// `finish` дописывает поля, которые зависят только от входов ключа (окружения и
    /// совпавших блоков). Запись идёт до сохранения, поэтому попадание не пишет в общие
    /// группы и не отделяет их копией.
    /// Карту каскада `map` строит лишь промах: попаданию она не нужна.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve<'m, 'a: 'm>(
        &self,
        rules: &[&Rule],
        inline: &DeclBlock,
        map: impl FnOnce() -> &'m CascadeMap<'a>,
        vars: &VarMap,
        cx: CascadeCx<'_>,
        vcx: ValueCx<'_>,
        finish: impl FnOnce(&mut Computed),
    ) -> Computed {
        self.0
            .borrow_mut()
            .resolve(rules, inline, map, vars, cx, vcx, finish)
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

pub(crate) struct Environment {
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
    pub(crate) fn new(vcx: ValueCx<'_>) -> Self {
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

    pub(crate) fn matches(&self, vcx: ValueCx<'_>) -> bool {
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
    /// Свёртка входов ключа ([`input_hash`]): равные входы дают равную свёртку.
    hash: u64,
    rules: Vec<RuleInput>,
    inline: DeclBlock,
    environment: Environment,
    style: Computed,
    _lease: Option<crate::budget::LiveReservation>,
}

impl Cache {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn resolve<'m, 'a: 'm>(
        &mut self,
        rules: &[&Rule],
        inline: &DeclBlock,
        map: impl FnOnce() -> &'m CascadeMap<'a>,
        vars: &VarMap,
        cx: CascadeCx<'_>,
        vcx: ValueCx<'_>,
        finish: impl FnOnce(&mut Computed),
    ) -> Computed {
        let bytes = cacheable(rules, inline, cx, vcx);
        let hash = bytes.map_or(0, |_| input_hash(rules, inline));
        if bytes.is_some()
            && let Some(hit) = self.entries.iter().rev().find(|entry| {
                entry.hash == hash
                    && entry.rules.len() == rules.len()
                    && entry.inline == *inline
                    && entry.environment.matches(vcx)
                    && entry.rules.iter().zip(rules).all(|(a, b)| a.matches(b))
            })
        {
            crate::work::add!(StyleSharingHits, 1);
            return hit.style.clone();
        }
        let mut style = vcx.doc.initial_style();
        style.apply_map(map(), vars, cx, vcx);
        finish(&mut style);
        if let Some(bytes) = bytes {
            let admission = vcx
                .doc
                .budget
                .borrow()
                .clone()
                .map(crate::budget::admission::Admission::new)
                .unwrap_or_default();
            if let Ok(lease) = admission.storage(bytes) {
                if self.entries.len() == CAPACITY {
                    self.entries.pop_front();
                }
                self.entries.push_back(Entry {
                    hash,
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

/// Свёртка тех входов ключа, что дёшево читать: порядок и слой правил, текст
/// `style=""`. Полное сравнение входов остаётся за [`RuleInput::matches`].
fn input_hash(rules: &[&Rule], inline: &DeclBlock) -> u64 {
    let mut h = DefaultHasher::new();
    rules.len().hash(&mut h);
    for rule in rules {
        rule.order.hash(&mut h);
        rule.layer.hash(&mut h);
    }
    for decl in inline.iter() {
        (&decl.name, &decl.value, decl.important).hash(&mut h);
    }
    h.finish()
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

mod pure;
use pure::pure_value;

#[cfg(test)]
mod gradient_tests;
#[cfg(test)]
mod tests;
