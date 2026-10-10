//! Замеры абзаца: поля строк, вырезы обтекания, отступы, висячие знаки, ширины, min-content.

mod cache_keys;
mod min_content;
mod padding;
mod width;

use crate::text::paragraph::*;

/// Забыть замеры. Зовётся при разборе новой страницы: одно и то же имя
/// семейства на разных страницах означает РАЗНЫЕ шрифты (`@font-face`), и
/// старые положения знаков стали бы чужими.
pub fn forget_measures() {
    MEASURED.with(|c| c.borrow_mut().clear());
    SPLITS.with(|c| c.borrow_mut().clear());
}

thread_local! {
    /// Память разрезов: ключ разреза → готовые строки.
    pub(super) static SPLITS: std::cell::RefCell<
        std::collections::HashMap<u64, std::rc::Rc<Vec<Line>>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько замеров абзацев помнить между кадрами.
const MEASURE_CACHE: usize = 64;

/// Память подбора кегля: ключ абзаца → найденный множитель.
pub(super) fn remember_fit(key: u64, k: f32) {
    FITTED.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(hit) = cache.iter_mut().find(|(hit, _)| *hit == key) {
            hit.1 = k;
            return;
        }
        if cache.len() >= MEASURE_CACHE {
            cache.remove(0);
        }
        cache.push((key, k));
    });
}

thread_local! {
    /// Найденные множители `text-fit`: ключ абзаца → во сколько раз крупнее.
    pub(super) static FITTED: std::cell::RefCell<Vec<(u64, f32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Память замеров: ключ стиля и текста → положения знаков по кускам.
    static MEASURED: std::cell::RefCell<Vec<(u64, Vec<Seg>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}
