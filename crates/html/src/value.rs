//! Разбор значений CSS: длины, цвета, числа.
//!
//! Отдельный модуль, потому что одно и то же значение приходит из трёх мест —
//! `style=""`, правило в `<style>` и значение по умолчанию тега, — и разбирать
//! его надо одинаково.

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

/// Число по грамматике CSS (css-syntax-3 §4.3.3), а не по правилам Rust.
///
/// `f32::from_str` берёт то, чего в CSS нет: `6.` (точка без дробной части) и
/// `6 ` вместе с внутренним пробелом. Из-за этого `height: 6.px` и
/// `height: 6 px` применялись шестью точками, тогда как оба объявления
/// НЕГОДНЫ и должны отбрасываться целиком (`units-003`).
fn css_number(s: &str) -> Option<f32> {
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

thread_local! {
    /// Кегль КОРНЕВОГО элемента: база единицы `rem` (css-values-4 §6.1.4 —
    /// «the computed value of the em unit on the root element»). Постоянные
    /// 16 врали на любом документе, где корню задан свой кегль:
    /// `:root { font-size: 25% }` делает `25rem` сотней точек, а у нас
    /// выходило 400 (`percentage-rem-low`, снимок 500×500 против 125×125).
    ///
    /// Слот, а не поле: разбор длины живёт далеко от дерева, а корень всё
    /// равно разбирается ПЕРВЫМ в порядке документа — к моменту, когда
    /// разбирается объявление любого потомка, значение уже верное.
    static ROOT_FONT_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(16.0) };
    /// Высота строки корня: база единицы `rlh` (§6.1.4).
    static ROOT_LINE_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(19.2) };
    /// Кегль корня в единицах окна (`html { font-size: 100vw }`): точек на
    /// разборе ещё нет, окно знает только сборщик дерева. Тогда `rem` доживает
    /// той же единицей окна, умноженной на долю (`vh-em-inherit`: `1rem` =
    /// `100vw`, а не начальные 16). Флаг — `vh` вместо `vw`.
    static ROOT_FONT_VIEW: std::cell::Cell<Option<(bool, f32)>> = const { std::cell::Cell::new(None) };
}

/// Записать кегль корня, заданный единицей окна (`None` — кегль в точках).
pub fn set_root_font_view(view: Option<(bool, f32)>) {
    ROOT_FONT_VIEW.with(|c| c.set(view));
}

/// Записать корневые метрики. Зовётся разбором дерева на элементе `html`
/// (`dom::walk`); сбрасывается на входе разбора (`dom::parse_media`):
/// вложенный документ рамки имеет СВОЙ корень.
pub fn set_root_metrics(font_px: f32, line_px: f32) {
    ROOT_FONT_PX.with(|c| c.set(if font_px > 0.0 { font_px } else { 16.0 }));
    ROOT_LINE_PX.with(|c| c.set(if line_px > 0.0 { line_px } else { 19.2 }));
}

/// Корневые метрики к умолчанию: документ без своего кегля на корне обязан
/// считать `rem` ровно как раньше — иначе правка была бы не про корень, а
/// про все страницы сразу.
pub fn reset_root_metrics() {
    set_root_metrics(16.0, 19.2);
    set_root_font_view(None);
}

pub fn root_font_px() -> f32 {
    ROOT_FONT_PX.with(|c| c.get())
}

