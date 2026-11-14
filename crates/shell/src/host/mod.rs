//! Связь с хостом kamin-host: типы событий и запросы по WS.
//!
//! Раскладка (`plan/100-refactor-250.md`):
//!
//! * `events` — типы событий UI (`ShellEvent` и спутники), без ввода-вывода;
//! * `connect` — старт сайдкара, WS-подключение, реконнект, ре-резолв вебвью;
//! * `requests_tree` / `requests_search` — запросы данных для дерева и поиска;
//! * `status`, `customize_snapshot`, `customize_manifests` — сбор счётчиков
//!   статус-бара и страниц Customize;
//! * `prefs` — app-prefs хоста (страница Settings и то, что от неё зависит);
//! * `deferred` — билеты отложенных ответов хосту, привязанные к соединению;
//! * `paths` — каталоги данных.
//!
//! Общие слоты (WS-клиент, endpoint, канал событий) живут в `host_link` —
//! он же остаётся публичным фасадом для остального шелла.

pub mod connect;
pub mod customize_manifests;
pub mod customize_snapshot;
pub(crate) mod deferred;
pub mod dialogs;
pub mod events;
pub mod events_cz;
pub mod events_editor;
pub mod events_term;
pub mod events_tree;
pub mod migrate_prod;
pub mod paths;
pub mod prefs;
pub(crate) mod rebind;
pub mod requests_search;
pub mod requests_tree;
pub mod show_message;
pub mod status;
pub(crate) mod view_resolution;
pub mod ws_events;
