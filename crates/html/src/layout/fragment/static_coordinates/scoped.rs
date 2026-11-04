//! Область статических координат восстанавливается при завершении и отмене.

use super::*;

struct Scope<'a> {
    entries: &'a RefCell<Vec<Mapping>>,
    len: usize,
}
impl Drop for Scope<'_> {
    fn drop(&mut self) {
        self.entries.borrow_mut().truncate(self.len);
    }
}

pub(crate) async fn with_async<R>(
    owner: &crate::layout::cx::deferred::Deferred,
    mapping: Option<(usize, Point<Pixels>, Places)>,
    f: impl AsyncFnOnce() -> R,
) -> R {
    let Some((kid, origin, places)) = mapping else {
        return f().await;
    };
    let entries = &owner.coordinates;
    let len = entries.borrow().len();
    entries.borrow_mut().push(Mapping {
        kid,
        origin,
        places,
    });
    let _scope = Scope { entries, len };
    f().await
}
