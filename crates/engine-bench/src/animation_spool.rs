//! Fresh edit references prepared before timing, with one frame resident on read.

use gpui::frame_capture::CapturedFrame;
use std::{
    fs::{File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

type Result<T> = std::result::Result<T, Box<dyn std::error::Error>>;
const MAGIC: &[u8; 8] = b"KAMFRAME";
static NEXT: AtomicU64 = AtomicU64::new(0);

pub(super) struct Spool {
    directory: PathBuf,
    count: usize,
    dimensions: Option<(u32, u32)>,
}

impl Spool {
    pub(super) fn new(parent: &Path, count: usize) -> Result<Self> {
        if count == 0 {
            return Err("fresh reference count must be positive".into());
        }
        std::fs::create_dir_all(parent)?;
        let time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)?
            .as_nanos();
        let directory = parent.join(format!(
            "animation-references-{}-{time}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed),
        ));
        // Never reuse a directory from another run, even if the name collides.
        std::fs::create_dir(&directory)?;
        Ok(Self {
            directory,
            count,
            dimensions: None,
        })
    }

    fn path(&self, index: usize) -> Result<PathBuf> {
        if !(1..=self.count).contains(&index) {
            return Err("fresh reference index outside measured series".into());
        }
        Ok(self.directory.join(format!("{index}.bgra")))
    }

    pub(super) fn write(&mut self, index: usize, frame: &CapturedFrame) -> Result<()> {
        let dimensions = (frame.width, frame.height);
        if self
            .dimensions
            .is_some_and(|expected| expected != dimensions)
        {
            return Err("fresh edit reference dimensions changed".into());
        }
        if frame.bgra.len() != byte_len(dimensions)? {
            return Err("fresh reference has an invalid pixel buffer".into());
        }
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(self.path(index)?)?;
        file.write_all(MAGIC)?;
        file.write_all(&frame.width.to_le_bytes())?;
        file.write_all(&frame.height.to_le_bytes())?;
        file.write_all(&frame.bgra)?;
        self.dimensions = Some(dimensions);
        Ok(())
    }

    pub(super) fn read(&self, index: usize) -> Result<CapturedFrame> {
        // Allocation is bounded by an actual capture, never by file contents.
        let dimensions = self.dimensions.ok_or("no fresh references were prepared")?;
        let len = byte_len(dimensions)?;
        let mut file = File::open(self.path(index)?)?;
        if file.metadata()?.len()
            != (len as u64)
                .checked_add(16)
                .ok_or("frame length overflow")?
        {
            return Err("fresh reference has an invalid file length".into());
        }
        let mut header = [0; 16];
        file.read_exact(&mut header)?;
        let width = u32::from_le_bytes(header[8..12].try_into()?);
        let height = u32::from_le_bytes(header[12..16].try_into()?);
        if &header[..8] != MAGIC || (width, height) != dimensions {
            return Err("fresh reference header does not match capture dimensions".into());
        }
        let mut bgra = vec![0; len];
        file.read_exact(&mut bgra)?;
        Ok(CapturedFrame {
            width,
            height,
            bgra,
            context: None,
        })
    }
}

fn byte_len((width, height): (u32, u32)) -> Result<usize> {
    if width == 0 || height == 0 {
        return Err("fresh reference dimensions must be positive".into());
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(4))
        .ok_or_else(|| "fresh reference size overflow".into())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn references_are_exact_unique_and_reject_corrupt_headers() {
        let mut spool = Spool::new(&std::env::temp_dir(), 2).unwrap();
        let frame = CapturedFrame {
            width: 2,
            height: 1,
            bgra: vec![1, 2, 3, 4, 5, 6, 7, 8],
            context: None,
        };
        spool.write(1, &frame).unwrap();
        assert_eq!(spool.read(1).unwrap().bgra, frame.bgra);
        assert!(spool.write(1, &frame).is_err());
        assert!(spool.read(0).is_err());
        assert!(spool.read(2).is_err());
        let path = spool.path(1).unwrap();
        let mut bytes = std::fs::read(&path).unwrap();
        bytes[8..12].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, &bytes).unwrap();
        assert!(spool.read(1).is_err());
        std::fs::remove_file(&path).unwrap();
        std::fs::remove_dir(&spool.directory).unwrap();
    }

    #[test]
    fn a_capture_must_have_complete_pixels_and_stable_dimensions() {
        let mut spool = Spool::new(&std::env::temp_dir(), 2).unwrap();
        let mut frame = CapturedFrame {
            width: 1,
            height: 1,
            bgra: vec![0; 3],
            context: None,
        };
        assert!(spool.write(1, &frame).is_err());
        frame.bgra.push(0);
        spool.write(1, &frame).unwrap();
        frame.width = 2;
        frame.bgra.resize(8, 0);
        assert!(spool.write(2, &frame).is_err());
        let mut file = OpenOptions::new()
            .append(true)
            .open(spool.path(1).unwrap())
            .unwrap();
        file.write_all(&[0]).unwrap();
        drop(file);
        assert!(spool.read(1).is_err());
        std::fs::remove_file(spool.path(1).unwrap()).unwrap();
        std::fs::remove_dir(&spool.directory).unwrap();
    }
}
