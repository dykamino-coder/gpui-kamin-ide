//! Один one-shot тик на RootView для видимых лоадеров: retry, подпись и deadline
//! продолжаются без GPUI AnimationElement и не опрашиваются на частоте дисплея.

use super::model::RootView;
use gpui::Context;

impl RootView {
    pub(super) fn schedule_loader_tick(&mut self, cx: &mut Context<Self>) {
        if self.loader_tick.is_some() {
            return;
        }
        self.loader_tick = Some(cx.spawn(async move |view, cx| {
            smol::Timer::after(super::loader_schedule::TICK).await;
            let _ = view.update(cx, |view, cx| {
                view.loader_tick = None;
                cx.notify();
            });
        }));
    }
}
