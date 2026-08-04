//! Отладочный канал probe: как автотесты и я смотрим на живое окно.
//!
//! * `registry` — границы регионов, снятые на прошлом кадре (кто где лежит);
//! * `host` + `cmds` — приём команд по TCP и их разбор;
//! * `emit*` — искусственные события (открыть меню, показать тост);
//! * `input`, `keys` — синтетические мышь и клавиатура;
//! * `shot` — снимок окна.

#[cfg(feature = "probe")]
pub mod cmds;
pub mod emit;
pub mod emit_tools;
pub mod emit_ui;
pub mod emit_view;
pub mod host;
pub mod input;
pub mod keys;
pub mod registry;
pub mod shot;
