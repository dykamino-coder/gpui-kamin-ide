//! Общие образцы тестов хранилища и кэша: сплошные PNG, адрес `data:` с PNG, точка растра
//! и временный каталог, который убирает за собой только созданные в нём файлы.

use super::types::CpuRaster;
use std::path::{Path, PathBuf};

/// Сплошной PNG `w×h` одного цвета.
pub(crate) fn png_sized(w: u32, h: u32, rgba: [u8; 4]) -> Vec<u8> {
    use image::ImageEncoder;
    let raw: Vec<u8> = (0..w * h).flat_map(|_| rgba).collect();
    let mut out = Vec::new();
    image::codecs::png::PngEncoder::new(&mut out)
        .write_image(&raw, w, h, image::ExtendedColorType::Rgba8)
        .unwrap();
    out
}

/// Сплошной PNG 16×16: точки весят 1 КиБ, что делает бюджеты кэша в тестах наглядными.
pub(crate) fn png_bytes(rgba: [u8; 4]) -> Vec<u8> {
    png_sized(16, 16, rgba)
}

/// Адрес `data:` (base64) с PNG одной точки.
pub(crate) fn data_png_url() -> String {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let bytes = png_sized(1, 1, [5, 6, 7, 255]);
    let mut out = String::from("data:image/png;base64,");
    for chunk in bytes.chunks(3) {
        let n = chunk
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | (*b as u32) << (16 - 8 * i));
        for i in 0..4 {
            if i <= chunk.len() {
                out.push(TABLE[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

/// Левая верхняя точка растра.
pub(crate) fn pixel(raster: &CpuRaster) -> [u8; 4] {
    raster.pixel(0, 0).unwrap()
}

/// Временный каталог теста; при сбросе удаляет записанные через него файлы и себя.
pub(crate) struct TempDir {
    path: PathBuf,
    files: Vec<PathBuf>,
}

impl TempDir {
    pub(crate) fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("kamin-imgstore-{}-{name}", std::process::id()));
        std::fs::create_dir_all(&path).unwrap();
        Self {
            path,
            files: Vec::new(),
        }
    }

    pub(crate) fn path(&self) -> &Path {
        &self.path
    }

    /// Записать файл каталога и запомнить его для уборки.
    pub(crate) fn write(&mut self, name: &str, bytes: &[u8]) -> PathBuf {
        let file = self.path.join(name);
        std::fs::write(&file, bytes).unwrap();
        if !self.files.contains(&file) {
            self.files.push(file.clone());
        }
        file
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        for file in &self.files {
            let _ = std::fs::remove_file(file);
        }
        let _ = std::fs::remove_dir(&self.path);
    }
}
