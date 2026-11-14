//! Отложенные ответы на запросы хоста к шеллу (BR-19A).
//!
//! QuickPick, тост `shell.showMessage` с кнопками и нативные диалоги отвечают
//! хосту не сразу, а после решения пользователя. kamin-host нумерует свои
//! запросы заново на каждом соединении (`RpcEndpoint.nextId` с 1), поэтому id
//! запроса без соединения ничего не значит. Ответ на пик, открытый до обрыва,
//! уходил текущему клиенту и завершал новый запрос с тем же id: следующий
//! `showQuickPick` получал `undefined` через несколько миллисекунд.
//!
//! Поэтому UI получает не id хоста, а билет, уникальный на всё время жизни
//! шелла. Билет помнит поколение соединения, принявшего запрос. Ответ уходит
//! только тогда, когда это соединение всё ещё текущее. Билеты оборванного
//! соединения забываются: отвечать уже некому.

use std::collections::HashMap;
use std::sync::{Mutex, OnceLock};

use serde_json::Value;

/// Билеты, ждущие ответа: билет → (поколение соединения, id запроса хоста).
#[derive(Default)]
pub(crate) struct Pending {
    last_ticket: u64,
    waiting: HashMap<u64, (u64, u64)>,
}

impl Pending {
    /// Выдать билет на запрос `req_id`, принятый соединением `generation`.
    pub(crate) fn defer(&mut self, generation: u64, req_id: u64) -> u64 {
        self.last_ticket += 1;
        self.waiting.insert(self.last_ticket, (generation, req_id));
        self.last_ticket
    }

    /// Погасить билет. Id запроса хоста возвращается, только если билет ещё ждал
    /// и принадлежит текущему соединению `current`.
    pub(crate) fn take(&mut self, ticket: u64, current: Option<u64>) -> Option<u64> {
        let (generation, req_id) = self.waiting.remove(&ticket)?;
        (current == Some(generation)).then_some(req_id)
    }

    /// Соединение оборвалось: его запросы больше некому завершать.
    pub(crate) fn forget(&mut self, generation: u64) {
        self.waiting.retain(|_, (owner, _)| *owner != generation);
    }
}

fn pending() -> &'static Mutex<Pending> {
    static PENDING: OnceLock<Mutex<Pending>> = OnceLock::new();
    PENDING.get_or_init(Default::default)
}

/// Билет на отложенный ответ запросу соединения `generation`.
pub(crate) fn defer(generation: u64, req_id: u64) -> u64 {
    pending().lock().unwrap().defer(generation, req_id)
}

/// Забыть билеты оборванного соединения.
pub(crate) fn forget(generation: u64) {
    pending().lock().unwrap().forget(generation);
}

/// Ответить по билету, в фоне. Ответ соединению, которое уже сменилось,
/// отбрасывается.
pub(crate) fn reply(ticket: u64, value: Value) {
    std::thread::spawn(move || {
        let link = crate::host_link::current_link();
        let current = link.as_ref().map(|(generation, _)| *generation);
        let req_id = pending().lock().unwrap().take(ticket, current);
        if let (Some(req_id), Some((_, client))) = (req_id, link) {
            client.respond(req_id, Ok(value));
        }
    });
}

#[cfg(test)]
mod tests {
    use super::Pending;

    #[test]
    fn a_pick_cancelled_after_reconnect_does_not_answer_the_new_pick() {
        let mut p = Pending::default();
        // Оба соединения прислали свой первый запрос: id хоста совпадают.
        let stale = p.defer(1, 1);
        let fresh = p.defer(2, 1);
        assert_ne!(stale, fresh);
        assert_eq!(p.take(stale, Some(2)), None, "the old connection is gone");
        assert_eq!(p.take(fresh, Some(2)), Some(1), "the new pick still waits");
    }

    #[test]
    fn a_ticket_is_answered_once() {
        let mut p = Pending::default();
        let t = p.defer(3, 7);
        assert_eq!(p.take(t, Some(3)), Some(7));
        assert_eq!(p.take(t, Some(3)), None);
    }

    #[test]
    fn nothing_is_answered_while_disconnected() {
        let mut p = Pending::default();
        let t = p.defer(4, 2);
        assert_eq!(p.take(t, None), None);
        assert_eq!(p.take(t, Some(4)), None, "the ticket was spent");
    }

    #[test]
    fn a_dropped_connection_forgets_only_its_own_tickets() {
        let mut p = Pending::default();
        let old = p.defer(5, 1);
        let new = p.defer(6, 1);
        p.forget(5);
        assert_eq!(p.take(old, Some(5)), None);
        assert_eq!(p.take(new, Some(6)), Some(1));
    }

    #[test]
    fn an_unknown_ticket_answers_nothing() {
        // Дев-пик из probe приходит с билетом 0, а выдача начинается с 1.
        let mut p = Pending::default();
        p.defer(1, 1);
        assert_eq!(p.take(0, Some(1)), None);
    }
}
