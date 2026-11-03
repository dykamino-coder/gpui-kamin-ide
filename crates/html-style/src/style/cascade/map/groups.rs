//! Кандидаты карты каскада одним массивом, сгруппированным по имени свойства
//! (внутри группы — от сильного к слабому; равные ключи — в порядке
//! поступления). Оглавление групп упорядочено по номеру длинного свойства
//! реестра, а имена вне реестра (`--*`, сокращения вне реестра) — по тексту:
//! без отдельного списка и хеша на имя (Blink `CascadeMap` хранит кандидатов
//! по `CSSPropertyID` так же, без строк).

use super::Candidate;
use crate::style::css::{LinkMatch, LonghandId};
use std::cmp::Ordering;

/// Номер имени в оглавлении: длинное свойство реестра — по таблице, прочие — в конце.
fn rank(id: Option<LonghandId>) -> u16 {
    id.map_or(u16::MAX, |id| id as u16)
}

fn by_name(a: (u16, &str), b: (u16, &str)) -> Ordering {
    // У длинного свойства реестра имя однозначно задаётся номером.
    a.0.cmp(&b.0).then_with(|| {
        if a.0 == u16::MAX {
            a.1.cmp(b.1)
        } else {
            Ordering::Equal
        }
    })
}

struct Entry<'a> {
    rank: u16,
    name: &'a str,
    seq: u32,
    cand: Candidate<'a>,
}

fn strongest_first(a: &Entry, b: &Entry) -> Ordering {
    by_name((a.rank, a.name), (b.rank, b.name))
        .then(b.cand.key.cmp(&a.cand.key))
        .then(a.seq.cmp(&b.seq))
}

#[derive(Default)]
pub(super) struct Groups<'a> {
    cands: Vec<Candidate<'a>>,
    /// (номер, имя, начало, конец) группы в `cands`, по [`by_name`].
    index: Vec<(u16, &'a str, u32, u32)>,
    /// Номера групп в порядке первого появления имени.
    order: Vec<u32>,
    names: Vec<&'a str>,
}

impl<'a> Groups<'a> {
    /// `all` (css-cascade-5 §3.2) — кандидат каждого имени, кроме
    /// пользовательских свойств, `direction` и `unicode-bidi`.
    pub(super) fn build(
        pending: Vec<(&'a str, Option<LonghandId>, Candidate<'a>)>,
        all: &[(Candidate<'a>, LinkMatch)],
    ) -> Self {
        let mut entries: Vec<Entry<'a>> = pending
            .into_iter()
            .zip(0u32..)
            .map(|((name, id, cand), seq)| Entry {
                rank: rank(id),
                name,
                seq,
                cand,
            })
            .collect();
        entries.sort_unstable_by(strongest_first);
        if !all.is_empty() {
            let mut seq = u32::try_from(entries.len()).unwrap_or(u32::MAX);
            let mut extra = vec![];
            for (i, e) in entries.iter().enumerate() {
                let first = i == 0 || entries[i - 1].name != e.name;
                if first
                    && !e.name.starts_with("--")
                    && !matches!(e.name, "direction" | "unicode-bidi")
                {
                    for (cand, _) in all.iter().filter(|(_, link)| link.admits(e.name)) {
                        extra.push(Entry {
                            rank: e.rank,
                            name: e.name,
                            seq,
                            cand: *cand,
                        });
                        seq = seq.saturating_add(1);
                    }
                }
            }
            entries.extend(extra);
            entries.sort_unstable_by(strongest_first);
        }
        let mut groups = Groups::default();
        let mut firsts: Vec<(u32, u32)> = vec![];
        for (i, e) in entries.iter().enumerate() {
            let at = u32::try_from(i).unwrap_or(u32::MAX);
            match groups.index.last_mut() {
                Some(g) if g.0 == e.rank && g.1 == e.name => {
                    g.3 = at + 1;
                    let first = firsts.last_mut().expect("group first");
                    first.0 = first.0.min(e.seq);
                }
                _ => {
                    let group = u32::try_from(groups.index.len()).unwrap_or(u32::MAX);
                    groups.index.push((e.rank, e.name, at, at + 1));
                    firsts.push((e.seq, group));
                }
            }
        }
        firsts.sort_unstable();
        groups.order = firsts.into_iter().map(|(_, group)| group).collect();
        groups.names = groups
            .order
            .iter()
            .map(|&g| groups.index[g as usize].1)
            .collect();
        groups.cands = entries.into_iter().map(|e| e.cand).collect();
        groups
    }

    pub(super) fn names(&self) -> &[&'a str] {
        &self.names
    }

    /// Группы в порядке первого появления имени.
    pub(super) fn named(&self) -> impl Iterator<Item = (&'a str, &[Candidate<'a>])> + '_ {
        self.order.iter().map(|&g| {
            let (_, name, start, end) = self.index[g as usize];
            (name, &self.cands[start as usize..end as usize])
        })
    }

    pub(super) fn get(&self, name: &str) -> &[Candidate<'a>] {
        let key = (rank(LonghandId::from_name(name)), name);
        self.index
            .binary_search_by(|g| by_name((g.0, g.1), key))
            .map_or(&[], |at| {
                let (_, _, start, end) = self.index[at];
                &self.cands[start as usize..end as usize]
            })
    }
}
