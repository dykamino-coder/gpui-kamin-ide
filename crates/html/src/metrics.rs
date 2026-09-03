//! Метрики шрифта для единиц `ch` и `ex`.
//!
//! `1ch` — ширина нуля выбранного шрифта, `1ex` — высота строчной буквы. Обе
//! зависят от ГЛИФОВ, а не от кегля: у служебного Ahem нуль занимает целый
//! кегль, у текстового шрифта — около половины. Разбор значений живёт далеко
//! от системы шрифтов, поэтому доли кегля берутся здесь через щуп, который
//! ставит вызывающий: у него есть `TextSystem`, у нас — нет.
//!
//! Пока щуп не поставлен, работает запасное значение спецификации (полкегля):
//! CSS сам разрешает его, когда нужного глифа в шрифте не нашлось.

use std::cell::RefCell;
use std::collections::HashMap;

/// Щуп: по имени семейства и кеглю отдаёт ширину нуля и высоту строчной.
type Probe = Box<dyn Fn(&str, f32) -> (f32, f32, f32, f32)>;

/// Кегль, на котором меряются метрики. Метрики линейны по кеглю, поэтому
/// хватает одного замера на семейство: остальное — умножение.
const PROBE_SIZE: f32 = 100.0;

/// Доли кегля по спецификации, когда глифа нет или щуп не поставлен.
// Продвижение иероглифа без замера — целый кегль: так его определяет
// спецификация для шрифта, в котором знака `水` нет (CSS Values §6.1.4).
const FALLBACK: (f32, f32, f32, f32) = (0.5, 0.5, 1.2, 1.0);

/// Моноширинные семейства в порядке предпочтения: своё встроенное, затем то,
/// что подставляет за `font-family: monospace` браузер на этой системе.
const MONO_FAMILIES: [&str; 3] = ["JetBrains Mono", "Consolas", "Courier New"];

thread_local! {
    static PROBE: RefCell<Option<Probe>> = const { RefCell::new(None) };
    static CACHE: RefCell<HashMap<String, (f32, f32, f32, f32)>> = RefCell::new(HashMap::new());
    static MONO: RefCell<&'static str> = const { RefCell::new(MONO_FAMILIES[0]) };
}

/// Семейство за родовое `monospace`.
///
/// Приложение встраивает JetBrains Mono, поэтому в нём ответ всегда один. А
/// вот стенды и чужие машины его не несут: подстановка системой даёт
/// пропорциональный шрифт, и всё, что держится на равной ширине знака,
/// разъезжается. Поэтому семейство выбирается из НАЛИЧНЫХ.
pub fn mono_family() -> &'static str {
    MONO.with(|m| *m.borrow())
}

/// Поставить щуп метрик. Вызывается один раз при старте: замер шрифта
/// возможен только там, где живёт система шрифтов.
pub fn install_probe(probe: impl Fn(&str, f32) -> (f32, f32, f32, f32) + 'static) {
    PROBE.with(|p| *p.borrow_mut() = Some(Box::new(probe)));
    CACHE.with(|c| c.borrow_mut().clear());
}

/// Длина `1ch` и `1ex` в точках для семейства и кегля.
pub fn ch_ex_px(family: &str, size_px: f32) -> (f32, f32) {
    let (ch, ex, _, _) = fractions(family);
    (ch * size_px, ex * size_px)
}

/// Продвижение знака `水` — единица `ic` (CSS Values §6.1.4).
///
/// Своя метрика, а не синоним `em`: у текстового шрифта иероглиф либо шире
/// кегля, либо его нет вовсе, и подмена кеглем врала на четверть
/// (`ic-unit-*`).
pub fn ic_px(family: &str, size_px: f32) -> f32 {
    fractions(family).3 * size_px
}

/// Межбуквенный и межсловный интервал в точках.
///
/// Единицы шрифта (`ch`, `ex`) сюда входят наравне с `em`: пока они молча
/// отбрасывались, `word-spacing: -1ch` не действовал вовсе
/// (`word-spacing-002`). Доля берётся от кегля — как `em`: в модели ширина
/// пробела шрифта отдельно не хранится.
pub fn spacing_px(len: Option<crate::value::Len>, family: &str, size_px: f32) -> f32 {
    use crate::value::Len;
    let (ch, ex) = ch_ex_px(family, size_px);
    match len {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size_px,
        Some(Len::Ch(k)) => k * ch,
        Some(Len::Ex(k)) => k * ex,
        _ => 0.0,
    }
}

/// Длина в точках при НЕИЗВЕСТНОМ контексте (узел вне наследования, фон,
/// скругление): кегль — запасной, метрики — по семейству (пустое — шрифт-
/// подмена). Доля родителя, единицы окна и размеры по содержимому здесь не
/// решаются — их значение знает только вызывающий.
///
/// Единая точка: прежде этот match был дословно повторён в пяти местах
/// (apply/background/computed), и правка запасных значений расходилась.
pub fn fallback_len_px(l: crate::value::Len, family: &str, font_px: f32) -> Option<f32> {
    use crate::value::Len;
    Some(match l {
        Len::Px(v) => v,
        Len::Em(k) => k * font_px,
        Len::EmPx(k, add) => k * font_px + add,
        Len::Ch(k) => k * ch_ex_px(family, font_px).0,
        Len::Ic(k) => k * ic_px(family, font_px),
        Len::Ex(k) => k * ch_ex_px(family, font_px).1,
        Len::Lh(k) => k * 1.2 * font_px,
        Len::LhPx(k, add) => k * 1.2 * font_px + add,
        _ => return None,
    })
}

