//! Нормализация измеренных возможностей разрыва: запрещённые диапазоны и
//! единое продолжение на общей границе блочного и строчного потоков.

use super::FlowGeometry;
use crate::tolerance::COORD_EPS;

impl FlowGeometry {
    pub(crate) fn normalize(&mut self) {
        self.cuts.retain(|&(a, _)| {
            a.is_finite()
                && a > COORD_EPS
                && !self
                    .solid
                    .iter()
                    .any(|&(s, e)| s + COORD_EPS < a && a < e - COORD_EPS)
        });
        self.cuts
            .sort_by(|a, b| a.0.total_cmp(&b.0).then(a.1.total_cmp(&b.1)));
        self.cuts.dedup_by(|a, b| {
            if (a.0 - b.0).abs() >= COORD_EPS {
                return false;
            }
            if (a.1 - b.1).abs() < COORD_EPS {
                return true;
            }
            // Конец последней строки совпадает с границей блоков (классы B/A,
            // css-break-3 §4.1), но не отменяет её продолжение после поля или
            // распорки роста (§5.2–5.3). Пропускать можно лишь пустой зазор:
            // измеренные строки параллельного потока должны остаться в плане.
            if b.1 >= b.0 - COORD_EPS
                && !self
                    .solid
                    .iter()
                    .any(|&(s, e)| s < a.1 - COORD_EPS && e > b.1 + COORD_EPS)
            {
                b.1 = a.1;
                return true;
            }
            false
        });
    }
}
