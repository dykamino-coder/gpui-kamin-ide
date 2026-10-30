//! Происхождение запуска и загруженного UMD; никаких запросов к чужим процессам.

use serde_json::{Value, json};
use std::{
    error::Error,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

pub(super) fn sidecar() -> PathBuf {
    std::env::var_os("WPT_GPU_CONTEXT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(format!(
                "{}.gpu.json",
                std::env::var("WPT_REPORT").unwrap_or_else(|_| "target/wpt-report.txt".into())
            ))
        })
}

pub(super) fn manifest(list: &str, width: f32, height: f32) -> Result<Value, Box<dyn Error>> {
    let exe = std::env::current_exe()?;
    let started = SystemTime::now().duration_since(UNIX_EPOCH)?.as_millis();
    Ok(
        json!({"schema": 1, "complete": false, "run_id": format!("{}-{started}", std::process::id()),
            "pid": std::process::id(), "process_start_ms": started,
            "exe": {"path": exe, "basename": exe.file_name().and_then(|s| s.to_str()),
                "sha256": super::hash(&std::fs::read(&exe)?)},
            "source_revision": option_env!("WPT_SOURCE_REVISION").unwrap_or("unknown"),
            "boot_session": boot_session()?,
            "list_sha256": super::hash(&std::fs::read(list)?),
            "capture": {"width": width, "height": height,
                "scale_factor": std::env::var("KAMIN_FORCE_SCALE").ok().and_then(|v| v.parse::<f32>().ok()).unwrap_or(1.),
                "path": "gpu-readback", "stage": "before-present", "text_mode": "grayscale",
                "color_space": "raw-unorm-no-colorspace-transform",
                "animation_time_ms": super::super::config::animation_elapsed().map(|t| t.as_millis()),
                "animation_frames": std::env::var("WPT_ANIMATION_FRAMES").ok()},
            "shader_policy": {"debug_assertions": cfg!(debug_assertions), "backend": "d3dcompiler"},
            "frames": [], "pngs": [], "driver_dlls": [], "report_sha256": null,
        }),
    )
}

pub(super) fn drivers() -> Result<Value, Box<dyn Error>> {
    use windows::Win32::{
        Foundation::HMODULE,
        System::{
            ProcessStatus::{EnumProcessModules, GetModuleFileNameExW},
            Threading::GetCurrentProcess,
        },
    };
    let process = unsafe { GetCurrentProcess() };
    let mut modules = vec![HMODULE::default(); 1024];
    let mut needed = 0;
    unsafe {
        EnumProcessModules(
            process,
            modules.as_mut_ptr(),
            (modules.len() * size_of::<HMODULE>()) as u32,
            &mut needed,
        )?
    };
    if needed as usize > modules.len() * size_of::<HMODULE>() {
        return Err("Module inventory exceeded buffer".into());
    }
    let mut rows = Vec::new();
    for module in modules
        .into_iter()
        .take(needed as usize / size_of::<HMODULE>())
    {
        let mut buffer = vec![0u16; 32768];
        let len =
            unsafe { GetModuleFileNameExW(Some(process), Some(module), &mut buffer) } as usize;
        if len == 0 || len >= buffer.len() {
            return Err("Cannot read loaded module path".into());
        }
        let path = PathBuf::from(String::from_utf16(&buffer[..len])?);
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or_default()
            .to_lowercase();
        if name.starts_with("igd")
            || name.starts_with("nvwgf")
            || name.starts_with("atid")
            || name.starts_with("amdxx")
            || name == "d3d10warp.dll"
        {
            rows.push(json!({"path": path, "name": name, "sha256": super::hash(&std::fs::read(&path)?), "version": file_version(&path)?}));
        }
    }
    rows.sort_by_key(|v| v["name"].as_str().unwrap_or_default().to_owned());
    if rows.is_empty() {
        return Err("No loaded GPU UMD found".into());
    }
    Ok(json!(rows))
}

fn file_version(path: &std::path::Path) -> Result<String, Box<dyn Error>> {
    use windows::{
        Win32::Storage::FileSystem::*,
        core::{HSTRING, w},
    };
    let name = HSTRING::from(path.as_os_str());
    let size = unsafe { GetFileVersionInfoSizeW(&name, None) };
    if size == 0 {
        return Err("UMD file has no version resource".into());
    }
    let mut data = vec![0u8; size as usize];
    let mut info = std::ptr::null_mut();
    let mut len = 0;
    unsafe {
        GetFileVersionInfoW(&name, None, size, data.as_mut_ptr().cast())?;
        if !VerQueryValueW(data.as_ptr().cast(), w!("\\"), &mut info, &mut len).as_bool()
            || len < size_of::<VS_FIXEDFILEINFO>() as u32
        {
            return Err("UMD file has no fixed version".into());
        }
        let value = std::ptr::read_unaligned(info.cast::<VS_FIXEDFILEINFO>());
        Ok(format!(
            "{}.{}.{}.{}",
            value.dwFileVersionMS >> 16,
            value.dwFileVersionMS & 65535,
            value.dwFileVersionLS >> 16,
            value.dwFileVersionLS & 65535
        ))
    }
}

fn boot_session() -> Result<String, Box<dyn Error>> {
    #[link(name = "ntdll")]
    unsafe extern "system" {
        fn NtQuerySystemInformation(
            class: i32,
            buffer: *mut u8,
            size: u32,
            returned: *mut u32,
        ) -> i32;
    }
    let mut data = [0u8; 48];
    if unsafe {
        NtQuerySystemInformation(
            3,
            data.as_mut_ptr(),
            data.len() as u32,
            std::ptr::null_mut(),
        )
    } != 0
    {
        return Err("Cannot query Windows boot session".into());
    }
    let boot = u64::from_le_bytes(data[..8].try_into()?);
    let session = format!("{}:{boot}", std::env::var("COMPUTERNAME")?);
    if std::env::var("WPT_GPU_SESSION").is_ok_and(|expected| expected != session) {
        return Err("Resolved LUID belongs to another Windows boot".into());
    }
    Ok(session)
}
