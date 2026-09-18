//! PowerShell script used to create and repair Windows shortcuts.

use std::path::Path;

pub(crate) fn creation_script(install_dir: &Path) -> String {
    creation_script_for_folders(
        install_dir,
        "@([Environment]::GetFolderPath('Programs'), [Environment]::GetFolderPath('Desktop'))",
    )
}

fn creation_script_for_folders(install_dir: &Path, folders: &str) -> String {
    let target = powershell_literal(&install_dir.join("kaminide-gpui.exe").display().to_string());
    let working_directory = powershell_literal(&install_dir.display().to_string());

    format!(
        "$w = New-Object -ComObject WScript.Shell; \
         foreach ($p in {folders}) {{ \
           $s = $w.CreateShortcut((Join-Path $p 'KaminIDE.lnk')); \
           $s.TargetPath = {target}; $s.WorkingDirectory = {working_directory}; $s.Save() }}"
    )
}

fn powershell_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creates_both_shortcuts_with_the_install_directory() {
        let install_dir = Path::new(r"C:\Users\Test\KaminIDE-GPUI");
        let script = creation_script(install_dir);
        let expected_target = install_dir.join("kaminide-gpui.exe");

        assert!(script.contains("[Environment]::GetFolderPath('Programs')"));
        assert!(script.contains("[Environment]::GetFolderPath('Desktop')"));
        assert!(script.contains(&format!("$s.TargetPath = '{}'", expected_target.display())));
        assert!(script.contains("$s.WorkingDirectory = 'C:\\Users\\Test\\KaminIDE-GPUI'"));
    }

    #[test]
    fn escapes_single_quotes_in_install_paths() {
        let script = creation_script(Path::new(r"C:\Users\O'Brien\KaminIDE-GPUI"));

        assert!(script.contains(r"C:\Users\O''Brien\KaminIDE-GPUI"));
        assert!(script.contains("kaminide-gpui.exe'"));
        assert!(!script.contains(r"C:\Users\O'Brien"));
    }

    #[cfg(windows)]
    #[test]
    fn writes_working_directory_to_a_real_windows_shortcut() {
        use std::process::Command;
        use std::time::{SystemTime, UNIX_EPOCH};

        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("clock after epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("kaminide-shortcut-test-{unique}"));
        let install_dir = root.join("install");
        let shortcut_dir = root.join("shortcuts");
        std::fs::create_dir_all(&install_dir).expect("create install directory");
        std::fs::create_dir_all(&shortcut_dir).expect("create shortcut directory");
        std::fs::write(install_dir.join("kaminide-gpui.exe"), []).expect("create target");

        let folders = format!(
            "@({})",
            powershell_literal(&shortcut_dir.display().to_string())
        );
        let script = creation_script_for_folders(&install_dir, &folders);
        let status = Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status()
            .expect("run PowerShell");
        assert!(status.success());

        let shortcut = shortcut_dir.join("KaminIDE.lnk");
        let inspect = format!(
            "$w = New-Object -ComObject WScript.Shell; $s = $w.CreateShortcut({}); \
             Write-Output $s.TargetPath; Write-Output $s.WorkingDirectory",
            powershell_literal(&shortcut.display().to_string())
        );
        let output = Command::new("powershell")
            .args(["-NoProfile", "-Command", &inspect])
            .output()
            .expect("inspect shortcut");
        assert!(output.status.success());
        let output_text = String::from_utf8_lossy(&output.stdout);
        let values: Vec<_> = output_text
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .collect();
        let actual_target = std::fs::canonicalize(values[0]).expect("canonicalize shortcut target");
        let expected_target = std::fs::canonicalize(install_dir.join("kaminide-gpui.exe"))
            .expect("canonicalize expected target");
        let actual_working_directory =
            std::fs::canonicalize(values[1]).expect("canonicalize shortcut working directory");
        let expected_working_directory =
            std::fs::canonicalize(&install_dir).expect("canonicalize expected working directory");
        assert_eq!(actual_target, expected_target);
        assert_eq!(actual_working_directory, expected_working_directory);

        std::fs::remove_dir_all(root).expect("remove test directory");
    }
}
