//! Важные победители карты каскада блоком обычных объявлений: слой, который
//! кадры анимаций не перебивают (css-cascade-5 §6.2).

use super::{Candidate, CascadeMap, Pick};
use crate::style::cascade::key::CascadeKey;
use crate::style::css::{DeclBlock, Declaration};

impl<'a> CascadeMap<'a> {
    /// Важные победители блоком обычных объявлений в порядке каскада —
    /// то, что сильнее происхождения анимаций (css-cascade-5 §6.2): кадр
    /// анимации их не перебивает (`important-prop`).
    pub fn important_block(&self) -> DeclBlock {
        let mut wins: Vec<(CascadeKey, &str, Candidate<'a>, &str)> = self
            .named()
            .filter(|(k, _)| !k.starts_with("--"))
            .filter_map(|(k, mut picks)| match picks.next()? {
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
}