pub fn root_line_px() -> f32 {
    ROOT_LINE_PX.with(|c| c.get())
}

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
        if lower.starts_with("calc(") && (lower.contains("anchor(") || lower.contains("anchor-size(")) {
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

/// Цвет в формате GPUI (`Rgba` → `Hsla` конвертируется на месте применения).
#[derive(Default, Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

thread_local! {
    /// Используемая схема цвета узла, чей каскад идёт сейчас (css-color-adjust-1
    /// §color-scheme-prop): по ней `light-dark()` выбирает вариант. Ставит
    /// обход дерева (`dom::walk`) на время узла и его потомков.
    static DARK_SCHEME: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

/// Тёмная ли схема у текущего узла.
pub fn dark_scheme() -> bool {
    DARK_SCHEME.with(|c| c.get())
}

/// Поставить схему текущего узла; возвращает прежнюю.
pub fn set_dark_scheme(dark: bool) -> bool {
    DARK_SCHEME.with(|c| c.replace(dark))
}

impl Color {
    pub fn to_hsla(self) -> gpui::Hsla {
        gpui::Rgba {
            r: self.r,
            g: self.g,
            b: self.b,
            a: self.a,
        }
        .into()
    }

    pub fn parse(raw: &str) -> Option<Self> {
        let s = raw.trim();
        // `light-dark(светлый, тёмный)` (css-color-5 §light-dark): вариант по
        // используемой схеме узла. Прежде функция не разбиралась, и
        // объявление пропадало целиком.
        if s.get(..11).is_some_and(|h| h.eq_ignore_ascii_case("light-dark(")) && s.ends_with(')') {
            let args = crate::css::split_args(&s[11..s.len() - 1]);
            if args.len() == 2 {
                return Self::parse(args[usize::from(dark_scheme())]);
            }
            return None;
        }
        if s.eq_ignore_ascii_case("transparent") {
            return Some(Color {
                r: 0.,
                g: 0.,
                b: 0.,
                a: 0.,
            });
        }
        if let Some(hex) = s.strip_prefix('#') {
            return Self::parse_hex(hex);
        }
        if let Some(inner) = s
            .strip_prefix("rgba(")
            .or_else(|| s.strip_prefix("rgb("))
            .and_then(|v| v.strip_suffix(')'))
        {
            return Self::parse_rgb(inner);
        }
        // `hsl()` — ирония: внутреннее представление GPUI и есть HSL, но записи
        // этой не понимали, и цвет молча терялся.
        if let Some(inner) = s
            .strip_prefix("hsla(")
            .or_else(|| s.strip_prefix("hsl("))
            .and_then(|v| v.strip_suffix(')'))
        {
            return Self::parse_hsl(inner);
        }
        // Записи CSS Color 4 (`lab`, `oklch`, `color()`, `color-mix()` и
        // родня) — своим разбором: они задают цвет в других системах
        // координат, и без настоящего преобразования не приблизить.
        if let Some((r, g, b, a)) = crate::color_space::parse(s) {
            return Some(Color {
                r: r.clamp(0.0, 1.0),
                g: g.clamp(0.0, 1.0),
                b: b.clamp(0.0, 1.0),
                a,
            });
        }
        named(s)
    }

    fn parse_hex(hex: &str) -> Option<Self> {
        let h = hex.trim();
        // Срезы ниже — байтовые: не-ASCII знак (`#aфa`) резал бы UTF-8
        // посреди кода и РОНЯЛ процесс на произвольной странице.
        if !h.is_ascii() {
            return None;
        }
        let byte = |i: usize| {
            u8::from_str_radix(&h[i..i + 2], 16)
                .ok()
                .map(|v| v as f32 / 255.0)
        };
        // Короткая форма `#abc` — каждый разряд удваивается.
        let nib = |i: usize| {
            u8::from_str_radix(&h[i..i + 1], 16)
                .ok()
                .map(|v| (v * 17) as f32 / 255.0)
        };
        match h.len() {
            3 => Some(Color {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: 1.0,
            }),
            4 => Some(Color {
                r: nib(0)?,
                g: nib(1)?,
                b: nib(2)?,
                a: nib(3)?,
            }),
            6 => Some(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: 1.0,
            }),
            8 => Some(Color {
                r: byte(0)?,
                g: byte(2)?,
                b: byte(4)?,
                a: byte(6)?,
            }),
            _ => None,
        }
    }

    /// `hsl(210 40% 50% / 80%)` и `hsl(210, 40%, 50%)`.
    fn parse_hsl(inner: &str) -> Option<Self> {
        // Компонент `calc(<число><ед.> * <число>)` и родня — сворачивается
        // заранее (css-values-4 §10: «calc() … can be used wherever <angle>,
        // <percentage> … are allowed»): `hsl(calc(50deg * 2) 100% 50%)` после
        // подстановки `sibling-index()` (`conic-gradient-color-with-sibling-index`).
        let folded = fold_simple_calc(inner);
        let inner = folded.as_str();
        // `none` — отсутствующий компонент, при счёте он ноль
        // (CSS Color 4 §4.4).
        let cleaned = inner.replace('/', " ").replace("none", "0");
        let parts: Vec<&str> = cleaned
            .split([',', ' '])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        // Тон — угол в любых угловых единицах (CSS Color 4 §7.1).
        let h = {
            let t = parts[0];
            if let Some(n) = t.strip_suffix("grad") {
                n.parse::<f32>().ok()? / 400.0
            } else if let Some(n) = t.strip_suffix("rad") {
                n.parse::<f32>().ok()? / std::f32::consts::TAU
            } else if let Some(n) = t.strip_suffix("turn") {
                n.parse::<f32>().ok()?
            } else {
                t.trim_end_matches("deg").parse::<f32>().ok()? / 360.0
            }
        }
        // Оборот сверх круга заворачивается: `600deg` = 240deg, а зажим в
        // границы делал из него красный.
        .rem_euclid(1.0);
        let s_ = parts[1].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        let l = parts[2].trim_end_matches('%').parse::<f32>().ok()? / 100.0;
        let a = parts.get(3).map_or(Some(1.0), |p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f32>().ok()
            }
        })?;
        let rgba: gpui::Rgba = gpui::hsla(h, s_, l, a).into();
        Some(Color {
            r: rgba.r,
            g: rgba.g,
            b: rgba.b,
            a: rgba.a,
        })
    }

    fn parse_rgb(inner: &str) -> Option<Self> {
        // Принимаем и запятые, и пробельный синтаксис `rgb(1 2 3 / 50%)`.
        // `none` — отсутствующий компонент, при отрисовке он ноль (CSS Color 4
        // §4.4); без этого стоп `rgb(0% 0% none)` выпадал из градиента целиком.
        let cleaned = inner.replace('/', " ").replace("none", "0");
        let parts: Vec<&str> = cleaned
            .split([',', ' '])
            .map(str::trim)
            .filter(|p| !p.is_empty())
            .collect();
        if parts.len() < 3 {
            return None;
        }
        let chan = |p: &str| -> Option<f32> {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f32>().ok().map(|v| v / 255.0)
            }
        };
        let alpha = parts.get(3).map_or(Some(1.0), |p| {
            if let Some(pct) = p.strip_suffix('%') {
                pct.parse::<f32>().ok().map(|v| v / 100.0)
            } else {
                p.parse::<f32>().ok()
            }
        })?;
        // Каналы и альфа зажимаются в допустимый диапазон (css-color-4
        // §5.1: «Values outside these ranges are not invalid, but are clamped
        // … at parsed-value time»). Незажатая альфа 30 красила текст в 30
        // слоёв (`t422-rgba-clamping-a1.0-b`: строки 4–6 жирнее).
        Some(Color {
            r: chan(parts[0])?.clamp(0.0, 1.0),
            g: chan(parts[1])?.clamp(0.0, 1.0),
            b: chan(parts[2])?.clamp(0.0, 1.0),
            a: alpha.clamp(0.0, 1.0),
        })
    }
}

