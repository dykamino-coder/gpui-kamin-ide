//! Debug-приёмка мышью прямо в CEF, без OS-фокуса и SetCapture.
//! Координаты — CSS px относительно webview, как в DOM clientX/clientY.

use serde_json::{Value, json};

pub(super) fn handle(req: &Value) -> Value {
    let id = req.get("id").and_then(Value::as_str).unwrap_or("browser");
    let action = req.get("action").and_then(Value::as_str).unwrap_or("");
    let number = |name: &str| req.get(name).and_then(Value::as_f64).unwrap_or(0.0) as f32;
    let flag = |name: &str| req.get(name).and_then(Value::as_bool).unwrap_or(false);
    let button = match req.get("button").and_then(Value::as_str).unwrap_or("left") {
        "left" => gpui::MouseButton::Left,
        "right" => gpui::MouseButton::Right,
        "middle" => gpui::MouseButton::Middle,
        _ => return json!({"ok": false, "err": "button must be left, right or middle"}),
    };
    let held = action == "down" || (action == "move" && flag("held"));
    let mods = crate::web::probe_mouse_modifiers(
        &gpui::Modifiers {
            control: flag("ctrl"),
            shift: flag("shift"),
            alt: flag("alt"),
            ..Default::default()
        },
        held.then_some(button),
    );
    let (x, y) = (number("x"), number("y"));
    match action {
        "down" | "up" => crate::web::probe_mouse_click(id, x, y, button, action == "up", 1, mods),
        "move" => crate::web::probe_mouse_move(id, x, y, mods, false),
        "wheel" => crate::web::probe_mouse_wheel(id, x, y, number("dx"), number("dy"), mods),
        _ => return json!({"ok": false, "err": "action must be down, up, move or wheel"}),
    }
    json!({"ok": true, "delivery": "cef-host"})
}
