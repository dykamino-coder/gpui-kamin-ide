//! Граф удержания кучи стенда (docs/memprof.md, «Who keeps a block alive»):
//! кто держит живой блок сейчас, а не где его выделили. Шим аллокатора ведёт
//! таблицу живых блоков с запуска процесса; после отчёта владельцев первого
//! режима (`holders::attach`) поток документа сканирует каждый живой блок и
//! корни (`roots`) и пишет граф с кратчайшими путями и доминаторами
//! (`kamin_heap::retain`) в файл для `scripts/memprof.py --retainers`.
//!
//! `ENGINE_BENCH_RETAINERS_OUT=<файл>` включает учёт; без переменной шим
//! делает одно чтение флага и ничего не выделяет.
//! `ENGINE_BENCH_RETAINERS_SLOTS=<k>` — таблица на `2^k` слотов по 16 байт
//! (по умолчанию 23: ~6 млн живых блоков); переполнение выключает учёт, и
//! снимок сообщает об ошибке вместо неполного графа.
//!
//! Скан читает блоки, пока другие потоки стенда могут работать; их стеки и
//! память ОС не сканируются, такие держатели видны как вход «без корня».

mod roots;
mod table;

use kamin_heap::retain::{Block, FLAG_CYCLE, FLAG_UNHELD, Graph, Memory};
use serde_json::{Value, json};
use std::{
    path::PathBuf,
    sync::{
        OnceLock,
        atomic::{AtomicBool, Ordering::Relaxed},
    },
    time::Instant,
};

const OUT: &str = "ENGINE_BENCH_RETAINERS_OUT";
const SLOTS: &str = "ENGINE_BENCH_RETAINERS_SLOTS";

static PATH: OnceLock<PathBuf> = OnceLock::new();
static TAKEN: AtomicBool = AtomicBool::new(false);

/// Читает окружение до первой аллокации стенда и включает таблицу.
pub(crate) fn init() -> Result<(), String> {
    let Some(path) = std::env::var_os(OUT) else {
        return Ok(());
    };
    let bits = match std::env::var(SLOTS) {
        Ok(text) => text
            .parse::<u32>()
            .ok()
            .filter(|bits| (16..=30).contains(bits))
            .ok_or_else(|| format!("{SLOTS}={text}: expected 16..30"))?,
        Err(_) => 23,
    };
    table::open(bits)?;
    let _ = PATH.set(path.into());
    Ok(())
}

/// Выделен блок: при выключенном учёте — одно чтение флага.
#[inline]
pub(super) fn track(pointer: *const u8, bytes: usize) {
    if table::tracking() {
        table::insert(pointer as usize, bytes);
    }
}
/// Блок `pointer` освобождается: до возврата аллокатору.
#[inline]
pub(super) fn untrack(pointer: *const u8) {
    if table::tracking() {
        table::remove(pointer as usize);
    }
}

/// Память живых блоков процесса.
struct Process;

impl Memory for Process {
    fn words(&self, address: usize, bytes: usize) -> &[usize] {
        // SAFETY: адрес и размер — живой блок таблицы или область корней;
        // блоки аллокатора выровнены не меньше чем по слову.
        unsafe { std::slice::from_raw_parts(address as *const usize, bytes / size_of::<usize>()) }
    }
}

fn ms(since: Instant) -> f64 {
    (since.elapsed().as_secs_f64() * 1e4).round() / 10.0
}

/// Снимок графа один раз за процесс: сводка для записи режима в JSON стенда.
pub(crate) fn snapshot(mode: &str) -> Option<Value> {
    let path = PATH.get()?;
    if TAKEN.swap(true, Relaxed) {
        return None;
    }
    if table::overflowed() {
        return Some(json!({"error": format!(
            "the live block table overflowed; raise {SLOTS} (now {} slots)", table::capacity())}));
    }
    let started = Instant::now();
    let mut count = 0usize;
    table::each(|_, _| count += 1);
    // Сам список — живой блок таблицы со всеми адресами; в граф он не входит.
    let mut blocks: Vec<Block> = Vec::with_capacity(count + count / 8 + 4096);
    let own = blocks.as_ptr() as usize;
    let mut missed = 0usize;
    table::each(|address, bytes| {
        if address == own {
            return;
        }
        if blocks.len() < blocks.capacity() {
            blocks.push(Block { address, bytes });
        } else {
            missed += 1;
        }
    });
    let collect = ms(started);
    let marker = 0usize;
    let regions = roots::regions(std::hint::black_box(&marker) as *const usize as usize);
    let scanned = Instant::now();
    let mut graph = Graph::scan(blocks, regions, &Process);
    let scan = ms(scanned);
    let analyzed = Instant::now();
    graph.analyze();
    let analyze = ms(analyzed);
    let mut summary = summary(&graph, mode);
    summary["missed_blocks"] = json!(missed);
    summary["table_slots"] = json!(table::capacity());
    summary["collect_ms"] = json!(collect);
    summary["scan_ms"] = json!(scan);
    summary["analyze_ms"] = json!(analyze);
    let written = Instant::now();
    let result = std::fs::File::create(path).and_then(|file| {
        let mut out = std::io::BufWriter::with_capacity(1 << 20, file);
        graph.write(&mut out, &summary.to_string())?;
        std::io::Write::flush(&mut out)
    });
    summary["write_ms"] = json!(ms(written));
    summary["file"] = json!(path.display().to_string());
    if let Err(error) = result {
        summary["error"] = json!(format!("{}: {error}", path.display()));
    }
    Some(summary)
}

fn summary(graph: &Graph, mode: &str) -> Value {
    let regions: Vec<Value> = graph
        .regions
        .iter()
        .enumerate()
        .map(|(index, region)| {
            json!({"name": region.name, "bytes": region.bytes,
                "retained_bytes": graph.retained[graph.region(index) as usize]})
        })
        .collect();
    let count = |flag| graph.flags.iter().filter(|value| **value == flag).count();
    json!({
        "mode": mode,
        "blocks": graph.blocks.len(),
        "bytes": graph.retained[graph.root() as usize],
        "edges": graph.edge_to.len(),
        "regions": regions,
        "unrooted_retained_bytes": graph.retained[graph.unrooted() as usize],
        "unheld_entries": count(FLAG_UNHELD),
        "cycle_entries": count(FLAG_CYCLE),
        "scope": "live Rust heap blocks, conservative scan of every aligned word; roots: the snapshot thread's stack, the image's writable sections and its TLS",
    })
}
