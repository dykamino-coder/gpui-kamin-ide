//! Реестр покрытия CSS: одна таблица, по которой считается процент.
//!
//! Зачем отдельный файл, а не список в документации: список в документации
//! устаревает молча. Здесь каждое свойство обязано быть классифицировано, а
//! тест проверяет, что помеченное `Mapped` действительно меняет разрешённый
//! стиль. Приписать свойству «перенесено» и не написать разбор — не выйдет.
//!
//! Классификация:
//! * `Mapped` — свойство доезжает до GPUI;
//! * `NoOp` — разбирается, но рисовать нечего, и это не искажает картинку
//!   (подсказки движку, вроде `will-change`);
//! * `Impossible` — примитива в GPUI нет, перенести невозможно. Причина
//!   обязательна и попадает в документацию.

mod field_audit;
mod properties;
#[cfg(test)]
mod report;
mod sources;
#[cfg(test)]
mod tests;
pub use field_audit::dead_fields;
pub use properties::PROPERTIES;

use crate::style::computed::Computed;
use crate::style::css::parse_decls;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Support {
    /// Значение из образца доезжает до стиля.
    Mapped,
    /// Разбирается и сознательно ничего не делает.
    NoOp(&'static str),
    /// Доезжает, но не во всех значениях или не во всех случаях.
    ///
    /// Отдельный класс появился после третьего аудита: без него «перенесено»
    /// скрывало разницу между «работает как в браузере» и «узнаваемо, но
    /// иначе». Причина обязательна и попадает в документацию.
    Partial(&'static str),
    /// Перенести нечем.
    Impossible(&'static str),
}

/// Свойство, образец значения и вердикт.
pub struct Prop {
    pub name: &'static str,
    pub sample: &'static str,
    pub support: Support,
}

/// Доля покрытия.
///
/// Полное совпадение с браузером и честная пустышка идут за единицу,
/// частичное — за половину, невозможное — за ноль. Половина за частичное не
/// научна, но не даёт спрятать «узнаваемо, но иначе» в графу «сделано»:
/// прошлая формула не могла опуститься ниже ста, потому что складывала
/// собственные пометки.
pub fn mapped_pct() -> f32 {
    let score: f32 = PROPERTIES
        .iter()
        .map(|p| match p.support {
            Support::Mapped | Support::NoOp(_) => 1.0,
            Support::Partial(_) => 0.5,
            Support::Impossible(_) => 0.0,
        })
        .sum();
    score * 100.0 / PROPERTIES.len() as f32
}

/// Свойства, которые обещаны как перенесённые, но стиль не меняют.
pub fn broken_promises() -> Vec<&'static str> {
    PROPERTIES
        .iter()
        .filter(|p| matches!(p.support, Support::Mapped | Support::Partial(_)))
        .filter(|p| {
            let decls = parse_decls(&format!("{}: {}", p.name, p.sample));
            let mut c = Computed::default();
            c.apply_decls(&decls);
            format!("{c:?}") == format!("{:?}", Computed::default())
        })
        .map(|p| p.name)
        .collect()
}
