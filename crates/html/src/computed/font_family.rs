//! Retain authored families for glyph fallback while keeping the first available font metrics.

use super::{Computed, family_name_ok, generic_family, is_generic, is_quote};

pub(super) fn apply(style: &mut Computed, v: &str) {
    if v.eq_ignore_ascii_case("inherit") {
        style.font_family = None;
        style.font_families = None;
        style.monospace = None;
        return;
    }
    // Имя семейства — либо строка в кавычках, либо ряд
    // ИДЕНТИФИКАТОРОВ (§15.3). Неверное имя делает объявление
    // недействительным целиком (§4.2): прежде разбор просто
    // пропускал негодное имя и брал следующее из списка, из-за
    // чего `font-family: 1Ahem, Ahem` набиралось шрифтом Ahem.
    if !v.split(',').all(|part| family_name_ok(part.trim())) {
        return;
    }
    style.font_families = Some(families(v));
    let lower = v.to_ascii_lowercase();
    // Моноширинный запрос несёт смысл (код) и решает выбор
    // встроенного шрифта, если названного в системе нет.
    style.monospace = Some(lower.contains("mono") || lower.contains("courier"));
    // Родовое имя — не пустое место: браузер подставляет за него
    // конкретный системный шрифт, и без подстановки разметка
    // набиралась умолчанием движка, шире браузерного. Берётся то
    // же семейство, что подставляет Chrome на этой системе.
    // Родовое имя — только БЕЗ кавычек (css-fonts-4 §4.1.1: names
    // that happen to be the same as a keyword value «must be quoted
    // to prevent confusion with the keywords»): `"fantasy", serif` —
    // семейство «fantasy», затем родовое `serif`
    // (`quoted-generic-ignored`).
    let quoted = |f: &str| f.starts_with('"') || f.starts_with('\'');
    let first_generic = v
        .split(',')
        .map(str::trim)
        .filter(|f| !quoted(f))
        .map(str::to_ascii_lowercase)
        .find(|f| is_generic(f));
    let generic = v
        .split(',')
        .map(str::trim)
        .filter(|f| !quoted(f))
        .map(str::to_ascii_lowercase)
        .find_map(|f| generic_family(&f));
    // Первое НЕ родовое имя списка уходит в шрифт как есть:
    // подстановкой недостающего занимается сама система шрифтов.
    // Имя нормализуется до сравнения: неквотированное имя из
    // нескольких слов — это один пробел между ними (§15.3).
    let norm = |f: &str| {
        let un = crate::css::unescape(f);
        un.split_whitespace().collect::<Vec<_>>().join(" ")
    };
    // Годно ли имя списка как ИМЯ СЕМЕЙСТВА: в кавычках — всегда
    // (даже `"serif"`), без кавычек — если это не родовое слово.
    let usable = |raw: &str| {
        let f = raw.trim_matches(is_quote);
        let lower = f.to_ascii_lowercase();
        !f.is_empty()
            && (quoted(raw)
                || (!is_generic(&lower) && !matches!(lower.as_str(), "inherit" | "initial")))
    };
    // Первое ДОСТУПНОЕ имя списка: браузер идёт по списку, пока
    // не найдёт шрифт (§15.3). Прежде бралось первое подходящее по
    // виду, и `font-family: Courier New, Ahem` при отсутствующем
    // `Courier New` набиралось подменой вместо `Ahem`.
    //
    // Доступность даёт не только система. Список установленных —
    // это снимок `all_font_names()`, снятый ОДИН РАЗ на старте
    // (`metrics::use_text_system`), и шрифт, принесённый самой
    // страницей через `@font-face`, в него не попадает никогда.
    // Без учёта подмен список семейств не доходил до второго
    // имени: `font-family: "WOFF Test", "WOFF Test CFF Fallback"`
    // при НЕГОДНОМ `woff2` обязан взять второе имя, а вместо
    // этого отдавал системе первое, которого нет, — и весь набор
    // WOFF2 держался на случайном совпадении подмен.
    // Мало ИМЕТЬ шрифт: «первым доступным» (css-fonts-4
    // §first-available-font) семейство становится, только если в
    // нём есть знак U+0020 — от первого доступного считаются
    // метрики строки, `line-height: normal`, `ch` и `ex`.
    // Правило `@font-face` с `unicode-range` без пробела обязано
    // быть ПРОПУЩЕНО: `font-family: 'A-no-space', 'B'` меряется
    // по `B`, а не по первому имени списка. Прежде подмена от
    // такого правила проходила как доступная, и после `7dbbfd2`
    // (замер по настоящему имени) доли кегля брались с ЧУЖОГО
    // файла — `first-available-font-002/007`, `ex-unit-004`.
    let available = |f: &str| {
        (crate::metrics::font_installed(f) || crate::fonts::alias(f).is_some())
            && crate::fonts::covers_space(f)
    };
    let installed = v
        .split(',')
        .map(str::trim)
        .filter(|raw| usable(raw))
        .map(|raw| norm(raw.trim_matches(is_quote)))
        .find(|f| available(f));
    if let Some(found) = installed {
        style.font_family = Some(found);
        return;
    }
    // Ни одно имя не доступно. Первое родовое имя списка — его
    // подстановка; `monospace` своего имени не даёт (семейство под
    // него берёт `metrics::mono_family` по признаку). Родового нет —
    // шрифт ДОКУМЕНТА (CSS 2.1 §15.3, «the user agent's default
    // font»), а не неизвестное имя: DirectWrite заменял его
    // системным UI-шрифтом (`select_font`, Segoe UI), тогда как
    // эталон без `font-family` набирается базой документа
    // (`font-family-name-017/018/022`, `standard-font-family`).
    // Метка «шрифт документа» — ПУСТОЕ имя: оно задано (родителя
    // не наследует, `-017`), а меряется и набирается как `None`.
    // Пока список установленных не снят (`fonts_known`), о
    // доступности судить нечем — ниже прежняя ветка.
    if crate::metrics::fonts_known() && lower.trim() != "inherit" {
        style.font_family = match first_generic.as_deref() {
            Some("monospace" | "ui-monospace") => None,
            Some(_) => generic.map(str::to_string),
            None => {
                style.monospace = Some(false);
                Some(String::new())
            }
        };
        return;
    }
    style.font_family = v
        .split(',')
        .map(|f| f.trim().trim_matches(is_quote))
        .find(|f| {
            let lower = f.to_ascii_lowercase();
            !f.is_empty() && !is_generic(&lower) && !matches!(lower.as_str(), "inherit" | "initial")
        })
        // Неквотированное имя из нескольких слов НОРМАЛИЗУЕТСЯ:
        // последовательность пробельных знаков (включая переводы
        // строк) — это один пробел (`Courier   New` == `Courier
        // New`, CSS2 §15.3; font-family-013 и родня). Экранирование
        // раскрывается как в любом идентификаторе.
        .map(|f| {
            let un = crate::css::unescape(f);
            un.split_whitespace().collect::<Vec<_>>().join(" ")
        })
        .or_else(|| generic.map(str::to_string));
}

