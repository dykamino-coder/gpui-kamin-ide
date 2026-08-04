//! Чистые функции про имена и пути файлов — без ввода-вывода и без состояния.
//!
//! Живут отдельно, потому что нужны ОБОИМ слоям: и состоянию (переименование,
//! удаление), и интерфейсу (контекст-меню подсвечивает недопустимое имя).
//! Раньше `ui` тянулся за ними в `state::fs_ops` — единственное место, где
//! интерфейс знал про внутренности состояния.

/// Зарезервированные имена устройств Windows (`isReservedName`
/// `utils/path.ts`): в корзину не уходят — только удаление навсегда.
pub fn is_reserved_name(name: &str) -> bool {
    let stem = name.split('.').next().unwrap_or(name).to_ascii_uppercase();
    matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit()
            && stem.as_bytes()[3] != b'0')
}

/// Валидация имени файла/папки — порт `nameError`: пусто, разделители, «.»/«..».
pub(crate) fn file_name_error(name: &str) -> Option<&'static str> {
    let t = name.trim();
    if t.is_empty() {
        return Some("Name required");
    }
    if t.contains('/') || t.contains('\\') {
        return Some("Name cannot contain path separators");
    }
    if t == "." || t == ".." {
        return Some("Invalid name");
    }
    // `nameError` оригинала (`file-context-menu.ts:57-63`) — РОВНО три
    // проверки. Запрещённые символы и Windows-reserved он не отбивает: имя
    // уходит в fs, а зарезервированное потом удаляется «навсегда»
    // (см. `is_reserved_name`). Наш лишний гейт резал ввод строже (ревью ц.23)
    None
}

/// Нормализация пути для сравнения: слэши → \, убрать `\.\`, lowercase
/// (notify отдаёт пути с `\.\`-сегментом от watch-root «.»).
pub(crate) fn norm_path(s: &str) -> String {
    s.replace('/', "\\").replace("\\.\\", "\\").to_lowercase()
}

/// Имя файла из пути (для тостов undo).
pub(crate) fn basename_of(p: &str) -> String {
    std::path::Path::new(p)
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| p.to_string())
}

/// Язык подсветки редактора по расширению файла (реестр gpui-component).
pub(crate) fn editor_lang(path: &str) -> &'static str {
    let ext = path.rsplit('.').next().unwrap_or("").to_ascii_lowercase();
    match ext.as_str() {
        "rs" => "rust",
        "ts" | "tsx" => "typescript",
        "js" | "jsx" | "mjs" | "cjs" => "javascript",
        "json" | "jsonc" => "json",
        "md" | "mdx" => "markdown",
        "go" => "go",
        "html" | "htm" => "html",
        "zig" => "zig",
        _ => "text",
    }
}
