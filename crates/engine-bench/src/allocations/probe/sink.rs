//! Файл пробника с общим буфером: одна запись ОС на мегабайт строк вместо
//! записи на каждую аллокацию. Строки всех потоков идут под одним замком, так
//! что `f` освобождения остаётся раньше повторного выделения того же адреса.
//! Ни запись, ни сброс не выделяют память.

use std::{fs::File, io::Write as _, path::Path, sync::Mutex};

const CAPACITY: usize = 1 << 20;

struct Sink {
    file: Option<File>,
    buffer: [u8; CAPACITY],
    length: usize,
}
impl Sink {
    fn drain(&mut self) {
        if let Some(file) = &mut self.file {
            let _ = file.write_all(&self.buffer[..self.length]);
        }
        self.length = 0;
    }
}

static SINK: Mutex<Sink> = Mutex::new(Sink {
    file: None,
    buffer: [0; CAPACITY],
    length: 0,
});

fn sink() -> std::sync::MutexGuard<'static, Sink> {
    SINK.lock().unwrap_or_else(|e| e.into_inner())
}

/// Создаёт файл и пишет заголовок; до первой записанной аллокации.
pub(super) fn open(path: &std::ffi::OsStr, header: &str) -> Result<(), String> {
    let path = Path::new(path);
    let mut file = File::create(path).map_err(|e| format!("{}: {e}", path.display()))?;
    file.write_all(header.as_bytes())
        .map_err(|e| format!("{}: {e}", path.display()))?;
    sink().file = Some(file);
    Ok(())
}

/// Одна готовая строка.
pub(super) fn write(line: &[u8]) {
    let mut sink = sink();
    if sink.length + line.len() > CAPACITY {
        sink.drain();
    }
    let start = sink.length;
    if let Some(room) = sink.buffer.get_mut(start..start + line.len()) {
        room.copy_from_slice(line);
        sink.length += line.len();
    }
}

/// Сбрасывает буфер в файл.
pub(super) fn flush() {
    let mut sink = sink();
    if sink.length > 0 {
        sink.drain();
    }
}

/// Последняя строка окна или процесса после всех записанных строк.
pub(super) fn trailer(text: &str) {
    let mut sink = sink();
    sink.drain();
    if let Some(file) = &mut sink.file {
        let _ = file.write_all(text.as_bytes());
    }
}
