//! Скрим-затемнение под оверлеями — ОДИН источник значений.
//!
//! Токены оригинала (`variables.css:45-47`, `light-theme.css:70-72`):
//! `--overlay-soft` .35 / светлая .14, `--overlay-modal` .5 / .28,
//! `--overlay-deep` .6 / .40; в светлой теме цвет не чёрный, а чернильный
//! `rgb(27, 26, 22)`.
//!
//! Рисует скрим ТОЛЬКО overlay-окно: оно композитится поверх main вместе с
//! нативными вебвью-поверхностями, поэтому затемняет и их. Скрим main-окна
//! оставлен прозрачным ловцом кликов — пока оба рисовали, альфа
//! складывалась (.6 + .6 ≈ .84 вместо .6) и светлая тема теряла чернильный
//! оттенок (ревью ц.13).

/// Чернильная база светлой темы (`rgb(27, 26, 22)`).
fn base() -> (f32, f32, f32) {
    if kamin_theme::current_is_light() {
        (27.0 / 255.0, 26.0 / 255.0, 22.0 / 255.0)
    } else {
        (0.0, 0.0, 0.0)
    }
}

fn scrim(dark_a: f32, light_a: f32) -> gpui::Rgba {
    let (r, g, b) = base();
    gpui::Rgba {
        r,
        g,
        b,
        a: if kamin_theme::current_is_light() {
            light_a
        } else {
            dark_a
        },
    }
}

// `--overlay-soft` — токен для списочных оверлеев. ВНИМАНИЕ: QuickOpen,
// Find in Files и Workspace Symbols токеном НЕ пользуются — у них в CSS
// литерал `rgba(0, 0, 0, 0.35)` в обеих темах (`QuickOpen.module.css:9`,
// `FindInFiles.module.css:9`), для них есть `soft_literal()`.

/// Литеральный `rgba(0, 0, 0, 0.35)` — ровно как в CSS этих трёх оверлеев
/// (ревью ц.14: тем-зависимая подмена осветлила их в светлой теме вдвое).
pub fn soft_literal() -> gpui::Rgba {
    gpui::Rgba {
        r: 0.,
        g: 0.,
        b: 0.,
        a: 0.35,
    }
}

/// `--overlay-modal` — палитра команд и QuickPick.
pub fn modal() -> gpui::Rgba {
    scrim(0.5, 0.28)
}

/// `--overlay-deep` — Confirm/Prompt-модалка.
pub fn deep() -> gpui::Rgba {
    scrim(0.6, 0.4)
}
