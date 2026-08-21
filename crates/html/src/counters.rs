//! Счётчики документа: стек областей видимости.
//!
//! Счётчик — не одна ячейка на имя, а СТЕК (css-lists-3 §5): вложенный
//! `counter-reset` создаёт новый счётчик того же имени внутри прежнего
//! («self-nesting»), `counter()` читает внутренний, `counters()` — всю
//! цепочку от внешнего к внутреннему. Область начинается на создателе и
//! включает его потомков и последующих братьев.
//!
//! Модель перенесена из Blink (`counters_attachment_context.cc`): вместо
//! честного удаления при выходе из области стек чистится ЛЕНИВО — перед
//! каждым обращением снимаются записи, чей создатель уже не по пути.
//! Владелец записи запоминается адресом в дереве КОРОБОК: `display:
//! contents` своего уровня не даёт, псевдоэлементы получают собственные
//! сегменты (они настоящие братья содержимого хозяина).

use std::collections::HashMap;

/// Сегмент пути для `::before` — больше любого настоящего номера ребёнка.
const PSEUDO_BEFORE: u32 = u32::MAX - 1;
/// Сегмент пути для `::after`.
const PSEUDO_AFTER: u32 = u32::MAX;

/// Один счётчик в стеке: кто его создал и текущее значение.
#[derive(Clone, Debug)]
struct Entry {
    owner: Vec<u32>,
    value: i32,
    /// Счётчик создан как `reversed(имя)`: пункты списка считают ВНИЗ
    /// (css-lists-3 §list-item-counter).
    reversed: bool,
}

/// Счётчики документа и адрес текущего узла в дереве коробок.
#[derive(Default)]
pub struct Counters {
    stack: HashMap<String, Vec<Entry>>,
    /// Адрес текущего узла: номера коробок по пути от корня.
    path: Vec<u32>,
    /// Сколько коробок уже пройдено на каждом уровне.
    next: Vec<u32>,
}

/// Путь родителя: адрес без последнего сегмента.
fn parent(owner: &[u32]) -> &[u32] {
    owner.split_last().map_or(&[], |(_, rest)| rest)
}

/// Предок ли `a` для `b` (или он сам): строгий префикс пути.
fn covers(a: &[u32], b: &[u32]) -> bool {
    b.len() >= a.len() && b[..a.len()] == *a
}

/// Снять записи, чья область уже кончилась (Blink `RemoveStaleCounters`).
///
/// Область создателя покрывает его самого, потомков и последующих братьев —
/// то есть всё, что лежит внутри его РОДИТЕЛЯ после него. Как только текущий
/// узел вышел за пределы родителя создателя, запись мертва.
fn remove_stale(st: &mut Vec<Entry>, cur: &[u32]) {
    while let Some(top) = st.last() {
        if covers(parent(&top.owner), cur) {
            break;
        }
        st.pop();
    }
}

impl Counters {
    /// Войти в коробку: занять следующий номер на своём уровне.
    pub fn enter(&mut self) {
        let level = self.path.len();
        if self.next.len() <= level {
            self.next.push(0);
        }
        let idx = self.next[level];
        self.next[level] = idx + 1;
        self.path.push(idx);
        // Внутри новой коробки нумерация детей начинается заново.
        self.next.truncate(level + 1);
        self.next.push(0);
    }

    /// Выйти из коробки.
    pub fn leave(&mut self) {
        self.path.pop();
        self.next.truncate(self.path.len() + 1);
    }

    /// Войти в псевдоэлемент хозяина: он брат его содержимого.
    pub fn enter_pseudo(&mut self, before: bool) {
        self.path
            .push(if before { PSEUDO_BEFORE } else { PSEUDO_AFTER });
        self.next.push(0);
    }

    /// `counter-reset` (Blink `CreateCounter`): своя запись заменяет запись
    /// СВОЕГО уровня (сам узел или предыдущий брат) и вкладывается внутрь
    /// записи предка.
    pub fn reset(&mut self, name: &str, value: i32) {
        self.reset_flagged(name, value, false);
    }

    /// То же, но с пометкой обратного счёта.
    pub fn reset_flagged(&mut self, name: &str, value: i32, reversed: bool) {
        let cur = self.path.clone();
        let st = self.stack.entry(name.to_string()).or_default();
        remove_stale(st, &cur);
        if let Some(top) = st.last()
            && parent(&top.owner) == parent(&cur)
        {
            st.pop();
        }
        st.push(Entry {
            owner: cur,
            value,
            reversed,
        });
    }

    /// Считает ли внутренний счётчик этого имени вниз.
    pub fn is_reversed(&mut self, name: &str) -> bool {
        let cur = self.path.clone();
        let Some(st) = self.stack.get_mut(name) else {
            return false;
        };
        remove_stale(st, &cur);
        st.last().is_some_and(|e| e.reversed)
    }

