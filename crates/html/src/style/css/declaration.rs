//! Блок объявлений в порядке записи (css-cascade-5 §6.4 «order of
//! appearance»): каждое объявление — имя, значение без пометки важности,
//! вид заданного значения, флаг `!important` и место в исходном блоке.
//! Сокращения реестра приходят уже раскрытыми в длинные свойства (они делят
//! место сокращения). Повтор свойства хранится отдельной записью: у свойств
//! вне реестра действительность значения пока известна только применению
//! (CSS 2.1 §4.1.7 — негодный повтор не гасит прежний).

use super::property::{LonghandId, Specified};
use std::collections::HashMap;

/// Вычисленные значения пользовательских свойств элемента (`--*` → поток
/// токенов) — словарь подстановки `var()`, а не блок объявлений.
pub type VarMap = HashMap<String, String>;

/// Одно объявление блока.
#[derive(Clone, Debug, PartialEq)]
pub struct Declaration {
    /// Имя свойства: обычное — строчными, пользовательское (`--*`) — как
    /// записано (CSS Variables §2).
    pub name: String,
    /// Значение без `!important`.
    pub value: String,
    pub important: bool,
    /// Место в исходном блоке; переживает удаление соседей откатом.
    pub pos: u32,
    /// Длинное свойство реестра, если имя в нём есть.
    pub id: Option<LonghandId>,
    /// Вид заданного значения: обычное, общее слово или с подстановкой.
    pub specified: Specified,
}

impl Declaration {
    /// Объявление с видом значения по его тексту.
    pub fn new(
        name: impl Into<String>,
        value: impl Into<String>,
        important: bool,
        pos: u32,
    ) -> Self {
        let (name, value) = (name.into(), value.into());
        Declaration {
            id: LonghandId::from_name(&name),
            specified: Specified::of(&value),
            name,
            value,
            important,
            pos,
        }
    }
}

/// Объявления блока в порядке записи.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclBlock(pub Vec<Declaration>);

impl DeclBlock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Дописать объявление в конец блока; сокращение реестра — его длинными
    /// свойствами (без проверки значений: блок собирает сам движок).
    pub fn push(&mut self, name: impl Into<String>, value: impl Into<String>, important: bool) {
        super::property::push_declared(self, name.into(), &value.into(), important, None);
    }

    /// Место следующего объявления в блоке.
    pub fn next_pos(&self) -> u32 {
        self.0.last().map_or(0, |d| d.pos + 1)
    }

    /// Дописать готовое объявление (длинное свойство раскрытого сокращения
    /// несёт место сокращения).
    pub fn push_decl(&mut self, decl: Declaration) {
        self.0.push(decl);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> std::slice::Iter<'_, Declaration> {
        self.0.iter()
    }

    /// Есть ли в блоке объявление свойства `name` (любой важности).
    pub fn contains(&self, name: &str) -> bool {
        self.0.iter().any(|d| d.name == name)
    }

    /// Значение последнего объявления `name` любой важности.
    pub fn last(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|d| d.name == name)
            .map(|d| d.value.as_str())
    }

    /// Значение дескриптора at-правила: последнее объявление без
    /// `!important` (у дескрипторов пометка важности делает объявление
    /// недействительным).
    pub fn descriptor(&self, name: &str) -> Option<&str> {
        self.0
            .iter()
            .rev()
            .find(|d| d.name == name && !d.important)
            .map(|d| d.value.as_str())
    }

    /// Имена свойств по первому появлению, без повторов.
    pub fn names(&self) -> Vec<&str> {
        let mut out: Vec<&str> = Vec::new();
        for d in &self.0 {
            if !out.contains(&d.name.as_str()) {
                out.push(&d.name);
            }
        }
        out
    }

    /// Задать свойство одним обычным объявлением: прежние объявления того
    /// же имени уходят, новое встаёт на место первого из них (или в конец).
    /// Сокращение реестра задаёт так каждое своё длинное свойство.
    pub fn set(&mut self, name: &str, value: &str) {
        let mut parsed = DeclBlock::new();
        parsed.push(name, value, false);
        for decl in parsed.0 {
            self.set_decl(decl);
        }
    }

    fn set_decl(&mut self, decl: Declaration) {
        let name = decl.name.clone();
        match self.0.iter().position(|d| d.name == name) {
            Some(at) => {
                let pos = self.0[at].pos;
                self.0[at] = Declaration { pos, ..decl };
                let mut seen = false;
                self.0.retain(|d| {
                    if d.name != name {
                        return true;
                    }
                    let keep = !seen;
                    seen = true;
                    keep
                });
            }
            None => {
                let pos = self.next_pos();
                self.0.push(Declaration { pos, ..decl });
            }
        }
    }

    /// Наложить блок `other`: его свойства заменяют одноимённые объявления
    /// этого блока и встают в конец в своём порядке.
    pub fn merge(&mut self, other: DeclBlock) {
        self.0.retain(|d| !other.contains(&d.name));
        let mut pos = self.next_pos();
        let mut last = None;
        for d in other.0 {
            // Длинные свойства одного сокращения сохраняют общее место.
            if last.is_some_and(|l| l != d.pos) {
                pos += 1;
            }
            last = Some(d.pos);
            self.0.push(Declaration { pos, ..d });
        }
    }

    /// Оставить объявления, для которых `keep` истинно.
    pub fn retain(&mut self, keep: impl FnMut(&Declaration) -> bool) {
        self.0.retain(keep);
    }
}

impl<'a> IntoIterator for &'a DeclBlock {
    type Item = &'a Declaration;
    type IntoIter = std::slice::Iter<'a, Declaration>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.iter()
    }
}

impl FromIterator<Declaration> for DeclBlock {
    fn from_iter<I: IntoIterator<Item = Declaration>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests;
