//! Допуск декодирования растра по ЗАЯВЛЕННЫМ размерам (ADR2 B4). Заголовок кодека даёт
//! стороны и формат точек до выделения: маленький сжатый файл не даёт права на огромный
//! растр. Точки — live `DecodedPixels`: physical квота host governor живёт вместе с буфером
//! (`Retention` в `CpuRaster`, переживает вытеснение из кэша при живом `Arc`), logical —
//! у запроса документа (`Admission`). Временные буферы выхода кодека и EXIF-поворота —
//! `LiveBytes` только на время декодирования.

use super::types::{ImageError, ImageLimit};
use crate::budget::{
    AdmittedStorage, BudgetError, BudgetKind, DocumentBudget, LiveReservation, SharedStorage,
    StorageError,
};
use std::sync::Arc;

/// Растр, заявленный заголовком кодека до декодирования точек.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Declared {
    pub(crate) width: u32,
    pub(crate) height: u32,
    /// Байты выхода кодека в его собственном формате точек.
    pub(crate) native: u64,
    /// Выход кодека не RGBA8: перевод держит оба буфера одновременно.
    pub(crate) converts: bool,
    /// EXIF-поворот строит развёрнутую копию рядом с исходной.
    pub(crate) orients: bool,
}

impl Declared {
    pub(crate) fn pixels(&self) -> u64 {
        u64::from(self.width) * u64::from(self.height)
    }

    /// Временные байты сверх удерживаемого растра; переполнение — отказ до выделения.
    fn temporary(&self) -> Result<u64, ImageError> {
        let turned = if self.orients {
            self.pixels().checked_mul(4)
        } else {
            Some(0)
        };
        let native = if self.converts { self.native } else { 0 };
        turned
            .and_then(|turned| turned.checked_add(native))
            .ok_or(ImageError::LimitExceeded(ImageLimit::Dimensions {
                width: self.width,
                height: self.height,
            }))
    }
}

/// Physical квота растра. Растры без декодирования (рисунки, градиенты) её не несут;
/// равенство точек от неё не зависит.
#[derive(Clone, Default)]
pub(crate) struct Retention(Option<Arc<SharedStorage<()>>>);

impl PartialEq for Retention {
    fn eq(&self, _: &Self) -> bool {
        true
    }
}

impl std::fmt::Debug for Retention {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(if self.0.is_some() { "Retained" } else { "None" })
    }
}

/// Logical квота документа на декодированные точки его запроса.
pub(crate) struct Admission {
    _pixels: AdmittedStorage<()>,
}

/// Квоты одного декодирования: physical уходит в растр, временная — до конца декодирования.
pub(crate) struct Grant {
    pub(crate) retention: Retention,
    pub(crate) _temporary: LiveReservation,
}

fn limit(error: BudgetError) -> ImageError {
    ImageError::LimitExceeded(ImageLimit::Budget(error))
}

/// Зарезервировать точки у host и документа, затем временные байты, — до декодирования.
pub(crate) fn grant(
    budget: &DocumentBudget,
    declared: &Declared,
) -> Result<(Grant, Admission), ImageError> {
    let temporary = declared.temporary()?;
    let storage = budget
        .governor()
        .allocate(BudgetKind::DecodedPixels, declared.pixels(), || {
            Ok::<(), std::convert::Infallible>(())
        })
        .map_err(|error| match error {
            StorageError::Budget(error) => limit(error),
            StorageError::Allocation(never) => match never {},
        })?;
    let admission = Admission {
        _pixels: budget.admit(storage.clone()).map_err(limit)?,
    };
    let temporary = budget
        .reserve(BudgetKind::LiveBytes, temporary)
        .map_err(limit)?;
    let grant = Grant {
        retention: Retention(Some(storage)),
        _temporary: temporary,
    };
    Ok((grant, admission))
}

/// Документ, получивший готовый растр из общего кэша, допускает его точки логически;
/// physical квоту уже держит буфер.
pub(crate) fn admit(
    budget: &DocumentBudget,
    retention: &Retention,
) -> Result<Option<Admission>, ImageError> {
    retention
        .0
        .as_ref()
        .map(|storage| {
            let pixels = budget.admit(storage.clone()).map_err(limit)?;
            Ok(Admission { _pixels: pixels })
        })
        .transpose()
}

/// Наблюдение предела или отмены картинки для журнала документа.
pub(crate) fn diagnostic(error: &ImageError) -> Option<crate::diagnostics::Diagnostic> {
    use crate::diagnostics::{Code, Diagnostic, Measure, Phase, Severity};
    let limit = |measure| {
        Diagnostic::observed(
            Code::ResourceLimit,
            Severity::Warning,
            Phase::Resource,
            measure,
        )
    };
    Some(match *error {
        ImageError::LimitExceeded(ImageLimit::ResourceBytes { limit: bytes }) => {
            limit(Some(Measure::Ceiling { limit: bytes }))
        }
        ImageError::LimitExceeded(ImageLimit::Budget(error)) => {
            Diagnostic::budget(Phase::Resource, error)
        }
        ImageError::LimitExceeded(ImageLimit::Dimensions { width, height }) => {
            limit(Some(Measure::Dimensions { width, height }))
        }
        ImageError::LimitExceeded(ImageLimit::Codec) => limit(None),
        ImageError::Cancelled => {
            Diagnostic::observed(Code::Cancelled, Severity::Info, Phase::Resource, None)
        }
        _ => return None,
    })
}

#[cfg(test)]
#[path = "admission_tests.rs"]
mod tests;
