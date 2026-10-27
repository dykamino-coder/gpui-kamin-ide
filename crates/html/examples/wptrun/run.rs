//! The pair loop: park the window, show every pair, write the report, timing and slow lists.

use super::frame_metrics::rss_mb;
use super::reftest_meta::{out_of_scope, references};
use super::show::Stage;
use super::{
    animation_frames, capture, capture_dump, capture_name, pair, pixel_compare, reference_result,
    retry,
};
use gpui::{AnyWindowHandle, AppContext as _, AsyncApp};
use std::rc::Rc;

/// Что и как прогонять.
pub(super) struct RunConfig {
    pub(super) capture: capture::Capture,
    pub(super) pairs: Vec<(String, String)>,
    pub(super) exact: bool,
    /// `WPT_DUMP` задан: снимки разошедшихся пар.
    pub(super) dumping: bool,
    /// `WPT_DUMP=all`: снимки всех пар подряд.
    pub(super) dump_all: bool,
}

/// Указатель окна (HWND) или ноль.
fn hwnd_of(cx: &mut AsyncApp, window: AnyWindowHandle) -> isize {
    cx.update_window(window, |_, window, _| {
        use raw_window_handle::{HasWindowHandle, RawWindowHandle};
        match window.window_handle().map(|h| h.as_raw()) {
            Ok(RawWindowHandle::Win32(handle)) => handle.hwnd.get(),
            _ => 0,
        }
    })
    .unwrap_or(0)
}