/// Именованные цвета. Полный список CSS — 148 имён; держим те, что реально
/// встречаются в разметке модели и в нашей вёрстке, плюс базовые 16.
fn named(name: &str) -> Option<Color> {
    let rgb = |r: u8, g: u8, b: u8| {
        Some(Color {
            r: r as f32 / 255.0,
            g: g as f32 / 255.0,
            b: b as f32 / 255.0,
            a: 1.0,
        })
    };
    match name.to_ascii_lowercase().as_str() {
        "aliceblue" => rgb(240, 248, 255),
        "antiquewhite" => rgb(250, 235, 215),
        "aqua" | "cyan" => rgb(0, 255, 255),
        "aquamarine" => rgb(127, 255, 212),
        "azure" => rgb(240, 255, 255),
        "beige" => rgb(245, 245, 220),
        "bisque" => rgb(255, 228, 196),
        "black" => rgb(0, 0, 0),
        "blanchedalmond" => rgb(255, 235, 205),
        "blue" => rgb(0, 0, 255),
        "blueviolet" => rgb(138, 43, 226),
        "brown" => rgb(165, 42, 42),
        "burlywood" => rgb(222, 184, 135),
        "cadetblue" => rgb(95, 158, 160),
        "chartreuse" => rgb(127, 255, 0),
        "chocolate" => rgb(210, 105, 30),
        "coral" => rgb(255, 127, 80),
        "cornflowerblue" => rgb(100, 149, 237),
        "cornsilk" => rgb(255, 248, 220),
        "crimson" => rgb(220, 20, 60),
        "darkblue" => rgb(0, 0, 139),
        "darkcyan" => rgb(0, 139, 139),
        "darkgoldenrod" => rgb(184, 134, 11),
        "darkgray" | "darkgrey" => rgb(169, 169, 169),
        "darkgreen" => rgb(0, 100, 0),
        "darkkhaki" => rgb(189, 183, 107),
        "darkmagenta" => rgb(139, 0, 139),
        "darkolivegreen" => rgb(85, 107, 47),
        "darkorange" => rgb(255, 140, 0),
        "darkorchid" => rgb(153, 50, 204),
        "darkred" => rgb(139, 0, 0),
        "darksalmon" => rgb(233, 150, 122),
        "darkseagreen" => rgb(143, 188, 143),
        "darkslateblue" => rgb(72, 61, 139),
        "darkslategray" | "darkslategrey" => rgb(47, 79, 79),
        "darkturquoise" => rgb(0, 206, 209),
        "darkviolet" => rgb(148, 0, 211),
        "deeppink" => rgb(255, 20, 147),
        "deepskyblue" => rgb(0, 191, 255),
        "dimgray" | "dimgrey" => rgb(105, 105, 105),
        "dodgerblue" => rgb(30, 144, 255),
        "firebrick" => rgb(178, 34, 34),
        "floralwhite" => rgb(255, 250, 240),
        "forestgreen" => rgb(34, 139, 34),
        "fuchsia" | "magenta" => rgb(255, 0, 255),
        "gainsboro" => rgb(220, 220, 220),
        "ghostwhite" => rgb(248, 248, 255),
        "gold" => rgb(255, 215, 0),
        "goldenrod" => rgb(218, 165, 32),
        "gray" | "grey" => rgb(128, 128, 128),
        "green" => rgb(0, 128, 0),
        "greenyellow" => rgb(173, 255, 47),
        "honeydew" => rgb(240, 255, 240),
        "hotpink" => rgb(255, 105, 180),
        "indianred" => rgb(205, 92, 92),
        "indigo" => rgb(75, 0, 130),
        "ivory" => rgb(255, 255, 240),
        "khaki" => rgb(240, 230, 140),
        "lavender" => rgb(230, 230, 250),
        "lavenderblush" => rgb(255, 240, 245),
        "lawngreen" => rgb(124, 252, 0),
        "lemonchiffon" => rgb(255, 250, 205),
        "lightblue" => rgb(173, 216, 230),
        "lightcoral" => rgb(240, 128, 128),
        "lightcyan" => rgb(224, 255, 255),
        "lightgoldenrodyellow" => rgb(250, 250, 210),
        "lightgray" | "lightgrey" => rgb(211, 211, 211),
        "lightgreen" => rgb(144, 238, 144),
        "lightpink" => rgb(255, 182, 193),
        "lightsalmon" => rgb(255, 160, 122),
        "lightseagreen" => rgb(32, 178, 170),
        "lightskyblue" => rgb(135, 206, 250),
        "lightslategray" | "lightslategrey" => rgb(119, 136, 153),
        "lightsteelblue" => rgb(176, 196, 222),
        "lightyellow" => rgb(255, 255, 224),
        "lime" => rgb(0, 255, 0),
        "limegreen" => rgb(50, 205, 50),
        "linen" => rgb(250, 240, 230),
        "maroon" => rgb(128, 0, 0),
        "mediumaquamarine" => rgb(102, 205, 170),
        "mediumblue" => rgb(0, 0, 205),
        "mediumorchid" => rgb(186, 85, 211),
        "mediumpurple" => rgb(147, 112, 219),
        "mediumseagreen" => rgb(60, 179, 113),
        "mediumslateblue" => rgb(123, 104, 238),
        "mediumspringgreen" => rgb(0, 250, 154),
        "mediumturquoise" => rgb(72, 209, 204),
        "mediumvioletred" => rgb(199, 21, 133),
        "midnightblue" => rgb(25, 25, 112),
        "mintcream" => rgb(245, 255, 250),
        "mistyrose" => rgb(255, 228, 225),
        "moccasin" => rgb(255, 228, 181),
        "navajowhite" => rgb(255, 222, 173),
        "navy" => rgb(0, 0, 128),
        "oldlace" => rgb(253, 245, 230),
        "olive" => rgb(128, 128, 0),
        "olivedrab" => rgb(107, 142, 35),
        "orange" => rgb(255, 165, 0),
        "orangered" => rgb(255, 69, 0),
        "orchid" => rgb(218, 112, 214),
        "palegoldenrod" => rgb(238, 232, 170),
        "palegreen" => rgb(152, 251, 152),
        "paleturquoise" => rgb(175, 238, 238),
        "palevioletred" => rgb(219, 112, 147),
        "papayawhip" => rgb(255, 239, 213),
        "peachpuff" => rgb(255, 218, 185),
        "peru" => rgb(205, 133, 63),
        "pink" => rgb(255, 192, 203),
        "plum" => rgb(221, 160, 221),
        "powderblue" => rgb(176, 224, 230),
        "purple" => rgb(128, 0, 128),
        "rebeccapurple" => rgb(102, 51, 153),
        "red" => rgb(255, 0, 0),
        "rosybrown" => rgb(188, 143, 143),
        "royalblue" => rgb(65, 105, 225),
        "saddlebrown" => rgb(139, 69, 19),
        "salmon" => rgb(250, 128, 114),
        "sandybrown" => rgb(244, 164, 96),
        "seagreen" => rgb(46, 139, 87),
        "seashell" => rgb(255, 245, 238),
        "sienna" => rgb(160, 82, 45),
        "silver" => rgb(192, 192, 192),
        "skyblue" => rgb(135, 206, 235),
        "slateblue" => rgb(106, 90, 205),
        "slategray" | "slategrey" => rgb(112, 128, 144),
        "snow" => rgb(255, 250, 250),
        "springgreen" => rgb(0, 255, 127),
        "steelblue" => rgb(70, 130, 180),
        "tan" => rgb(210, 180, 140),
        "teal" => rgb(0, 128, 128),
        "thistle" => rgb(216, 191, 216),
        "tomato" => rgb(255, 99, 71),
        "turquoise" => rgb(64, 224, 208),
        "violet" => rgb(238, 130, 238),
        "wheat" => rgb(245, 222, 179),
        "white" => rgb(255, 255, 255),
        "whitesmoke" => rgb(245, 245, 245),
        "yellow" => rgb(255, 255, 0),
        "yellowgreen" => rgb(154, 205, 50),
        other => system(other, rgb),
    }
}

