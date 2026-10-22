//! Тесты выбора процессов установленной копии (INC-2026-0075): чистое решение
//! по путям, канонизация реальных путей Windows и живой обход процессов на
//! копиях тестового exe с уникальным именем в двух временных каталогах.

use super::*;

const IMAGES: [&str; 3] = ["kaminide-gpui.exe", "kaminhost.exe", "kaminide-web.exe"];

fn p(s: &str) -> PathBuf {
    PathBuf::from(s)
}

#[test]
fn same_image_name_in_another_directory_is_not_selected() {
    let dir = p(r"C:\Users\u\AppData\Local\Programs\KaminIDE-GPUI");
    assert!(selected(&dir, &IMAGES, &dir.join("kaminide-gpui.exe")));
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"C:\src\gpui-kamin-ide\target\debug\kaminide-gpui.exe")
    ));
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"D:\portable\KaminIDE-GPUI\kaminide-gpui.exe")
    ));
}

#[test]
fn helpers_in_subdirectories_are_selected() {
    let dir = p(r"C:\Apps\KaminIDE");
    assert!(selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE\runtime\kaminhost.exe")
    ));
    assert!(selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE\kaminide-web.exe")
    ));
}

#[test]
fn other_images_inside_the_directory_are_not_selected() {
    let dir = p(r"C:\Apps\KaminIDE");
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE\kaminide-setup.exe")
    ));
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE\runtime\node.exe")
    ));
}

#[test]
fn sibling_directory_with_a_common_prefix_is_not_inside() {
    let dir = p(r"C:\Apps\KaminIDE");
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE2\kaminide-gpui.exe")
    ));
    assert!(!selected(
        &dir,
        &IMAGES,
        &p(r"C:\Apps\KaminIDE-old\kaminide-gpui.exe")
    ));
    assert!(!inside(&dir, &dir));
}

#[test]
fn windows_paths_compare_case_insensitively_and_ignore_verbatim_prefixes() {
    let dir = p(r"C:\Users\U\AppData\Local\Programs\KaminIDE-GPUI\");
    for exe in [
        r"c:\users\u\appdata\local\programs\kaminide-gpui\KAMINIDE-GPUI.EXE",
        r"\\?\C:\Users\U\AppData\Local\Programs\KaminIDE-GPUI\kaminide-gpui.exe",
        "C:/Users/U/AppData/Local/Programs/KaminIDE-GPUI/runtime/kaminhost.exe",
    ] {
        assert!(selected(&dir, &IMAGES, &p(exe)), "{exe}");
    }
    let unc = p(r"\\?\UNC\server\share\KaminIDE");
    assert!(inside(
        &unc,
        &p(r"\\server\share\KaminIDE\kaminide-gpui.exe")
    ));
    assert!(!inside(&unc, &p(r"\\server\share\Other\kaminide-gpui.exe")));
}

#[test]
fn a_drive_root_install_dir_still_needs_a_component_boundary() {
    let root = p(r"C:\");
    assert!(inside(&root, &p(r"C:\kaminide-gpui.exe")));
    assert!(!inside(&root, &p(r"D:\kaminide-gpui.exe")));
    assert!(!inside(&p(""), &p(r"C:\kaminide-gpui.exe")));
}

#[test]
fn kill_report_names_failures_and_foreign_count() {
    let r = KillReport {
        killed: vec![(1, p(r"C:\A\kaminide-gpui.exe"))],
        failed: vec![(2, p(r"C:\A\kaminhost.exe"))],
        foreign: 3,
    };
    let s = r.to_string();
    assert!(s.contains("killed 1") && s.contains("failed 1") && s.contains("left running 3"));
    assert!(s.contains(r"pid 2 C:\A\kaminhost.exe"));
}

#[cfg(windows)]
mod live {
    use super::*;
    use std::process::{Child, Command};
    use std::time::{Duration, Instant};

    /// Уникальное имя: тест никогда не выбирает настоящие процессы KaminIDE.
    const PROBE: &str = "kamin-procs-probe.exe";

    fn tmp(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("kamin-procs-{tag}-{}", std::process::id()));
        std::fs::create_dir_all(&d).unwrap();
        d
    }

    fn cmd_out(line: &str) -> String {
        use std::os::windows::process::CommandExt as _;
        let out = Command::new("cmd").raw_arg(line).output().unwrap();
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    }

    #[test]
    fn short_8_3_and_junction_paths_resolve_to_the_same_directory() {
        let base = tmp("canon");
        let real = base.join("Long Directory Name");
        std::fs::create_dir_all(real.join("runtime")).unwrap();
        let exe = real.join("runtime").join("kaminhost.exe");
        std::fs::write(&exe, b"x").unwrap();

        let link = base.join("link");
        cmd_out(&format!(
            "/c mklink /J \"{}\" \"{}\" >nul",
            link.display(),
            real.display()
        ));
        if link.exists() {
            let via_link = link.join("runtime").join("kaminhost.exe");
            assert!(selected(&resolve(&real), &IMAGES, &resolve(&via_link)));
            assert!(selected(&resolve(&link), &IMAGES, &resolve(&exe)));
        }

        // 8.3 может быть отключено на томе — тогда короткое имя совпадает с длинным.
        let short = cmd_out(&format!(
            "/c for %I in (\"{}\") do @echo %~sI",
            real.display()
        ));
        if !short.is_empty() {
            assert!(
                selected(&resolve(&p(&short)), &IMAGES, &resolve(&exe)),
                "{short}"
            );
        }
        std::fs::remove_dir_all(&base).ok();
    }

    fn spawn_probe(dir: &Path) -> Child {
        let exe = dir.join(PROBE);
        std::fs::copy(std::env::current_exe().unwrap(), &exe).unwrap();
        Command::new(exe)
            .args(["--exact", "procs::tests::live::sleeper_child", "--ignored"])
            .env("KAMIN_PROCS_SLEEPER", "1")
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .unwrap()
    }

    fn wait_until(mut f: impl FnMut() -> bool) -> bool {
        let t = Instant::now();
        while t.elapsed() < Duration::from_secs(15) {
            if f() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(100));
        }
        false
    }

    /// Процесс-заглушка для живого теста: просто спит, пока его не завершат.
    #[test]
    #[ignore = "spawned as a child by the live kill test"]
    #[allow(clippy::disallowed_methods, reason = "test child marker")]
    fn sleeper_child() {
        if std::env::var_os("KAMIN_PROCS_SLEEPER").is_some() {
            std::thread::sleep(Duration::from_secs(120));
        }
    }

    #[test]
    fn kill_in_spares_the_same_image_running_from_another_directory() {
        let (a, b) = (tmp("install"), tmp("foreign"));
        let mut in_a = spawn_probe(&a);
        let mut in_b = spawn_probe(&b);
        let images = [PROBE];
        assert!(wait_until(|| any_in(&a, &images) && any_in(&b, &images)));

        let report = kill_in(&a, &images);
        assert_eq!(report.killed.len(), 1, "{report}");
        assert!(report.failed.is_empty(), "{report}");
        assert!(report.foreign >= 1, "{report}");
        assert!(wait_until(|| !any_in(&a, &images)));
        assert!(in_a.wait().is_ok());

        assert!(
            in_b.try_wait().unwrap().is_none(),
            "foreign copy must survive"
        );
        assert!(any_in(&b, &images));

        in_b.kill().ok();
        in_b.wait().ok();
        assert!(wait_until(|| std::fs::remove_dir_all(&a).is_ok()));
        assert!(wait_until(|| std::fs::remove_dir_all(&b).is_ok()));
    }
}
