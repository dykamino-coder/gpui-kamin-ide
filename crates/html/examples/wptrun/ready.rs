//! Сигнал готовности документа + завершённый GPU-кадр + контрольный кадр.
//! Любая неопределённость возвращает управление прежнему протоколу show.

use super::capture::{Capture, CaptureCursor};
use super::capture_dump::Shot;
use super::frame_metrics::flat;
use super::page::Page;
use super::phase_timing::PageTiming;
use super::show::Stage;
use gpui::{AppContext as _, AsyncApp};

pub(super) fn enabled(cx: &mut AsyncApp, capture: &Capture) -> bool {
    capture.offscreen
        && capture.ready
        // Анимация может прийти из внешнего CSS: поиск в HTML недостаточен.
        && cx.update(|cx| cx.try_global::<gpui::AnimationElapsedTime>().is_some())
}

/// Первый кадр окна подтверждается readback; таймер остаётся пределом ожидания.
pub(super) async fn startup(cx: &mut AsyncApp, capture: &Capture) {
    use futures::future::select;
    let timeout = cx
        .background_executor()
        .timer(std::time::Duration::from_millis(900));
    if enabled(cx, capture) && !capture.first {
        let _ = select(capture.session.as_ref().unwrap().after(0), timeout).await;
    } else {
        timeout.await;
    }
}

impl Stage {
    pub(super) fn redraw(&self, cx: &mut AsyncApp) {
        let _ = cx.update_window(self.window, |view, window, cx| {
            if let Ok(page) = view.downcast::<Page>() {
                page.update(cx, |_, cx| cx.notify());
            }
            window.refresh();
        });
    }

    pub(super) async fn ready_frame(
        &self,
        cx: &mut AsyncApp,
        captures: &mut CaptureCursor,
        prev: &Option<Vec<u8>>,
        want_flat: bool,
        timing: &mut PageTiming<'_>,
    ) -> Option<Shot> {
        let mut candidate: Option<Shot> = None;
        // Это только бюджет нового пути. Полный старый бюджет не сокращается.
        for _ in 0..8 {
            if !captures.wait(cx).await {
                break;
            }
            timing.steps += 1;
            let started = std::time::Instant::now();
            let shot = captures.next(self.hwnd);
            timing.readback += started.elapsed();
            let Some(shot) = shot else { break };
            timing.first.get_or_insert_with(|| timing.started.elapsed());
            let ready = cx
                .update_window(self.window, |view, _, cx| {
                    view.downcast::<Page>()
                        .is_ok_and(|page| page.read(cx).doc.frame_ready())
                })
                .unwrap_or(false);
            if ready && (!want_flat || flat(&shot.2)) {
                if let Some(before) = &candidate {
                    if before == &shot {
                        self.trace
                            .borrow_mut()
                            .push(if prev.as_ref() == Some(&shot.2) {
                                'b'
                            } else {
                                'r'
                            });
                        return Some(shot);
                    }
                    // Контроль не совпал: никакого принятия по двум случайным кадрам.
                    break;
                }
                candidate = Some(shot);
            } else {
                candidate = None;
            }
            self.redraw(cx);
        }
        self.redraw(cx);
        None
    }
}
