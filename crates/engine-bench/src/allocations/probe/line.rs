//! Строка пробника в буфере на стеке: запись из аллокатора не выделяет память.

use super::{FRAME, PHASE, sink};
use kamin_html::work::Phase;
use std::{fmt::Write as _, sync::atomic::Ordering::Relaxed};

/// Строка в буфере на стеке: запись из аллокатора не может выделять память.
pub(super) struct Line {
    pub(super) bytes: [u8; 1024],
    pub(super) length: usize,
}
impl std::fmt::Write for Line {
    fn write_str(&mut self, text: &str) -> std::fmt::Result {
        let end = self.length + text.len();
        self.bytes
            .get_mut(self.length..end)
            .ok_or(std::fmt::Error)?
            .copy_from_slice(text.as_bytes());
        self.length = end;
        Ok(())
    }
}

/// Начало строки: вид, кадр, фаза, байты и указатель.
pub(super) fn head(kind: u8, bytes: usize, pointer: *const u8) -> (Line, std::fmt::Result) {
    let phase = PHASE.load(Relaxed);
    let name = Phase::NAMES.get(phase).copied().unwrap_or("outside");
    let mut line = Line {
        bytes: [0; 1024],
        length: 0,
    };
    let frame = FRAME.load(Relaxed);
    let pointer = pointer as usize;
    let text = write!(line, "{} {frame} {name} {bytes} {pointer:x}", kind as char);
    (line, text)
}

pub(super) fn emit(mut line: Line, text: std::fmt::Result) {
    if text.and_then(|()| line.write_str("\n")).is_ok() {
        sink::write(&line.bytes[..line.length]);
    }
}

/// Блок, который измерил отчёт памяти по владельцам: `o <путь> <указатель> <байты>`.
/// Текстура рендерера в отчёте владельцев: путь, байты, из них занято.
#[cfg(feature = "heap-report")]
pub(crate) fn gpu(path: &str, bytes: u64, used: u64) {
    let mut line = Line {
        bytes: [0; 1024],
        length: 0,
    };
    let text = write!(line, "g {path} {bytes} {used}");
    emit(line, text);
}

/// Пишется и вне окна: отчёт сверяет его с живыми блоками окна.
#[cfg(feature = "heap-report")]
pub(crate) fn owned(path: &str, pointer: *const u8, bytes: usize) {
    let mut line = Line {
        bytes: [0; 1024],
        length: 0,
    };
    let text = write!(line, "o {path} {:x} {bytes}", pointer as usize);
    emit(line, text);
}
