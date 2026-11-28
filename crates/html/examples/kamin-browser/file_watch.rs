//! Keep the parent directory watched across atomic file replacements, without polling.

use crate::{app::Browser, paths};
use futures::{StreamExt, channel::mpsc};
use gpui::{Context, Task, Window};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;

pub struct FileWatch {
    directory: PathBuf,
    _watcher: RecommendedWatcher,
    _task: Task<()>,
}

impl Browser {
    pub fn watch_current_file(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let Some(directory) = self.current.as_ref().and_then(|path| path.parent()) else {
            self.file_watch = None;
            return;
        };
        if self
            .file_watch
            .as_ref()
            .is_some_and(|watch| watch.directory == directory)
        {
            return;
        }
        self.file_watch = None;
        let directory = directory.to_path_buf();
        let (mut sender, mut receiver) = mpsc::channel(1);
        let watched = (|| -> notify::Result<_> {
            let mut watcher = RecommendedWatcher::new(
                move |event: notify::Result<notify::Event>| {
                    // Reads can produce access notifications on some backends.
                    // Only content/name changes may cause another read.
                    if event.is_ok_and(|event| !matches!(event.kind, notify::EventKind::Access(_)))
                    {
                        let _ = sender.try_send(());
                    }
                },
                notify::Config::default(),
            )?;
            watcher.watch(&directory, RecursiveMode::NonRecursive)?;
            Ok(watcher)
        })();
        match watched {
            Ok(watcher) => {
                let task = cx.spawn_in(window, async move |this, cx| {
                    while receiver.next().await.is_some() {
                        if this
                            .update_in(cx, |this, window, cx| {
                                if this
                                    .current
                                    .as_ref()
                                    .is_some_and(|path| paths::stamp(path) != this.stamp)
                                {
                                    this.reload(window, cx);
                                }
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                });
                self.file_watch = Some(FileWatch {
                    directory,
                    _watcher: watcher,
                    _task: task,
                });
            }
            Err(error) => eprintln!("Cannot watch {}: {error}", directory.display()),
        }
    }
}
