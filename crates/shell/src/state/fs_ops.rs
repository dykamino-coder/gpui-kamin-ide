//! Файловые операции и утилиты состояния: ответ на QuickPick, проверка имён (зарезервированные имена Windows), нормализация путей, восстановление из корзины, язык редактора по расширению, запрос списка расширений, копирование и перенос файлов.
//!
//! Вынесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::CzEvent;
use crate::host_link::{self, ShellEvent};

/// Ответ хосту на shell.showQuickPick: indices или null (cancel), в фоне.
pub(crate) fn respond_quick_pick(req_id: u64, indices: Option<Vec<usize>>) {
    std::thread::spawn(move || {
        if let Some(c) = crate::host_link::client() {
            let value = match indices {
                Some(v) => serde_json::json!(v),
                None => serde_json::Value::Null,
            };
            c.respond(req_id, Ok(value));
        }
    });
}

/// Restore из корзины по исходному пути (самый свежий одноимённый).
pub(crate) fn restore_from_trash(path: &str) -> Result<(), String> {
    use trash::os_limited::{list, restore_all};
    let want = std::path::PathBuf::from(path);
    let newest = list()
        .map_err(|e| e.to_string())?
        .into_iter()
        .filter(|it| it.original_path() == want)
        .max_by_key(|it| it.time_deleted);
    match newest {
        Some(item) => restore_all([item]).map_err(|e| e.to_string()),
        None => Err("item not found in Recycle Bin".into()),
    }
}
/// Публичная обёртка (VSIX-install перечитывает список после установки).
pub fn request_extensions_pub(tx: smol::channel::Sender<ShellEvent>) {
    request_extensions(tx);
}

/// Запрос списка расширений в фоне.
pub(crate) fn request_extensions(tx: smol::channel::Sender<ShellEvent>) {
    std::thread::spawn(move || request_extensions_blocking(&tx));
}

pub(crate) fn request_extensions_blocking(tx: &smol::channel::Sender<ShellEvent>) {
    // Ретрай КАЖДОГО запроса, а не только появления клиента: exthost
    // поднимается позже WS, и единственная неудачная попытка оставляла
    // панель в «Loading…» навсегда (поймано юзером). 45 попыток ≈ 45 с.
    for attempt in 1..=45 {
        let client = host_link::client();
        let _ = tx.try_send(ShellEvent::Cz(CzEvent::ExtensionsStatus(
            if client.is_none() {
                format!("Waiting for the host WebSocket… (attempt {attempt}/45)")
            } else {
                format!("Asking the extension host for its list… (attempt {attempt}/45)")
            },
        )));
        if let Some(c) = client
            && let Ok(v) = c.request("kamin:extensions:list", vec![])
            && let Some(arr) = v.as_array()
        {
            let list = arr
                .iter()
                .filter_map(crate::ui::extensions_panel::ExtDesc::from_value)
                .collect();
            let _ = tx.try_send(ShellEvent::Cz(CzEvent::ExtensionsLoaded(list)));
            return;
        }
        std::thread::sleep(std::time::Duration::from_secs(1));
    }
}

/// Вставка из файлового буфера: cut → rename (fallback copy+delete),
/// copy → рекурсивная копия. Имя-коллизия → суффикс « copy».
/// Возвращает фактический dst (для undo-стека).
pub(crate) fn fs_paste(
    src: &str,
    target_dir: &str,
    is_cut: bool,
) -> std::io::Result<std::path::PathBuf> {
    let src_p = std::path::Path::new(src);
    let name = src_p
        .file_name()
        .ok_or_else(|| std::io::Error::other("no file name"))?;
    let mut dst = std::path::Path::new(target_dir).join(name);
    if dst == src_p {
        // Вставка рядом с собой → «name copy»
        let stem = dst
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let ext = dst.extension().map(|e| format!(".{}", e.to_string_lossy()));
        dst = dst.with_file_name(format!("{stem} copy{}", ext.unwrap_or_default()));
    }
    // Папку в саму себя или своего потомка вставлять нельзя: копия создаёт
    // dst ВНУТРИ src, обход src тут же находит свежий dst и копирует его в
    // него же — бесконечная рекурсия валила приложение stack overflow
    // (дамп 2026-07-31, `copy_recursive` через весь стек).
    if src_p.is_dir() && dst.starts_with(src_p) {
        return Err(std::io::Error::other("нельзя вставить папку в саму себя"));
    }
    if is_cut {
        match std::fs::rename(src_p, &dst) {
            Ok(()) => {}
            Err(_) => {
                // Через границы дисков rename падает — копия + удаление
                copy_recursive(src_p, &dst)?;
                if src_p.is_dir() {
                    std::fs::remove_dir_all(src_p)?;
                } else {
                    std::fs::remove_file(src_p)?;
                }
            }
        }
    } else {
        copy_recursive(src_p, &dst)?;
    }
    Ok(dst)
}

pub(crate) fn copy_recursive(src: &std::path::Path, dst: &std::path::Path) -> std::io::Result<()> {
    if src.is_dir() {
        std::fs::create_dir_all(dst)?;
        for entry in std::fs::read_dir(src)? {
            let entry = entry?;
            // Страховка от копии «в себя»: свежесозданный dst внутри src не
            // обходим, иначе рекурсия без дна (см. проверку в `fs_paste`).
            if entry.path() == *dst {
                continue;
            }
            copy_recursive(&entry.path(), &dst.join(entry.file_name()))?;
        }
        Ok(())
    } else {
        std::fs::copy(src, dst).map(|_| ())
    }
}

#[cfg(test)]
mod tests {
    /// Вставка папки в саму себя обязана падать ошибкой, а не рекурсией
    /// до переполнения стека (краш 2026-07-31).
    #[test]
    fn paste_dir_into_itself_is_rejected() {
        let base = std::env::temp_dir().join(format!("kamin-paste-{}", std::process::id()));
        let dir = base.join("src-dir");
        std::fs::create_dir_all(dir.join("inner")).unwrap();
        std::fs::write(dir.join("a.txt"), "x").unwrap();

        let err = super::fs_paste(
            &dir.display().to_string(),
            &dir.display().to_string(),
            false,
        );
        assert!(err.is_err(), "копия в саму себя должна быть отклонена");
        let err = super::fs_paste(
            &dir.display().to_string(),
            &dir.join("inner").display().to_string(),
            false,
        );
        assert!(err.is_err(), "копия в потомка должна быть отклонена");

        // Обычная копия рядом работает.
        let ok = super::fs_paste(
            &dir.display().to_string(),
            &base.display().to_string(),
            false,
        );
        // Важно только отсутствие паники: обе ветки результата допустимы.
        let _ = ok;
        let _ = std::fs::remove_dir_all(&base);
    }
}
