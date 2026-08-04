//! Приклеенный payload: [exe][tar.zst][len:u64 LE][b"KMNSETUP"].
//! Футер в конце файла — exe читает сам себя, внешних файлов нет.

use anyhow::{Context, Result, bail};
use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

const MAGIC: &[u8; 8] = b"KMNSETUP";

/// Распаковать payload в `dest` (создаётся; существующие файлы перезаписываются).
pub fn unpack_to(dest: &Path) -> Result<()> {
    let exe = std::env::current_exe().context("current_exe")?;
    let mut f = std::fs::File::open(&exe).context("open self")?;
    let total = f.metadata()?.len();
    if total < 16 {
        bail!("no payload footer");
    }
    f.seek(SeekFrom::End(-16))?;
    let mut footer = [0u8; 16];
    f.read_exact(&mut footer)?;
    if &footer[8..16] != MAGIC {
        bail!("payload magic missing — exe built without append step");
    }
    let len = u64::from_le_bytes(footer[..8].try_into().unwrap());
    if len == 0 || len > total - 16 {
        bail!("payload length {len} out of range (file {total})");
    }
    f.seek(SeekFrom::Start(total - 16 - len))?;
    let payload = f.take(len);

    std::fs::create_dir_all(dest).with_context(|| format!("mkdir {}", dest.display()))?;
    let zst = zstd::stream::read::Decoder::new(payload).context("zstd decoder")?;
    let mut archive = tar::Archive::new(zst);
    // tar сохраняет mtime; overwrite существующих — поведение File /r.
    archive.set_preserve_permissions(false);
    archive.unpack(dest).context("tar unpack")?;
    Ok(())
}
