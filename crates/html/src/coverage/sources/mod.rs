//! Список исходников, читающих разрешённый стиль, для аудита полей `Computed`.
//!
//! Три части (`dom_text`, `layout_paint`, `layout_flow`) склеиваются
//! `const fn`-конкатенацией в `CONSUMERS` без изменения порядка. Тест
//! `coverage::tests` требует, чтобы каждое поле стиля читалось хотя бы в одном
//! из них: иначе свойство числится поддержанным, а на картинке его нет.

mod dom_text;
mod layout_flow;
mod layout_paint;

const fn concatenate<const N: usize>(groups: &[&[&'static str]]) -> [&'static str; N] {
    let mut out = [""; N];
    let mut at = 0;
    let mut group = 0;
    while group < groups.len() {
        let mut i = 0;
        while i < groups[group].len() {
            out[at] = groups[group][i];
            at += 1;
            i += 1;
        }
        group += 1;
    }
    assert!(at == N);
    out
}

/// Исходники, которые ЧИТАЮТ разрешённый стиль.
///
/// Реестр доказывает, что свойство разобрано. Этого мало: поле могло быть
/// заполнено и не прочитано никем — свойство тогда числится поддержанным, а
/// на картинке его нет. Аудит нашёл пять таких. Список ниже — все места, где
/// стиль превращается в элементы; тест требует, чтобы каждое поле
/// разрешённого стиля было прочитано хотя бы в одном из них.
pub(super) const CONSUMERS: &[&str] = &concatenate::<
    { dom_text::SOURCES.len() + layout_paint::SOURCES.len() + layout_flow::SOURCES.len() },
>(&[
    dom_text::SOURCES,
    layout_paint::SOURCES,
    layout_flow::SOURCES,
]);
