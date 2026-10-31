//! Console/log представление диагностик: одна строка `level CODE key=value…`
//! на запись. Это только формат typed полей, не канал доставки и не источник
//! истины; embedder может показывать записи иначе.

use super::{Diagnostic, DiagnosticBatch, Location, Measure, Phase, Scope, Severity};
use std::fmt::{self, Write as _};

/// Уровень консоли для записи: ошибки области и предупреждения о fallback.
pub fn level(severity: Severity) -> &'static str {
    match severity {
        Severity::Info => "info",
        Severity::Warning => "warning",
        Severity::Error => "error",
    }
}

fn scope(scope: Scope) -> &'static str {
    match scope {
        Scope::Document => "document",
        Scope::EmbeddedDocument => "embedded-document",
    }
}

impl fmt::Display for Phase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Admission => f.write_str("admission"),
            Self::Parse => f.write_str("parse"),
            Self::Traversal(kind) => write!(f, "traversal-{kind:?}"),
            Self::Resource => f.write_str("resource"),
            Self::Style => f.write_str("style"),
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} {} scope={} phase={}",
            level(self.severity),
            self.code.as_str(),
            scope(self.scope),
            self.phase
        )?;
        match self.measure {
            Some(Measure::Depth { limit, attempted }) => {
                write!(f, " limit={limit} attempted={attempted} unit=depth")?
            }
            Some(Measure::Budget(e)) => write!(
                f,
                " budget={:?}/{:?} limit={} consumed={} increment={} unit={:?} overflow={}",
                e.scope, e.kind, e.limit, e.consumed, e.increment, e.unit, e.overflow
            )?,
            Some(Measure::Ceiling { limit }) => write!(f, " limit={limit} unit=bytes")?,
            Some(Measure::Length { limit, attempted }) => {
                write!(f, " limit={limit} attempted={attempted} unit=bytes")?
            }
            Some(Measure::Dimensions { width, height }) => {
                write!(f, " width={width} height={height} unit=pixels")?
            }
            None => {}
        }
        if let Some(Location::Source { row, column }) = self.location {
            write!(f, " at={row}:{column}")?;
        }
        write!(f, " count={}", self.occurrences)
    }
}

/// Строки порции: terminal cause первой, затем записи и итог подавленных.
/// `prefix` добавляется к каждой строке (например, имя пары WPT).
pub fn write_batch(out: &mut String, prefix: &str, batch: &DiagnosticBatch) {
    let revision = batch
        .revision
        .map_or_else(|| "none".to_owned(), |key| format!("{key:016x}"));
    let terminal = batch.terminal.as_ref().map(Diagnostic::from);
    for record in terminal.iter().chain(&batch.records) {
        let _ = writeln!(out, "{prefix}{record} revision={revision}");
    }
    if batch.suppressed > 0 {
        let _ = writeln!(
            out,
            "{prefix}warning KAMINIDE_DIAGNOSTICS_SUPPRESSED count={} revision={revision}",
            batch.suppressed
        );
    }
}
