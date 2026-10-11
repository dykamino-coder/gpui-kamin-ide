//! One pair's captures and its primary verdict against the first reference.

use super::capture_dump::Shot;
use super::frame_metrics::{INK_MIN, SEPARATOR, diff, ink, red_pixels};
use super::links::resolve_links;
use super::show::Stage;
use super::{pixel_compare, reference_result};
use gpui::AsyncApp;

/// Снимки теста и эталона (в этом порядке) и последний кадр разделителя.
pub(super) async fn capture_sides(
    cx: &mut AsyncApp,
    stage: &Stage,
    test: &str,
    reference: &str,
) -> (Vec<Option<Shot>>, Option<Vec<u8>>) {
    let mut shots = vec![];
    // Разделитель между страницами: без него «тест совпал с
    // эталоном» неотличимо от «ни один не перерисовался».
    // Разделитель обязан быть РОВНЫМ: если он сам не нарисовался,
    // «сменился» дальше считается от чужого кадра, и страница
    // принимается по устаревшему снимку. В числах это скачок вида
    // 17.45 % → 0.00 % между двумя одинаковыми прогонами.
    let mut blank = None;
    for path in [test, reference] {
        // Разделитель показывается перед КАЖДОЙ стороной, а не один
        // раз на пару. Иначе эталон снимается, пока на экране ещё
        // устоявшийся ТЕСТ: его кадр отличается от разделителя, а
        // значит принимается за кадр эталона — и пара получает
        // ложный ноль расхождения.
        // Разделитель — не печатная страница: флаг печати снимается, иначе
        // он рисуется стопкой листов, `flat()` его не узнаёт и `show`
        // выжидает все 400 шагов (по 6 с на каждый показ разделителя).
        kamin_html::css::PRINT_MEDIA.store(false, std::sync::atomic::Ordering::Relaxed);
        blank = stage
            .show(cx, SEPARATOR.into(), None, true)
            .await
            .map(|s| s.2);
        // Печатные пары смотрят печатным носителем: `@media print`
        // истинен, `screen` — ложен (background-image-only-for-print).
        // Печатность — свойство ПАРЫ (WPT: `-print` в имени ТЕСТА,
        // эталон печатается тем же носителем): `page-name-001-print`
        // против `page-name-001-ref.html` иначе сравнивал стопку
        // листов с экранным документом.
        let print = |p: &str| p.contains("-print.") || p.contains("-print-ref");
        kamin_html::css::PRINT_MEDIA.store(
            print(test) || print(path),
            std::sync::atomic::Ordering::Relaxed,
        );
        let html = resolve_links(&std::fs::read_to_string(path).unwrap_or_default(), path);
        let mut shot = stage.show(cx, html.clone(), blank.clone(), false).await;
        // Страница, не отличившаяся от разделителя, не нарисовалась:
        // повторяем показ, а не записываем пустоту в отчёт.
        for _ in 0..2 {
            // Мерой служат ЧЕРНИЛА, а не побайтовое отличие: кадр с
            // одной кромкой уже «не равен» разделителю, но рисовать
            // на нём нечего. Страница, которая рисоваться умеет,
            // получает ещё две попытки — пустой вердикт должен
            // означать «действительно пусто», а не «не успела».
            let drew = shot
                .as_ref()
                .is_some_and(|s| ink(&s.2, blank.as_ref()) >= INK_MIN);
            // Повтор нужен странице, которая НЕ УСПЕЛА, а не той,
            // что устоялась пустой: `b` в следе означает 120 равных
            // разделителю кадров подряд — рисовать там нечего, и
            // ещё два показа только жгут по две секунды каждый
            // (css-backgrounds: 23 с на пару вместо 0.6 с).
            let settled_blank = stage.trace.borrow().ends_with('b');
            if drew || settled_blank {
                break;
            }
            shot = stage.show(cx, html.clone(), blank.clone(), false).await;
        }
        shots.push(shot);
    }
    (shots, blank)
}

/// Вердикт пары по первому эталону.
pub(super) fn primary_verdict(
    shots: &[Option<Shot>],
    blank: Option<&Vec<u8>>,
    test: &str,
    exact: bool,
    negative_primary: bool,
) -> String {
    // Тест сам сказал, чего быть не должно: «no red». Проверяем
    // это ДО сравнения с эталоном — оно слепо к случаю, когда обе
    // стороны сломаны одинаково.
    let source = std::fs::read_to_string(test).unwrap_or_default();
    let forbids_red = source.to_ascii_lowercase().contains("no red");
    let red_seen = forbids_red
        && shots[0]
            .as_ref()
            .is_some_and(|s| red_pixels(&s.2) > s.2.len() / 4 / 2000);
    let primary_relation = if negative_primary {
        reference_result::Relation::Mismatch
    } else {
        reference_result::Relation::Match
    };
    match (&shots[0], &shots[1]) {
        _ if red_seen => "красное видно".into(),
        _ if negative_primary && !exact => "negative references require exact comparison".into(),
        // Пустая страница совпадает с разделителем, и такая пара
        // дала бы ложный ноль. Это не «сошлось», это «нечего
        // сравнивать»: страница не нарисовалась вовсе.
        // Белый экран у ЛЮБОЙ из сторон — ошибка. Побайтового
        // равенства разделителю мало: страница с единственной
        // кромкой сглаживания уже «не равна», а рисовать на ней
        // нечего, и пара уходит в зелёные с нулём расхождения.
        // Пусто у любой из сторон — либо одна нарисовала на порядок
        // меньше другой: значит она не отрисовалась, а сравнение
        // ничего не проверяет.
        (Some((_, _, a)), Some((_, _, b)))
            if {
                let (x, y) = (ink(a, blank), ink(b, blank));
                // `ink` отдаёт `usize::MAX`, когда кадр и разделитель
                // разной длины: голое `* 8` переполнялось и роняло
                // задачу стенда (197 HUNG прошлого захода с листом).
                x.min(y) < INK_MIN || x.min(y).saturating_mul(8) < x.max(y)
            } =>
        {
            format!("пустая страница {}/{}", ink(a, blank), ink(b, blank))
        }
        (Some(a), Some(b)) if exact => reference_result::verdict(a, b, primary_relation),
        (Some((_, _, a)), Some((_, _, b))) => {
            let d = diff(a, b);
            // Тест сам объявил допуск (`meta name=fuzzy`) — часть
            // протокола reftest: расхождение в пределах названного
            // ЧИСЛА точек не провал. Процент переводится в точки
            // по размеру снимка.
            let allowed = pixel_compare::legacy_fuzzy_pixels(&source);
            let points = (d as f64 / 400.0 * a.len() as f64) as usize;
            if allowed > 0 && points <= allowed {
                "0.00".to_string()
            } else {
                format!("{d:.2}")
            }
        }
        _ => "снимок не получен".into(),
    }
}
