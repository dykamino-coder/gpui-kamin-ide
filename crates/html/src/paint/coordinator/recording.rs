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

/// Запись `record`, чьи позиционированные задания собственного кадра рисуются в `slices`.
pub(crate) fn with_late_jobs<R>(
    slices: Rc<[PaintSegmentSlice]>,
    window: &mut Window,
    record: impl FnOnce(&mut Window) -> R,
) -> R {
    let late = LateJobs {
        slices,
        claimed: Cell::new(false),
    };
    window.with_render_context(Some(Rc::new(late)), record)
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
        finish: Rc::new(|guard, window, cx| {
            if let Some(RecordFrame { frame, late }) = guard.downcast_mut::<RecordFrame>() {
                match late {
                    Some(late) => frame.pass.drain_late(frame.depth, late, window, cx),
                    None => frame.pass.drain(frame.depth, window, cx),
                }
            }
        }),
    }
}
