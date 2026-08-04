//! Watcher воркспейса (chokidar-аналог оригинала): notify (ReadDirectoryChangesW)
//! рекурсивно на root; события дебаунсятся 300ms → RefreshTree (перечитка
//! root + раскрытых) + FilesChanged(пути) — reload чистых открытых табов.
//! Смена воркспейса пересоздаёт watcher (старый дропается).

use crate::host::events::EdEvent;
use crate::host::events::TreeEvent;
use std::sync::Mutex;
use std::time::Duration;

use notify::{RecursiveMode, Watcher};
use smol::channel::Sender;

use crate::host_link::ShellEvent;

static WATCHER: Mutex<Option<notify::RecommendedWatcher>> = Mutex::new(None);

/// Следить за `root`; None — остановить.
pub fn watch(root: Option<String>, tx: Sender<ShellEvent>) {
    let mut guard = WATCHER.lock().unwrap();
    *guard = None; // дроп старого (смена сессии/воркспейса)
    let Some(root) = root else { return };

    // Дебаунс: burst событий (git/сборка) → один RefreshTree; пути копятся
    let (dtx, drx) = std::sync::mpsc::channel::<Vec<String>>();
    std::thread::Builder::new()
        .name("kamin-fswatch-debounce".into())
        .spawn(move || {
            while let Ok(first) = drx.recv() {
                let mut paths = first;
                // копим окно 300ms
                while let Ok(more) = drx.recv_timeout(Duration::from_millis(300)) {
                    paths.extend(more);
                }
                paths.sort();
                paths.dedup();
                let _ = tx.try_send(ShellEvent::Ed(EdEvent::FilesChanged(paths)));
                let _ = tx.try_send(ShellEvent::Tree(TreeEvent::RefreshTree));
            }
        })
        .ok();

    let watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
        if let Ok(ev) = res {
            let paths = ev
                .paths
                .iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>();
            let _ = dtx.send(paths);
        }
    });
    match watcher {
        Ok(mut w) => {
            if let Err(e) = w.watch(std::path::Path::new(&root), RecursiveMode::Recursive) {
                eprintln!("fs watch {root}: {e}");
                return;
            }
            *guard = Some(w);
        }
        Err(e) => eprintln!("fs watcher create: {e}"),
    }
}
