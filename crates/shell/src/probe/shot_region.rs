//! probe `shotRegion`: снимок ОДНОЙ области окна по id из реестра регионов.
//!
//! Приёмка панелей (Agents, Console, чат) сравнивает конкретную карту, а
//! полный кадр окна меняется от соседей (часы, статус, курсор). Регион
//! режется из того же PrintWindow-кадра, что и `screenshot`; bounds в
//! реестре логические, кадр физический — множитель берётся из DPI окна,
//! как в синтетическом вводе (`probe/input.rs`).

/// Прямоугольник региона в физических px кадра, обрезанный по кадру.
pub(crate) fn physical_rect(
    bounds: [f32; 4],
    scale: f32,
    frame_w: u32,
    frame_h: u32,
) -> Option<(u32, u32, u32, u32)> {
    let [x, y, w, h] = bounds;
    let x0 = ((x * scale).round().max(0.0) as u32).min(frame_w);
    let y0 = ((y * scale).round().max(0.0) as u32).min(frame_h);
    let x1 = (((x + w) * scale).round().max(0.0) as u32).min(frame_w);
    let y1 = (((y + h) * scale).round().max(0.0) as u32).min(frame_h);
    (x1 > x0 && y1 > y0).then_some((x0, y0, x1 - x0, y1 - y0))
}

/// Вырезать `rect` из RGBA-кадра `src` шириной `src_w`.
pub(crate) fn crop_rgba(src: &[u8], src_w: u32, rect: (u32, u32, u32, u32)) -> Vec<u8> {
    let (x, y, w, h) = rect;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let start = ((row * src_w + x) * 4) as usize;
        out.extend_from_slice(&src[start..start + (w * 4) as usize]);
    }
    out
}

/// Снять окно, вырезать регион `id`, записать PNG в `path`.
#[cfg(windows)]
pub(crate) fn capture_region(id: &str, path: &std::path::Path) -> Result<[u32; 4], String> {
    use windows::Win32::UI::HiDpi::GetDpiForWindow;
    let bounds = crate::probe::registry::bounds_of(id).ok_or(format!("unknown region: {id}"))?;
    let hwnd = crate::probe::shot::find_window(false).ok_or("window not found")?;
    // Safety: hwnd только что найден перечислением окон нашего процесса.
    let scale = unsafe { GetDpiForWindow(hwnd) } as f32 / 96.0;
    let full = path.with_extension("full.png");
    crate::probe::shot::capture_to_png_ex(&full, false)?;
    let decoder = png::Decoder::new(std::io::BufReader::new(
        std::fs::File::open(&full).map_err(|e| e.to_string())?,
    ));
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size().ok_or("png too large")?];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    let _ = std::fs::remove_file(&full);
    if info.color_type != png::ColorType::Rgba {
        return Err("unexpected frame format".into());
    }
    let rect = physical_rect(bounds, scale, info.width, info.height).ok_or("empty region")?;
    let pixels = crop_rgba(&buf, info.width, rect);
    let file = std::fs::File::create(path).map_err(|e| e.to_string())?;
    let mut enc = png::Encoder::new(std::io::BufWriter::new(file), rect.2, rect.3);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    let mut writer = enc.write_header().map_err(|e| e.to_string())?;
    writer
        .write_image_data(&pixels)
        .map_err(|e| e.to_string())?;
    Ok([rect.0, rect.1, rect.2, rect.3])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn logical_bounds_scale_and_clip_to_frame() {
        assert_eq!(
            physical_rect([10.0, 20.0, 100.0, 50.0], 1.25, 1000, 800),
            Some((13, 25, 125, 62))
        );
        assert_eq!(
            physical_rect([900.0, 0.0, 400.0, 10.0], 1.0, 1000, 800),
            Some((900, 0, 100, 10))
        );
        assert_eq!(physical_rect([0.0, 0.0, 0.0, 10.0], 1.0, 10, 10), None);
    }

    #[test]
    fn crop_takes_rows_of_the_rect() {
        // 3×2 кадр, пиксель = [номер,0,0,255]
        let src: Vec<u8> = (0u8..6).flat_map(|i| [i, 0, 0, 255]).collect();
        let out = crop_rgba(&src, 3, (1, 0, 2, 2));
        let ids: Vec<u8> = out.chunks(4).map(|p| p[0]).collect();
        assert_eq!(ids, vec![1, 2, 4, 5]);
    }
}
