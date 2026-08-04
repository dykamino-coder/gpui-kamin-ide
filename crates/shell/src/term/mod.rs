//! Нативный терминал (Zed-путь): portable-pty (ConPTY) + alacritty_terminal
//! (VTE-парсер + grid). PTY локальный — не зависит от kamin-host (в оригинале
//! node-pty так же локален). Рендер — построчный текст из grid (v1 без
//! пер-ячеечных цветов), ввод — key events → байты в writer.

pub mod colors;
pub mod keys;
pub mod profiles;
pub mod spawn;

use crate::host::events::TermEvent;
pub use crate::term::colors::color_u32;
pub use crate::term::keys::keystroke_bytes;
pub use crate::term::profiles::{ShellProfile, profile_by_id, profiles};
use std::io::Write;
use std::sync::{Arc, Mutex};

use alacritty_terminal::event::{Event, EventListener};
use alacritty_terminal::term::Term;
use portable_pty::PtySize;
use smol::channel::Sender;

use crate::host_link::ShellEvent;

/// Размеры терминала (для Term::new + resize).
#[derive(Clone, Copy)]
pub(crate) struct Dims {
    pub(crate) cols: usize,
    pub(crate) rows: usize,
}

impl alacritty_terminal::grid::Dimensions for Dims {
    fn total_lines(&self) -> usize {
        self.rows
    }
    fn screen_lines(&self) -> usize {
        self.rows
    }
    fn columns(&self) -> usize {
        self.cols
    }
}

/// Прокси событий alacritty: PtyWrite (ответы терминала — DSR/CPR, без них
/// PSReadLine ВИСНЕТ в ожидании) → обратно в PTY; остальное → notify UI.
#[derive(Clone)]
pub struct Proxy {
    pub(crate) tx: Sender<ShellEvent>,
    pub(crate) writer: Arc<Mutex<Box<dyn Write + Send>>>,
}

impl EventListener for Proxy {
    fn send_event(&self, event: Event) {
        if let Event::PtyWrite(text) = &event {
            let mut w = self.writer.lock().unwrap();
            let _ = w.write_all(text.as_bytes());
            let _ = w.flush();
        }
        let _ = self.tx.try_send(ShellEvent::Term(TermEvent::TermWakeup));
    }
}

/// Живой терминал: grid под мьютексом + writer в PTY.
pub struct TermSession {
    pub term: Arc<Mutex<Term<Proxy>>>,
    pub(crate) writer: Arc<Mutex<Box<dyn Write + Send>>>,
    pub(crate) master: Box<dyn portable_pty::MasterPty + Send>,
    /// Держим ребёнка (drop мог бы прибить процесс).
    pub(crate) child: Box<dyn portable_pty::Child + Send + Sync>,
    pub cols: u16,
    pub rows: u16,
    pub title: String,
}

impl TermSession {
    /// Закрытие таба: прибить шелл (drop Child сам не убивает conhost).
    pub fn kill(&mut self) {
        let _ = self.child.kill();
    }

    /// Пиксель-точка вьюпорта → grid Point (учёт display_offset).
    fn vp_point(
        &self,
        t: &Term<Proxy>,
        col: usize,
        row: usize,
    ) -> alacritty_terminal::index::Point {
        let offset = t.grid().display_offset() as i32;
        alacritty_terminal::index::Point::new(
            alacritty_terminal::index::Line(row.min(self.rows as usize - 1) as i32 - offset),
            alacritty_terminal::index::Column(col.min(self.cols as usize - 1)),
        )
    }

    /// Начало выделения мышью: click_count 2 = слово, 3 = строка.
    pub fn selection_start(&mut self, col: usize, row: usize, clicks: usize) {
        use alacritty_terminal::selection::{Selection, SelectionType};
        let ty = match clicks {
            2 => SelectionType::Semantic,
            n if n >= 3 => SelectionType::Lines,
            _ => SelectionType::Simple,
        };
        let mut t = self.term.lock().unwrap();
        let point = self.vp_point(&t, col, row);
        t.selection = Some(Selection::new(
            ty,
            point,
            alacritty_terminal::index::Side::Left,
        ));
    }

