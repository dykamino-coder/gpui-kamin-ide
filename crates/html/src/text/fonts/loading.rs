//! Loading for fonts; split out to keep the owning module within 250 lines.

use super::{
    ALIASES, declaration, faces, is_quote, read_font, sfnt_romn_baseline, source, stretch_desc,
};
use super::{FACES, FEATURES, HAS_SPACE, LOADED, LOADER, ROMN, ROMN_BY_SRC, SIZE_ADJUST};

/// Есть ли в семействе знак пробела (U+0020).
///
/// Вопрос не праздный: «первым доступным» шрифтом (css-fonts-4
/// §first-available-font) семейство становится, только если пробел в нём
/// есть, — от первого доступного считаются метрики строки, `line-height:
/// normal`, `ch` и `ex`. Ответ «нет» бывает единственно у правила
/// `@font-face` с дескриптором `unicode-range`, где пробел не назван. Про
/// все прочие имена ответ утвердительный: дескриптора у них нет, а его
/// умолчание — весь набор знаков.
pub fn covers_space(family: &str) -> bool {
    HAS_SPACE.with(|s| {
        s.borrow()
            .get(&family.to_ascii_lowercase())
            .copied()
            .unwrap_or(true)
    })
}

/// Покрывает ли дескриптор `unicode-range` знак U+0020.
///
/// Видов записи три (css-fonts-4 §unicode-range-desc): одиночный знак `U+20`,
/// отрезок `U+0-7F` и маска `U+00??`. Негодная запись делает дескриптор
/// недействительным целиком, а его умолчание — `U+0-10FFFF`, то есть пробел
/// покрыт.
pub(super) fn range_has_space(value: Option<&str>) -> bool {
    let Some(value) = value else { return true };
    let mut any = false;
    for part in value.split(',') {
        let part = part.trim();
        let Some(body) = part.strip_prefix("U+").or_else(|| part.strip_prefix("u+")) else {
            return true;
        };
        let (lo, hi) = match body.split_once('-') {
            Some((a, b)) => (
                u32::from_str_radix(a.trim(), 16).ok(),
                u32::from_str_radix(b.trim(), 16).ok(),
            ),
            None if body.contains('?') => (
                u32::from_str_radix(&body.replace('?', "0"), 16).ok(),
                u32::from_str_radix(&body.replace('?', "F"), 16).ok(),
            ),
            None => {
                let one = u32::from_str_radix(body, 16).ok();
                (one, one)
            }
        };
        match (lo, hi) {
            (Some(lo), Some(hi)) if lo <= hi => any |= (lo..=hi).contains(&0x20),
            _ => return true,
        }
    }
    any
}

/// Разобрать правила `@font-face` из таблицы стилей и загрузить шрифты.
///
/// Путь в `url(...)` берётся как есть: страницу до движка доводит стенд, и
/// адреса в ней уже разрешены в файлы.
pub fn load_faces(css: &str) {
    // Имена семейств придумывает страница, и на соседней странице то же имя
    // значит другой файл — поэтому таблица подмены живёт РОВНО одну страницу.
    ALIASES.with(|a| a.borrow_mut().clear());
    FEATURES.with(|f| f.borrow_mut().clear());
    FACES.with(|f| f.borrow_mut().clear());
    HAS_SPACE.with(|s| s.borrow_mut().clear());
    SIZE_ADJUST.with(|s| s.borrow_mut().clear());
    ROMN.with(|r| r.borrow_mut().clear());
    load_faces_into(css);
}

/// Дозагрузка БЕЗ сброса подмен: вложенный документ (`<iframe>`) разбирается
/// посреди отрисовки внешнего — сброс крал бы шрифты хозяина, и весь текст
/// после рамки падал в подстановочный шрифт.
pub fn load_faces_additive(css: &str) {
    load_faces_into(css);
}