/// Системные цвета CSS Color 4 §6.1 и устаревшие имена §6.2.
///
/// Устаревшее имя обязано давать РОВНО тот же цвет, что и его современная
/// замена: на этом стоит целое семейство проверок (`deprecated-sameas-*`).
/// Сами величины взяты светлой темой браузера — точных значений спецификация
/// не задаёт, важна только согласованность имён между собой.
fn system(name: &str, rgb: impl Fn(u8, u8, u8) -> Option<Color>) -> Option<Color> {
    match name {
        // Полотно страницы и текст на нём.
        "canvas" | "activecaption" | "appworkspace" | "background" | "inactivecaption"
        | "infobackground" | "menu" | "scrollbar" | "window" => rgb(255, 255, 255),
        "canvastext" | "captiontext" | "infotext" | "menutext" | "windowtext" => rgb(0, 0, 0),
        // Кнопки.
        "buttonface" | "buttonhighlight" | "buttonshadow" | "threedface" => rgb(240, 240, 240),
        "buttontext" => rgb(0, 0, 0),
        "buttonborder" | "activeborder" | "inactiveborder" | "threeddarkshadow"
        | "threedhighlight" | "threedlightshadow" | "threedshadow" | "windowframe" => {
            rgb(118, 118, 118)
        }
        // Поля ввода.
        "field" => rgb(255, 255, 255),
        "fieldtext" => rgb(0, 0, 0),
        // Выделение и подсветка.
        "highlight" => rgb(0, 117, 255),
        "highlighttext" => rgb(255, 255, 255),
        "selecteditem" => rgb(0, 117, 255),
        "selecteditemtext" => rgb(255, 255, 255),
        "mark" => rgb(255, 255, 0),
        "marktext" => rgb(0, 0, 0),
        // Ссылки.
        "linktext" => rgb(0, 0, 238),
        "visitedtext" => rgb(85, 26, 139),
        "activetext" => rgb(255, 0, 0),
        // Погашенный текст.
        "graytext" | "inactivecaptiontext" => rgb(109, 109, 109),
        // Цвет выделения оформления.
        "accentcolor" => rgb(0, 117, 255),
        "accentcolortext" => rgb(255, 255, 255),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lengths() {
        assert_eq!(Len::parse("12px"), Some(Len::Px(12.0)));
        assert_eq!(Len::parse(" 1.5rem "), Some(Len::Px(24.0)));
        assert_eq!(Len::parse("50%"), Some(Len::Pct(0.5)));
        assert_eq!(Len::parse("auto"), Some(Len::Auto));
        // Голое число принимаем: модель часто пишет `padding: 8`.
        assert_eq!(Len::parse("8"), Some(Len::Px(8.0)));
        assert_eq!(Len::parse("нет"), None);
    }

    #[test]
    fn colors_hex() {
        assert_eq!(
            Color::parse("#fff"),
            Some(Color {
                r: 1.,
                g: 1.,
                b: 1.,
                a: 1.
            })
        );
        let c = Color::parse("#8ab4f8").unwrap();
        assert!((c.r - 0.541).abs() < 0.01 && (c.b - 0.972).abs() < 0.01);
        assert_eq!(
            Color::parse("#00000080").map(|c| (c.a * 100.).round()),
            Some(50.0)
        );
    }

    #[test]
    fn colors_functions_and_names() {
        assert_eq!(
            Color::parse("rgb(255, 0, 0)"),
            Some(Color {
                r: 1.,
                g: 0.,
                b: 0.,
                a: 1.
            })
        );
        assert_eq!(Color::parse("rgba(0 0 0 / 50%)").map(|c| c.a), Some(0.5));
        assert_eq!(
            Color::parse("teal").map(|c| (c.g * 255.).round()),
            Some(128.0)
        );
        assert_eq!(Color::parse("transparent").map(|c| c.a), Some(0.0));
        assert_eq!(Color::parse("не-цвет"), None);
    }
}

/// Сумма длины по единицам: `calc()` считается покомпонентно, а свернуть её
/// в одну длину получается только когда живой остаётся одна природа.
///
/// Модель раскладки хранит длину как ОДНУ величину: либо доля, либо точки,
/// либо `em`. Поэтому `calc(100% - 24px)` честно отбрасывается — приблизительная
/// длина в сравнении с браузером не видна, а неверная видна.
#[derive(Clone, Copy, Default, PartialEq, Debug)]
pub struct Sum {
    pub lh: f32,
    pub px: f32,
    pub pct: f32,
    pub em: f32,
    pub ch: f32,
    pub ex: f32,
    pub ic: f32,
    /// Высота прописной — единица `cap`. Своего варианта `Len` у неё нет:
    /// природа живёт только в сумме и сворачивается в точки там же, где
    /// `em`/`ch`/`ex`/`ic` (`Computed::resolve_em`).
    pub cap: f32,
    pub vh: f32,
    pub vw: f32,
}

/// Арена смешанных сумм `calc()`: `Len` несёт индекс, не тело. Арена
/// append-only и копеечная (смеси редки); чистится вместе с документом
/// (`calc_reset` на входе разбора) — старые индексы умирают с его деревом.
static CALC_POOL: std::sync::Mutex<Vec<Sum>> = std::sync::Mutex::new(Vec::new());

pub fn calc_store(s: Sum) -> u32 {
    let mut pool = CALC_POOL.lock().unwrap();
    pool.push(s);
    (pool.len() - 1) as u32
}

pub fn calc_get(i: u32) -> Sum {
    CALC_POOL
        .lock()
        .unwrap()
        .get(i as usize)
        .copied()
        .unwrap_or_default()
}

pub fn calc_reset() {
    CALC_POOL.lock().unwrap().clear();
}

/// Сторона якоря в `anchor()` (css-anchor-position-1 §anchor-fn);
/// `center` разбирается как `Pct(0.5)` — спека так и определяет.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnchorSide {
    Inside,
    Outside,
    Top,
    Right,
    Bottom,
    Left,
    Start,
    End,
    SelfStart,
    SelfEnd,
    Pct(f32),
}

/// Мера якоря в `anchor-size()` (css-anchor-position-1 §anchor-size-fn);
/// `Implicit` — ключевое слово опущено: берётся ось свойства.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum AnchorSize {
    Implicit,
    Width,
    Height,
    Block,
    Inline,
    SelfBlock,
    SelfInline,
}

/// Разобранная `anchor()` или `anchor-size()`: имя (нет — якорь по умолчанию
/// из `position-anchor`), сторона, запасное значение и довесок в точках из
/// `calc(anchor(…) + 10px)`. `size` задан — это `anchor-size()`, `side` тогда
/// не читается.
#[derive(Clone, Debug, PartialEq)]
pub struct AnchorFn {
    pub name: Option<String>,
    pub side: AnchorSide,
    pub size: Option<AnchorSize>,
    pub fallback: Option<Len>,
    pub add: f32,
    /// `min(anchor(…), anchor(…), …)` / `max(…)` (css-values-4
    /// §comparison-functions): остальные доводы; сама запись — первый.
    /// Пусто — обычная `anchor()`.
    pub alts: Vec<AnchorFn>,
    /// `max()` при непустых `alts`, иначе `min()`.
    pub max: bool,
}

