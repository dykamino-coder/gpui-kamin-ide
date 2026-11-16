//! `kamin-search [--idle-ms N]`: answers ripgrep command lines of the Bridge
//! Grep and Glob tools with what ripgrep 15.2.0 would print, without starting
//! a process per call.
//!
//! Protocol: one JSON object per line on stdin and stdout.
//! - request `{"id":1,"args":[…],"cwd":"…","timeoutMs":N,"maxBytes":N,"maxRecords":N}`;
//! - cancel `{"id":1,"cancel":true}`;
//! - answer `{"id":1,"code":0,"stdoutLen":N,"stderr":"…","truncated":false,"timedOut":false}`
//!   followed by exactly N bytes of stdout (no encoding: large listings would
//!   cost more to escape and parse than to search),
//!   or `{"id":1,"unsupported":"why"}` when the extension must run ripgrep.
//!
//! The process ends when stdin closes (its parent is gone) or after the idle
//! time without a request. Requests run on their own threads; cancel and the
//! deadline stop the walk between files.

mod args;
mod run;
#[cfg(test)]
mod run_diff_tests;

use serde::Deserialize;
use std::collections::HashMap;
use std::io::{BufRead as _, Write as _};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

const DEFAULT_IDLE_MS: u64 = 10 * 60 * 1000;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: u64,
    #[serde(default)]
    cancel: bool,
    #[serde(default)]
    args: Vec<String>,
    #[serde(default)]
    cwd: String,
    timeout_ms: Option<u64>,
    max_bytes: Option<usize>,
    max_records: Option<usize>,
}

type Running = Arc<Mutex<HashMap<u64, Arc<AtomicBool>>>>;

/// Writes the header line and the raw stdout bytes in one piece: stdout is
/// line buffered, and megabytes written through it in pieces cost a system
/// call each.
fn write_answer(out: &Mutex<std::io::Stdout>, header: &serde_json::Value, body: &[u8]) {
    let Ok(mut frame) = serde_json::to_vec(header) else {
        return;
    };
    frame.push(b'\n');
    frame.extend_from_slice(body);
    if let Ok(o) = out.lock() {
        let mut o = o.lock();
        let _ = o.write_all(&frame);
        let _ = o.flush();
    }
}

fn answer(req: &Request, cancelled: &AtomicBool) -> (serde_json::Value, Vec<u8>) {
    let unsupported = |why: String| {
        (
            serde_json::json!({ "id": req.id, "unsupported": why, "stdoutLen": 0 }),
            Vec::new(),
        )
    };
    let q = match args::parse(&req.args) {
        Ok(q) => q,
        Err(why) => return unsupported(why),
    };
    if !q.no_config && config_path_set() {
        return unsupported("RIPGREP_CONFIG_PATH is set".into());
    }
    let control = run::Control {
        cancelled,
        deadline: req
            .timeout_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms)),
        max_bytes: req.max_bytes.unwrap_or(usize::MAX),
        max_records: req.max_records.unwrap_or(0),
    };
    match run::run(&q, &PathBuf::from(&req.cwd), &control) {
        Ok(o) => (
            serde_json::json!({
                "id": req.id,
                "code": o.code,
                "stdoutLen": o.stdout.len(),
                "stderr": o.stderr,
                "truncated": o.truncated,
                "timedOut": o.timed_out,
                "cancelled": o.cancelled,
            }),
            o.stdout,
        ),
        Err(why) => unsupported(why),
    }
}

#[allow(
    clippy::disallowed_methods,
    reason = "a process entry point reads its environment"
)]
fn config_path_set() -> bool {
    std::env::var_os("RIPGREP_CONFIG_PATH").is_some_and(|v| !v.is_empty())
}

fn idle_ms() -> u64 {
    let mut it = std::env::args().skip(1);
    while let Some(a) = it.next() {
        if a == "--idle-ms" {
            return it
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or(DEFAULT_IDLE_MS);
        }
    }
    DEFAULT_IDLE_MS
}

fn now_ms(start: Instant) -> u64 {
    u64::try_from(start.elapsed().as_millis()).unwrap_or(u64::MAX)
}

/// Windows throttles a window-less process that runs in the background
/// (EcoQoS): measured, a search in this long-lived process became 2–4 times
/// slower than in a freshly started `rg.exe` within seconds of starting.
/// Searches answer a waiting user, so this process opts out, as a foreground
/// tool would be treated.
#[cfg(windows)]
fn opt_out_of_power_throttling() {
    use windows::Win32::System::Threading::{
        GetCurrentProcess, PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        PROCESS_POWER_THROTTLING_EXECUTION_SPEED, PROCESS_POWER_THROTTLING_STATE,
        ProcessPowerThrottling, SetProcessInformation,
    };
    let state = PROCESS_POWER_THROTTLING_STATE {
        Version: PROCESS_POWER_THROTTLING_CURRENT_VERSION,
        ControlMask: PROCESS_POWER_THROTTLING_EXECUTION_SPEED,
        StateMask: 0,
    };
    // SAFETY: the pointer and size describe state, which outlives the call.
    let _ = unsafe {
        SetProcessInformation(
            GetCurrentProcess(),
            ProcessPowerThrottling,
            (&raw const state).cast(),
            u32::try_from(std::mem::size_of_val(&state)).unwrap_or(0),
        )
    };
}

#[cfg(not(windows))]
fn opt_out_of_power_throttling() {}

fn main() {
    opt_out_of_power_throttling();
    let out = Arc::new(Mutex::new(std::io::stdout()));
    let running: Running = Arc::new(Mutex::new(HashMap::new()));
    let start = Instant::now();
    let last = Arc::new(AtomicU64::new(0));
    let idle = idle_ms();
    {
        let (running, last) = (running.clone(), last.clone());
        std::thread::spawn(move || {
            loop {
                std::thread::sleep(Duration::from_millis(idle.clamp(50, 60_000) / 2));
                let busy = running.lock().map(|r| !r.is_empty()).unwrap_or(true);
                if !busy && now_ms(start).saturating_sub(last.load(Ordering::SeqCst)) >= idle {
                    std::process::exit(0);
                }
            }
        });
    }
    for line in std::io::stdin().lock().lines() {
        let Ok(line) = line else { break };
        last.store(now_ms(start), Ordering::SeqCst);
        let Ok(req) = serde_json::from_str::<Request>(&line) else {
            continue;
        };
        if req.cancel {
            if let Ok(r) = running.lock()
                && let Some(flag) = r.get(&req.id)
            {
                flag.store(true, Ordering::SeqCst);
            }
            continue;
        }
        let flag = Arc::new(AtomicBool::new(false));
        if let Ok(mut r) = running.lock() {
            r.insert(req.id, flag.clone());
        }
        let (out, running, last) = (out.clone(), running.clone(), last.clone());
        std::thread::spawn(move || {
            let (header, body) = answer(&req, &flag);
            if let Ok(mut r) = running.lock() {
                r.remove(&req.id);
            }
            last.store(now_ms(start), Ordering::SeqCst);
            write_answer(&out, &header, &body);
        });
    }
    // stdin closed: the extension host is gone.
}
