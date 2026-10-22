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
use crate::style::css::{DeclBlock, Declaration, LinkMatch, Origin, Rule, Specified};
use std::collections::HashMap;

mod picks;
pub(crate) use picks::{Pick, Picks};

/// Одно объявление-кандидат.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Candidate<'a> {
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
pub(crate) struct CascadeMap<'a> {
    names: Vec<&'a str>,
    lists: HashMap<&'a str, Vec<Candidate<'a>>>,
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
            let list = self.lists.entry(decl.name.as_str()).or_default();
            if list.is_empty() {
                self.names.push(decl.name.as_str());
            }
            list.push(cand);
        }
    }

    fn finish(&mut self) {
        for (name, list) in self.lists.iter_mut() {
            if !name.starts_with("--") && !matches!(*name, "direction" | "unicode-bidi") {
                list.extend(
                    self.all
                        .iter()
                        .filter(|(_, link)| link.admits(name))
                        .map(|(c, _)| *c),
                );
            }
            list.sort_by_key(|c| std::cmp::Reverse(c.key));
        }
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
        &self.names
    }

    /// Выбор каскада для имени: первый элемент — победитель; следующие —
    /// запасные на случай, если значение победителя окажется негодным при
    /// применении (негодное объявление не участвует в каскаде, CSS Syntax
    /// §4.1.7 / CSS 2.1 §4.2).
    pub fn picks(&self, name: &str) -> Picks<'_, 'a> {
        Picks::new(self.lists.get(name).map_or(&[], Vec::as_slice))
    }

    /// Важные победители блоком обычных объявлений в порядке каскада —
    /// то, что сильнее происхождения анимаций (css-cascade-5 §6.2): кадр
    /// анимации их не перебивает (`important-prop`).
    pub fn important_block(&self) -> DeclBlock {
        let mut wins: Vec<(CascadeKey, &str, Candidate<'a>, &str)> = self
            .names
            .iter()
            .filter(|k| !k.starts_with("--"))
            .filter_map(|&k| match self.picks(k).next()? {
                Pick::Value(c, v) if c.key.important() => Some((c.key, k, c, v)),
                _ => None,
            })
            .collect();
        wins.sort_by_key(|w| w.0);
        let mut block = DeclBlock::new();
        for (_, k, c, v) in wins {
            let pos = block.next_pos();
            // Значение с подстановкой у длинного свойства сокращения — текст
            // сокращения: объявление переносится с его видом значения.
            let decl = if c.decl.name == k {
                Declaration {
                    important: false,
                    pos,
                    ..c.decl.clone()
                }
            } else {
                Declaration::new(k, v, false, pos)
            };
            block.push_decl(decl);
        }
        block
    }

    /// Выбор каскада для самого `all`: значение, которое получают свойства,
    /// не названные ни одним объявлением.
    pub fn all_picks(&self) -> Picks<'_, 'a> {
        Picks::new(&self.all_any).without_unset()
    }
}

#[cfg(test)]
mod tests;