    /// Расширение выделения при drag.
    pub fn selection_update(&mut self, col: usize, row: usize) {
        let mut t = self.term.lock().unwrap();
        let point = self.vp_point(&t, col, row);
        if let Some(sel) = t.selection.as_mut() {
            sel.update(point, alacritty_terminal::index::Side::Right);
        }
    }

    pub fn selection_text(&self) -> Option<String> {
        self.term.lock().unwrap().selection_to_string()
    }

    pub fn has_selection(&self) -> bool {
        self.term.lock().unwrap().selection.is_some()
    }

    pub fn selection_clear(&mut self) {
        self.term.lock().unwrap().selection = None;
    }

    /// Скроллбэк: delta>0 — вверх (в историю), <0 — вниз.
    pub fn scroll(&mut self, delta: i32) {
        self.term
            .lock()
            .unwrap()
            .scroll_display(alacritty_terminal::grid::Scroll::Delta(delta));
    }

    /// Ввод: байты в PTY.
    pub fn write(&mut self, bytes: &[u8]) {
        let mut w = self.writer.lock().unwrap();
        let _ = w.write_all(bytes);
        let _ = w.flush();
    }

    /// Ресайз по размеру панели (в ячейках).
    pub fn resize(&mut self, cols: u16, rows: u16) {
        if cols == self.cols && rows == self.rows || cols < 4 || rows < 2 {
            return;
        }
        self.cols = cols;
        self.rows = rows;
        let _ = self.master.resize(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        });
        self.term.lock().unwrap().resize(Dims {
            cols: cols as usize,
            rows: rows as usize,
        });
    }

    /// Снять экран как раны (текст, fg, selected) + курсор (col,row); курсор
    /// None когда вьюпорт отскроллен в историю. fg=None → цвет темы по
    /// умолчанию. Соседние ячейки одного цвета+селекта склеены в один ран.
    #[allow(clippy::type_complexity)]
    pub fn screen_styled(
        &self,
    ) -> (
        Vec<Vec<(String, Option<u32>, bool)>>,
        Option<(usize, usize)>,
    ) {
        let t = self.term.lock().unwrap();
        let sel_range = t.selection.as_ref().and_then(|s| s.to_range(&t));
        let grid = t.grid();
        let offset = grid.display_offset() as i32;
        let mut rows = Vec::with_capacity(self.rows as usize);
        for row in 0..self.rows as i32 {
            // Line<0 — скроллбэк-история над экраном
            let line = alacritty_terminal::index::Line(row - offset);
            let mut runs: Vec<(String, Option<u32>, bool)> = Vec::new();
            for col in 0..self.cols as usize {
                let column = alacritty_terminal::index::Column(col);
                let cell = &grid[line][column];
                let fg = color_u32(cell.fg);
                let sel = sel_range.is_some_and(|r| {
                    r.contains(alacritty_terminal::index::Point::new(line, column))
                });
                match runs.last_mut() {
                    Some((text, run_fg, run_sel)) if *run_fg == fg && *run_sel == sel => {
                        text.push(cell.c)
                    }
                    _ => runs.push((cell.c.to_string(), fg, sel)),
                }
            }
            // Хвостовые пробельные НЕселектированные раны — долой
            while runs
                .last()
                .is_some_and(|(t, _, sel)| !sel && t.trim().is_empty())
            {
                runs.pop();
            }
            if let Some((text, _, sel)) = runs.last_mut()
                && !*sel
            {
                while text.ends_with(' ') {
                    text.pop();
                }
            }
            rows.push(runs);
        }
        let cur = grid.cursor.point;
        let cursor = (offset == 0).then_some((cur.column.0, cur.line.0 as usize));
        (rows, cursor)
    }
}

/// ANSI 16 — палитра терминала VS Code Dark (как xterm в оригинале).
pub(crate) const ANSI16: [u32; 16] = [
    0x000000ff, 0xcd3131ff, 0x0dbc79ff, 0xe5e510ff, 0x2472c8ff, 0xbc3fbcff, 0x11a8cdff, 0xe5e5e5ff,
    0x666666ff, 0xf14c4cff, 0x23d18bff, 0xf5f543ff, 0x3b8eeaff, 0xd670d6ff, 0x29b8dbff, 0xffffffff,
];
