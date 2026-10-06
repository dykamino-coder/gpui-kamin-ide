//! Preserve the primary pair and the actual successful alternate as separate evidence.

pub(super) type Shot = (u32, u32, Vec<u8>);

pub(super) fn pair(stem: &str, shots: &[Option<Shot>], alternate: Option<&(String, Shot)>) {
    if let Some(shot) = &shots[0] {
        image(stem, shot);
    }
    if let Some(shot) = &shots[1] {
        image(&format!("{stem}--ref"), shot);
    }
    if let Some((reference, shot)) = alternate {
        image(&format!("{stem}--matched-ref"), shot);
        // Keep the input pair intact: this name belongs to a different declared match.
        let path = std::path::Path::new("target/wpt-shots")
            .join(format!("{stem}--matched-ref-source.txt"));
        if let Err(error) = std::fs::write(path, reference) {
            eprintln!("Could not record alternate reference provenance: {error}");
        }
    }
}

fn image(name: &str, shot: &Shot) {
    let dir = std::path::Path::new("target/wpt-shots");
    if std::fs::create_dir_all(dir).is_err() {
        return;
    }
    let Ok(file) = std::fs::File::create(dir.join(format!("{name}.png"))) else {
        return;
    };
    let mut encoder = png::Encoder::new(std::io::BufWriter::new(file), shot.0, shot.1);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let Ok(mut writer) = encoder.write_header() else {
        return;
    };
    // GDI captures are BGRA with a zero alpha channel; the PNG must remain visible.
    let mut rgba = shot.2.clone();
    for pixel in rgba.chunks_exact_mut(4) {
        pixel.swap(0, 2);
        pixel[3] = 255;
    }
    let _ = writer.write_image_data(&rgba);
}
