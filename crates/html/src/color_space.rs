//! Цветовые пространства CSS Color 4: `hwb()`, `lab()`, `lch()`, `oklab()`,
//! `oklch()`, `color()` и `color-mix()`.
//!
//! Зачем отдельным разбором. Все эти записи задают ОДИН И ТОТ ЖЕ цвет разными
//! системами координат, и привести их к точкам экрана можно только через
//! настоящее преобразование: сначала в XYZ, оттуда в линейный sRGB, оттуда —
//! степенной кривой в sRGB. Приблизить их подстановкой нельзя: `lab(50% 40 60)`
//! и близко не похож ни на один компонент записи.
//!
//! Числа матриц — из спецификации (CSS Color 4, приложение о преобразованиях).
//! Менять их «на глаз» нельзя: они согласованы между собой и с точками белого.

/// Точка белого D50 в XYZ — от неё считаются `lab`, `lch` и ProPhoto.
const D50: [f32; 3] = [0.964_295_7, 1.0, 0.825_104_6];

/// XYZ (D65) → линейный sRGB.
const XYZ_TO_LINEAR_SRGB: [f32; 9] = [
    3.240_97, -1.537_383_2, -0.498_610_76, -0.969_243_65, 1.875_967_5, 0.041_555_06, 0.055_630_08,
    -0.203_976_96, 1.056_971_5,
];

/// Линейный sRGB → XYZ (D65).
const LINEAR_SRGB_TO_XYZ: [f32; 9] = [
    0.412_390_8, 0.357_584_33, 0.180_480_79, 0.212_639, 0.715_168_65, 0.072_192_32, 0.019_330_82,
    0.119_194_78, 0.950_532_15,
];

/// XYZ D50 → XYZ D65 (преобразование Брэдфорда).
const D50_TO_D65: [f32; 9] = [
    0.955_473_45,
    -0.023_098_537,
    0.063_259_31,
    -0.028_369_707,
    1.009_995_5,
    0.021_041_399,
    0.012_314_002,
    -0.020_507_697,
    1.330_365_9,
];

/// Линейный Display P3 → XYZ (D65).
const P3_TO_XYZ: [f32; 9] = [
    0.486_570_95,
    0.265_667_7,
    0.198_217_28,
    0.228_974_56,
    0.691_738_5,
    0.079_286_91,
    0.0,
    0.045_113_38,
    1.043_944_4,
];

/// Линейный Adobe RGB (1998) → XYZ (D65).
const A98_TO_XYZ: [f32; 9] = [
    0.576_669,
    0.185_558_24,
    0.188_228_65,
    0.297_345,
    0.627_363_6,
    0.075_291_46,
    0.027_031_361,
    0.070_688_85,
    0.991_337_53,
];

/// Линейный ProPhoto → XYZ (D50).
const PROPHOTO_TO_XYZ: [f32; 9] = [
    0.797_760_5,
    0.135_185_84,
    0.031_349_35,
    0.288_071_13,
    0.711_843_2,
    0.000_085_654,
    0.0,
    0.0,
    0.825_104_6,
];

/// Линейный Rec. 2020 → XYZ (D65).
const REC2020_TO_XYZ: [f32; 9] = [
    0.636_958,
    0.144_616_9,
    0.168_881,
    0.262_700_2,
    0.677_998_1,
    0.059_301_71,
    0.0,
    0.028_072_693,
    1.060_985_1,
];

fn mul(m: [f32; 9], v: [f32; 3]) -> [f32; 3] {
    [
        m[0] * v[0] + m[1] * v[1] + m[2] * v[2],
        m[3] * v[0] + m[4] * v[1] + m[5] * v[2],
        m[6] * v[0] + m[7] * v[1] + m[8] * v[2],
    ]
}

/// Кривая sRGB: линейная величина → величина записи.
fn srgb_gamma(v: f32) -> f32 {
    let sign = v.signum();
    let v = v.abs();
    sign * if v <= 0.003_130_8 {
        12.92 * v
    } else {
        1.055 * v.powf(1.0 / 2.4) - 0.055
    }
}

/// Обратная кривая sRGB: величина записи → линейная.
fn srgb_linear(v: f32) -> f32 {
    let sign = v.signum();
    let v = v.abs();
    sign * if v <= 0.040_45 {
        v / 12.92
    } else {
        ((v + 0.055) / 1.055).powf(2.4)
    }
}

