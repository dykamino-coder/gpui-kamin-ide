//! Карта каскада элемента: для каждого имени свойства — все кандидаты-
//! объявления, упорядоченные по [`CascadeKey`] от сильного к слабому (Blink
//! `CascadeMap`, Stylo `rule_tree` + `cascade.rs`). Победитель — сильнейший
//! кандидат; `revert` и `revert-layer` (css-cascade-5 §7.2, §7.3) решаются
//! откатом по этому же списку, а не правкой копий правил. `all`
//! (css-cascade-5 §3.2) участвует кандидатом в каждом имени, кроме
//! `direction`, `unicode-bidi` и пользовательских свойств.
//!
//! Ключ карты — имя свойства как записано. Сокращения реестра раскрыты
//! разбором и приходят длинными свойствами; остальные сокращения пока
//! раскрываются при применении (сокращение и его длинные свойства — разные
//! имена).

use super::key::{CascadeKey, KeySource};
use crate::style::css::{DeclBlock, Declaration, LinkMatch, LonghandId, Origin, Rule, Specified};
mod groups;
mod important;
mod picks;
use groups::Groups;
pub use picks::{Pick, Picks};

/// Одно объявление-кандидат.
#[derive(Clone, Copy, Debug)]
pub struct Candidate<'a> {
    pub key: CascadeKey,
    pub decl: &'a Declaration,
    /// Блок объявления: нужен порядку применения внутри блока.
    pub block: &'a DeclBlock,
}

impl<'a> Candidate<'a> {
    pub fn value(&self) -> &'a str {
        self.decl.value.trim()
    }
}

/// Карта каскада одного элемента.
#[derive(Default)]
pub struct CascadeMap<'a> {
    /// Объявления в порядке поступления, пока карта собирается.
    pending: Vec<(&'a str, Option<LonghandId>, Candidate<'a>)>,
    groups: Groups<'a>,
    /// Кандидаты `all` с ограничением `:visited` их правила.
    all: Vec<(Candidate<'a>, LinkMatch)>,
    /// Кандидаты `all` без ограничения: они задают и неназванные свойства.
    all_any: Vec<Candidate<'a>>,
}

impl<'a> CascadeMap<'a> {
    /// Карта по совпавшим правилам и `style=""`.
    pub fn collect(matched: &[&'a Rule], inline: &'a DeclBlock) -> Self {
        let mut map = CascadeMap::default();
        // Слои — линейный порядок путей (`LayerRegistry`): собственные
        // правила слоя идут после подслоёв, неслоевые — последними.
        let mut layers: Vec<&[u32]> = matched.iter().map(|r| r.layer.as_slice()).collect();
        layers.sort();
        layers.dedup();
        let rank = |layer: &[u32]| -> u16 {
            let at = layers.binary_search(&layer).unwrap_or(0);
            u16::try_from(at).unwrap_or(u16::MAX)
        };
        for rule in matched {
            map.push_block(
                &rule.decls,
                rule.link,
                &KeySource {
                    origin: rule.origin,
                    tree_order: 0,
                    inline: false,
                    layer: rank(&rule.layer),
                    specificity: rule.sel.specificity(),
                    proximity: 0,
                    rule_order: u32::try_from(rule.order).unwrap_or(u32::MAX),
                },
            );
        }
        map.push_block(
            inline,
            LinkMatch::Any,
            &KeySource {
                origin: Origin::Author,
                tree_order: 0,
                inline: true,
                layer: 0,
                specificity: (0, 0, 0),
                proximity: 0,
                rule_order: 0,
            },
        );
        map.finish();
        map
    }

    /// Карта одного блока без правил (`apply_decls`): обычные и важные
    /// объявления автора.
    pub fn single(block: &'a DeclBlock, origin: Origin) -> Self {
        let mut map = CascadeMap::default();
        map.push_block(
            block,
            LinkMatch::Any,
            &KeySource {
                origin,
                tree_order: 0,
                inline: false,
                layer: 0,
                specificity: (0, 0, 0),
                proximity: 0,
                rule_order: 0,
            },
        );
        map.finish();
        map
    }

    fn push_block(&mut self, block: &'a DeclBlock, link: LinkMatch, src: &KeySource) {
        for decl in block {
            let cand = Candidate {
                key: CascadeKey::new(src, decl.important, decl.pos),
                decl,
                block,
            };
            if decl.name == "all" {
                // `all` принимает только общие слова (§3.2); иное значение
                // делает объявление недействительным.
                if matches!(decl.specified, Specified::Wide(_)) {
                    self.all.push((cand, link));
                }
                continue;
            }
            // Ограничение `:visited`: объявление вне свойств своего
            // состояния ссылки в каскад не попадает.
            if !link.admits(&decl.name) {
                continue;
            }
            debug_assert_eq!(decl.id, LonghandId::from_name(&decl.name));
            self.pending.push((decl.name.as_str(), decl.id, cand));
        }
    }

    fn finish(&mut self) {
        self.groups = Groups::build(std::mem::take(&mut self.pending), &self.all);
        self.all.sort_by_key(|(c, _)| std::cmp::Reverse(c.key));
        self.all_any = self
            .all
            .iter()
            .filter(|(_, link)| *link == LinkMatch::Any)
            .map(|(c, _)| *c)
            .collect();
    }

    /// Имена свойств карты в порядке первого появления.
    pub fn names(&self) -> &[&'a str] {
        self.groups.names()
    }

    /// Имена в порядке первого появления вместе с их выбором каскада.
    pub fn named(&self) -> impl Iterator<Item = (&'a str, Picks<'_, 'a>)> + '_ {
        self.groups
            .named()
            .map(|(name, list)| (name, Picks::new(list)))
    }

    /// Выбор каскада для имени: первый элемент — победитель; следующие —
    /// запасные на случай, если значение победителя окажется негодным при
    /// применении (негодное объявление не участвует в каскаде, CSS Syntax
    /// §4.1.7 / CSS 2.1 §4.2).
    pub fn picks(&self, name: &str) -> Picks<'_, 'a> {
        Picks::new(self.groups.get(name))
    }
    /// Выбор каскада для самого `all`: значение, которое получают свойства,
    /// не названные ни одним объявлением.
    pub fn all_picks(&self) -> Picks<'_, 'a> {
        Picks::new(&self.all_any).without_unset()
    }
}

#[cfg(test)]
mod tests;
