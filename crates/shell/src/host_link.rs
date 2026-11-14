//! Связка шелл ↔ kamin-host: сайдкар-супервизор + WS-клиент на std-потоках,
//! события в GPUI — через smol-канал (foreground-цикл в main.rs).
//! Реконнект: on_disconnect ждёт RETRY_DELAY_MS и переподключается к
//! последнему endpoint из HostState (hostEndpoint.get-семантика host-rpc.ts).

//!
//! Сам модуль после разбора (`plan/100-refactor-250.md`) держит только общие
//! слоты (WS-клиент, endpoint, канал событий) и остаётся ФАСАДОМ: запросы
//! живут в `crate::host::*`, а здесь реэкспортируются под прежними именами —
//! места вызова не изменились.

pub use crate::host::connect::{KNOWN_WEBVIEWS, register_dynamic_webview, resolve_webview, start};
pub use crate::host::events::{CzContainer, FsUndo, HoverPillSource, ShellEvent};
pub use crate::host::paths::data_dirs;
pub use crate::host::prefs::request_app_prefs;
pub use crate::host::requests_search::{
    request_commands, request_find_in_files, request_read_file, request_workspace_symbols,
};
pub use crate::host::requests_tree::{
    report_tree, request_decorations, request_extension_icon, request_find_file, request_list_dir,
    request_tree_children, request_tree_dnd, request_tree_meta,
};
pub use crate::host::status::request_status;

/// Замер старта (диагностика «долгой» отрисовки вью).
pub fn t0() -> std::time::Instant {
    static T: std::sync::OnceLock<std::time::Instant> = std::sync::OnceLock::new();
    *T.get_or_init(std::time::Instant::now)
}

use std::sync::{Arc, Mutex, OnceLock};

use kamin_sidecar::HostEndpoint;
use kamin_ws::WsClient;
use smol::channel::Sender;

/// Канал UI-событий (для probe-инъекций оверлеев; dev-инструмент).
pub(crate) static EVENT_TX: OnceLock<Sender<ShellEvent>> = OnceLock::new();

/// Клон канала событий UI (probe emit). None до host_link::start.
#[allow(dead_code)]
pub fn event_tx() -> Option<Sender<ShellEvent>> {
    EVENT_TX.get().cloned()
}

/// Текущее соединение: его поколение и WS-клиент. Поколение отличает
/// отложенные ответы разных соединений (`host::deferred`, BR-19A).
pub(crate) static CLIENT: OnceLock<Mutex<Option<Link>>> = OnceLock::new();

/// Поколение соединения и его WS-клиент.
pub(crate) type Link = (u64, Arc<WsClient>);

pub(crate) fn client_slot() -> &'static Mutex<Option<Link>> {
    CLIENT.get_or_init(|| Mutex::new(None))
}

/// Текущий WS-клиент (для запросов из UI-действий).
pub fn client() -> Option<Arc<WsClient>> {
    current_link().map(|(_, client)| client)
}

/// Текущее соединение вместе с его поколением.
pub(crate) fn current_link() -> Option<Link> {
    client_slot().lock().unwrap().clone()
}

pub(crate) static ENDPOINT: OnceLock<Mutex<Option<HostEndpoint>>> = OnceLock::new();

pub(crate) fn endpoint_slot() -> &'static Mutex<Option<HostEndpoint>> {
    ENDPOINT.get_or_init(|| Mutex::new(None))
}

/// Requested views and last HTML, scoped to one extension-host generation.
pub(crate) fn view_resolution() -> &'static Mutex<crate::host::view_resolution::ViewResolution> {
    static STATE: OnceLock<Mutex<crate::host::view_resolution::ViewResolution>> = OnceLock::new();
    STATE.get_or_init(Default::default)
}