/// Разложить содержимое записи на числа и долю прозрачности.
///
/// Косая отделяет прозрачность, запятые и пробелы равноправны — так велит
/// современная запись (CSS Color 4 §4).
fn parts(inner: &str) -> (Vec<String>, f32) {
    let (body, alpha) = match inner.split_once('/') {
        Some((body, a)) => (body, number(a.trim(), 1.0).unwrap_or(1.0)),
        None => (inner, 1.0),
    };
    let list = body
        .split([',', ' ', '\t', '\n'])
        .map(str::trim)
        .filter(|p| !p.is_empty())
        .map(str::to_string)
        .collect();
    (list, alpha.clamp(0.0, 1.0))
}

/// Число записи: доля от базы, `none` — ноль, обычное число — как есть.
fn number(raw: &str, base: f32) -> Option<f32> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(0.0);
    }
    if let Some(pct) = raw.strip_suffix('%') {
        return pct.parse::<f32>().ok().map(|v| v / 100.0 * base);
    }
    raw.trim_end_matches("deg").parse().ok()
}

/// Разобрать одну из записей CSS Color 4. `None` — запись не наша.
pub fn parse(raw: &str) -> Option<(f32, f32, f32, f32)> {
    let text = raw.trim();
    let lower = text.to_ascii_lowercase();
    let inner = |name: &str| -> Option<&str> {
        lower
            .starts_with(&format!("{name}("))
            .then(|| text[name.len() + 1..text.len().saturating_sub(1)].trim())
    };
    if let Some(body) = inner("hwb") {
        return hwb(body);
    }
    if let Some(body) = inner("lab") {
        return lab(body, false);
    }
    if let Some(body) = inner("oklab") {
        return lab(body, true);
    }
    if let Some(body) = inner("lch") {
        return lch(body, false);
    }
    if let Some(body) = inner("oklch") {
        return lch(body, true);
    }
    if let Some(body) = inner("color") {
        return color_fn(body);
    }
    if let Some(body) = inner("color-mix") {
        return color_mix(body);
    }
    None
}

/// `hwb(H W B[/A])` — тон, подмешанная белизна и чернота (§7).
fn hwb(body: &str) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 3 {
        return None;
    }
    let h = number(&list[0], 1.0)? / 360.0;
    let w = number(&list[1], 1.0)?.clamp(0.0, 1.0);
    let b = number(&list[2], 1.0)?.clamp(0.0, 1.0);
    // Белизна с чернотой, вместе перекрывающие целое, дают ровно серый.
    if w + b >= 1.0 {
        let gray = w / (w + b);
        return Some((gray, gray, gray, a));
    }
    let rgba: gpui::Rgba = gpui::hsla(h, 1.0, 0.5, 1.0).into();
    let mix = |c: f32| c * (1.0 - w - b) + w;
    Some((mix(rgba.r), mix(rgba.g), mix(rgba.b), a))
}

/// `lab()`/`oklab()`: светлота и две оси цветности.
fn lab(body: &str, ok: bool) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 3 {
        return None;
    }
    // Доля светлоты считается от 100 в CIE Lab и от единицы в OKLab.
    // Светлота ЗАЖИМАЕТСЯ в свой диапазон (CSS Color 4 §9.2): значения
    // сверх сотни законны в записи, но обрезаются при вычислении.
    let l = number(&list[0], if ok { 1.0 } else { 100.0 })?.clamp(0.0, if ok { 1.0 } else { 100.0 });
    let x = number(&list[1], if ok { 0.4 } else { 125.0 })?;
    let y = number(&list[2], if ok { 0.4 } else { 125.0 })?;
    let (r, g, b) = if ok {
        oklab_to_srgb(l, x, y)
    } else {
        lab_to_srgb(l, x, y)
    };
    let (r, g, b) = gamut_map(r, g, b);
    Some((r, g, b, a))
}

/// `lch()`/`oklch()`: та же светлота, но цветность задана длиной и углом.
fn lch(body: &str, ok: bool) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 3 {
        return None;
    }
    let l = number(&list[0], if ok { 1.0 } else { 100.0 })?.clamp(0.0, if ok { 1.0 } else { 100.0 });
    let c = number(&list[1], if ok { 0.4 } else { 150.0 })?;
    let h = number(&list[2], 1.0)?.to_radians();
    let (x, y) = (c * h.cos(), c * h.sin());
    let (r, g, b) = if ok {
        oklab_to_srgb(l, x, y)
    } else {
        lab_to_srgb(l, x, y)
    };
    let (r, g, b) = gamut_map(r, g, b);
    Some((r, g, b, a))
}

