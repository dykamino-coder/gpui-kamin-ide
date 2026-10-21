//! Загрузка шрифтов страницы: по правилам `@font-face` читает файлы, отдаёт их системе и
//! заполняет таблицы семейства документа — подмену имени, лица с дескрипторами, возможности,
//! `size-adjust` по наклону, покрытие пробела `unicode-range` (css-fonts-4 §first-available-font)
//! и базовую линию `romn`. Полная загрузка сбрасывает таблицы документа, дозагрузка
//! (`<iframe>`) — нет.

use super::{declaration, faces, is_quote, read_font, sfnt_romn_baseline, source, stretch_desc};
use crate::document::state::ValueCx;

/// Есть ли в семействе знак пробела (U+0020).
///
/// Вопрос не праздный: «первым доступным» шрифтом (css-fonts-4
/// §first-available-font) семейство становится, только если пробел в нём
/// есть, — от первого доступного считаются метрики строки, `line-height:
/// normal`, `ch` и `ex`. Ответ «нет» бывает единственно у правила
/// `@font-face` с дескриптором `unicode-range`, где пробел не назван. Про
/// все прочие имена ответ утвердительный: дескриптора у них нет, а его
/// умолчание — весь набор знаков.
pub fn covers_space(family: &str, vcx: ValueCx<'_>) -> bool {
    vcx.doc
        .font_faces
        .has_space
        .borrow()
        .get(&family.to_ascii_lowercase())
        .copied()
        .unwrap_or(true)
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
pub fn load_faces(css: &str, vcx: ValueCx<'_>) {
    load_faces_with_base(css, None, vcx);
}

pub(crate) fn load_faces_with_base(css: &str, base: Option<&url::Url>, vcx: ValueCx<'_>) {
    // Имена семейств придумывает страница, и на соседней странице то же имя
    // значит другой файл — поэтому таблица подмены живёт РОВНО одну страницу.
    vcx.doc.font_faces.aliases.borrow_mut().clear();
    vcx.doc.font_faces.features.borrow_mut().clear();
    vcx.doc.font_faces.faces.borrow_mut().clear();
    vcx.doc.font_faces.has_space.borrow_mut().clear();
    vcx.doc.font_faces.size_adjust.borrow_mut().clear();
    super::metric_overrides::clear(vcx);
    vcx.doc.font_faces.romn.borrow_mut().clear();
    super::math::clear_aliases(vcx);
    load_faces_into(css, base, vcx);
}

/// Дозагрузка БЕЗ сброса подмен: вложенный документ (`<iframe>`) разбирается
/// посреди отрисовки внешнего — сброс крал бы шрифты хозяина, и весь текст
/// после рамки падал в подстановочный шрифт.
pub fn load_faces_additive(css: &str, vcx: ValueCx<'_>) {
    load_faces_into(css, None, vcx);
}

pub(crate) fn load_faces_sheets(sheets: &[String], base: Option<&url::Url>, vcx: ValueCx<'_>) {
    load_faces_with_base("", base, vcx);
    for css in sheets {
        load_faces_into(css, base, vcx);
    }
}

pub(super) fn load_faces_into(css: &str, base: Option<&url::Url>, vcx: ValueCx<'_>) {
    for block in faces(css, vcx) {
        let (Some(family), Some(src)) = (declaration(&block, "font-family"), source(&block)) else {
            continue;
        };
        let src = crate::document::resource_url::resolve(&src, base);
        let real = vcx.doc.font_faces.loaded.borrow().get(&src).cloned();
        let real = match real {
            Some(hit) => hit,
            None => {
                let loaded = read_font(&src).and_then(|bytes| {
                    super::math::remember(&src, &bytes, vcx);
                    let romn = sfnt_romn_baseline(&bytes);
                    vcx.doc
                        .font_faces
                        .romn_by_src
                        .borrow_mut()
                        .insert(src.clone(), romn);
                    crate::text::metrics::load_font(bytes)
                });
                vcx.doc
                    .font_faces
                    .loaded
                    .borrow_mut()
                    .insert(src.clone(), loaded.clone());
                loaded
            }
        };
        if let Some(real) = real {
            super::math::register(&src, &real, vcx);
            let name = family.trim_matches(is_quote).to_ascii_lowercase();
            if let Some(Some(v)) = vcx.doc.font_faces.romn_by_src.borrow().get(&src).copied() {
                vcx.doc.font_faces.romn.borrow_mut().insert(name.clone(), v);
            }
            // Правил на одно семейство бывает много: пробел у семейства есть,
            // если его несёт хоть одно из них.
            let space = range_has_space(declaration(&block, "unicode-range").as_deref());
            {
                let mut s = vcx.doc.font_faces.has_space.borrow_mut();
                let seen = s.entry(name.clone()).or_insert(false);
                *seen |= space;
            };
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
            vcx.doc
                .font_faces
                .size_adjust
                .borrow_mut()
                .entry(name.clone())
                .or_default()
                .push((slope, adjust));
            super::metric_overrides::register(&name, slope, &block, vcx);
            let width = stretch_desc(declaration(&block, "font-stretch").as_deref());
            let weight =
                super::matching::range(declaration(&block, "font-weight").as_deref(), 400.0);
            let (slope, angle) =
                super::matching::slope(declaration(&block, "font-style").as_deref());
            let features = declaration(&block, "font-feature-settings")
                .and_then(|v| crate::style::computed::feature_list(&v))
                .unwrap_or_default();
            vcx.doc
                .font_faces
                .faces
                .borrow_mut()
                .entry(name.clone())
                .or_default()
                .push(super::matching::Face {
                    width,
                    weight,
                    slope,
                    angle,
                    family: real.clone(),
                    adjust,
                    features,
                });
            if let Some(list) = declaration(&block, "font-feature-settings")
                .and_then(|v| crate::style::computed::feature_list(&v))
            {
                vcx.doc
                    .font_faces
                    .features
                    .borrow_mut()
                    .insert(name.clone(), list);
            }
            vcx.doc.font_faces.aliases.borrow_mut().insert(name, real);
        }
    }
}

/// Множитель `size-adjust` лица, которое подбор по наклону (css-fonts-4
/// §font-style-matching; Blink `font_face_cache.cc`) выберет для
/// запроса `want`: 0 normal, 1 italic, 2 oblique. Порядок наклонов лиц:
/// italic → italic, oblique, normal; normal → normal, oblique, italic
/// (§5.2 шаг 2); oblique → oblique, normal, italic — наклонный запрос на
/// курсивное лицо не падает (csswg#9389, `italic-oblique-fallback`). Среди
/// лиц одного наклона вес и ширину мы не выбираем: расходятся их множители —
/// 1.0 (`oblique-last-resort-weight-selection`: два oblique-лица, 100% и 50%).
pub fn size_adjust(family: &str, want: u8, vcx: ValueCx<'_>) -> f32 {
    {
        let s = vcx.doc.font_faces.size_adjust.borrow();
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
    }
}
