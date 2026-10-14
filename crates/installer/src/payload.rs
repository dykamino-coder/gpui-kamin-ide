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
    let mut aside = 0u32;
    for entry in archive.entries().context("tar entries")? {
        let mut entry = entry.context("tar entry")?;
        let rel = entry.path().context("tar path")?.into_owned();
        if entry.unpack_in(dest).is_ok() {
            continue;
        }
        // 2026-10-14 (acceptance 1.0.70): a killed KaminIDE can stay alive for
        // over a minute (CEF/GPU teardown) with its DLLs mapped, and a mapped
        // image cannot be overwritten. Windows does allow renaming it, so move
        // the busy file aside and write the new one in its place; the dying
        // process keeps its mapping, `crate::unlock::remove_aside` deletes the
        // leftovers on the next run.
        let target = dest.join(&rel);
        if !target.is_file() {
            entry
                .unpack_in(dest)
                .with_context(|| format!("unpack {}", rel.display()))?;
            continue;
        }
        let old = crate::unlock::aside_path(&target, aside);
        aside += 1;
        std::fs::rename(&target, &old)
            .with_context(|| format!("move busy {} aside", target.display()))?;
        crate::steps::klog(&format!("moved busy {} aside", rel.display()));
        entry
            .unpack_in(dest)
            .with_context(|| format!("unpack {}", rel.display()))?;
    }
    Ok(())
}