fn lab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    // Lab → XYZ при точке белого D50 (§10.3).
    const K: f32 = 24389.0 / 27.0;
    const E: f32 = 216.0 / 24389.0;
    let fy = (l + 16.0) / 116.0;
    let fx = a / 500.0 + fy;
    let fz = fy - b / 200.0;
    let cube = |f: f32, k: f32| {
        let c = f * f * f;
        if c > E { c } else { (116.0 * f - 16.0) / K }
    };
    let _ = K;
    let x = cube(fx, 0.0) * D50[0];
    let y = if l > K * E {
        fy * fy * fy * D50[1]
    } else {
        l / K * D50[1]
    };
    let z = cube(fz, 0.0) * D50[2];
    let xyz = mul(D50_TO_D65, [x, y, z]);
    let lin = mul(XYZ_TO_LINEAR_SRGB, xyz);
    (
        srgb_gamma(lin[0]),
        srgb_gamma(lin[1]),
        srgb_gamma(lin[2]),
    )
}

fn oklab_to_srgb(l: f32, a: f32, b: f32) -> (f32, f32, f32) {
    // OKLab устроен так, что до линейного sRGB доходит без XYZ (§9.2).
    let l_ = l + 0.396_337_78 * a + 0.215_803_76 * b;
    let m_ = l - 0.105_561_346 * a - 0.063_854_17 * b;
    let s_ = l - 0.089_484_18 * a - 1.291_485_5 * b;
    let (l3, m3, s3) = (l_ * l_ * l_, m_ * m_ * m_, s_ * s_ * s_);
    let r = 4.076_741_7 * l3 - 3.307_711_6 * m3 + 0.230_969_94 * s3;
    let g = -1.268_438 * l3 + 2.609_757_4 * m3 - 0.341_319_38 * s3;
    let b = -0.004_196_086_3 * l3 - 0.703_418_6 * m3 + 1.707_614_7 * s3;
    (srgb_gamma(r), srgb_gamma(g), srgb_gamma(b))
}

/// Линейный sRGB (возможно, вне охвата) → OKLab.
fn linear_srgb_to_oklab(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let l = 0.412_221_47 * r + 0.536_332_54 * g + 0.051_445_995 * b;
    let m = 0.211_903_5 * r + 0.680_699_55 * g + 0.107_396_96 * b;
    let s = 0.088_302_46 * r + 0.281_718_84 * g + 0.629_978_7 * b;
    let (l_, m_, s_) = (l.cbrt(), m.cbrt(), s.cbrt());
    (
        0.210_454_26 * l_ + 0.793_617_79 * m_ - 0.004_072_047 * s_,
        1.977_998_5 * l_ - 2.428_592_2 * m_ + 0.450_593_7 * s_,
        0.025_904_037 * l_ + 0.782_771_77 * m_ - 0.808_675_77 * s_,
    )
}

/// Втягивание цвета в охват sRGB СЖАТИЕМ ЦВЕТНОСТИ (CSS Color 4 §13.1.5).
///
/// Каналы за пределами [0,1] нельзя просто отрезать: срез меняет и тон, и
/// светлоту (`lch(100% 110 60)` обязан стать БЕЛЫМ, а срез давал жёлтый).
/// Спекой задано уменьшение цветности в OKLCh до входа в охват; светлота с
/// краёв диапазона решается сразу.
fn gamut_map(r: f32, g: f32, b: f32) -> (f32, f32, f32) {
    let eps = 1e-4;
    let inside =
        |v: (f32, f32, f32)| (-eps..=1.0 + eps).contains(&v.0) && (-eps..=1.0 + eps).contains(&v.1) && (-eps..=1.0 + eps).contains(&v.2);
    if inside((r, g, b)) {
        return (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0));
    }
    let (l, a, bb) = linear_srgb_to_oklab(srgb_linear(r), srgb_linear(g), srgb_linear(b));
    if l >= 1.0 {
        return (1.0, 1.0, 1.0);
    }
    if l <= 0.0 {
        return (0.0, 0.0, 0.0);
    }
    let c0 = (a * a + bb * bb).sqrt();
    let h = bb.atan2(a);
    let (mut lo, mut hi) = (0.0f32, c0);
    for _ in 0..24 {
        let c = (lo + hi) / 2.0;
        let v = oklab_to_srgb(l, c * h.cos(), c * h.sin());
        if inside(v) {
            lo = c;
        } else {
            hi = c;
        }
    }
    let v = oklab_to_srgb(l, lo * h.cos(), lo * h.sin());
    (v.0.clamp(0.0, 1.0), v.1.clamp(0.0, 1.0), v.2.clamp(0.0, 1.0))
}

