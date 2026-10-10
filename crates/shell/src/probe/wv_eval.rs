//! probe `wveval`: выполнить JS в странице вью и ВЕРНУТЬ результат стенду.
//!
//! `wvjs` только запускает скрипт — ответ страницы снаружи не виден, и
//! приёмка сценариев Bridge (состояние чата, агентов, консоли) была
//! невозможна без чтения DOM. Обратного канала, кроме нашей схемы, у страницы
//! нет, поэтому обёртка шлёт результат POST-ом на `kamin.localhost/__probe`
//! (`web/scheme.rs`), а probe-поток ждёт его по одноразовому номеру.
//! Номер, на который никто не ждёт, отбрасывается: поздний ответ после
//! таймаута не копится в памяти.

use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Condvar, LazyLock, Mutex};
use std::time::{Duration, Instant};

use serde_json::{Value, json};

#[derive(Default)]
struct Slots {
    waiting: HashSet<u64>,
    done: HashMap<u64, Value>,
}

static SLOTS: LazyLock<(Mutex<Slots>, Condvar)> =
    LazyLock::new(|| (Mutex::new(Slots::default()), Condvar::new()));
static NEXT: AtomicU64 = AtomicU64::new(1);

/// Зарезервировать номер ответа: только такие номера принимает [`deliver`].
pub(crate) fn reserve() -> u64 {
    let nonce = NEXT.fetch_add(1, Ordering::Relaxed);
    if let Ok(mut s) = SLOTS.0.lock() {
        s.waiting.insert(nonce);
    }
    nonce
}

/// Тело POST от страницы: `{"nonce":N,"ok":..,"value"|"err":..}`.
pub(crate) fn deliver(body: &str) {
    let Ok(v) = serde_json::from_str::<Value>(body) else {
        return;
    };
    let Some(nonce) = v.get("nonce").and_then(Value::as_u64) else {
        return;
    };
    let (lock, cv) = &*SLOTS;
    if let Ok(mut s) = lock.lock()
        && s.waiting.contains(&nonce)
    {
        s.done.insert(nonce, v);
        cv.notify_all();
    }
}

/// Дождаться ответа на `nonce`. По таймауту номер снимается с ожидания.
pub(crate) fn wait(nonce: u64, timeout: Duration) -> Option<Value> {
    let (lock, cv) = &*SLOTS;
    let deadline = Instant::now() + timeout;
    let mut s = lock.lock().ok()?;
    loop {
        if let Some(v) = s.done.remove(&nonce) {
            s.waiting.remove(&nonce);
            return Some(v);
        }
        let left = deadline.saturating_duration_since(Instant::now());
        if left.is_zero() {
            s.waiting.remove(&nonce);
            return None;
        }
        s = cv.wait_timeout(s, left).ok()?.0;
    }
}

/// Обёртка: `expr` — выражение (может быть Promise), `body` — тело async
/// функции с `return`. Результат сериализует сама страница.
pub(crate) fn wrap(nonce: u64, expr: Option<&str>, body: Option<&str>) -> String {
    let inner = match (expr, body) {
        (Some(e), _) => format!("return ({e});"),
        (None, Some(b)) => b.to_string(),
        (None, None) => "return null;".to_string(),
    };
    format!(
        "(async()=>{{let r;try{{const v=await (async()=>{{{inner}\n}})();\
r={{ok:true,value:v===undefined?null:v}};}}catch(e){{r={{ok:false,err:String(e&&e.stack||e)}};}}\
let b;try{{b=JSON.stringify(Object.assign({{nonce:{nonce}}},r));}}\
catch(e){{b=JSON.stringify({{nonce:{nonce},ok:false,err:'unserializable: '+e}});}}\
fetch('http://kamin.localhost/__probe',{{method:'POST',body:b}});}})();"
    )
}

/// Ответ probe по результату ожидания.
pub(crate) fn reply(view: &str, got: Option<Value>, timeout: Duration) -> Value {
    match got {
        Some(mut v) => {
            if let Some(o) = v.as_object_mut() {
                o.remove("nonce");
                o.insert("view".into(), json!(view));
            }
            v
        }
        None => json!({
            "ok": false,
            "view": view,
            "err": format!("no reply from view within {} ms", timeout.as_millis()),
        }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delivered_value_reaches_waiter() {
        let n = reserve();
        let body = json!({"nonce": n, "ok": true, "value": {"a": 1}}).to_string();
        std::thread::spawn(move || deliver(&body));
        let got = wait(n, Duration::from_secs(5)).expect("reply");
        assert_eq!(got["value"]["a"], 1);
        assert_eq!(reply("chat", Some(got), Duration::ZERO)["view"], "chat");
    }

    #[test]
    fn unknown_or_late_nonce_is_dropped() {
        let n = reserve();
        assert!(wait(n, Duration::from_millis(10)).is_none());
        deliver(&json!({"nonce": n, "ok": true}).to_string());
        deliver(&json!({"nonce": 999_999_999u64, "ok": true}).to_string());
        deliver("not json");
        let s = SLOTS.0.lock().unwrap();
        assert!(!s.done.contains_key(&n));
        assert!(!s.done.contains_key(&999_999_999));
    }

    #[test]
    fn wrapper_posts_nonce_and_result() {
        let js = wrap(7, Some("1+1"), None);
        assert!(js.contains("return (1+1);"));
        assert!(js.contains("nonce:7"));
        assert!(js.contains("kamin.localhost/__probe"));
        assert!(wrap(8, None, Some("return 2;")).contains("return 2;"));
        let timeout = reply("x", None, Duration::from_millis(5));
        assert_eq!(timeout["ok"], false);
    }
}
