//! Реестр bounds для kamin-probe (plan/96 §2.1): каждый трекаемый регион
//! кладёт свои экранные границы на каждый prepaint через canvas-хук
//! (приём Zed для снятия bounds). Читается probe-потоком без прыжка в UI.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use gpui::prelude::*;
use gpui::{Bounds, Pixels, canvas};
use serde_json::{Value, json};

static REGISTRY: LazyLock<Mutex<HashMap<&'static str, [f32; 4]>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Когда регион последний раз ПОКРАСИЛСЯ (paint-фаза, не prepaint).
/// Диагностика «карта пропала»: prepaint резервирует место и пишет bounds,
/// а вот пропуск paint-фазы иначе снаружи не виден (жалоба «нижняя карта
/// пропадает» — на скрине место есть, пикселей нет).
static PAINTED: LazyLock<Mutex<HashMap<&'static str, std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// Когда регион последний раз проходил prepaint (место в раскладке есть).
static PREPAINTED: LazyLock<Mutex<HashMap<&'static str, std::time::Instant>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

fn record(id: &'static str, bounds: Bounds<Pixels>) {
    let rect = [
        f32::from(bounds.origin.x),
        f32::from(bounds.origin.y),
        f32::from(bounds.size.width),
        f32::from(bounds.size.height),
    ];
    REGISTRY.lock().unwrap().insert(id, rect);
    PREPAINTED
        .lock()
        .unwrap()
        .insert(id, std::time::Instant::now());
}

/// Сторож «место есть — краски нет»: карта прошла prepaint только что, а
/// paint-фазу не проходила дольше 3 с. Ловит жалобу «нижняя карта пропадает
/// при определённых размерах» (скрин: ручка и место на месте, пикселей нет).
/// Зовётся раз в секунду из насоса CEF; лог одноразовый на эпизод.
pub fn paint_watchdog() {
    use std::sync::atomic::{AtomicBool, Ordering};
    static IN_EPISODE: AtomicBool = AtomicBool::new(false);
    const CARDS: [&str; 5] = [
        "central-bottom",
        "right-top",
        "right-bottom",
        "main",
        "sidebar",
    ];
    let pre = PREPAINTED.lock().unwrap();
    let painted = PAINTED.lock().unwrap();
    let mut stuck: Vec<String> = Vec::new();
    for id in CARDS {
        let fresh = pre.get(id).is_some_and(|t| t.elapsed().as_millis() < 1000);
        if !fresh {
            continue;
        }
        let stale = painted.get(id).is_none_or(|t| t.elapsed().as_secs() >= 3);
        if stale {
            let b = REGISTRY.lock().unwrap().get(id).copied();
            stuck.push(format!("{id} bounds={b:?}"));
        }
    }
    if stuck.is_empty() {
        IN_EPISODE.store(false, Ordering::Relaxed);
    } else if !IN_EPISODE.swap(true, Ordering::Relaxed) {
        eprintln!(
            "[paint-watchdog] место есть, paint не идёт: {}",
            stuck.join("; ")
        );
    }
}

/// Невидимый absolute-элемент на всю площадь родителя: пишет bounds
/// родителя в реестр. Вставлять `.child(probe_area("id"))` в трекаемый div.
pub fn probe_area(id: &'static str) -> impl IntoElement {
    canvas(
        move |bounds, _window, _cx| record(id, bounds),
        move |_bounds, _state, _window, _cx| {
            PAINTED
                .lock()
                .unwrap()
                .insert(id, std::time::Instant::now());
        },
    )
    // `inset: 0`, а НЕ `size_full`: канвас с `size_full` gpui считает
    // участником flex-строки и он отжимает соседей (замер: бейдж-счётчик
    // группы уезжал влево, как только строка становилась hovered).
    // С явными инсетами бокс — честно абсолютный и места не занимает.
    .absolute()
    .top_0()
    .left_0()
    .right_0()
    .bottom_0()
}

