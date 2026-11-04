//! Допуск записываемой копии до завершения или отмены продолжения.

use super::Routes;

pub(super) struct Admission<'a>(pub(super) &'a Routes);
impl Drop for Admission<'_> {
    fn drop(&mut self) {
        self.0.admit(false);
    }
}
