//! Разбор длины: Len::parse — числа с единицами, проценты, calc()/min()/max()/clamp(), anchor(), ключевые слова размеров.

use super::*;

impl Len {
    pub fn parse(raw: &str) -> Option<Self> {
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
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): выражать довод парой «`max-width` = N,
        // `min-width` = min-content». Формула §4.1 —
        // `min(max-content, max(min-content, N))`, то есть довод НЕ потолок:
        // при `min-content > N` он вообще не действует. Срез из 74 пар с
        // `fit-content(` в разметке: 32 -> 32, зато `fit-content-length-
        // percentage-002/005` 2.08 -> «красное видно» (коробка ужалась до
        // довода 50 и 20 вместо минимального содержимого 100), а `min-width:
        // min-content` этой ветке не доходит вовсе. Чинить надо сам зажим
        // `fit-content` (он берёт max-content без учёта переноса), а не
        // добавлять довод сбоку.
        let lower = s.to_ascii_lowercase();
        if let Some(arg) = lower
            .strip_prefix("fit-content(")
            .and_then(|r| r.strip_suffix(')'))
            && Len::parse(arg).is_some()
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
            return parse_anchor(&s[7..s.len() - 1], 0.0, false);
        }
        // `anchor-size()` (§anchor-size-fn) — та же арена, `AnchorFn::size`
        // задан. В размер попадает уже точками: `anchor::resolve_sizes`
        // решает её ДО раскладки из реестра прошлого кадра.
        if lower.starts_with("anchor-size(") && s.ends_with(')') {
            return parse_anchor(&s[12..s.len() - 1], 0.0, true);
        }
        // `min()`/`max()` от `anchor()` (css-values-4 §comparison-functions):
        // прежде запись проваливалась в общий разбор и выбрасывалась, и
        // абсолют падал на статическую позицию.
        if (lower.starts_with("min(") || lower.starts_with("max("))
            && lower.contains("anchor(")
            && s.ends_with(')')
        {
            return parse_anchor_minmax(s, lower.starts_with("max("));
        }
        if lower.starts_with("calc(")
            && (lower.contains("anchor(") || lower.contains("anchor-size("))
        {
            return parse_anchor_calc(s);
        }
        // `min()`/`max()`/`clamp()` на верхнем уровне (css-values-4 §10.2):
        // разбираются тем же выражением, что и `calc()` — функцию сравнения
        // понимает `Calc::factor`. Прежде запись падала в `css_number` и
        // объявление роняло: `height: max(calc(100%))` оставляло 0 (`calc-in-max`).
        if lower.starts_with("min(") || lower.starts_with("max(") || lower.starts_with("clamp(") {
            return parse_calc(&lower);
        }
        // Имена единиц регистронезависимы (css-values-4 §6: «unit identifiers
        // are ASCII case-insensitive»): `105.83333Q` — те же четверть-
        // миллиметры, что `105.83333q` (`q-unit-case-insensitivity-*`), а
        // `CALC(` — тот же `calc(`. Единственное чувствительное к регистру
        // место — имя якоря — разобрано выше по исходной строке.
        let s = lower.as_str();
        if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
            return parse_calc(inner);
        }
        for (suffix, unit) in [
            ("vh", Len::Vh as fn(f32) -> Len),
            ("vw", Len::Vw as fn(f32) -> Len),
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return css_number(num).map(|v| unit(v / 100.0));
            }
        }
        // `ic` — ширина иероглифа (advance у 水). У шрифтов CJK она равна
        // `ic` — продвижение знака `水`, своя метрика шрифта (CSS Values
        // §6.1.4). Синонимом `em` она была только потому, что замера не было:
        // у текстового шрифта иероглиф либо шире кегля, либо его нет вовсе.
        if let Some(num) = s.strip_suffix("ic") {
            return css_number(num).map(Len::Ic);
        }
        // `ch` и `ex` разбираются в свои единицы: перевести их в точки можно
        // только зная шрифт, а он известен после наследования.
        if let Some(num) = s.strip_suffix("ch") {
            return css_number(num).map(Len::Ch);
        }
        if let Some(num) = s.strip_suffix("ex") {
            return css_number(num).map(Len::Ex);
        }
        // `cap` — высота ПРОПИСНОЙ первого доступного шрифта (css-values-4
        // §6.1.4). Метрика известна только после наследования (семейство плюс
        // кегль), поэтому единица доживает СУММОЙ в арене `calc`, а не своим
        // вариантом `Len`: `Len` разбирается полусотней `match` по крейту, и
        // новый вариант потянул бы правку каждого из них.
        if let Some(num) = s.strip_suffix("cap") {
            return css_number(num).map(|v| {
                Len::Calc(calc_store(Sum {
                    cap: v,
                    ..Sum::default()
                }))
            });
        }
        // `rem` — кегль КОРНЯ, а не постоянные 16 (§6.1.4).
        if let Some(num) = s.strip_suffix("rem") {
            return css_number(num).map(|v| match ROOT_FONT_VIEW.with(|c| c.get()) {
                Some((true, k)) => Len::Vh(v * k),
                Some((false, k)) => Len::Vw(v * k),
                None => Len::Px(v * root_font_px()),
            });
        }
        if let Some(num) = s.strip_suffix("em") {
            return css_number(num).map(Len::Em);
        }
        // `rlh` разбирается ДО `lh`: иначе `"1rlh".strip_suffix("lh")` даёт
        // `"1r"`, число не читается, и объявление роняется целиком
        // (`rlh-unit-001`: `inline-size: calc(1rlh - 1rlh)`).
        if let Some(num) = s.strip_suffix("rlh") {
            return css_number(num).map(|v| Len::Px(v * root_line_px()));
        }
        if let Some(num) = s.strip_suffix("lh") {
            return css_number(num).map(Len::Lh);
        }
        // Абсолютные единицы (css-values-4 §6.2): 1in = 96px = 2.54cm =
        // 25.4mm = 101.6q = 6pc. Порядок важен: «pc» раньше «c»-хвостов нет,
        // но «in»/«cm»/«mm» не пересекаются с уже разобранными.
        for (suffix, factor) in [
            ("px", 1.0),
            ("pt", 96.0 / 72.0),
            ("in", 96.0),
            ("cm", 96.0 / 2.54),
            ("mm", 96.0 / 25.4),
            ("pc", 16.0),
            ("q", 96.0 / 101.6),
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return css_number(num).map(|v| Len::Px(v * factor));
            }
        }
        // Голое число: в CSS допустимо только для 0, но модель часто пишет
        // `padding: 8` — принимаем как px, иначе виджет разъезжается.
        css_number(s).map(Len::Px)
    }
}
