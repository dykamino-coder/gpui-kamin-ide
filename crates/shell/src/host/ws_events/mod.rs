//! Разбор событий WS-канала хоста: одно событие → одно `ShellEvent`.
//!
//! Каналов много, поэтому они разложены по темам; каждая тема отвечает
//! `true`, если канал её. Порядок опроса неважен — имена каналов не
//! пересекаются (`plan/100-refactor-250.md`).

mod shell;
mod status;
mod tree;
mod views;

use serde_json::Value;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

pub(crate) fn dispatch(on_event_tx: &Sender<ShellEvent>, channel: &str, payload: &Value) {
    if status::handle(on_event_tx, channel, payload)
        || tree::handle(on_event_tx, channel, payload)
        || views::handle(on_event_tx, channel, payload)
        || shell::handle(on_event_tx, channel, payload)
    {
        return;
    }
    let _ = on_event_tx.try_send(ShellEvent::HostEvent(channel.into(), payload.clone()));
}
