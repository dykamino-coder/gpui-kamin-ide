//! Длина CSS (Len) и её разбор для spacing/смешанных записей; разбор числа css_number.

use super::*;

/// Длина в терминах, которые понимает GPUI.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Len {
    /// `12px`, `1.5rem` — переводим всё в px по базовому размеру шрифта.
    Px(f32),
    /// `50%` — доля родителя.
    Pct(f32),
    /// `min-content`/`max-content` — размер по СОДЕРЖИМОМУ. Раскладка под
    /// нами такого размера у коробки не знает, зато знает такую дорожку
    /// сетки: элемент с этим размером заворачивается в сетку из одной
    /// дорожки (см. `render::content_sized`).
    MinContent,
    MaxContent,
    /// `fit-content` — «по содержимому, но не шире доступного места»:
    /// `min(max-content, max(min-content, доступное))`. Дорожка сетки с
    /// таким поведением называется `auto`, ею он и выражается.
    FitContent,
    /// `1.5em` — доля СВОЕГО размера шрифта (для `font-size` — родительского).
    ///
    /// Отдельная единица нужна потому, что размер шрифта известен только
    /// после каскада: раньше `em` считался от постоянных 16 точек, и на
    /// вложенных размерах шрифта отступы расходились с браузером.
    Em(f32),
    /// Смешанный `calc()`: слагаемые разных природ, сворачивается по мере
    /// появления баз (css-values-4 §10.9). Хранится ИНДЕКСОМ в арене:
    /// сумма внутри варианта раздувала `Len` с 12 до 40 байт, и рекурсивный
    /// рендер вложенного документа переполнял стек (vh-support-transform-*).
    Calc(u32),
    /// `1.5vh`, `50vw` — доля высоты и ширины окна. Размер окна известен
    /// только сборщику дерева, поэтому единица доживает до него как есть.
    Vh(f32),
    Vw(f32),
    /// `calc(1em + 8px)`: доля кегля плюс довесок в точках — решается на
    /// том же шаге, что `em` (text-shadow-orientation-upright-001: поле).
    EmPx(f32, f32),
    /// `calc(4lh + 10px)`: кратное высоты строки плюс довесок в точках.
    /// Высота строки известна только при слиянии стилей — смесь доживает
    /// до него как есть (как одиночный `lh`).
    LhPx(f32, f32),
    /// `5ch` — ширина нуля, `2ex` — высота строчной. Зависят от ГЛИФОВ
    /// выбранного шрифта, а не от кегля (у Ahem нуль в целый кегль, у
    /// текстового — около половины), поэтому доживают до наследования вместе
    /// с семейством шрифта и меряются в `metrics.rs`.
    Ch(f32),
    Ex(f32),
    /// Продвижение знака `水` — единица `ic` (CSS Values §6.1.4).
    Ic(f32),
    /// `5lh` — доля ВЫСОТЫ СТРОКИ (CSS Values §6.1.2): она известна только
    /// после каскада, единица доживает до слияния стилей.
    Lh(f32),
    /// `anchor(<name>? <side>, <fallback>?)` (css-anchor-position-1
    /// §anchor-fn) — ИНДЕКС в арене `anchor_store`, как у `Calc`. Раскладке
    /// такая вставка уходит нулём (`apply::len_to_gpui`), а сдвиг до края
    /// якоря считает `anchor::AnchorPlace` на подготовке кадра. Законна
    /// только во вставках абсолюта; в размерах разбор её не порождает.
    Anchor(u32),
    /// `auto`
    Auto,
}

impl Len {
    /// Разбор для `word-spacing` и `letter-spacing`: там доля берётся от кегля
    /// так же, как `em`, поэтому смешанное `calc(400% + 1em)` складывается в
    /// одну длину вместо того чтобы пропасть (`word-spacing-001`).
    pub fn parse_spacing(raw: &str) -> Option<Self> {
        let s = raw.trim();
        // `normal` — ЗАДАННЫЙ ноль, а не «не задано»: без него наследованное
        // значение не сбросить, и `<span style="letter-spacing: normal">`
        // внутри разреженного абзаца оставался разреженным (§16.4).
        if s.eq_ignore_ascii_case("normal") {
            return Some(Len::Px(0.0));
        }
        if let Some(inner) = s
            .strip_prefix("calc(")
            .or_else(|| s.strip_prefix("CALC("))
            .and_then(|r| r.strip_suffix(')'))
        {
            return eval_calc(inner)?.collapse_spacing();
        }
        Len::parse(s)
    }

    /// Разбор для свойств, которые доли решают САМИ при отрисовке, зная
    /// размер коробки: стопы градиента, `background-position/size`,
    /// `text-indent`. У них процентная смесь `calc(100% - 10px)` ДОЖИВАЕТ
    /// как `Len::Calc` — css-values-4 §10.9: «`background-position`
    /// computation preserves the percentage in a `calc()`», доля решается в
    /// used-value time. Раскладка (taffy) этим разбором НЕ пользуется:
    /// туда смесь по-прежнему не попадает (замерено `gap-003-ltr`,
    /// см. `Sum::collapse`).
    pub fn parse_mixed(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
            return eval_calc(inner)?.collapse_mixed();
        }
        Len::parse(s)
    }
}

/// Число по грамматике CSS (css-syntax-3 §4.3.3), а не по правилам Rust.
///
/// `f32::from_str` берёт то, чего в CSS нет: `6.` (точка без дробной части) и
/// `6 ` вместе с внутренним пробелом. Из-за этого `height: 6.px` и
/// `height: 6 px` применялись шестью точками, тогда как оба объявления
/// НЕГОДНЫ и должны отбрасываться целиком (`units-003`).
pub(crate) fn css_number(s: &str) -> Option<f32> {
    let b = s.as_bytes();
    if b.is_empty() {
        return None;
    }
    let mut i = 0usize;
    if b[i] == b'+' || b[i] == b'-' {
        i += 1;
    }
    let start = i;
    while i < b.len() && b[i].is_ascii_digit() {
        i += 1;
    }
    let целых = i - start;
    let mut дробных = 0usize;
    if i < b.len() && b[i] == b'.' {
        i += 1;
        let fs = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        дробных = i - fs;
        if дробных == 0 {
            return None;
        }
    }
    if целых == 0 && дробных == 0 {
        return None;
    }
    if i < b.len() && (b[i] == b'e' || b[i] == b'E') {
        i += 1;
        if i < b.len() && (b[i] == b'+' || b[i] == b'-') {
            i += 1;
        }
        let es = i;
        while i < b.len() && b[i].is_ascii_digit() {
            i += 1;
        }
        if i == es {
            return None;
        }
    }
    if i != b.len() {
        return None;
    }
    s.parse::<f32>().ok()
}