    /// `counter-increment` и `counter-set` (Blink `UpdateCounterValue`):
    /// при пустом стеке счётчик сперва создаётся со значения 0.
    pub fn update(&mut self, name: &str, delta: i32, is_set: bool) {
        let cur = self.path.clone();
        let st = self.stack.entry(name.to_string()).or_default();
        remove_stale(st, &cur);
        match st.last_mut() {
            Some(top) => {
                top.value = if is_set {
                    delta
                } else {
                    top.value.saturating_add(delta)
                }
            }
            None => st.push(Entry {
                owner: cur,
                value: delta,
                reversed: false,
            }),
        }
    }

    /// Выход из узла, который счётчик СОЗДАВАЛ (Blink
    /// `RemoveCounterIfAncestorExists`): если под своей записью лежит запись
    /// предка — свою снять, иначе последующие братья унаследовали бы её
    /// вместо предковой.
    pub fn leave_scope(&mut self, name: &str) {
        let cur = self.path.clone();
        let Some(st) = self.stack.get_mut(name) else {
            return;
        };
        if st.len() <= 1 || st.last().is_none_or(|e| e.owner != cur) {
            return;
        }
        let prev = st[st.len() - 2].owner.clone();
        if covers(&prev, &cur) {
            st.pop();
            return;
        }
        // Запись «дяди»: её создатель не предок, но лежит внутри предка —
        // свою всё равно снимаем (Blink §600), иначе область вложенного
        // сброса протекает на последующих братьев.
        let pp = parent(&prev);
        if covers(pp, &cur) && pp != parent(&cur) {
            st.pop();
        }
    }

    /// Переживёт ли будущая запись выход из своего создателя.
    ///
    /// Зеркало `leave_scope`: если под ней окажется запись предка или
    /// «дяди», её снимут на выходе — значит последующие братья её не
    /// увидят, и область обратного счёта обрывается поддеревом создателя.
    /// ВАЖНО: правило обязано меняться вместе с `leave_scope`.
    pub fn escapes_creator(&mut self, name: &str) -> bool {
        let cur = self.path.clone();
        let Some(st) = self.stack.get_mut(name) else {
            return true;
        };
        remove_stale(st, &cur);
        // Запись своего уровня будет вытеснена новой — смотреть надо под неё.
        let mut i = st.len();
        if i > 0 && parent(&st[i - 1].owner) == parent(&cur) {
            i -= 1;
        }
        let Some(prev) = i.checked_sub(1).map(|k| &st[k]) else {
            return true;
        };
        let pp = parent(&prev.owner);
        !(covers(&prev.owner, &cur) || (covers(pp, &cur) && pp != parent(&cur)))
    }

    /// `counter(имя)`: значение ВНУТРЕННЕГО счётчика; чтение счётчик не
    /// создаёт — пустой стек читается нулём.
    pub fn value_of(&mut self, name: &str) -> i32 {
        let cur = self.path.clone();
        let Some(st) = self.stack.get_mut(name) else {
            return 0;
        };
        remove_stale(st, &cur);
        st.last().map_or(0, |e| e.value)
    }

    /// `counters(имя, разделитель)`: вся цепочка от внешнего к внутреннему.
    pub fn chain_of(&mut self, name: &str) -> Vec<i32> {
        let cur = self.path.clone();
        let Some(st) = self.stack.get_mut(name) else {
            return vec![0];
        };
        remove_stale(st, &cur);
        if st.is_empty() {
            return vec![0];
        }
        st.iter().map(|e| e.value).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Область кончается там, где кончается родитель создателя.
    #[test]
    fn scope_ends_with_owners_parent() {
        let mut c = Counters::default();
        c.enter(); // корень
        c.enter(); // первый ребёнок — создатель
        c.reset("n", 0);
        c.update("n", 1, false);
        assert_eq!(c.value_of("n"), 1);
        c.leave();
        c.enter(); // следующий брат — внутри области
        c.update("n", 1, false);
        assert_eq!(c.value_of("n"), 2);
        c.leave();
        c.leave(); // вышли из родителя создателя
        c.enter();
        assert_eq!(c.value_of("n"), 0, "область кончилась вместе с родителем");
    }

    /// Вложенный сброс вкладывается, а сброс на своём уровне — заменяет.
    #[test]
    fn nesting_versus_replacement() {
        let mut c = Counters::default();
        c.enter();
        c.reset("n", 1);
        c.enter();
        c.reset("n", 7);
        assert_eq!(c.chain_of("n"), vec![1, 7], "вложенный счётчик рядом с внешним");
        c.leave_scope("n");
        c.leave();
        c.enter();
        c.reset("n", 9);
        assert_eq!(c.chain_of("n"), vec![1, 9], "брат заменил запись брата");
    }
}