/// `color(<пространство> c1 c2 c3[/A])` (§10).
fn color_fn(body: &str) -> Option<(f32, f32, f32, f32)> {
    let (list, a) = parts(body);
    if list.len() < 4 {
        return None;
    }
    let space = list[0].to_ascii_lowercase();
    let c = [
        number(&list[1], 1.0)?,
        number(&list[2], 1.0)?,
        number(&list[3], 1.0)?,
    ];
    let lin = |v: [f32; 3]| [srgb_linear(v[0]), srgb_linear(v[1]), srgb_linear(v[2])];
    let xyz = match space.as_str() {
        "srgb" => return Some((c[0], c[1], c[2], a)),
        "srgb-linear" => mul(LINEAR_SRGB_TO_XYZ, c),
        "display-p3" => mul(P3_TO_XYZ, lin(c)),
        // Линейный вариант: те же основные цвета, но без кривой.
        "display-p3-linear" => mul(P3_TO_XYZ, c),
        "rec2020-linear" => mul(REC2020_TO_XYZ, c),
        "a98-rgb-linear" => mul(A98_TO_XYZ, c),
        "prophoto-rgb-linear" => mul(D50_TO_D65, mul(PROPHOTO_TO_XYZ, c)),
        // У Adobe RGB своя степень кривой, простая и без прямого куска.
        "a98-rgb" => {
            let g = |v: f32| v.signum() * v.abs().powf(563.0 / 256.0);
            mul(A98_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])])
        }
        "prophoto-rgb" => {
            let g = |v: f32| {
                let s = v.signum();
                let v = v.abs();
                s * if v <= 16.0 / 512.0 {
                    v / 16.0
                } else {
                    v.powf(1.8)
                }
            };
            // ProPhoto считается при D50 — переводим к D65.
            mul(D50_TO_D65, mul(PROPHOTO_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])]))
        }
        "rec2020" => {
            const A: f32 = 1.099_296_8;
            const B: f32 = 0.018_053_97;
            let g = |v: f32| {
                let s = v.signum();
                let v = v.abs();
                s * if v < B * 4.5 {
                    v / 4.5
                } else {
                    ((v + A - 1.0) / A).powf(1.0 / 0.45)
                }
            };
            mul(REC2020_TO_XYZ, [g(c[0]), g(c[1]), g(c[2])])
        }
        "xyz" | "xyz-d65" => c,
        "xyz-d50" => mul(D50_TO_D65, c),
        _ => return None,
    };
    let out = mul(XYZ_TO_LINEAR_SRGB, xyz);
    // Втягивание в охват тем же сжатием цветности, что у lab/lch: пары
    // «lab против color(display-p3 …)» обязаны сходиться в ОДИН цвет sRGB.
    let (r, g, b) = gamut_map(srgb_gamma(out[0]), srgb_gamma(out[1]), srgb_gamma(out[2]));
    Some((r, g, b, a))
}

/// `color-mix(in <пространство>, <цвет> <доля>?, <цвет> <доля>?)` (§12).
///
/// Смешиваем в sRGB независимо от заявленного пространства: разница видна
/// лишь на насыщенных парах, а без смешивания цвета нет вовсе.
fn color_mix(body: &str) -> Option<(f32, f32, f32, f32)> {
    let mut it = crate::css::split_args(body).into_iter();
    let _space = it.next()?;
    let one = |raw: &str| -> Option<((f32, f32, f32, f32), Option<f32>)> {
        let raw = raw.trim();
        // Доля стоит рядом с цветом и записывается процентом.
        let (color, share) = match raw.rfind('%') {
            Some(at) => {
                let head = raw[..at].trim_end();
                let cut = head.rfind(char::is_whitespace).map(|i| i + 1).unwrap_or(0);
                let pct: f32 = head[cut..].parse().ok()?;
                (raw[..cut].trim(), Some(pct / 100.0))
            }
            None => (raw, None),
        };
        let c = crate::value::Color::parse(color)?;
        Some(((c.r, c.g, c.b, c.a), share))
    };
    let (first, p1) = one(&it.next()?)?;
    let (second, p2) = one(&it.next()?)?;
    let (w1, w2) = match (p1, p2) {
        (Some(a), Some(b)) if a + b > 0.0 => (a / (a + b), b / (a + b)),
        (Some(a), None) => (a, 1.0 - a),
        (None, Some(b)) => (1.0 - b, b),
        _ => (0.5, 0.5),
    };
    Some((
        first.0 * w1 + second.0 * w2,
        first.1 * w1 + second.1 * w2,
        first.2 * w1 + second.2 * w2,
        first.3 * w1 + second.3 * w2,
    ))
}


