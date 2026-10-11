//! Профили ICC (@color-profile): загрузка, теги, матрица и кривые, перевод в sRGB.

use super::*;

/// Применить вшитый цветовой профиль ICC к готовому образу.
///
/// Понимается матричный профиль RGB (v2): колоранты `rXYZ/gXYZ/bXYZ` и
/// кривые `rTRC/gTRC/bTRC` (`curv`: линейная, гамма или таблица). Точки
/// линеаризуются кривыми, матрица ведёт в XYZ D50, дальше та же дорога, что
/// у `lab()`: D50 -> D65 -> линейный sRGB -> гамма. Не разобрался профиль —
/// `None`, образ остаётся как есть.
pub(crate) fn apply_icc(
    image: &std::sync::Arc<gpui::RenderImage>,
    profile: &[u8],
) -> Option<std::sync::Arc<gpui::RenderImage>> {
    let m = icc_matrix(profile)?;
    let curves = [
        icc_curve(profile, b"rTRC")?,
        icc_curve(profile, b"gTRC")?,
        icc_curve(profile, b"bTRC")?,
    ];
    let size = image.size(0);
    let (w, h) = (size.width.0 as u32, size.height.0 as u32);
    let bytes = image.as_bytes(0)?;
    let mut out = Vec::with_capacity(bytes.len());
    for px in bytes.chunks_exact(4) {
        // Порядок BGRA, цвета премультиплицированы; при полной непрозрачности
        // (обычный случай картинок-эталонов) это просто цвет.
        let a = px[3] as f32 / 255.0;
        let un = |v: u8| {
            if a > 0.0 {
                (v as f32 / 255.0 / a).min(1.0)
            } else {
                0.0
            }
        };
        let (b, g, r) = (un(px[0]), un(px[1]), un(px[2]));
        let lin = [
            curve_at(&curves[0], r),
            curve_at(&curves[1], g),
            curve_at(&curves[2], b),
        ];
        let xyz50 = mul(m, lin);
        let xyz = mul(D50_TO_D65, xyz50);
        let srgb = mul(XYZ_TO_LINEAR_SRGB, xyz);
        let (r, g, b) = (
            srgb_gamma(srgb[0]),
            srgb_gamma(srgb[1]),
            srgb_gamma(srgb[2]),
        );
        // Погрешность пути профиль → D50 → D65 → sRGB (округление колорантов
        // в профиле, s15Fixed16) выводит чистые цвета чуть за край охвата:
        // у профиля «sRGB IEC61966-2.1» синий 0000ff выходил (−0.01, 0.003,
        // 1.0x). Охватное отображение (OKLCh) по такой мелочи сдвигало тон —
        // 0033e6 вместо 0000ff (`order-of-images`). Вблизи края — простой
        // зажим; отображение — только настоящему выходу за охват.
        const SLACK: f32 = 0.02;
        let near = |v: f32| (-SLACK..=1.0 + SLACK).contains(&v);
        let (r, g, b) = if near(r) && near(g) && near(b) {
            (r.clamp(0.0, 1.0), g.clamp(0.0, 1.0), b.clamp(0.0, 1.0))
        } else {
            gamut_map(r, g, b)
        };
        out.push((b * a * 255.0).round() as u8);
        out.push((g * a * 255.0).round() as u8);
        out.push((r * a * 255.0).round() as u8);
        out.push(px[3]);
    }
    gpui::bgra_bytes_to_image(w, h, out)
}