/// Арена `anchor()` — тем же порядком, что `CALC_POOL`: append-only, `Len`
/// несёт индекс, тело живёт здесь.
static ANCHOR_POOL: std::sync::Mutex<Vec<AnchorFn>> = std::sync::Mutex::new(Vec::new());

pub fn anchor_store(f: AnchorFn) -> u32 {
    let mut pool = ANCHOR_POOL.lock().unwrap();
    pool.push(f);
    (pool.len() - 1) as u32
}

pub fn anchor_get(i: u32) -> Option<AnchorFn> {
    ANCHOR_POOL.lock().unwrap().get(i as usize).cloned()
}

/// Тело `anchor(...)` без скобок: `[<name> || <side>] , <fallback>?`.
/// Запятая ищется на верхнем уровне — запасным значением бывает вложенный
/// `anchor(--a1 bottom)` (`position-anchor-none-pseudo-element-named`).
fn parse_anchor(inner: &str, add: f32, size_fn: bool) -> Option<Len> {
    let mut depth = 0i32;
    let mut cut = None;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                cut = Some(i);
                break;
            }
            _ => {}
        }
    }
    let (head, tail) = match cut {
        Some(i) => (&inner[..i], Some(inner[i + 1..].trim())),
        None => (inner, None),
    };
    let mut name = None;
    let mut side = None;
    let mut size = None;
    for tok in head.split_whitespace() {
        if tok.starts_with("--") {
            name = Some(tok.to_string());
            continue;
        }
        let t = tok.to_ascii_lowercase();
        // `anchor-size()`: вместо стороны — мера (§anchor-size-fn).
        if size_fn {
            size = Some(match t.as_str() {
                "width" => AnchorSize::Width,
                "height" => AnchorSize::Height,
                "block" => AnchorSize::Block,
                "inline" => AnchorSize::Inline,
                "self-block" => AnchorSize::SelfBlock,
                "self-inline" => AnchorSize::SelfInline,
                _ => return None,
            });
            continue;
        }
        side = Some(match t.as_str() {
            "inside" => AnchorSide::Inside,
            "outside" => AnchorSide::Outside,
            "top" => AnchorSide::Top,
            "right" => AnchorSide::Right,
            "bottom" => AnchorSide::Bottom,
            "left" => AnchorSide::Left,
            "start" => AnchorSide::Start,
            "end" => AnchorSide::End,
            "self-start" => AnchorSide::SelfStart,
            "self-end" => AnchorSide::SelfEnd,
            "center" => AnchorSide::Pct(0.5),
            _ => AnchorSide::Pct(css_number(t.strip_suffix('%')?)? / 100.0),
        });
    }
    let fallback = match tail {
        Some(t) if !t.is_empty() => Some(Len::parse(t)?),
        _ => None,
    };
    Some(Len::Anchor(anchor_store(AnchorFn {
        name,
        side: if size_fn { AnchorSide::Inside } else { side? },
        size: size_fn.then(|| size.unwrap_or(AnchorSize::Implicit)),
        fallback,
        add,
        alts: Vec::new(),
        max: false,
    })))
}

/// `min(anchor(…), …)` / `max(anchor(…), …)` во вставке абсолюта
/// (`anchor-in-css-min-max-function`: `top: min(anchor(--a1 bottom),
/// anchor(--a2 bottom), anchor(--a3 top))`). Доводы режутся запятыми
/// верхнего уровня, каждый обязан быть `anchor()` (в том числе
/// `calc(anchor() ± px)`); `anchor-size()` и смесь с длинами — негодны:
/// крайний член сравнивается с краем якоря только в одной системе отсчёта.
fn parse_anchor_minmax(s: &str, max: bool) -> Option<Len> {
    let inner = &s[4..s.len() - 1];
    let mut args = Vec::new();
    let mut depth = 0i32;
    let mut start = 0usize;
    for (i, ch) in inner.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            ',' if depth == 0 => {
                args.push(inner[start..i].trim());
                start = i + 1;
            }
            _ => {}
        }
    }
    args.push(inner[start..].trim());
    let mut fns = Vec::with_capacity(args.len());
    for a in args {
        let Len::Anchor(i) = Len::parse(a)? else {
            return None;
        };
        let f = anchor_get(i)?;
        if f.size.is_some() || !f.alts.is_empty() {
            return None;
        }
        fns.push(f);
    }
    let mut first = fns.remove(0);
    first.alts = fns;
    first.max = max;
    Some(Len::Anchor(anchor_store(first)))
}

/// `calc(anchor(…) ± <length>)`: функция вырезается и заменяется нулём,
/// остаток обязан свернуться в точки — он и становится довеском. Доля или
/// шрифтовая единица рядом с якорем честно не разбирается (запись падает).
fn parse_anchor_calc(s: &str) -> Option<Len> {
    let lower = s.to_ascii_lowercase();
    // `anchor-size(` не содержит подстроки `anchor(` — ветки не путаются.
    let (at, head, size_fn) = match lower.find("anchor-size(") {
        Some(i) => (i, 12, true),
        None => (lower.find("anchor(")?, 7, false),
    };
    let mut depth = 0i32;
    let mut end = None;
    for (i, ch) in s[at..].char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    end = Some(at + i);
                    break;
                }
            }
            _ => {}
        }
    }
    let end = end?;
    let body = &s[at + head..end];
    let rest = format!("{}0px{}", &s[..at], &s[end + 1..]);
    let add = match Len::parse(&rest)? {
        Len::Px(v) => v,
        _ => return None,
    };
    parse_anchor(body, add, size_fn)
}

/// Операнд выражения: голое число участвует только в умножении и делении.
#[derive(Clone, Copy, PartialEq, Debug)]
enum Val {
    Num(f32),
    Len(Sum),
}

impl Sum {
    fn from_len(len: Len) -> Option<Self> {
        let mut s = Sum::default();
        match len {
            Len::Px(v) => s.px = v,
            Len::Pct(v) => s.pct = v,
            Len::Em(v) => s.em = v,
            Len::Ch(v) => s.ch = v,
            Len::Ic(v) => s.ic = v,
            Len::Ex(v) => s.ex = v,
            Len::Lh(v) => s.lh = v,
            Len::LhPx(l, p) => {
                s.lh = l;
                s.px = p;
            }
            Len::EmPx(e, p) => {
                s.em = e;
                s.px = p;
            }
            Len::Vh(v) => s.vh = v,
            Len::Vw(v) => s.vw = v,
            Len::Calc(i) => s = calc_get(i),
            // Якорная вставка в арифметику не входит: её довесок уже внутри
            // `AnchorFn::add`, а сама она решается на подготовке кадра.
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent | Len::Anchor(_) => {
                return None;
            }
        }
        Some(s)
    }

