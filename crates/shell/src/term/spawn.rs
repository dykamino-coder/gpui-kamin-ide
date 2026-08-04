//! Запуск PTY-сессии терминала.
//!
//! Перенесено без изменения поведения (`plan/100-refactor-250.md`).

use crate::host::events::TermEvent;
use crate::host_link::ShellEvent;
use crate::term::profiles::ShellProfile;
use crate::term::{Dims, Proxy, TermSession};
use alacritty_terminal::term::{Config, Term};
use alacritty_terminal::vte::ansi::Processor;
use portable_pty::{CommandBuilder, PtySize, native_pty_system};
use smol::channel::Sender;
use std::io::{Read, Write};
use std::sync::{Arc, Mutex};

impl TermSession {
    /// Спавн шелла по профилю в cwd.
    pub fn spawn(
        profile: &ShellProfile,
        cwd: Option<&str>,
        tx: Sender<ShellEvent>,
    ) -> anyhow::Result<TermSession> {
        let (cols, rows) = (100u16, 30u16);
        let pty = native_pty_system().openpty(PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        })?;
        let mut cmd = CommandBuilder::new(&profile.program);
        for a in &profile.args {
            cmd.arg(a);
        }
        if let Some(cwd) = cwd {
            cmd.cwd(cwd);
        }
        let child = pty.slave.spawn_command(cmd)?;
        drop(pty.slave); // обязательный drop: иначе вывод/EOF зависают
        let writer: Arc<Mutex<Box<dyn Write + Send>>> =
            Arc::new(Mutex::new(pty.master.take_writer()?));
        let mut reader = pty.master.try_clone_reader()?;

        // `scrollback: 5000` (`TerminalSession.tsx:79`); alacritty по умолчанию
        // держит 10000 строк
        let config = Config {
            scrolling_history: 5000,
            ..Config::default()
        };
        let term = Arc::new(Mutex::new(Term::new(
            config,
            &Dims {
                cols: cols as usize,
                rows: rows as usize,
            },
            Proxy {
                tx: tx.clone(),
                writer: writer.clone(),
            },
        )));

        // Читатель PTY: байты → VTE-парсер → grid → notify
        {
            let term = term.clone();
            std::thread::Builder::new()
                .name("kamin-term-read".into())
                .spawn(move || {
                    let mut parser: Processor = Processor::new();
                    let mut buf = [0u8; 8192];
                    loop {
                        match reader.read(&mut buf) {
                            Ok(0) | Err(_) => break,
                            Ok(n) => {
                                let mut t = term.lock().unwrap();
                                parser.advance(&mut *t, &buf[..n]);
                                drop(t);
                                let _ = tx.try_send(ShellEvent::Term(TermEvent::TermWakeup));
                            }
                        }
                    }
                })?;
        }

        Ok(TermSession {
            term,
            writer,
            master: pty.master,
            child,
            cols,
            rows,
            title: profile.label.clone(),
        })
    }
}
