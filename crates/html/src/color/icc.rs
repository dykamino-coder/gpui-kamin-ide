//! Разбор матричного RGB-профиля ICC: каталог тегов, колоранты `rXYZ/gXYZ/bXYZ`
//! и кривые `curv`. Чистое чтение байт профиля, без пикселей и без CSS.

/// Найти запись каталога ICC по подписи: (смещение, длина).
pub(crate) fn icc_tag(profile: &[u8], sig: &[u8; 4]) -> Option<(usize, usize)> {
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
// ICC colorant indices become corresponding columns of the conversion matrix.
pub(crate) fn icc_matrix(profile: &[u8]) -> Option<[f32; 9]> {
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
pub(crate) enum IccCurve {
    Gamma(f32),
    Table(Vec<f32>),
}

pub(crate) fn icc_curve(profile: &[u8], sig: &[u8; 4]) -> Option<IccCurve> {
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
pub(crate) fn curve_at(c: &IccCurve, v: f32) -> f32 {
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
