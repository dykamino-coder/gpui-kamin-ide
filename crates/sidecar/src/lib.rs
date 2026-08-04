//! kamin-sidecar — супервизор kamin-host (порт src-tauri/sidecar.rs, 1:1 логика).
//! Спавнит node-хост (stdio NDJSON), парсит kamin-host:ready → {wsPort,wsToken},
//! Job Object (KILL_ON_JOB_CLOSE), рестарт-супервизор с лимитом 3.
//! Tauri-обвязка заменена колбэком on_endpoint (шелл эмитит своё событие).

mod job;
mod ready;
mod spawn;

pub use ready::HostEndpoint;
pub use spawn::{HostConfig, HostMode};

use std::sync::{Arc, Mutex};
use std::time::Duration;

const MAX_CONSECUTIVE_RESTARTS: u32 = 3;
const RESTART_DELAY: Duration = Duration::from_secs(1);

/// Последний известный endpoint (None до первого ready). Меняется на каждый
/// (ре)спавн: порт эфемерный, токен свежий.
#[derive(Default, Clone)]
pub struct HostState {
    endpoint: Arc<Mutex<Option<HostEndpoint>>>,
}

impl HostState {
    pub fn set(&self, endpoint: HostEndpoint) {
        *self.endpoint.lock().unwrap() = Some(endpoint);
    }

    pub fn snapshot(&self) -> Option<HostEndpoint> {
        self.endpoint.lock().unwrap().clone()
    }
}

/// Запуск супервизора в фоновом потоке. `on_endpoint` дёргается на каждый
/// ready (шелл переподключает WS-клиента — аналог kamin://host-endpoint).
pub fn start(
    config: HostConfig,
    state: HostState,
    on_endpoint: Arc<dyn Fn(HostEndpoint) + Send + Sync>,
) {
    std::thread::Builder::new()
        .name("kamin-sidecar".into())
        .spawn(move || {
            let mut failures: u32 = 0;
            loop {
                let became_ready = spawn::run_once(&config, &state, on_endpoint.as_ref());
                if became_ready {
                    failures = 0;
                } else {
                    failures += 1;
                }
                if failures > MAX_CONSECUTIVE_RESTARTS {
                    eprintln!(
                        "kamin-host failed {failures} times in a row — giving up; restart app"
                    );
                    break;
                }
                std::thread::sleep(RESTART_DELAY);
            }
        })
        .expect("spawn kamin-sidecar thread");
}
