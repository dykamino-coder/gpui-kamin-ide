//! Свежие контрольные кадры: точное RGB-сравнение, хеши и парные PNG вне замеров.

use super::pixels;
use gpui::frame_capture::CapturedFrame;
use serde_json::{Value, json};
use std::path::Path;

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;

pub(super) fn compare(
    incremental: &CapturedFrame,
    fresh: &CapturedFrame,
    directory: Option<&Path>,
    name: &str,
) -> Result<Value> {
    let different = pixels::differing(incremental, fresh)?;
    let mut check = json!({
        "different_rgb_pixels": different,
        "incremental_bgr_fnv1a64": pixels::fingerprint(incremental),
        "fresh_bgr_fnv1a64": pixels::fingerprint(fresh),
    });
    if different != 0 {
        check["detail"] = pixels::detail(incremental, fresh);
    }
    if let Some(directory) = directory {
        std::fs::create_dir_all(directory)?;
        let baseline = directory.join(format!("{name}-incremental.png"));
        let control = directory.join(format!("{name}-fresh.png"));
        pixels::save(incremental, &baseline)?;
        pixels::save(fresh, &control)?;
        check["artifacts"] = json!({"incremental": baseline, "fresh": control});
    }
    Ok(check)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fresh_hash_identifies_a_single_rgb_pixel_change_and_ignores_alpha() {
        let incremental = CapturedFrame {
            width: 1,
            height: 1,
            bgra: vec![106, 89, 80, 255],
            context: None,
        };
        let mut fresh = incremental.clone();
        fresh.bgra[3] = 0;
        let equal = compare(&incremental, &fresh, None, "test").unwrap();
        assert_eq!(equal["different_rgb_pixels"], 0);
        assert_eq!(equal["incremental_bgr_fnv1a64"], equal["fresh_bgr_fnv1a64"]);
        assert!(equal.get("artifacts").is_none());
        fresh.bgra[..3].copy_from_slice(&[42, 25, 16]);
        let changed = compare(&incremental, &fresh, None, "test").unwrap();
        assert_eq!(changed["different_rgb_pixels"], 1);
        assert_ne!(
            changed["incremental_bgr_fnv1a64"],
            changed["fresh_bgr_fnv1a64"]
        );
        assert!(changed.get("detail").is_some());
    }

    #[test]
    fn opt_in_dump_retains_both_frames_even_when_they_match() {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let directory =
            std::env::temp_dir().join(format!("kamin-fresh-check-{}-{unique}", std::process::id()));
        let frame = CapturedFrame {
            width: 1,
            height: 1,
            bgra: vec![106, 89, 80, 255],
            context: None,
        };
        let report = compare(&frame, &frame, Some(&directory), "relayout-0").unwrap();
        for name in ["incremental", "fresh"] {
            let path = Path::new(report["artifacts"][name].as_str().unwrap());
            let input = std::io::BufReader::new(std::fs::File::open(path).unwrap());
            let mut reader = png::Decoder::new(input).read_info().unwrap();
            let mut decoded = vec![0; reader.output_buffer_size().unwrap()];
            let info = reader.next_frame(&mut decoded).unwrap();
            assert_eq!((info.width, info.height), (1, 1));
            assert_eq!(decoded, [80, 89, 106, 255]);
            std::fs::remove_file(path).unwrap();
        }
        std::fs::remove_dir(directory).unwrap();
    }
}
