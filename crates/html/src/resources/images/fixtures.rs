//! Байтовые образцы для тестов декодера, собранные в коде: PNG с точками по координатам,
//! вставка кусков `eXIf`/`iCCP` (CRC-32 и zlib «stored» вручную), JPEG с EXIF в `APP1`,
//! матричный профиль ICC с основными цветами sRGB (D50) и линейной кривой.

/// Точка `(x, y)` образца: R = x, G = y, B = 200, A = 255 (каждая точка различима).
pub(crate) fn pixel_at(x: u32, y: u32) -> [u8; 4] {
    [x as u8 * 40, y as u8 * 40, 200, 255]
}

/// PNG `w×h` RGBA с точками `pixel_at`.
pub(crate) fn png(w: u32, h: u32) -> Vec<u8> {
    use image::ImageEncoder;
    let mut raw = Vec::new();
    for y in 0..h {
        for x in 0..w {
            raw.extend_from_slice(&pixel_at(x, y));
        }
    }
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&raw, w, h, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

/// Сплошной PNG одного цвета.
pub(crate) fn solid_png(rgba: [u8; 4]) -> Vec<u8> {
    use image::ImageEncoder;
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&rgba, 1, 1, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

/// Вставить кусок PNG сразу после `IHDR` (сигнатура 8 байт + IHDR 25 байт).
pub(crate) fn with_chunk(png: &[u8], kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut chunk = Vec::new();
    chunk.extend_from_slice(&(data.len() as u32).to_be_bytes());
    chunk.extend_from_slice(kind);
    chunk.extend_from_slice(data);
    chunk.extend_from_slice(&crc32(&chunk[4..]).to_be_bytes());
    let mut out = png[..33].to_vec();
    out.extend_from_slice(&chunk);
    out.extend_from_slice(&png[33..]);
    out
}

/// Блок TIFF (big-endian) с одной записью `Orientation`.
pub(crate) fn tiff_orientation(tag: u16) -> Vec<u8> {
    let mut t = b"MM\0\x2A".to_vec();
    t.extend_from_slice(&8u32.to_be_bytes());
    t.extend_from_slice(&1u16.to_be_bytes());
    t.extend_from_slice(&0x0112u16.to_be_bytes());
    t.extend_from_slice(&3u16.to_be_bytes());
    t.extend_from_slice(&1u32.to_be_bytes());
    t.extend_from_slice(&tag.to_be_bytes());
    t.extend_from_slice(&[0, 0]);
    t.extend_from_slice(&0u32.to_be_bytes());
    t
}

/// JPEG `w×h` с EXIF-меткой ориентации в сегменте `APP1` сразу после `SOI`.
pub(crate) fn jpeg_with_orientation(w: u32, h: u32, tag: u16) -> Vec<u8> {
    let raw = vec![128u8; (w * h * 3) as usize];
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new(&mut jpeg)
        .encode(&raw, w, h, image::ExtendedColorType::Rgb8)
        .unwrap();
    let mut body = b"Exif\0\0".to_vec();
    body.extend_from_slice(&tiff_orientation(tag));
    let mut out = jpeg[..2].to_vec();
    out.extend_from_slice(&[0xFF, 0xE1]);
    out.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(&body);
    out.extend_from_slice(&jpeg[2..]);
    out
}

/// Тело `iCCP`: имя, нуль, метод сжатия 0 и zlib-поток профиля.
pub(crate) fn iccp(profile: &[u8]) -> Vec<u8> {
    let mut out = b"test\0\0".to_vec();
    out.extend_from_slice(&[0x78, 0x01, 0x01]);
    let len = profile.len() as u16;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(&(!len).to_le_bytes());
    out.extend_from_slice(profile);
    out.extend_from_slice(&adler32(profile).to_be_bytes());
    out
}

/// Матричный профиль: основные цвета sRGB, приведённые к D50, и кривые с гаммой 1.
pub(crate) fn linear_srgb_profile() -> Vec<u8> {
    let xyz = |v: [f32; 3]| {
        let mut t = b"XYZ \0\0\0\0".to_vec();
        for c in v {
            t.extend_from_slice(&((c * 65536.0).round() as i32).to_be_bytes());
        }
        t
    };
    let curv = b"curv\0\0\0\0\0\0\0\0".to_vec();
    let tags: [(&[u8; 4], Vec<u8>); 6] = [
        (b"rXYZ", xyz([0.436_074_7, 0.222_504_5, 0.013_932_2])),
        (b"gXYZ", xyz([0.385_064_9, 0.716_878_6, 0.097_104_5])),
        (b"bXYZ", xyz([0.143_080_4, 0.060_616_9, 0.714_173_3])),
        (b"rTRC", curv.clone()),
        (b"gTRC", curv.clone()),
        (b"bTRC", curv),
    ];
    let mut profile = vec![0u8; 128];
    profile.extend_from_slice(&(tags.len() as u32).to_be_bytes());
    let mut data = Vec::new();
    let data_start = 128 + 4 + tags.len() * 12;
    for (sig, body) in &tags {
        profile.extend_from_slice(*sig);
        profile.extend_from_slice(&((data_start + data.len()) as u32).to_be_bytes());
        profile.extend_from_slice(&(body.len() as u32).to_be_bytes());
        data.extend_from_slice(body);
    }
    profile.extend_from_slice(&data);
    let size = profile.len() as u32;
    profile[..4].copy_from_slice(&size.to_be_bytes());
    profile
}

/// Адрес `data:` с процентным кодированием каждого байта.
pub(crate) fn data_url(mime: &str, bytes: &[u8]) -> String {
    let mut out = format!("data:{mime},");
    for b in bytes {
        out.push_str(&format!("%{b:02X}"));
    }
    out
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xFFFF_FFFFu32;
    for &b in bytes {
        crc ^= b as u32;
        for _ in 0..8 {
            crc = if crc & 1 != 0 {
                (crc >> 1) ^ 0xEDB8_8320
            } else {
                crc >> 1
            };
        }
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let (mut a, mut b) = (1u32, 0u32);
    for &x in bytes {
        a = (a + x as u32) % 65521;
        b = (b + a) % 65521;
    }
    (b << 16) | a
}
