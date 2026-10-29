//! Куски записи главной копии разрезанного ребёнка стопки для слотов внепоточных корней
//! (`paint::continuation::replayed`) и сегменты `LATE`, которые стопка играет после потока
//! всех своих кусков.

use crate::hooks::BareRecords;
use gpui::{Bounds, PaintSegment, PaintSegmentId, Pixels};
use std::rc::Rc;

/// Сегменты продолжений внепоточных корней в записи главной копии: отдельные от потока
/// куска, их стопка воспроизводит после потока всех своих кусков — фаза позиционированных
/// (CSS 2.1 Appendix E), в которую копия отдавала свой слот (`coordinator::REPLAY`).
pub(crate) const LATE: u64 = 1 << 31;

/// Кусок записи: его размещение (сегмент `LATE`), фрагментаинер (сегмент нативных записей) и область
/// краски его слотов в координатах окна (маска куска или полоса фрагментаинера).
pub(crate) struct ReplayPiece {
    pub placement: PaintSegment,
    pub column: PaintSegmentId,
    pub area: Bounds<Pixels>,
}

/// Куски записи главной копии и нативные записи её стопки.
pub(crate) struct ReplayPieces {
    pub records: Option<BareRecords>,
    pub pieces: Rc<[ReplayPiece]>,
}
