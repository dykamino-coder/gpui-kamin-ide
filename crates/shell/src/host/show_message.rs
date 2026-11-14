//! `shell.showMessage(severity, message, items?, options?)` → тост стека.
//!
//! Тост с кнопками по умолчанию ждёт решения (VS Code: уведомление с actions
//! висит до выбора) — так обязаны вести себя approval и elicitation. Явный
//! `options.transientMs` (API `kaminide.window.showTransientMessage`) — отказ
//! от ожидания для уведомлений, где кнопка лишь ярлык: «Session finished —
//! Open» сам закрывается по каунтдауну, а запрос хоста завершается `null`
//! при снятии, как при dismiss (BR-23). Без items ответ уходит сразу.

use serde_json::Value;

use crate::ui::toasts::Toast;

/// Границы заказанного каунтдауна: меньше секунды тост не прочесть, больше
/// минуты — это уже не transient, а забытый sticky.
pub const TRANSIENT_MIN_MS: u32 = 1_000;
pub const TRANSIENT_MAX_MS: u32 = 60_000;

/// Есть ли у сообщения кнопки: тогда ответ хосту откладывается до решения.
pub fn has_actions(params: &[Value]) -> bool {
    params
        .get(2)
        .and_then(Value::as_array)
        .is_some_and(|a| a.iter().any(Value::is_string))
}

/// Тост и признак отложенного ответа (`true` — ответ даст снятие тоста).
/// `req_id` — билет отложенного ответа (`host::deferred`), а не id хоста.
pub fn toast_for(req_id: u64, params: &[Value]) -> (Toast, bool) {
    let severity = params.first().and_then(Value::as_str).unwrap_or("info");
    let message = params
        .get(1)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let items: Vec<String> = params
        .get(2)
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .filter_map(Value::as_str)
                .map(String::from)
                .collect()
        })
        .unwrap_or_default();
    let transient = params
        .get(3)
        .and_then(|o| o.get("transientMs"))
        .and_then(Value::as_f64)
        .filter(|ms| ms.is_finite() && *ms > 0.0)
        .map(|ms| (ms.round() as u32).clamp(TRANSIENT_MIN_MS, TRANSIENT_MAX_MS));
    let has_items = !items.is_empty();
    let toast = Toast {
        id: if has_items {
            format!("shellreq-{req_id}")
        } else {
            format!("shellmsg-{severity}-{message:.32}")
        },
        severity: severity.to_string(),
        title: None,
        message,
        actions: items,
        // ждём выбора — не авто-скрывать, если не заказан transient
        sticky: has_items && transient.is_none(),
        duration_ms: transient,
    };
    (toast, has_items)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn params(v: Value) -> Vec<Value> {
        v.as_array().cloned().unwrap_or_default()
    }

    #[test]
    fn кнопки_без_опций_ждут_решения() {
        let (t, deferred) = toast_for(3, &params(json!(["info", "Approve?", ["Yes", "No"]])));
        assert!(t.sticky);
        assert!(deferred);
        assert_eq!(t.duration_ms, None);
        assert_eq!(t.id, "shellreq-3");
    }

    #[test]
    fn transient_с_кнопкой_закрывается_сам_и_ждёт_ответа_снятием() {
        let p = params(json!(["info", "Session finished", ["Open"], { "transientMs": 8000 }]));
        let (t, deferred) = toast_for(9, &p);
        assert!(!t.sticky, "transient toast must count down");
        assert!(deferred, "the host request settles on click or on removal");
        assert_eq!(t.duration_ms, Some(8000));
        assert_eq!(t.life_ms(), 8000);
        assert_eq!(t.id, "shellreq-9");
    }

    #[test]
    fn длительность_без_кнопок_не_теряется() {
        let (t, deferred) = toast_for(
            1,
            &params(json!(["info", "Busy", [], { "transientMs": 6000 }])),
        );
        assert!(!t.sticky);
        assert!(!deferred);
        assert_eq!(t.life_ms(), 6000);
    }

    #[test]
    fn длительность_ограничена() {
        let short = toast_for(1, &params(json!(["info", "a", [], { "transientMs": 10 }]))).0;
        let long = toast_for(
            1,
            &params(json!(["info", "a", [], { "transientMs": 9.0e9 }])),
        )
        .0;
        let bad = toast_for(
            1,
            &params(json!(["info", "a", ["Open"], { "transientMs": -5 }])),
        )
        .0;
        assert_eq!(short.life_ms(), TRANSIENT_MIN_MS);
        assert_eq!(long.life_ms(), TRANSIENT_MAX_MS);
        assert!(bad.sticky, "an invalid duration keeps the waiting default");
    }

    #[test]
    fn без_опций_прежний_каунтдаун() {
        let (t, _) = toast_for(1, &params(json!(["warning", "x"])));
        assert_eq!(t.life_ms(), crate::ui::toasts::TOAST_MS);
    }
}
