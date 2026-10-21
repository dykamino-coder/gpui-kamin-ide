//! Блок объявлений в порядке записи (css-cascade-5 §6.4 «order of
//! appearance»): каждое объявление — имя, значение без пометки важности,
//! флаг `!important` и место в исходном блоке. Повтор свойства хранится
//! отдельной записью: действительность значения пока известна только
//! применению (CSS 2.1 §4.1.7 — негодный повтор не гасит прежний), поэтому
//! блок не сводит повторы при разборе.

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
}

/// Объявления блока в порядке записи.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct DeclBlock(pub Vec<Declaration>);

impl DeclBlock {
    pub fn new() -> Self {
        Self::default()
    }

    /// Дописать объявление в конец блока.
    pub fn push(&mut self, name: impl Into<String>, value: impl Into<String>, important: bool) {
        let pos = self.0.last().map_or(0, |d| d.pos + 1);
        self.0.push(Declaration {
            name: name.into(),
            value: value.into(),
            important,
            pos,
        });
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
    pub fn set(&mut self, name: &str, value: &str) {
        let (name, value) = (name.to_string(), value.to_string());
        match self.0.iter().position(|d| d.name == name) {
            Some(at) => {
                let pos = self.0[at].pos;
                self.0[at] = Declaration {
                    name: name.clone(),
                    value,
                    important: false,
                    pos,
                };
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
            None => self.push(name, value, false),
        }
    }

    /// Наложить блок `other`: его свойства заменяют одноимённые объявления
    /// этого блока и встают в конец в своём порядке.
    pub fn merge(&mut self, other: DeclBlock) {
        self.0.retain(|d| !other.contains(&d.name));
        for d in other.0 {
            self.push(d.name, d.value, d.important);
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
mod tests {
    use super::DeclBlock;

    #[test]
    fn keeps_write_order_and_repeats() {
        let mut d = DeclBlock::new();
        d.push("color", "red", false);
        d.push("margin", "0", true);
        d.push("color", "green", false);
        assert_eq!(d.names(), ["color", "margin"]);
        assert_eq!(d.last("color"), Some("green"));
        assert_eq!(d.descriptor("margin"), None);
        assert_eq!(d.iter().map(|d| d.pos).collect::<Vec<_>>(), [0, 1, 2]);
    }

    #[test]
    fn set_and_merge_replace_same_names() {
        let mut d = DeclBlock::new();
        d.push("width", "1px", false);
        d.push("height", "2px", false);
        d.push("width", "3px", true);
        d.set("width", "4px");
        assert_eq!(d.len(), 2);
        assert_eq!(d.0[0].value, "4px");
        assert!(!d.0[0].important);
        let mut other = DeclBlock::new();
        other.push("height", "5px", false);
        d.merge(other);
        assert_eq!(d.names(), ["width", "height"]);
        assert_eq!(d.last("height"), Some("5px"));
    }
}