    fn scaled(self, k: f32) -> Self {
        Sum {
            lh: self.lh * k,
            px: self.px * k,
            pct: self.pct * k,
            em: self.em * k,
            ch: self.ch * k,
            ic: self.ic * k,
            ex: self.ex * k,
            cap: self.cap * k,
            vh: self.vh * k,
            vw: self.vw * k,
        }
    }

    fn add(self, other: Self, sign: f32) -> Self {
        Sum {
            lh: self.lh + sign * other.lh,
            px: self.px + sign * other.px,
            pct: self.pct + sign * other.pct,
            em: self.em + sign * other.em,
            ch: self.ch + sign * other.ch,
            ic: self.ic + sign * other.ic,
            ex: self.ex + sign * other.ex,
            cap: self.cap + sign * other.cap,
            vh: self.vh + sign * other.vh,
            vw: self.vw + sign * other.vw,
        }
    }

    /// Единственная живая природа суммы: номер поля и величина. Пустая сумма —
    /// ноль в точках. Две и больше природ — `None`: такие доводы `min()`/`max()`
    /// сравнимы только на раскладке.
    fn nature(self) -> Option<(u8, f32)> {
        let f = [
            self.px, self.pct, self.em, self.ch, self.ex, self.ic, self.cap, self.lh, self.vh,
            self.vw,
        ];
        let mut alive = f.iter().enumerate().filter(|(_, v)| **v != 0.0);
        match (alive.next(), alive.next()) {
            (None, _) => Some((0, 0.0)),
            (Some((i, v)), None) => Some((i as u8, *v)),
            _ => None,
        }
    }

    /// Свёртка в длину: сокращение слагаемых учтено, поэтому
    /// `calc(100% + 6em + 50%*4 - 12em/2)` даёт чистые 300 % — `em` в нём
    /// взаимно уничтожаются.
    pub fn collapse(self) -> Option<Len> {
        // Живая `cap` своего варианта `Len` не имеет — сумма доживает
        // индексом и сворачивается в `resolve_em`, где известны семейство и
        // кегль. Ранний возврат, а НЕ правка веток ниже: те ветки замерены
        // (★ `gap-003-ltr` 0.00 → 4.12), и трогать их из-за новой природы
        // нельзя.
        if self.cap != 0.0 && self.pct == 0.0 {
            return Some(Len::Calc(calc_store(self)));
        }
        let rel = [
            (self.pct, Len::Pct as fn(f32) -> Len),
            (self.em, Len::Em as fn(f32) -> Len),
            (self.ch, Len::Ch as fn(f32) -> Len),
            (self.ic, Len::Ic as fn(f32) -> Len),
            (self.ex, Len::Ex as fn(f32) -> Len),
            (self.vh, Len::Vh as fn(f32) -> Len),
            (self.vw, Len::Vw as fn(f32) -> Len),
        ];
        let mut alive = rel.iter().filter(|(v, _)| *v != 0.0);
        match (alive.next(), alive.next(), self.lh != 0.0) {
            (None, _, false) => Some(Len::Px(self.px)),
            (Some((v, unit)), None, false) if self.px == 0.0 => Some(unit(*v)),
            // Кегльная доля с довеском в точках: разрешится вместе с `em`
            // (text-shadow-orientation-upright-001: `calc(1em + 8px)`).
            (Some((v, unit)), None, false) if matches!(unit(*v), Len::Em(_)) => {
                Some(Len::EmPx(*v, self.px))
            }
            // Кратное строки с довеском в точках: разрешится при слиянии.
            (None, _, true) => Some(if self.px == 0.0 {
                Len::Lh(self.lh)
            } else {
                Len::LhPx(self.lh, self.px)
            }),
            // Смесь природ живёт дальше НЕсвёрнутой: шрифтовые единицы
            // сложит каскад (`resolve_em`), окно — сборщик дерева, а
            // проценты с точками — раскладка (css-values-4 §10.9:
            // «резолвится всё, что уже резолвится»).
            // Смесь природ живёт дальше НЕсвёрнутой: шрифтовые единицы
            // сложит каскад (`resolve_em`), окно — сборщик дерева, а
            // проценты с точками — раскладка (css-values-4 §10.9).
            // Процентная смесь для РАСКЛАДКИ по-прежнему отбрасывается:
            // taffy через gpui её отдать нечем (gpui знает «px ИЛИ доля»), а
            // замена на одну из половин ЗАМЕРЕНА в минус (gap-003-ltr 0.00 ->
            // 4.12 на width: calc(50% - 10px)) — честный путь ждёт таффи-calc.
            // Отрисовка движка (стопы, фон, text-indent) просит смесь явно —
            // `collapse_mixed` через `Len::parse_mixed`.
            _ if self.pct == 0.0 => Some(Len::Calc(calc_store(self))),
            _ => None,
        }
    }

    /// Свёртка, при которой процентная смесь ДОЖИВАЕТ индексом в арене —
    /// для потребителей с известным размером коробки (`Len::parse_mixed`).
    /// `collapse` отдаёт `None` ровно в одном случае — доля вместе с другой
    /// природой, — и только он сюда и попадает.
    pub fn collapse_mixed(self) -> Option<Len> {
        self.collapse().or_else(|| Some(Len::Calc(calc_store(self))))
    }

    /// Смесь ТОЛЬКО долей и точек — парой `(доля, точки)`. Любая другая живая
    /// природа (`ch`, `vw`, `em`…) даёт `None`: складывать её на отрисовке
    /// не с чем, и запись, как прежде, не применяется. Чистые точки и чистая
    /// доля до `Calc` не доживают (их сворачивает `collapse`), поэтому
    /// `pct != 0` здесь — признак смеси, а не пустой суммы.
    pub fn pct_px(self) -> Option<(f32, f32)> {
        let rest = Sum {
            px: 0.0,
            pct: 0.0,
            ..self
        };
        (self.pct != 0.0 && rest == Sum::default()).then_some((self.pct, self.px))
    }

    /// Свёртка для межбуквенного и межсловного интервала: там и доля, и `em`
    /// считаются от кегля, поэтому смешанное `calc(400% + 1em)` складывается
    /// вместо того чтобы пропасть.
    fn collapse_spacing(self) -> Option<Len> {
        Sum {
            pct: 0.0,
            em: self.em + self.pct,
            ..self
        }
        .collapse()
    }
}

