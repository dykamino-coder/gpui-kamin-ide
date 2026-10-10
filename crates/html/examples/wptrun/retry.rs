//! Further references of a pair: alternate `rel=match` files and `rel=mismatch` anti-references.

use super::capture_dump::Shot;
use super::frame_metrics::{SEPARATOR, diff};
use super::links::resolve_links;
use super::show::Stage;
use super::{pixel_compare, reference_result};
use gpui::AsyncApp;

/// Не сошлось с первым эталоном — пробуем остальные. Возвращает сошедшийся
/// запасной эталон и его снимок.
pub(super) async fn try_alternates(
    cx: &mut AsyncApp,
    stage: &Stage,
    exact: bool,
    alternates: &[String],
    shots: &[Option<Shot>],
    verdict: &mut String,
) -> Option<(String, Shot)> {
    let passed = move |v: &str| pixel_compare::passed(exact, v);
    let mut matched_alternate = None;
    if (!exact && verdict.parse::<f32>().is_ok_and(|d| d > 0.5))
        || (exact && verdict.starts_with("pixel mismatch"))
    {
        for other in alternates {
            if !std::path::Path::new(other).is_file() {
                continue;
            }
            let html = resolve_links(&std::fs::read_to_string(other).unwrap_or_default(), other);
            // Разделитель — не печатная страница: флаг печати снимается, иначе
            // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
            // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
            kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
            let blank = stage
                .show(cx, SEPARATOR.into(), None, true)
                .await
                .map(|s| s.2);
            let Some(shot) = stage.show(cx, html, blank.clone(), false).await else {
                continue;
            };
            let Some(test_shot) = &shots[0] else { continue };
            if exact {
                let candidate = pixel_compare::verdict(test_shot, &shot);
                if passed(&candidate) {
                    *verdict = candidate;
                    matched_alternate = Some((other.clone(), shot));
                    break;
                }
                continue;
            }
            let a = &test_shot.2;
            let d = diff(a, &shot.2);
            if d < verdict.parse::<f32>().unwrap_or(f32::MAX) {
                *verdict = format!("{d:.2}");
            }
            if d <= 0.5 {
                break;
            }
        }
    }
    matched_alternate
}

/// Анти-эталоны пары; при совпадении с любым `verdict` становится провалом.
#[allow(clippy::too_many_arguments)]
pub(super) async fn check_mismatches(
    cx: &mut AsyncApp,
    stage: &Stage,
    exact: bool,
    mismatches: &[String],
    negative_primary: bool,
    reference: &str,
    shots: &[Option<Shot>],
    verdict: &mut String,
) {
    let passed = move |v: &str| pixel_compare::passed(exact, v);
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
    if verdict.parse::<f32>().is_ok() || (exact && verdict.starts_with("pixel mismatch")) {
        for other in mismatches {
            if negative_primary && reference_result::same_file(other, reference) {
                continue;
            }
            if !std::path::Path::new(other).is_file() {
                *verdict = "анти-эталон не найден".into();
                break;
            }
            // Разделитель — не печатная страница: флаг печати снимается, иначе
            // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
            // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
            kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
            let blank = stage
                .show(cx, SEPARATOR.into(), None, true)
                .await
                .map(|s| s.2);
            let html = resolve_links(&std::fs::read_to_string(other).unwrap_or_default(), other);
            let Some(shot) = stage.show(cx, html, blank.clone(), false).await else {
                *verdict = "снимок анти-эталона не получен".into();
                break;
            };
            let Some(test_shot) = &shots[0] else { continue };
            if exact && pixel_compare::compare(test_shot, &shot).is_none() {
                *verdict = "invalid screenshot dimensions".into();
                break;
            }
            let identical = if exact {
                passed(&pixel_compare::verdict(test_shot, &shot))
            } else {
                diff(&test_shot.2, &shot.2) <= 0.5
            };
            if identical {
                *verdict = "совпал с анти-эталоном".into();
                break;
            }
        }
    }
}
