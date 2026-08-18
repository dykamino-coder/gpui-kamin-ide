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
    /// `auto`
    Auto,
}

impl Len {
    /// `1rem` = 16px, как в браузере по умолчанию. Свой базовый размер шрифта
    /// мы не задаём: документ рисуется внутри чата, где размер уже выбран.
    const REM_PX: f32 = 16.0;

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
        if let Some(num) = s.strip_suffix('%') {
            return num.trim().parse::<f32>().ok().map(|v| Len::Pct(v / 100.0));
        }
        // `em` разбирается ДО `rem`: иначе `1rem` съедалось бы правилом для
        // `em` вместе с буквой `r`.
        // `calc()` с одним действием над однородными операндами: этого
        // хватает на `calc(100% - 24px)` только когда обе части одной природы,
        // поэтому смешанные записи честно не разбираются — приблизительная
        // длина хуже отсутствующей, её не видно в тесте.
        if let Some(inner) = s.strip_prefix("calc(").and_then(|r| r.strip_suffix(')')) {
            return parse_calc(inner);
        }
        for (suffix, unit) in [
            ("vh", Len::Vh as fn(f32) -> Len),
            ("vw", Len::Vw as fn(f32) -> Len),
        ] {
            if let Some(num) = s.strip_suffix(suffix) {
                return num.trim().parse::<f32>().ok().map(|v| unit(v / 100.0));
            }
        }
        // `ic` — ширина иероглифа (advance у 水). У шрифтов CJK она равна
        // `ic` — продвижение знака `水`, своя метрика шрифта (CSS Values
        // §6.1.4). Синонимом `em` она была только потому, что замера не было:
        // у текстового шрифта иероглиф либо шире кегля, либо его нет вовсе.
        if let Some(num) = s.strip_suffix("ic") {
            return num.trim().parse::<f32>().ok().map(Len::Ic);
        }
        // `ch` и `ex` разбираются в свои единицы: перевести их в точки можно
        // только зная шрифт, а он известен после наследования.
        if let Some(num) = s.strip_suffix("ch") {
            return num.trim().parse::<f32>().ok().map(Len::Ch);
        }
        if let Some(num) = s.strip_suffix("ex") {
            return num.trim().parse::<f32>().ok().map(Len::Ex);
        }
        if let Some(num) = s.strip_suffix("rem") {
            return num
                .trim()
                .parse::<f32>()
                .ok()
                .map(|v| Len::Px(v * Self::REM_PX));
        }
        if let Some(num) = s.strip_suffix("em") {
            return num.trim().parse::<f32>().ok().map(Len::Em);
        }
        if let Some(num) = s.strip_suffix("lh") {
            return num.trim().parse::<f32>().ok().map(Len::Lh);
        }
        for (suffix, factor) in [("px", 1.0), ("pt", 96.0 / 72.0)] {
            if let Some(num) = s.strip_suffix(suffix) {
                return num.trim().parse::<f32>().ok().map(|v| Len::Px(v * factor));
            }
        }
        // Голое число: в CSS допустимо только для 0, но модель часто пишет
        // `padding: 8` — принимаем как px, иначе виджет разъезжается.
        s.parse::<f32>().ok().map(Len::Px)
    }

    /// Разбор для `word-spacing` и `letter-spacing`: там доля берётся от кегля
    /// так же, как `em`, поэтому смешанное `calc(400% + 1em)` складывается в
    /// одну длину вместо того чтобы пропасть (`word-spacing-001`).
    pub fn parse_spacing(raw: &str) -> Option<Self> {
        let s = raw.trim();
        if let Some(inner) = s
            .strip_prefix("calc(")
            .or_else(|| s.strip_prefix("CALC("))
            .and_then(|r| r.strip_suffix(')'))
        {
            return eval_calc(inner)?.collapse_spacing();
        }
        Len::parse(s)
    }
}

/// Цвет в формате GPUI (`Rgba` → `Hsla` конвертируется на месте применения).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
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
        let cleaned = inner.replace('/', " ");
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
        Some(Color {
            r: chan(parts[0])?,
            g: chan(parts[1])?,
            b: chan(parts[2])?,
            a: alpha,
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
struct Sum {
    lh: f32,
    px: f32,
    pct: f32,
    em: f32,
    ch: f32,
    ex: f32,
    ic: f32,
    vh: f32,
    vw: f32,
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
            Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent => return None,
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
            vh: self.vh + sign * other.vh,
            vw: self.vw + sign * other.vw,
        }
    }

    /// Свёртка в длину: сокращение слагаемых учтено, поэтому
    /// `calc(100% + 6em + 50%*4 - 12em/2)` даёт чистые 300 % — `em` в нём
    /// взаимно уничтожаются.
    fn collapse(self) -> Option<Len> {
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
            _ => None,
        }
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

fn parse_calc(inner: &str) -> Option<Len> {
    eval_calc(inner)?.collapse()
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
    fn mixed_calc_is_dropped_rather_than_guessed() {
        // Долю и точки сложить нечем: приблизительная длина хуже отсутствия.
        assert_eq!(Len::parse("calc(100% - 24px)"), None);
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
