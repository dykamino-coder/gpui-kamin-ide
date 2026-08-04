//! Профили шеллов: обнаружение и выбор по id.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

/// Профиль шелла для «+» дропдауна (`ShellProfile` оригинала:
/// `kamin-host/services/shells.ts`). Поля владеющие: список собирается
/// обнаружением в рантайме, а не константами.
pub struct ShellProfile {
    pub id: String,
    pub label: String,
    pub(crate) program: String,
    pub(crate) args: Vec<String>,
    /// Имя кодикона (`terminal-powershell` / `-cmd` / `-bash` / `-linux`).
    pub icon: &'static str,
}
fn exists(p: &str) -> bool {
    !p.is_empty() && std::path::Path::new(p).exists()
}
fn env_or(key: &str, fallback: &str) -> String {
    std::env::var(key).unwrap_or_else(|_| fallback.to_string())
}
/// Обнаружение по порядку оригинала (`discoverWindows`): Windows PowerShell,
/// PowerShell 7, Command Prompt, Git Bash, затем каждый WSL-дистрибутив.
/// Считается ОДИН раз: `wsl -l -q` стоит десятки миллисекунд.
fn discover() -> Vec<ShellProfile> {
    let mut out: Vec<ShellProfile> = Vec::new();
    let sys_root = env_or("SystemRoot", "C:\\Windows");
    let pf = env_or("ProgramFiles", "C:\\Program Files");
    let pf86 = env_or("ProgramFiles(x86)", "C:\\Program Files (x86)");
    let local = env_or(
        "LOCALAPPDATA",
        &format!(
            "{}\\AppData\\Local",
            env_or("USERPROFILE", "C:\\Users\\Default")
        ),
    );

    let ps5 = format!("{sys_root}\\System32\\WindowsPowerShell\\v1.0\\powershell.exe");
    if exists(&ps5) {
        out.push(ShellProfile {
            id: "powershell".into(),
            label: "Windows PowerShell".into(),
            program: ps5,
            // `-NoProfile` у оригинала нет; профиль юзера грузится, как в нём
            args: vec!["-NoLogo".into()],
            icon: "terminal-powershell",
        });
    }

    for cand in [
        format!("{pf}\\PowerShell\\7\\pwsh.exe"),
        format!("{pf86}\\PowerShell\\7\\pwsh.exe"),
    ] {
        if exists(&cand) {
            out.push(ShellProfile {
                id: "pwsh".into(),
                label: "PowerShell 7".into(),
                program: cand,
                args: vec!["-NoLogo".into()],
                icon: "terminal-powershell",
            });
            break;
        }
    }

    let cmd = format!("{sys_root}\\System32\\cmd.exe");
    if exists(&cmd) {
        out.push(ShellProfile {
            id: "cmd".into(),
            label: "Command Prompt".into(),
            program: cmd,
            args: Vec::new(),
            icon: "terminal-cmd",
        });
    }

    for cand in [
        format!("{pf}\\Git\\bin\\bash.exe"),
        format!("{pf86}\\Git\\bin\\bash.exe"),
        format!("{local}\\Programs\\Git\\bin\\bash.exe"),
    ] {
        if exists(&cand) {
            out.push(ShellProfile {
                id: "git-bash".into(),
                label: "Git Bash".into(),
                program: cand,
                args: vec!["--login".into(), "-i".into()],
                icon: "terminal-bash",
            });
            break;
        }
    }

    let wsl = format!("{sys_root}\\System32\\wsl.exe");
    if exists(&wsl) {
        // `wsl -l -q` печатает UTF-16LE — иначе имена приходят с нулями
        if let Ok(o) = std::process::Command::new(&wsl).args(["-l", "-q"]).output() {
            let raw: Vec<u16> = o
                .stdout
                .chunks_exact(2)
                .map(|c| u16::from_le_bytes([c[0], c[1]]))
                .collect();
            for d in String::from_utf16_lossy(&raw)
                .lines()
                .map(|l| l.replace(' ', "").trim().to_string())
                .filter(|l| !l.is_empty())
            {
                out.push(ShellProfile {
                    id: format!("wsl-{d}"),
                    label: d.clone(),
                    program: wsl.clone(),
                    args: vec!["-d".into(), d],
                    icon: "terminal-linux",
                });
            }
        }
    }
    out
}
/// Обнаруженные шеллы (кэш на процесс).
pub fn profiles() -> &'static [ShellProfile] {
    static ALL: std::sync::OnceLock<Vec<ShellProfile>> = std::sync::OnceLock::new();
    ALL.get_or_init(discover)
}
pub fn profile_by_id(id: &str) -> &'static ShellProfile {
    let all = profiles();
    all.iter().find(|p| p.id == id).unwrap_or(&all[0])
}
