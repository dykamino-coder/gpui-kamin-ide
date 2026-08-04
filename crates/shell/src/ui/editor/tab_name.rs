//! Имя и ширина таба редактора.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

pub fn base_name(path: &str) -> String {
    path.replace('\\', "/")
        .rsplit('/')
        .next()
        .unwrap_or(path)
        .to_string()
}
/// Ширина таба по ИЗМЕРЕННОМУ тексту. Состав (`FileViewerTabs.module.css`):
/// pl 10 + [pin 11 + gap 6] + иконка 14 + gap 6 + текст **кеглем 11** +
/// [gap 6 + точка ●] + gap 6 + close 16 + pr 6. Прежняя формула мерила текст
/// кеглем 12, теряла второй gap и не знала про pin/dirty — детект
/// переполнения врал (ревью ц.13).
pub fn tab_width(name: &str, pinned: bool, dirty: bool, window: &mut gpui::Window) -> f32 {
    let mut w =
        10.0 + 14.0 + 6.0 + crate::ui::text_fit::measure(name, 11.0, window) + 6.0 + 16.0 + 6.0;
    if pinned {
        // Эффективный кегль пина — 16 (каскад), а не 11 (ревью ц.14)
        w += 16.0 + 6.0;
    }
    if dirty {
        // Глиф ● кеглем 10 — advance ≈ 0.8 em
        w += 8.0 + 6.0;
    }
    w
}