pub(super) fn load_faces_into(css: &str) {
    for block in faces(css) {
        let (Some(family), Some(src)) = (declaration(&block, "font-family"), source(&block)) else {
            continue;
        };
        let real = LOADED.with(|c| c.borrow().get(&src).cloned());
        let real = match real {
            Some(hit) => hit,
            None => {
                let loaded = read_font(&src).and_then(|bytes| {
                    let romn = sfnt_romn_baseline(&bytes);
                    ROMN_BY_SRC.with(|m| m.borrow_mut().insert(src.clone(), romn));
                    LOADER.with(|l| l.borrow().as_ref().and_then(|load| load(bytes)))
                });
                LOADED.with(|c| c.borrow_mut().insert(src.clone(), loaded.clone()));
                loaded
            }
        };
        if let Some(real) = real {
            let name = family.trim_matches(is_quote).to_ascii_lowercase();
            if let Some(Some(v)) = ROMN_BY_SRC.with(|m| m.borrow().get(&src).copied()) {
                ROMN.with(|r| r.borrow_mut().insert(name.clone(), v));
            }
            // Правил на одно семейство бывает много: пробел у семейства есть,
            // если его несёт хоть одно из них.
            let space = range_has_space(declaration(&block, "unicode-range").as_deref());
            HAS_SPACE.with(|s| {
                let mut s = s.borrow_mut();
                let seen = s.entry(name.clone()).or_insert(false);
                *seen |= space;
            });
            // Лицо запоминается вместе с шириной из дескриптора: выбор между
            // правилами одного семейства ведёт §font-matching.
            // `size-adjust: <percentage [0,∞]>`; без дескриптора — 100%.
            let adjust = declaration(&block, "size-adjust")
                .and_then(|v| v.trim().strip_suffix('%')?.trim().parse::<f32>().ok())
                .filter(|p| *p >= 0.0)
                .map_or(1.0, |p| p / 100.0);
            // Наклон лица — дескриптор `font-style` (css-fonts-4
            // §font-prop-desc): `italic`, `oblique [<angle>{1,2}]`, иначе normal.
            let slope =
                match declaration(&block, "font-style").map(|v| v.trim().to_ascii_lowercase()) {
                    Some(v) if v.starts_with("italic") => 1u8,
                    Some(v) if v.starts_with("oblique") => 2,
                    _ => 0,
                };
            SIZE_ADJUST.with(|s| {
                s.borrow_mut()
                    .entry(name.clone())
                    .or_default()
                    .push((slope, adjust));
            });
            let width = stretch_desc(declaration(&block, "font-stretch").as_deref());
            FACES.with(|f| {
                f.borrow_mut()
                    .entry(name.clone())
                    .or_default()
                    .push((width, real.clone()));
            });
            if let Some(list) = declaration(&block, "font-feature-settings")
                .and_then(|v| crate::style::computed::feature_list(&v))
            {
                FEATURES.with(|f| f.borrow_mut().insert(name.clone(), list));
            }
            ALIASES.with(|a| a.borrow_mut().insert(name, real));
        }
    }
}

/// Множитель `size-adjust` лица, которое подбор по наклону (css-fonts-4
/// §font-style-matching; Blink `font_face_cache.cc:227-259`) выберет для
/// запроса `want`: 0 normal, 1 italic, 2 oblique. Порядок наклонов лиц:
/// italic → italic, oblique, normal; normal → normal, oblique, italic
/// (§5.2 шаг 2); oblique → oblique, normal, italic — наклонный запрос на
/// курсивное лицо не падает (csswg#9389, `italic-oblique-fallback`). Среди
/// лиц одного наклона вес и ширину мы не выбираем: расходятся их множители —
/// 1.0 (`oblique-last-resort-weight-selection`: два oblique-лица, 100% и 50%).
pub fn size_adjust(family: &str, want: u8) -> f32 {
    SIZE_ADJUST.with(|s| {
        let s = s.borrow();
        let Some(faces) = s.get(&family.to_ascii_lowercase()) else {
            return 1.0;
        };
        let order: [u8; 3] = match want {
            1 => [1, 2, 0],
            2 => [2, 0, 1],
            _ => [0, 2, 1],
        };
        for slope in order {
            let mut hit = faces.iter().filter(|(k, _)| *k == slope).map(|(_, a)| *a);
            if let Some(first) = hit.next() {
                return if hit.all(|a| a == first) { first } else { 1.0 };
            }
        }
        1.0
    })
}