// Реестр профилей `@color-profile`: имя (`--foo`) — байты ICC.
//
// Живёт одну страницу, как и подмена шрифтов: имена придумывает страница.
thread_local! {
    pub(super) static PROFILES: std::cell::RefCell<std::collections::HashMap<String, Vec<u8>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Разобрать правила `@color-profile` страницы и загрузить их файлы.
///
/// Путь в `url(...)` берётся как есть: адреса в странице уже разрешены.
pub fn load_profiles(css: &str) {
    PROFILES.with(|p| p.borrow_mut().clear());
    // Комментарии срезаются ДО поиска — как у `@font-face` (fonts::faces).
    let css = &crate::style::css::strip_comments(css);
    let lower = css.to_ascii_lowercase();
    let mut from = 0usize;
    while let Some(at) = lower[from..].find("@color-profile") {
        let start = from + at;
        let Some(open) = css[start..].find('{') else {
            break;
        };
        let name = css[start + "@color-profile".len()..start + open]
            .trim()
            .to_string();
        // Слова `@color-profile` встречаются и в ТЕКСТЕ страницы (заголовок
        // теста): правилом считается только запись с именем `--…` прямо
        // перед скобкой.
        if !name.starts_with("--") || open > 64 {
            from = start + "@color-profile".len();
            continue;
        }
        let Some(close) = css[start + open..].find('}') else {
            break;
        };
        let block = &css[start + open + 1..start + open + close];
        from = start + open + close;
        let Some(src) = block.split(';').find_map(|d| {
            let (k, v) = d.split_once(':')?;
            k.trim()
                .eq_ignore_ascii_case("src")
                .then(|| v.trim().to_string())
        }) else {
            continue;
        };
        let Some(path) = crate::style::computed::parse_url(&src) else {
            continue;
        };
        let clean = path.strip_prefix("file:///").unwrap_or(&path);
        let read = std::fs::read(clean);
        if let Ok(bytes) = read
            && name.starts_with("--")
        {
            PROFILES.with(|p| p.borrow_mut().insert(name, bytes));
        }
    }
}

/// Прогнать цвет через профиль: кривые -> матрица -> XYZ D50 -> sRGB.
pub(super) fn icc_to_srgb(profile: &[u8], c: [f32; 3]) -> Option<(f32, f32, f32)> {
    let m = icc_matrix(profile)?;
    let curves = [
        icc_curve(profile, b"rTRC")?,
        icc_curve(profile, b"gTRC")?,
        icc_curve(profile, b"bTRC")?,
    ];
    let lin = [
        curve_at(&curves[0], c[0]),
        curve_at(&curves[1], c[1]),
        curve_at(&curves[2], c[2]),
    ];
    let xyz = mul(D50_TO_D65, mul(m, lin));
    let srgb = mul(XYZ_TO_LINEAR_SRGB, xyz);
    Some(gamut_map(
        srgb_gamma(srgb[0]),
        srgb_gamma(srgb[1]),
        srgb_gamma(srgb[2]),
    ))
}

/// Найти запись каталога ICC по подписи: (смещение, длина).
fn icc_tag(profile: &[u8], sig: &[u8; 4]) -> Option<(usize, usize)> {
    let be32 = |at: usize| -> Option<u32> {
        Some(u32::from_be_bytes(
            profile.get(at..at + 4)?.try_into().ok()?,
        ))
    };
    let count = be32(128)? as usize;
    for i in 0..count.min(256) {
        let at = 132 + i * 12;
        if profile.get(at..at + 4)? == sig {
            let off = be32(at + 4)? as usize;
            let len = be32(at + 8)? as usize;
            return (profile.len() >= off + len).then_some((off, len));
        }
    }
    None
}

/// Матрица колорантов профиля: столбцы rXYZ/gXYZ/bXYZ (в PCS D50).
fn icc_matrix(profile: &[u8]) -> Option<[f32; 9]> {
    let mut cols = [[0.0f32; 3]; 3];
    for (i, sig) in [b"rXYZ", b"gXYZ", b"bXYZ"].into_iter().enumerate() {
        let (off, _) = icc_tag(profile, sig)?;
        for row in 0..3 {
            let at = off + 8 + row * 4;
            let v = i32::from_be_bytes(profile.get(at..at + 4)?.try_into().ok()?);
            cols[i][row] = v as f32 / 65536.0;
        }
    }
    // Хранение по столбцам -> матрица по строкам.
    Some([
        cols[0][0], cols[1][0], cols[2][0], cols[0][1], cols[1][1], cols[2][1], cols[0][2],
        cols[1][2], cols[2][2],
    ])
}

/// Кривая `curv`: пустая — линейная, одно число — гамма, иначе таблица.
enum IccCurve {
    Gamma(f32),
    Table(Vec<f32>),
}

fn icc_curve(profile: &[u8], sig: &[u8; 4]) -> Option<IccCurve> {
    let (off, _) = icc_tag(profile, sig)?;
    if profile.get(off..off + 4)? != b"curv" {
        return None;
    }
    let count = u32::from_be_bytes(profile.get(off + 8..off + 12)?.try_into().ok()?) as usize;
    match count {
        0 => Some(IccCurve::Gamma(1.0)),
        1 => {
            let v = u16::from_be_bytes(profile.get(off + 12..off + 14)?.try_into().ok()?);
            Some(IccCurve::Gamma(v as f32 / 256.0))
        }
        n => {
            let mut table = Vec::with_capacity(n.min(65536));
            for i in 0..n.min(65536) {
                let at = off + 12 + i * 2;
                let v = u16::from_be_bytes(profile.get(at..at + 2)?.try_into().ok()?);
                table.push(v as f32 / 65535.0);
            }
            Some(IccCurve::Table(table))
        }
    }
}

/// Значение кривой в точке 0..1 (таблица — с интерполяцией).
fn curve_at(c: &IccCurve, v: f32) -> f32 {
    match c {
        IccCurve::Gamma(g) => v.max(0.0).powf(*g),
        IccCurve::Table(t) => {
            if t.len() < 2 {
                return v;
            }
            let x = v.clamp(0.0, 1.0) * (t.len() - 1) as f32;
            let i = (x as usize).min(t.len() - 2);
            let k = x - i as f32;
            t[i] + (t[i + 1] - t[i]) * k
        }
    }
}
