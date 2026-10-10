//! Source lists joined in their original order for computed-field coverage.

mod dom_text;
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
pub(super) const CONSUMERS: &[&str] =
    &concatenate::<223>(&[dom_text::SOURCES, layout_paint::SOURCES]);