pub(super) async fn run(cx: &mut AsyncApp, window: AnyWindowHandle, cfg: RunConfig) {
    let RunConfig {
        capture,
        pairs,
        exact,
        dumping,
        dump_all,
    } = cfg;
    let frames = animation_frames::configured();
    let passed = move |v: &str| pixel_compare::passed(exact, v);
    // Off-screen capture: park the window before its first frame.
    if capture.offscreen {
        let early = hwnd_of(cx, window);
        if early != 0 {
            capture::park_offscreen(early);
        }
    }
    // Первый кадр окна: до него снимок пустой.
    super::ready::startup(cx, &capture).await;
    // Указатель окна нужен снимку: рисует его система, а не мы.
    let hwnd = hwnd_of(cx, window);
    // Свой файл отчёта на шард: параллельные прогоны не бьются за один путь.
    let report_path =
        std::env::var("WPT_REPORT").unwrap_or_else(|_| "target/wpt-report.txt".into());
    let mut report = String::new();
    let mut engine_report = String::new();
    let _ = std::fs::write(format!("{report_path}.engine"), &engine_report);
    let trace = Rc::new(std::cell::RefCell::new(String::new()));
    let stage = Stage {
        capture,
        window,
        hwnd,
        trace: trace.clone(),
        frame_errors: Default::default(),
        document_failure: Default::default(),
        engine_lines: Default::default(),
    };
    // Время на пару: страница, у которой один кадр считается
    // секундами, выглядит на экране долгой пустотой и неотличима от
    // зависшего стенда. В отчёт оно не идёт (там сравниваются числа
    // расхождения), зато сразу видно, что тормозит.
    let mut slow: Vec<(u128, String)> = vec![];
    let mut timing = String::new();
    let mut timing_lines = 0usize;
    for (pair_index, (test, reference)) in pairs.iter().enumerate() {
        let started = std::time::Instant::now();
        if let Some(why) = out_of_scope(test, reference) {
            report.push_str(&format!("{test}|{reference}|{why}\n"));
            let _ = std::fs::write(report_path.as_str(), &report);
            continue;
        }
        let times = animation_frames::for_pair(frames.as_deref(), test, reference);
        let previous = cx.update(|cx| cx.try_global::<gpui::AnimationElapsedTime>().copied());
        let mut first_failure = None;
        let mut final_verdict = String::new();
        for time in times {
            let exact = time.is_some() || exact;
            if let Some(time) = time {
                cx.update(|cx| cx.set_global(gpui::AnimationElapsedTime(time)));
            }
            let (shots, blank) = pair::capture_sides(cx, &stage, test, reference).await;
            let refs = references(test, reference);
            let negative_primary = !refs.has_match
                && refs
                    .mismatches
                    .iter()
                    .any(|path| reference_result::same_file(path, reference));
            let mut verdict =
                pair::primary_verdict(&shots, blank.as_ref(), test, exact, negative_primary);
            let matched_alternate =
                retry::try_alternates(cx, &stage, exact, &refs.alternates, &shots, &mut verdict)
                    .await;
            // ПРОБОВАЛИ И ОТКАТИЛИ: применять вердикт только когда
            // анти-эталон отличается от ЭТАЛОНА больше порога. Замерено:
            // CSS2 +5, CSS3 +4. Но выборочная проверка показала, что
            // правка ПРЯЧЕТ настоящие провалы: у `text-wrap-nowrap-001`
            // анти-эталон отличается настоящим `width: 20ch`, и совпадение
            // эталона с ним значит, что ширину не держим МЫ. Отличить это
            // от честно неразличимого анти-эталона (`selectors/grouping-002`
            // — только цвет двух строк текста) стенд не может, а ослаблять
            // мерило ради счёта нельзя.
            //
            // Анти-эталон (`rel="mismatch"`): совпадение с ним — ПРОВАЛ.
            // Проверяется всегда, даже когда пара уже зелёная: именно
            // зелёная пара и подозрительна — обе стороны могли сломаться
            // одинаково.
            retry::check_mismatches(
                cx,
                &stage,
                exact,
                &refs.mismatches,
                negative_primary,
                reference,
                &shots,
                &mut verdict,
            )
            .await;
            if dumping && (dump_all || !passed(&verdict)) {
                let mut stem = capture_name::stem(pair_index, test);
                if let Some(time) = time {
                    stem.push_str(&format!("--frame-{}ms", time.as_millis()));
                }
                capture_dump::pair(&stem, &shots, matched_alternate.as_ref());
            }
            if let Some(time) = time
                && verdict != "0.00"
                && first_failure.is_none()
            {
                first_failure = Some(format!("frame {}ms: {verdict}", time.as_millis()));
            }
            // Preserve the ordinary verdict for the single-capture path.
            // Animated pairs are exact even in legacy diagnostic mode.
            final_verdict = verdict;
        }
        if frames.is_some() {
            cx.update(|cx| {
                if let Some(previous) = previous {
                    cx.set_global(previous);
                } else if cx.try_global::<gpui::AnimationElapsedTime>().is_some() {
                    cx.remove_global::<gpui::AnimationElapsedTime>();
                }
            });
        }
        let engine_lines = stage.engine_lines.take();
        if !engine_lines.is_empty() {
            for line in engine_lines.lines() {
                let line = format!("{test}|{reference}|{line}\n");
                eprint!("{line}");
                engine_report.push_str(&line);
            }
            let _ = std::fs::write(format!("{report_path}.engine"), &engine_report);
        }
        let errors = stage.frame_errors.take();
        let verdict = if let Some(failure) = stage.document_failure.take() {
            format!(
                "ERROR engine {} {:?} {:?}",
                failure.code(),
                failure.reason,
                failure.report
            )
        } else if errors.is_empty() {
            first_failure.unwrap_or(final_verdict)
        } else {
            format!("ERROR first-frame != settled: {}", errors.join(", "))
        };
        report.push_str(&format!("{test}|{reference}|{verdict}\n"));
        // Отчёт пишется после каждой пары: падение на одном файле не
        // должно стирать результат всего прогона.
        let _ = std::fs::write(report_path.as_str(), &report);
        // Порог — вдвое больше, чем нужно на три страницы с полным
        // опросом кадра. Всё, что дольше, тормозит не из-за ожидания.
        let spent = started.elapsed().as_millis();
        if spent > 8000 {
            slow.push((spent, test.clone()));
        }
        // Замер по КАЖДОЙ паре, не только по медленным. Стенд
        // деградирует на длинном прогоне — страницы перестают отдавать
        // кадр, показ выбирает весь запас опроса, и числа раздела
        // становятся недостоверными. Порог в 8 секунд ловит только
        // хвост; кривая времени и памяти показывает, КОГДА поворот.
        timing.push_str(&format!(
            "{}|{spent}|{}|{}\n",
            timing_lines,
            rss_mb(),
            trace.replace(String::new()),
        ));
        timing_lines += 1;
        let _ = std::fs::write("target/wpt-timing.txt", &timing);
    }
    let _ = std::fs::write(report_path.as_str(), report);
    slow.sort_by_key(|s| std::cmp::Reverse(s.0));
    let mut lines = String::new();
    for (ms, test) in &slow {
        lines.push_str(&format!("{ms}|{test}\n"));
    }
    let _ = std::fs::write("target/wpt-slow.txt", lines);
    eprintln!("медленных пар: {}", slow.len());
    cx.update(|cx| cx.quit());
}