fn families(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .map(|name| {
            if !name.starts_with(['\'', '"']) {
                let lower = name.to_ascii_lowercase();
                if matches!(lower.as_str(), "monospace" | "ui-monospace") {
                    return crate::metrics::mono_family_for(None).to_string();
                }
                if let Some(family) = generic_family(&lower) {
                    return family.to_string();
                }
            }
            crate::css::unescape(name.trim_matches(is_quote))
                .split_whitespace()
                .collect::<Vec<_>>()
                .join(" ")
        })
        .collect()
}

pub(crate) fn inherit(target: &mut Computed, own: &Computed, parent: &Computed) {
    target.font_family = own
        .font_family
        .clone()
        .or_else(|| parent.font_family.clone());
    target.font_families = own
        .font_families
        .clone()
        .or_else(|| parent.font_families.clone());
}

pub(crate) fn fallbacks(
    style: &Computed,
    base: Option<gpui::FontFallbacks>,
) -> Option<gpui::FontFallbacks> {
    let Some(families) = &style.font_families else {
        return base.or_else(crate::fonts::document_fallbacks);
    };
    // CSS Fonts 4 section 5.2: missing glyphs continue through the authored
    // family list before system fallback. Metrics still use the first available
    // font; DirectWrite maps only characters absent from that primary font.
    let mut list: Vec<String> = families
        .iter()
        .filter(|name| Some(name.as_str()) != style.font_family.as_deref())
        .filter(|name| crate::metrics::font_installed(name) || crate::fonts::alias(name).is_some())
        .map(|name| {
            crate::fonts::alias_stretch(name, style.font_stretch).unwrap_or_else(|| name.clone())
        })
        .collect();
    if let Some(document) = crate::fonts::document_fallbacks() {
        list.extend(document.fallback_list().iter().cloned());
    }
    (!list.is_empty()).then(|| gpui::FontFallbacks::from_fonts(list))
}
