//! Память по владельцам после кадра замера (`ENGINE_BENCH_MEMORY_REPORT=1`,
//! docs/memprof.md): документ и окно GPUI меряют блоки кучи, которые держат.
//! Сумма сверяется со счётчиком аудита; разница — неразмеченная куча. С
//! пробником стеков каждый измеренный блок пишется в его файл строкой `o`,
//! и `scripts/memprof.py` показывает неразмеченные живые блоки по местам.

use super::view::View;
use gpui::{App, AsyncApp, WindowHandle};
use kamin_heap::Ops;
use serde_json::{Value, json};
use std::sync::OnceLock;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

/// Включён ли отчёт: переменная окружения читается один раз.
fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("ENGINE_BENCH_MEMORY_REPORT").is_some_and(|v| v == "1"))
}

/// Отчёт сейчас: владельцы, их сумма и счётчик живой кучи до обхода.
fn report(doc: &kamin_html::Document, window: &gpui::Window, cx: &App) -> Value {
    let live = super::allocations::Snapshot::read().live;
    let mut sink = |path: &'static str, pointer: *const u8, bytes: usize| {
        #[cfg(all(windows, any(feature = "allocation-audit", feature = "heap-profile")))]
        super::allocations::probe::owned(path, pointer, bytes);
        #[cfg(not(all(windows, any(feature = "allocation-audit", feature = "heap-profile"))))]
        let _ = (path, pointer, bytes);
    };
    let mut ops = Ops::with_sink(&mut sink);
    doc.heap_report(&mut ops);
    // Блок, который документ уже назвал (раскладка строки в его замерах), не
    // считается второй раз.
    window.heap_report(cx, &mut |path, pointer, bytes| {
        if pointer.is_null() || ops.once(pointer) {
            ops.at(path, |ops| ops.block(pointer, bytes));
        }
    });
    let owners = ops.report();
    drop(ops);
    let gpu = gpu(window);
    let reported = owners.total();
    let audit = cfg!(feature = "allocation-audit");
    json!({
        "owners": owners.entries(),
        "reported_bytes": reported,
        "audit_live_bytes": audit.then_some(live),
        "unclassified_bytes": audit.then(|| live as i64 - reported as i64),
        "gpu": gpu,
        "scope": "Rust heap held by the document and the GPUI window after the frame; requested bytes, shared Rc blocks counted once",
    })
}

/// Память GPU окна по владельцам: текстуры атласов и целей отрисовки. Это не
/// куча: в сумму владельцев и сверку со счётчиком аудита не входит.
fn gpu(window: &gpui::Window) -> Value {
    // путь, байты, из них занято, число текстур
    let mut owners: Vec<(&'static str, u64, u64, u64)> = Vec::new();
    window.gpu_memory(&mut |path, bytes, used| {
        #[cfg(all(windows, any(feature = "allocation-audit", feature = "heap-profile")))]
        super::allocations::probe::gpu(path, bytes, used);
        match owners.iter_mut().find(|owner| owner.0 == path) {
            Some(owner) => {
                owner.1 += bytes;
                owner.2 += used;
                owner.3 += 1;
            }
            None => owners.push((path, bytes, used, 1)),
        }
    });
    let entries: Vec<Value> = owners
        .iter()
        .map(|&(path, bytes, used, textures)| {
            json!({"path": path, "bytes": bytes, "used_bytes": used, "textures": textures})
        })
        .collect();
    json!({
        "owners": entries,
        "bytes": owners.iter().map(|owner| owner.1).sum::<u64>(),
        "scope": "textures the window's renderer holds, by their descriptions; not the driver's reservation",
    })
}

/// Отчёт после последнего кадра режима — в его запись `memory_owners`.
pub(super) fn attach(
    cx: &mut AsyncApp,
    window: WindowHandle<View>,
    mode: &mut Value,
) -> Result<()> {
    if enabled() {
        mode["memory_owners"] =
            window.update(cx, |view, window, cx| report(&view.doc, window, cx))?;
    }
    Ok(())
}
