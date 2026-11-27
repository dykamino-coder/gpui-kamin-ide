//! An event-driven DevTools transport; only an external observer waits five seconds.

use futures::{StreamExt, channel::mpsc};
use kamin_html::why::{IdleCpuWindow, WindowJournal};
use notify::{RecommendedWatcher, RecursiveMode, Watcher};
use serde_json::{Value, json};
use std::{error::Error, path::Path};

#[cfg(windows)]
fn creation_time() -> Result<u64, Box<dyn Error>> {
    use windows::Win32::{
        Foundation::FILETIME,
        System::Threading::{GetCurrentProcess, GetProcessTimes},
    };
    let mut creation = FILETIME::default();
    let mut exit = FILETIME::default();
    let mut kernel = FILETIME::default();
    let mut user = FILETIME::default();
    unsafe {
        GetProcessTimes(
            GetCurrentProcess(),
            &mut creation,
            &mut exit,
            &mut kernel,
            &mut user,
        )?;
    }
    Ok((u64::from(creation.dwHighDateTime) << 32) | u64::from(creation.dwLowDateTime))
}

#[cfg(not(windows))]
fn creation_time() -> Result<u64, Box<dyn Error>> {
    Err("idle CPU transport requires Windows process counters".into())
}

pub async fn serve(journal: &WindowJournal, directory: &Path) -> Result<(), Box<dyn Error>> {
    let directory = directory.canonicalize()?;
    let pid = std::process::id();
    let creation = creation_time()?;
    let command = directory.join("idle-control.json");
    let reply = directory.join("idle-reply.json");
    let watched_command = command.clone();
    let (mut sender, mut receiver) = mpsc::channel(1);
    let mut watcher = RecommendedWatcher::new(
        move |event: notify::Result<notify::Event>| {
            if event.is_ok_and(|event| {
                !matches!(event.kind, notify::EventKind::Access(_))
                    && event.paths.iter().any(|path| path == &watched_command)
            }) {
                let _ = sender.try_send(());
            }
        },
        notify::Config::default(),
    )?;
    watcher.watch(&directory, RecursiveMode::NonRecursive)?;
    let write_reply = |id: &str, state: &str| -> Result<(), Box<dyn Error>> {
        std::fs::write(
            &reply,
            serde_json::to_vec(&json!({
                "schema": 1, "pid": pid, "creation_100ns": creation, "id": id, "state": state,
            }))?,
        )?;
        Ok(())
    };
    write_reply("", "ready")?;
    let mut last = None;
    let mut active = None;
    while receiver.next().await.is_some() {
        // The sender publishes through a rename, never a partially written JSON file.
        let value: Value = serde_json::from_slice(&std::fs::read(&command)?)?;
        let id = value["id"].as_str().ok_or("idle command has no id")?;
        let op = value["op"]
            .as_str()
            .ok_or("idle command has no operation")?;
        let identity = (id.to_owned(), op.to_owned());
        if last.as_ref() == Some(&identity) {
            continue;
        }
        last = Some(identity);
        match op {
            "begin" => {
                journal.begin_idle();
                active = Some(id.to_owned());
                write_reply(id, "observing")?;
            }
            "finish" if active.as_deref() == Some(id) => {
                journal.finish_idle(IdleCpuWindow::from_json(
                    value["cpu_raw"].clone(),
                    pid,
                    creation,
                )?)?;
                journal.write(&directory)?;
                active = None;
                write_reply(id, "complete")?;
            }
            "quit" => return Ok(()),
            _ => return Err("idle operation has no matching observation".into()),
        }
    }
    Ok(())
}