/// Высота строки при `line-height: normal` — доля кегля по метрикам шрифта.
pub fn normal_line(family: &str) -> f32 {
    fractions(family).2
}

/// Доли кегля для семейства: замер идёт один раз и запоминается.
fn fractions(family: &str) -> (f32, f32, f32, f32) {
    if let Some(hit) = CACHE.with(|c| c.borrow().get(family).copied()) {
        return hit;
    }
    let measured = PROBE.with(|p| {
        p.borrow()
            .as_ref()
            .map(|probe| probe(family, PROBE_SIZE))
            .map(|(ch, ex, line, ic)| {
                (
                    ch / PROBE_SIZE,
                    ex / PROBE_SIZE,
                    line / PROBE_SIZE,
                    ic / PROBE_SIZE,
                )
            })
    });
    // Нулевая метрика — это не «шрифт шириной ноль», а неудавшийся замер:
    // такой ответ хуже запасного значения, потому что схлопывает коробку.
    let out = match measured {
        Some((ch, ex, line, ic)) if ch > 0.0 && ex > 0.0 && line > 0.0 => {
            (ch, ex, line, if ic > 0.0 { ic } else { FALLBACK.3 })
        }
        Some((ch, _, line, ic)) if ch > 0.0 => (
            ch,
            FALLBACK.1,
            if line > 0.0 { line } else { FALLBACK.2 },
            if ic > 0.0 { ic } else { FALLBACK.3 },
        ),
        _ => FALLBACK,
    };
    CACHE.with(|c| c.borrow_mut().insert(family.to_string(), out));
    out
}

/// Поставить щуп поверх системы шрифтов GPUI.
///
/// Вызывается один раз при старте приложения, ПОСЛЕ регистрации своих
/// шрифтов: до неё `Ahem` ещё не найден и замер вернул бы метрики подмены.
pub fn use_text_system(text_system: std::sync::Arc<gpui::TextSystem>) {
    let names = text_system.all_font_names();
    if let Some(found) = MONO_FAMILIES
        .into_iter()
        .find(|want| names.iter().any(|have| have.eq_ignore_ascii_case(want)))
    {
        MONO.with(|m| *m.borrow_mut() = found);
    }
    install_probe(move |family, size| {
        // Родовое имя системе шрифтов отдавать нельзя: `sans-serif` — это не
        // шрифт, а разряд, и поиск по нему кончается ничем. Подставляется то
        // же семейство, что и в каскаде за `sans-serif`.
        let name: gpui::SharedString = if family.is_empty() {
            crate::computed::GENERIC_SANS.into()
        } else {
            family.to_string().into()
        };
        let font = gpui::font(name);
        let id = text_system.resolve_font(&font);
        let size = gpui::px(size);
        let ch = text_system
            .ch_advance(id, size)
            .map(f32::from)
            .unwrap_or(0.0);
        // `line-height: normal` — это НЕ постоянная доля кегля, а метрика
        // шрифта: подъём плюс спуск (и зазор строк, если он есть). У Ahem она
        // ровно кегль, у текстовых шрифтов около 1.15–1.3 — из-за постоянной
        // 1.31 соседние коробки одной страницы расходились по высоте строк.
        let line =
            f32::from(text_system.ascent(id, size)) - f32::from(text_system.descent(id, size));
        // `ic` — продвижение знака `水`. Шрифт без него отдаёт запасной глиф,
        // и такой замер отбрасывается в пользу целого кегля.
        let ic = text_system
            .advance(id, size, '水')
            .map(|a| f32::from(a.width))
            .unwrap_or(0.0);
        (ch, f32::from(text_system.x_height(id, size)), line, ic)
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn falls_back_to_half_the_font_size() {
        assert_eq!(ch_ex_px("нет такого шрифта", 20.0), (10.0, 10.0));
    }

    #[test]
    fn probe_result_scales_with_the_font_size() {
        install_probe(|family, size| {
            // У Ahem все знаки в кегль, включая иероглиф.
            if family == "Ahem" {
                (size, size * 0.8, size, size)
            } else {
                (size * 0.5, size * 0.5, size * 1.2, size)
            }
        });
        assert_eq!(ch_ex_px("Ahem", 20.0), (20.0, 16.0));
        assert_eq!(ch_ex_px("Segoe UI", 20.0), (10.0, 10.0));
        // Щуп снимается: иначе он утечёт в соседние тесты того же потока.
        PROBE.with(|p| *p.borrow_mut() = None);
        CACHE.with(|c| c.borrow_mut().clear());
    }
}