/// Относительный цвет (css-color-5 §4): `hsl(from currentColor h s l)`.
///
/// Тождественные каналы дают сам базовый цвет: преобразование туда-обратно в
/// плавающих пространствах без потерь. Подстановки каналов считаются честно
/// для `rgb()` и `hsl()` — прочим пространствам хватает тождества: обратные
/// преобразования им пока не заведены.
pub(crate) fn resolve_relative(
    expr: &str,
    current: crate::value::Color,
) -> Option<crate::value::Color> {
    use crate::value::Color;
    // Голое слово: цвет текста этого же элемента.
    if expr.eq_ignore_ascii_case("currentcolor") {
        return Some(current);
    }
    // `color-mix` с `currentColor`: слово подставляется уже решённым цветом,
    // дальше работает обычный разбор смеси.
    let low = expr.to_ascii_lowercase();
    if low.starts_with("color-mix(") {
        let rgb = format!(
            "rgb({} {} {} / {})",
            (current.r * 255.0).round(),
            (current.g * 255.0).round(),
            (current.b * 255.0).round(),
            current.a
        );
        return Color::parse(&low.replace("currentcolor", &rgb));
    }
    let open = expr.find('(')?;
    let name = expr[..open].trim();
    let inner = expr[open + 1..].trim().strip_suffix(')')?;
    let rest = inner.trim().strip_prefix("from ")?;
    let words = crate::computed::split_outside_parens(rest);
    let base_tok = words.first()?;
    let base = if base_tok.eq_ignore_ascii_case("currentcolor") {
        current
    } else {
        Color::parse(base_tok)?
    };
    // Прозрачность после косой: `alpha` — своя, число — новая.
    let mut channels: Vec<&str> = vec![];
    let mut alpha = base.a;
    let mut after_slash = false;
    for w in words[1..].iter().map(|w| w.as_str()) {
        if w == "/" {
            after_slash = true;
        } else if after_slash {
            alpha = match w {
                "alpha" => base.a,
                t => t
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                    .or_else(|| t.parse().ok())
                    .unwrap_or(base.a),
            };
        } else {
            channels.push(w);
        }
    }
    let identity: &[&str] = match name {
        "rgb" | "rgba" => &["r", "g", "b"],
        "hsl" | "hsla" => &["h", "s", "l"],
        "hwb" => &["h", "w", "b"],
        "lab" | "oklab" => &["l", "a", "b"],
        "lch" | "oklch" => &["l", "c", "h"],
        "color" => {
            // Первый канал — имя пространства.
            let space = channels.first().copied().unwrap_or("");
            let rest = &channels[1..];
            let names: &[&str] = if space.starts_with("xyz") {
                &["x", "y", "z"]
            } else {
                &["r", "g", "b"]
            };
            return (rest == names).then_some(Color { a: alpha, ..base });
        }
        _ => return None,
    };
    if channels == identity {
        return Some(Color { a: alpha, ..base });
    }
    match name {
        "rgb" | "rgba" => {
            let chan = |t: &str| match t {
                "r" => Some(base.r),
                "g" => Some(base.g),
                "b" => Some(base.b),
                t => t
                    .strip_suffix('%')
                    .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                    .or_else(|| t.parse::<f32>().ok().map(|v| v / 255.0)),
            };
            Some(Color {
                r: chan(channels.first()?)?.clamp(0.0, 1.0),
                g: chan(channels.get(1)?)?.clamp(0.0, 1.0),
                b: chan(channels.get(2)?)?.clamp(0.0, 1.0),
                a: alpha,
            })
        }
        "hsl" | "hsla" => {
            let (h, s, l) = rgb_to_hsl(base);
            let chan = |t: &str, own: f32, pct: bool| match t {
                "h" => Some(h),
                "s" => Some(s),
                "l" => Some(l),
                t => {
                    let _ = own;
                    if pct {
                        t.strip_suffix('%')
                            .and_then(|n| n.parse::<f32>().ok().map(|v| v / 100.0))
                            .or_else(|| t.parse().ok())
                    } else {
                        t.strip_suffix("deg")
                            .unwrap_or(t)
                            .parse::<f32>()
                            .ok()
                    }
                }
            };
            let (h, s, l) = (
                chan(channels.first()?, h, false)?,
                chan(channels.get(1)?, s, true)?.clamp(0.0, 1.0),
                chan(channels.get(2)?, l, true)?.clamp(0.0, 1.0),
            );
            let (r, g, b) = hsl_to_rgb(h, s, l);
            Some(Color { r, g, b, a: alpha })
        }
        _ => None,
    }
}