/// probe `tree`: все известные регионы {id: {x,y,w,h}}.
pub fn snapshot() -> Value {
    let map = REGISTRY.lock().unwrap();
    let mut out = serde_json::Map::new();
    for (id, [x, y, w, h]) in map.iter() {
        out.insert((*id).to_string(), json!({"x": x, "y": y, "w": w, "h": h}));
    }
    Value::Object(out)
}

/// probe `metric`: один регион по id. `paint_age_ms` — сколько мс назад
/// регион красился paint-фазой; большой возраст при свежих bounds = «место
/// зарезервировано, а карта не рисуется» (диагностика пропажи панелей).
pub fn metric(id: &str) -> Option<Value> {
    let map = REGISTRY.lock().unwrap();
    let painted = PAINTED.lock().unwrap();
    map.iter()
        .find(|(k, _)| **k == id)
        .map(|(k, [x, y, w, h])| {
            let age = painted.get(*k).map(|t| t.elapsed().as_millis() as u64);
            json!({"x": x, "y": y, "w": w, "h": h, "paint_age_ms": age})
        })
}

/// Прямой доступ к bounds региона (лог. px) — hit-test dnd тулзов.
pub fn bounds_of(id: &str) -> Option<[f32; 4]> {
    REGISTRY.lock().unwrap().get(id).copied()
}

/// Попадает ли точка в регион.
pub fn hit(id: &str, x: f32, y: f32) -> bool {
    bounds_of(id).is_some_and(|[rx, ry, rw, rh]| x >= rx && x < rx + rw && y >= ry && y < ry + rh)
}

/// Список активных оверлей-стейтов последнего кадра overlay-окна
/// (диагностика «залипшего квадрата»: probe cmd "overlay" читает).
static OVERLAY_STATES: LazyLock<Mutex<Vec<&'static str>>> =
    LazyLock::new(|| Mutex::new(Vec::new()));

pub fn set_overlay_states(states: Vec<&'static str>) {
    *OVERLAY_STATES.lock().unwrap() = states;
}

pub fn overlay_states() -> Vec<&'static str> {
    OVERLAY_STATES.lock().unwrap().clone()
}

/// Снимок выделения дерева для probe: (выбранные пути, якорь Shift).
/// Замер ширины строки шейпером: probe кладёт запрос, кадр считает, probe
/// читает результат. Пиксельный поиск по кадру трижды находил соседнюю
/// строку — точную ширину даёт только сам шейпер (ц.32).
pub struct ShapeReq {
    pub text: String,
    pub size: f32,
    pub weight: f32,
    pub spacing: f32,
    pub mono: bool,
}

static SHAPE_REQ: std::sync::LazyLock<std::sync::Mutex<Option<ShapeReq>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));
static SHAPE_RES: std::sync::LazyLock<std::sync::Mutex<Option<(String, f32)>>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new(None));

pub fn request_shape(req: ShapeReq) {
    *SHAPE_REQ.lock().unwrap() = Some(req);
    *SHAPE_RES.lock().unwrap() = None;
}

pub fn take_shape_request() -> Option<ShapeReq> {
    SHAPE_REQ.lock().unwrap().take()
}

pub fn record_shape(text: String, width: f32) {
    *SHAPE_RES.lock().unwrap() = Some((text, width));
}

pub fn shape_result() -> Option<(String, f32)> {
    SHAPE_RES.lock().unwrap().clone()
}

static TREE_SEL: std::sync::LazyLock<std::sync::Mutex<(Vec<String>, Option<String>)>> =
    std::sync::LazyLock::new(|| std::sync::Mutex::new((Vec::new(), None)));

pub fn record_tree_selection(sel: Vec<String>, anchor: Option<String>) {
    *TREE_SEL.lock().unwrap() = (sel, anchor);
}

pub fn tree_selection() -> (Vec<String>, Option<String>) {
    TREE_SEL.lock().unwrap().clone()
}
