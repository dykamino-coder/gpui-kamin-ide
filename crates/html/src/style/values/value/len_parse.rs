//! Разбор длины: Len::parse — числа с единицами, проценты, calc()/min()/max()/clamp(), anchor(),
//! ключевые слова размеров.

use super::*;
use crate::document::state::ValueCx;

impl Len {
    pub fn parse(raw: &str, vcx: ValueCx<'_>) -> Option<Self> {
        let s = raw.trim();
        if s.eq_ignore_ascii_case("auto") {
            return Some(Len::Auto);
        }
        if s.eq_ignore_ascii_case("min-content") {
            return Some(Len::MinContent);
        }
        if s.eq_ignore_ascii_case("fit-content") {
            return Some(Len::FitContent);
        }
        if s.eq_ignore_ascii_case("max-content") {
            return Some(Len::MaxContent);
        }
        // `stretch` (css-sizing-4 §4.1) — «занять всё место содержащего
        // блока»: для коробки без боковых полей это и есть его ширина.
        // Приставочные написания того же значения живут в вёрстке дольше
        // самого ключевого слова. Прежде вся запись выбрасывалась, и коробка
        // падала в `auto`.
        if s.eq_ignore_ascii_case("stretch")
            || s.eq_ignore_ascii_case("-webkit-fill-available")
            || s.eq_ignore_ascii_case("-moz-available")
        {
            return Some(Len::Pct(1.0));
        }
        // `fit-content(<length-percentage>)` (css-sizing-3 §4.1) — это
        // `fit-content`, зажатый сверху аргументом. Самого зажима у нас пока
        // нет, но `fit-content` куда ближе к истине, чем выброшенная запись:
        // без неё коробка растягивалась во всю ширину как при `auto`.
        //
        // Не выражать довод парой «`max-width` = N, `min-width` =
        // min-content». Формула §4.1 — `min(max-content, max(min-content,
        // N))`, то есть довод НЕ потолок: при `min-content > N` он вообще не
        // действует (`fit-content-length-percentage-002/005`: коробка
        // ужимается до довода 50 и 20 вместо минимального содержимого 100),
        // а `min-width: min-content` этой ветке не доходит вовсе. Чинить надо
        // сам зажим `fit-content` (он берёт max-content без учёта переноса),
        // а не добавлять довод сбоку.
        let lower = s.to_ascii_lowercase();
        if let Some(arg) = lower
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
            && Len::parse(arg, vcx).is_some()
        {
            return Some(Len::FitContent);
        }
        if let Some(num) = s.strip_suffix('%') {
            return css_number(num).map(|v| Len::Pct(v / 100.0));
        }
        // `em` разбирается ДО `rem`: иначе `1rem` съедалось бы правилом для
        // `em` вместе с буквой `r`.
        // `calc()` с одним действием над однородными операндами: этого
        // хватает на `calc(100% - 24px)` только когда обе части одной природы,
        // поэтому смешанные записи честно не разбираются — приблизительная
        // длина хуже отсутствующей, её не видно в тесте.
        // `anchor()` и `calc(anchor(…) + 10px)` (css-anchor-position-1
        // §anchor-fn). Прежде запись выбрасывалась целиком, и абсолют падал на
        // статическую позицию. Имя якоря чувствительно к регистру
        // (`--myAnchor`), поэтому режется `s`, а не `lower`.
        if lower.starts_with("anchor(") && s.ends_with(')') {
            return parse_anchor(&s[7..s.len() - 1], 0.0, false, vcx);
        }
        // `anchor-size()` (§anchor-size-fn) — та же арена, `AnchorFn::size`
        // задан. В размер попадает уже точками: `anchor::resolve_sizes`
        // решает её ДО раскладки из реестра прошлого кадра.
        if lower.starts_with("anchor-size(") && s.ends_with(')') {
            return parse_anchor(&s[12..s.len() - 1], 0.0, true, vcx);
        }
        // `min()`/`max()` от `anchor()` (css-values-4 §comparison-functions):
        // прежде запись проваливалась в общий разбор и выбрасывалась, и
        // абсолют падал на статическую позицию.
        if (lower.starts_with("min(") || lower.starts_with("max("))
            && lower.contains("anchor(")
            && s.ends_with(')')
        {
            return parse_anchor_minmax(s, lower.starts_with("max("), vcx);
        }
        if lower.starts_with("calc(")
            && (lower.contains("anchor(") || lower.contains("anchor-size("))
        {
            return parse_anchor_calc(s, vcx);
        }
        // `min()`/`max()`/`clamp()` на верхнем уровне (css-values-4 §10.2):
        // разбираются тем же выражением, что и `calc()` — функцию сравнения
        // понимает `Calc::factor`. Прежде запись падала в `css_number` и
        // объявление роняло: `height: max(calc(100%))` оставляло 0 (`calc-in-max`).
        // round()/abs()/sin()… на верхнем уровне — тоже выражение (§10.3–§10.8).
        if lower.starts_with("min(")
            || lower.starts_with("max(")
            || lower.starts_with("clamp(")
            || calc_math::is_math_fn(&lower)
        {
            return parse_calc(&lower, vcx);
        }
        // Имена единиц регистронезависимы (css-values-4 §6: «unit identifiers
        // are ASCII case-insensitive»): `105.83333Q` — те же четверть-
        // миллиметры, что `105.83333q` (`q-unit-case-insensitivity-*`), а
        // `CALC(` — тот же `calc(`. Единственное чувствительное к регистру
        // место — имя якоря — разобрано выше по исходной строке.
        let s = lower.as_str();
        if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
            return parse_calc(inner, vcx);
        }
        super::atom::Atom::parse(s, vcx)
            .map(|atom| atom.length(vcx))
            // Bare numbers retain the existing permissive px contract.
            .or_else(|| css_number(s).map(Len::Px))
    }
}