/// sRGB → HSL: тон в градусах, насыщенность и светлота в долях.
fn rgb_to_hsl(c: crate::value::Color) -> (f32, f32, f32) {
    let (max, min) = (c.r.max(c.g).max(c.b), c.r.min(c.g).min(c.b));
    let l = (max + min) / 2.0;
    if (max - min).abs() < 1e-6 {
        return (0.0, 0.0, l);
    }
    let d = max - min;
    let s = if l > 0.5 { d / (2.0 - max - min) } else { d / (max + min) };
    let h = if (max - c.r).abs() < 1e-6 {
        ((c.g - c.b) / d).rem_euclid(6.0)
    } else if (max - c.g).abs() < 1e-6 {
        (c.b - c.r) / d + 2.0
    } else {
        (c.r - c.g) / d + 4.0
    };
    (h * 60.0, s, l)
}

/// HSL → sRGB.
fn hsl_to_rgb(h: f32, s: f32, l: f32) -> (f32, f32, f32) {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let hp = h.rem_euclid(360.0) / 60.0;
    let x = c * (1.0 - (hp % 2.0 - 1.0).abs());
    let (r, g, b) = match hp as u32 {
        0 => (c, x, 0.0),
        1 => (x, c, 0.0),
        2 => (0.0, c, x),
        3 => (0.0, x, c),
        4 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    (r + m, g + m, b + m)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Сравнение идёт по ЗАЖАТЫМ величинам: цвет шире sRGB выходит за
    /// пределы, и зажимает его разбор цвета, а не преобразование.
    fn close(a: (f32, f32, f32, f32), b: (f32, f32, f32), what: &str) {
        let a = (
            a.0.clamp(0.0, 1.0),
            a.1.clamp(0.0, 1.0),
            a.2.clamp(0.0, 1.0),
            a.3,
        );
        for (got, want) in [(a.0, b.0), (a.1, b.1), (a.2, b.2)] {
            assert!(
                (got - want).abs() < 0.02,
                "{what}: {got} против {want}"
            );
        }
    }

    /// Белый и красный обязаны совпасть во всех записях.
    #[test]
    fn known_colors_survive_the_conversion() {
        close(parse("lab(100% 0 0)").unwrap(), (1.0, 1.0, 1.0), "lab белый");
        close(
            parse("oklch(0.628 0.2577 29.23)").unwrap(),
            (1.0, 0.0, 0.0),
            "oklch красный",
        );
        close(
            parse("color(display-p3 0 1 0)").unwrap(),
            (0.0, 1.0, 0.0),
            "p3 зелёный шире sRGB",
        );
        close(
            parse("color(srgb 0.2 0.4 0.6)").unwrap(),
            (0.2, 0.4, 0.6),
            "srgb как есть",
        );
    }

    /// `hwb`: белизна с чернотой в сумме за единицу дают серый.
    #[test]
    fn hwb_mixes_white_and_black() {
        close(parse("hwb(0 100% 0%)").unwrap(), (1.0, 1.0, 1.0), "белый");
        close(parse("hwb(0 0% 100%)").unwrap(), (0.0, 0.0, 0.0), "чёрный");
        close(parse("hwb(0 50% 50%)").unwrap(), (0.5, 0.5, 0.5), "серый");
    }

    /// Доли смешивания приводятся к единице.
    #[test]
    fn color_mix_honours_shares() {
        close(
            parse("color-mix(in srgb, white 25%, black 75%)").unwrap(),
            (0.25, 0.25, 0.25),
            "четверть белого",
        );
    }
}
