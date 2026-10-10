//! Frame metrics: ink against the separator, flat frames, red pixels, legacy diff, memory.

/// Своя рабочая память в мегабайтах — второй след деградации стенда рядом со
/// временем пары. Растёт вместе с ним, если дело в утечке ресурсов окна.
pub(super) fn rss_mb() -> u64 {
    #[cfg(windows)]
    {
        use windows::Win32::System::ProcessStatus::{
            GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS,
        };
        use windows::Win32::System::Threading::GetCurrentProcess;
        let mut counters = PROCESS_MEMORY_COUNTERS {
            cb: std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32,
            ..Default::default()
        };
        unsafe {
            if GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb).is_ok() {
                return (counters.WorkingSetSize / (1024 * 1024)) as u64;
            }
        }
        0
    }
    #[cfg(not(windows))]
    0
}

/// Сколько точек снимка ЯВНО красные.
///
/// Второй признак провала, не зависящий от эталона. Сравнение снимков слепо
/// там, где движок ломает обе стороны одинаково: пара сходится в ноль, а тест
/// провален. Но добрая половина набора прямо пишет «no red», и рисует красное
/// подложкой под зелёным — если красное видно, тест провален независимо от
/// совпадения. Порог по каналам взят с запасом, чтобы не считать красным
/// сглаживание кромок тёмного текста.
pub(super) fn red_pixels(shot: &[u8]) -> usize {
    shot.chunks_exact(4)
        .filter(|p| p[2] > 150 && p[1] < 100 && p[0] < 100)
        .count()
}

/// Ниже этого числа нарисованных точек сторона считается ПУСТОЙ.
///
/// ★ Порог намеренно низкий. Прежняя тысяча объявляла пустыми целые семейства,
/// где обе стороны рисовали ОДИНАКОВО и немного (892/892, 675/675) — страница
/// из десятка знаков Ahem это норма, а не пустота. Пустота — это когда рисовать
/// нечего вовсе; несравнимость ловится отдельно, по РАЗНИЦЕ чернил сторон.
pub(super) const INK_MIN: usize = 40;

/// Разделитель между страницами. Он РОВНЫЙ (все точки одного цвета) — по этому
/// признаку стенд понимает, что прошлая страница уже стёрта. Цвет заметный, а
/// не белый: окно стенда висит на экране десятками минут, и белизна неотличима
/// от зависшего стенда — на неё уже дважды жаловались.
pub(super) const SEPARATOR: &str = "<body style=\"background:#cfd8e8;margin:0\"></body>";

pub(super) fn diff(a: &[u8], b: &[u8]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 100.0;
    }
    let mut bad = 0usize;
    for (x, y) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        // Порог тот же, что у стенда против Chrome: сглаживание кромок — не
        // расхождение раскладки.
        if x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40) {
            bad += 1;
        }
    }
    bad as f32 * 400.0 / a.len() as f32
}

/// Сколько точек кадра отличается от разделителя — «чернила» страницы.
///
/// Белый экран — это ОШИБКА, а не совпадение: пара, где обе стороны почти
/// ничего не нарисовали, сходится с нулевым расхождением и уходит в зелёные,
/// хотя сравнивать там нечего.
pub(super) fn ink(shot: &[u8], blank: Option<&Vec<u8>>) -> usize {
    let Some(blank) = blank else {
        return usize::MAX;
    };
    if blank.len() != shot.len() {
        return usize::MAX;
    }
    shot.chunks_exact(4)
        .zip(blank.chunks_exact(4))
        .filter(|(x, y)| x[..3].iter().zip(&y[..3]).any(|(p, q)| p.abs_diff(*q) > 40))
        .count()
}

/// Ровный ли кадр — все точки одного цвета. Таким выходит пустая страница.
pub(super) fn flat(buf: &[u8]) -> bool {
    let Some(first) = buf.get(..4) else {
        return true;
    };
    buf.chunks_exact(4).all(|px| px == first)
}