/// `expr := term (('+'|'-') term)*`, `term := factor (('*'|'/') factor)*`.
struct Calc<'a> {
    rest: &'a str,
}

impl<'a> Calc<'a> {
    fn eat(&mut self, want: &[char]) -> Option<char> {
        self.rest = self.rest.trim_start();
        let c = self.rest.chars().next()?;
        if !want.contains(&c) {
            return None;
        }
        self.rest = &self.rest[c.len_utf8()..];
        Some(c)
    }

    fn expr(&mut self) -> Option<Val> {
        let mut acc = self.term()?;
        while let Some(op) = self.eat(&['+', '-']) {
            let rhs = self.term()?;
            let sign = if op == '-' { -1.0 } else { 1.0 };
            acc = match (acc, rhs) {
                (Val::Num(a), Val::Num(b)) => Val::Num(a + sign * b),
                (Val::Len(a), Val::Len(b)) => Val::Len(a.add(b, sign)),
                // Число плюс длина — запись без смысла, значение теряется.
                _ => return None,
            };
        }
        Some(acc)
    }

    fn term(&mut self) -> Option<Val> {
        let mut acc = self.factor()?;
        while let Some(op) = self.eat(&['*', '/']) {
            let rhs = self.factor()?;
            acc = match (op, acc, rhs) {
                ('*', Val::Len(a), Val::Num(k)) | ('*', Val::Num(k), Val::Len(a)) => {
                    Val::Len(a.scaled(k))
                }
                ('*', Val::Num(a), Val::Num(b)) => Val::Num(a * b),
                (_, _, Val::Num(k)) if k == 0.0 => return None,
                ('/', Val::Len(a), Val::Num(k)) => Val::Len(a.scaled(1.0 / k)),
                ('/', Val::Num(a), Val::Num(k)) => Val::Num(a / k),
                // Делить на длину и умножать длину на длину нечем.
                _ => return None,
            };
        }
        Some(acc)
    }

    fn factor(&mut self) -> Option<Val> {
        self.rest = self.rest.trim_start();
        // `min()`/`max()`/`clamp()` (css-values-4 §10.2): доводы — полные
        // выражения через запятую, сворачивает их `compare`.
        let rest = self.rest;
        for (name, kind) in [("min(", 0u8), ("max(", 1u8), ("clamp(", 2u8)] {
            if rest
                .get(..name.len())
                .is_some_and(|h| h.eq_ignore_ascii_case(name))
            {
                let mut sub = Calc {
                    rest: &rest[name.len()..],
                };
                let mut args = vec![sub.expr()?];
                while sub.eat(&[',']).is_some() {
                    args.push(sub.expr()?);
                }
                sub.eat(&[')'])?;
                self.rest = sub.rest;
                return compare(kind, &args);
            }
        }
        let inner = self
            .rest
            .strip_prefix('(')
            .map(|r| (r, 1usize))
            .or_else(|| {
                let low = self.rest.get(..5)?;
                low.eq_ignore_ascii_case("calc(")
                    .then(|| (&self.rest[5..], 5usize))
            });
        if let Some((body, open)) = inner {
            let mut sub = Calc { rest: body };
            let val = sub.expr()?;
            sub.eat(&[')'])?;
            let used = self.rest.len() - sub.rest.len();
            debug_assert!(used >= open);
            self.rest = sub.rest;
            return Some(val);
        }
        // Одиночный операнд: знак, число, единица. Разбор единицы отдан
        // `Len::parse`, чтобы правила были ровно одни и те же.
        let end = self
            .rest
            .char_indices()
            .find(|(i, c)| {
                let signed = matches!(c, '+' | '-') && *i == 0;
                !(c.is_ascii_digit() || *c == '.' || c.is_ascii_alphabetic() || *c == '%' || signed)
            })
            .map(|(i, _)| i)
            .unwrap_or(self.rest.len());
        let (head, tail) = self.rest.split_at(end);
        if head.is_empty() {
            return None;
        }
        self.rest = tail;
        if let Ok(n) = head.parse::<f32>() {
            return Some(Val::Num(n));
        }
        Sum::from_len(Len::parse(head)?).map(Val::Len)
    }
}

/// `min()` (0), `max()` (1), `clamp()` (2) над разобранными доводами.
///
/// Сворачивается только однородное (css-values-4 §10.10, упрощение
/// функций сравнения): все доводы — числа, либо все — длины ОДНОЙ природы.
/// Тогда ответ — сам один из доводов, и природа доживает как у `calc()`.
/// Смесь природ (`min(50%, 100px)`) решается только раскладкой — запись, как
/// и смешанный `calc()`, отбрасывается. Число вместе с длиной — несовместимые
/// типы: `min(0, 100%)` недействительно (`max-unitless-zero-invalid`).
fn compare(kind: u8, args: &[Val]) -> Option<Val> {
    if args.is_empty() || (kind == 2 && args.len() != 3) {
        return None;
    }
    let keys: Vec<(u8, f32)> = args
        .iter()
        .map(|a| match a {
            Val::Num(n) => Some((u8::MAX, *n)),
            Val::Len(s) => s.nature(),
        })
        .collect::<Option<_>>()?;
    let nature = keys[0].0;
    if keys.iter().any(|k| k.0 != nature) {
        return None;
    }
    let want = match kind {
        0 => keys.iter().map(|k| k.1).fold(f32::INFINITY, f32::min),
        1 => keys.iter().map(|k| k.1).fold(f32::NEG_INFINITY, f32::max),
        // `clamp(MIN, VAL, MAX)` = `max(MIN, min(VAL, MAX))`: при MIN > MAX
        // побеждает MIN (§10.2).
        _ => keys[1].1.min(keys[2].1).max(keys[0].1),
    };
    let at = keys.iter().position(|k| k.1 == want)?;
    Some(args[at])
}

fn eval_calc(inner: &str) -> Option<Sum> {
    let mut calc = Calc { rest: inner };
    let val = calc.expr()?;
    if !calc.rest.trim().is_empty() {
        return None;
    }
    match val {
        Val::Len(sum) => Some(sum),
        // Голое число длиной не станет: `calc(2 * 3)` — не длина.
        Val::Num(_) => None,
    }
}

/// Интерполяция длин РАЗНЫХ природ покомпонентно, как `calc()` (css-values-4
/// §3.2): середина `0px → 200vw` — `100vw`. Результат сворачивается обычным
/// `collapse`; смесь, которую он не держит (доля вместе с точками), даёт
/// `None` — вызывающий берёт ближайший кадр, как прежде.
pub fn lerp_len(a: Len, b: Len, k: f32) -> Option<Len> {
    let (a, b) = (Sum::from_len(a)?, Sum::from_len(b)?);
    a.scaled(1.0 - k).add(b.scaled(k), 1.0).collapse()
}

