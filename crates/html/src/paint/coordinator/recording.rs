//! Завершение CSS-заданий внутри записи до закрытия её механического scope.

use super::*;
use gpui::{PaintRecording, PaintSegmentSlice};

/// Отложенные задания записи главной копии разрезанного ребёнка стопки: копия куска
/// отдавала их во внешний кадр — после потока всех кусков (CSS 2.1 Appendix E), поэтому
/// кадр этой записи рисует их в отдельных сегментах `slices`, которые стопка
/// воспроизводит после потока всех своих кусков. Берёт их только сама запись, начатая
/// первой под этим контекстом; вложенные записи сливают свои кадры как обычно.
struct LateJobs {
    slices: Rc<[PaintSegmentSlice]>,
    claimed: Cell<bool>,
}

/// Кадр записи и сегменты, в которые идут его позиционированные задания.
struct RecordFrame {
    frame: FrameGuard,
    late: Option<Rc<[PaintSegmentSlice]>>,
}

pub(crate) async fn with_late_jobs_async<R>(
    slices: Rc<[PaintSegmentSlice]>,
    window: &mut Window,
    record: impl AsyncFnOnce(&mut Window) -> R,
) -> R {
    let late = LateJobs {
        slices,
        claimed: Cell::new(false),
    };
    window
        .with_render_context_async(Some(Rc::new(late)), record)
        .await
}

pub(super) fn recording() -> PaintRecording {
    PaintRecording {
        begin: Rc::new(|window| match current(window) {
            Some(pass) => {
                let late = (window.render_context::<LateJobs>())
                    .filter(|late| !late.claimed.replace(true))
                    .map(|late| late.slices.clone());
                Box::new(RecordFrame {
                    frame: pass.open(window),
                    late,
                })
            }
            None => Box::new(()),
        }),
        finish: Rc::new(|guard, window, cx, stack| {
            Box::pin(async move {
                if let Some(RecordFrame { frame, late }) = guard.downcast_mut::<RecordFrame>() {
                    match late {
                        Some(late) => {
                            frame
                                .pass
                                .drain_late_async(frame.depth, late, window, cx, stack)
                                .await
                        }
                        None => frame.pass.drain_async(frame.depth, window, cx, stack).await,
                    }
                }
            })
        }),
    }
}
