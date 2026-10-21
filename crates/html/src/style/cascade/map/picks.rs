//! Выбор каскада по списку кандидатов одного имени (Blink
//! `StyleCascade::ResolveRevert`/`ResolveRevertLayer`, `CascadeMap::Find`):
//! победитель и следующие кандидаты; `revert` сужает выбор до происхождений
//! не выше целевого (css-cascade-5 §7.2), `revert-layer` — до уровней слоя
//! ниже своего (§7.3); откат без цели — `unset`.

use super::Candidate;
use crate::style::cascade::key::{CascadeKey, LayerLevel};
use crate::style::css::Origin;

/// Значение, выбранное каскадом для имени.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Pick<'a> {
    /// Объявление-победитель и его значение (у `all` — значение `all`).
    Value(Candidate<'a>, &'a str),
    /// `revert`/`revert-layer` откатить не к чему — значит `unset`
    /// (§7.2: «as if the property had no value»). Ключ — ключ отката.
    Unset(CascadeKey),
}

impl<'a> Pick<'a> {
    pub fn key(&self) -> CascadeKey {
        match self {
            Pick::Value(c, _) => c.key,
            Pick::Unset(k) => *k,
        }
    }
}

fn wide(value: &str) -> Option<&'static str> {
    ["revert", "revert-layer"]
        .into_iter()
        .find(|w| value.eq_ignore_ascii_case(w))
}

#[derive(Clone, Copy, Debug)]
enum Crit {
    Any,
    /// После `revert`: кандидаты не выше этого происхождения.
    OriginAtMost(Origin),
    /// После `revert-layer`: кандидаты слабее этого уровня слоя.
    LayerBelow(LayerLevel),
}

/// Последовательный выбор по списку кандидатов имени (см. [`CascadeMap::picks`]).
#[derive(Clone)]
pub(crate) struct Picks<'m, 'a> {
    list: &'m [Candidate<'a>],
    at: usize,
    crit: Crit,
    revert_key: Option<CascadeKey>,
    unset_on_empty: bool,
    done: bool,
}

impl<'m, 'a> Picks<'m, 'a> {
    pub(super) fn new(list: &'m [Candidate<'a>]) -> Self {
        Picks {
            list,
            at: 0,
            crit: Crit::Any,
            revert_key: None,
            unset_on_empty: true,
            done: false,
        }
    }

    /// Откат без цели у самого `all` ничего не задаёт (а не `unset`).
    pub(super) fn without_unset(mut self) -> Self {
        self.unset_on_empty = false;
        self
    }

    fn fits(&self, c: &Candidate<'a>) -> bool {
        match self.crit {
            Crit::Any => true,
            Crit::OriginAtMost(o) => c.key.origin() <= o,
            Crit::LayerBelow(level) => c.key.layer_level() < level,
        }
    }
}

impl<'a> Iterator for Picks<'_, 'a> {
    type Item = Pick<'a>;

    fn next(&mut self) -> Option<Pick<'a>> {
        if self.done {
            return None;
        }
        while self.at < self.list.len() {
            let c = self.list[self.at];
            self.at += 1;
            if !self.fits(&c) {
                continue;
            }
            match wide(c.value()) {
                Some("revert") => match c.key.origin().revert_target() {
                    Some(target) => self.crit = Crit::OriginAtMost(target),
                    None => {
                        self.done = true;
                        return Some(Pick::Unset(c.key));
                    }
                },
                Some(_) => self.crit = Crit::LayerBelow(c.key.layer_level()),
                None => return Some(Pick::Value(c, c.value())),
            }
            self.revert_key.get_or_insert(c.key);
        }
        self.done = true;
        match self.revert_key {
            Some(k) if self.unset_on_empty => Some(Pick::Unset(k)),
            _ => None,
        }
    }
}
