//! Сильная ссылка на текущую генерацию; retire выполняется до публикации новой.

pub(super) struct Current<T>(Option<T>);
impl<T> Default for Current<T> {
    fn default() -> Self {
        Self(None)
    }
}
impl<T: Clone + PartialEq> Current<T> {
    pub(super) fn snapshot(&self) -> Option<T> {
        self.0.clone()
    }
    pub(super) fn matches(&self, device: &T) -> bool {
        self.0.as_ref() == Some(device)
    }
    pub(super) fn observe(&mut self, device: T, retire: impl FnOnce()) {
        if self.matches(&device) {
            return;
        }
        if self.0.is_some() {
            retire();
        }
        self.0 = Some(device);
    }
}
