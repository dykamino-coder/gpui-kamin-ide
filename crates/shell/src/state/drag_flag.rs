//! Флаг «идёт драг ручки»: на время драга ховер-ИСТОЧНИКИ (тултипы,
//! hover-пилюли) молчат — мышь обгоняет ручку и они мерцали. Ввод при этом
//! живёт как обычно: перекрывающий блокер был отвергнут — несработавший
//! mouse-up оставлял его навсегда и «вешал» приложение (поймано юзером).

use std::sync::atomic::{AtomicBool, Ordering};

static DRAGGING: AtomicBool = AtomicBool::new(false);

pub(crate) fn set(on: bool) {
    DRAGGING.store(on, Ordering::Relaxed);
}

pub(crate) fn active() -> bool {
    DRAGGING.load(Ordering::Relaxed)
}