/// Число `<number>` или `calc()` из одних чисел (css-values-4 §10.1).
///
/// `font-size-adjust: cap-height calc(1462 / 2048)` — не длина, и
/// `Len::parse` её не берёт: голое число `eval_calc` отбрасывает намеренно.
pub fn number(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
        let mut calc = Calc { rest: inner };
        return match calc.expr()? {
            Val::Num(n) if calc.rest.trim().is_empty() => Some(n),
            _ => None,
        };
    }
    css_number(s)
}

fn parse_calc(inner: &str) -> Option<Len> {
    eval_calc(inner)?.collapse()
}

/// `calc()` из долей и точек — парой `(доля, точки)`, БЕЗ записи в арену.
///
/// Для потребителей на отрисовке (`shape()`, центр `circle()`/`ellipse()`):
/// они зовутся каждый кадр, а арена `CALC_POOL` чистится только вместе с
/// документом — `parse_mixed` копил бы по записи на кадр. Любая другая
/// природа (`em`, `vw`…) — `None`.
pub fn calc_pct_px(raw: &str) -> Option<(f32, f32)> {
    let inner = raw.trim().strip_prefix("calc(")?.strip_suffix(')')?;
    let s = eval_calc(inner)?;
    let rest = Sum {
        px: 0.0,
        pct: 0.0,
        ..s
    };
    (rest == Sum::default()).then_some((s.pct, s.px))
}

#[cfg(test)]
mod calc_tests {
    use super::*;

    #[test]
    fn calc_adds_homogeneous_operands() {
        assert_eq!(Len::parse("calc(100px + 20px)"), Some(Len::Px(120.0)));
        assert_eq!(Len::parse("calc(100% - 25%)"), Some(Len::Pct(0.75)));
    }

    #[test]
    fn mixed_calc_with_percent_is_still_dropped() {
        // Долю и точки сложить пока нечем (замерено на gap-003);
        // шрифтовая смесь живёт.
        assert_eq!(Len::parse("calc(100% - 24px)"), None);
        let Some(Len::Calc(idx)) = Len::parse("calc(120px + 3.1ch)") else {
            panic!("шрифтовая смесь обязана дожить как Calc");
        };
        let s = calc_get(idx);
        assert_eq!((s.px, s.ch), (120.0, 3.1));
    }

    #[test]
    fn mixed_calc_survives_for_paint_consumers() {
        // Потребители с известным размером коробки просят смесь ЯВНО.
        let Some(Len::Calc(idx)) = Len::parse_mixed("calc(100% - 24px)") else {
            panic!("процентная смесь обязана дожить как Calc для parse_mixed");
        };
        assert_eq!(calc_get(idx).pct_px(), Some((1.0, -24.0)));
        // Однородные записи сворачиваются как и прежде.
        assert_eq!(Len::parse_mixed("calc(25% + 25%)"), Some(Len::Pct(0.5)));
        let pair = Len::parse_mixed("calc(200% / 2 - 40px)").and_then(|l| match l {
            Len::Calc(i) => calc_get(i).pct_px(),
            _ => None,
        });
        assert_eq!(pair, Some((1.0, -40.0)));
        // Третья природа в смеси парой не отдаётся.
        let Some(Len::Calc(idx)) = Len::parse_mixed("calc(50% + 1vw)") else {
            panic!("смесь с vw обязана дожить как Calc");
        };
        assert_eq!(calc_get(idx).pct_px(), None);
    }

    #[test]
    fn calc_sums_many_terms_with_precedence() {
        // Слагаемые сокращаются: `6em - 12em/2` даёт ноль, остаётся чистая доля.
        assert_eq!(
            Len::parse("calc(100% + 6em + 50%*4 - 12em/2)"),
            Some(Len::Pct(3.0))
        );
        assert_eq!(Len::parse("calc(25% + 0px)"), Some(Len::Pct(0.25)));
        assert_eq!(Len::parse("calc((2 + 3) * 4px)"), Some(Len::Px(20.0)));
        assert_eq!(Len::parse("calc(-1em * 2)"), Some(Len::Em(-2.0)));
        assert_eq!(Len::parse("calc(2 * 3)"), None);
        assert_eq!(Len::parse("calc(4px / 0)"), None);
    }

    #[test]
    fn spacing_calc_folds_percent_into_em() {
        // Для интервалов доля и `em` считаются от кегля — их можно сложить.
        assert_eq!(Len::parse_spacing("calc(400% + 1em)"), Some(Len::Em(5.0)));
        // Ширина такой свободы не имеет: доля там от контейнера.
        assert_eq!(Len::parse("calc(400% + 1em)"), None);
    }

    #[test]
    fn viewport_units_survive_parsing() {
        assert_eq!(Len::parse("50vw"), Some(Len::Vw(0.5)));
        assert_eq!(Len::parse("100vh"), Some(Len::Vh(1.0)));
    }
}

/// Свернуть простые `calc(a op b)` (одно действие, единица не больше чем у
/// одного операнда) в число с единицей: компоненту цвета больше и не нужно.
/// Сложнее запись остаётся как есть — разбор её отвергнет, как и прежде.
fn fold_simple_calc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut rest = s;
    while let Some(at) = rest.find("calc(") {
        out.push_str(&rest[..at]);
        let body = &rest[at + 5..];
        let Some(end) = body.find(')') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let expr = &body[..end];
        let split = |t: &str| -> Option<(f32, String)> {
            let t = t.trim();
            let cut = t
                .find(|c: char| !(c.is_ascii_digit() || c == '.' || c == '-' || c == '+'))
                .unwrap_or(t.len());
            Some((t[..cut].parse::<f32>().ok()?, t[cut..].to_string()))
        };
        let folded = ['*', '/', '+', '-'].iter().find_map(|op| {
            let (a, b) = expr.split_once(&format!(" {op} "))?;
            let ((x, ux), (y, uy)) = (split(a)?, split(b)?);
            let unit = if ux.is_empty() { uy.clone() } else { ux.clone() };
            if !ux.is_empty() && !uy.is_empty() && matches!(op, '*' | '/') {
                return None;
            }
            let v = match op {
                '*' => x * y,
                '/' if y != 0.0 => x / y,
                '+' if ux == uy => x + y,
                '-' if ux == uy => x - y,
                _ => return None,
            };
            Some(format!("{v}{unit}"))
        });
        match folded {
            Some(v) => out.push_str(&v),
            None => out.push_str(&rest[at..at + 5 + end + 1]),
        }
        rest = &body[end + 1..];
    }
    out.push_str(rest);
    out
}
