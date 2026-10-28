//! Ряд таблицы, который не «freely fragmentable» (css-tables-3 §breaking-rules): условие
//! зависит от размеров фрагментаинера, поэтому решается при выборе разрыва, а не при сборе точек.

use crate::tolerance::COORD_EPS;

/// Ряд без охвата ячеек в следующие ряды, после предыдущего ряда той же таблицы.
/// Координаты — от верха ребёнка стопки.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct RowKeep {
    /// Начало предыдущего ряда: «at least one row did fit entirely» — он целиком во
    /// фрагменте, когда начинается не выше начала фрагмента.
    pub(crate) prev: f32,
    /// Граница перед рядом (низ предыдущего ряда): точка класса A с усечением зазора.
    pub(crate) open: f32,
    pub(crate) start: f32,
    pub(crate) end: f32,
}

impl RowKeep {
    /// Разрыв `point` во фрагменте, начатом с `from`, во фрагментаинере `(block, inline)`
    /// уходит на границу перед рядом: ряд не выше половины обоих размеров фрагментаинера
    /// («at least twice smaller than both the fragmentainer height and width»), точка
    /// внутри ряда, а предыдущий ряд целиком в этом фрагменте. Точка в зазоре перед рядом
    /// ряд не режет: зазор на разрыве и так усекается.
    /// Неизвестный строчный размер правила не включает: ряд остаётся freely fragmentable.
    pub(crate) fn holds(&self, point: f32, from: f32, block: f32, inline: Option<f32>) -> bool {
        let twice = 2.0 * (self.end - self.start);
        let Some(inline) = inline else {
            return false;
        };
        twice <= block + COORD_EPS
            && twice <= inline + COORD_EPS
            && self.prev >= from - COORD_EPS
            && self.open > from + COORD_EPS
            && self.start < point - COORD_EPS
            && point < self.end - COORD_EPS
    }
}

#[cfg(test)]
mod tests {
    use super::RowKeep;

    const ROW: RowKeep = RowKeep {
        prev: 10.0,
        open: 40.0,
        start: 42.0,
        end: 82.0,
    };

    #[test]
    fn a_row_at_most_half_of_both_fragmentainer_sizes_is_kept_whole() {
        // Ряд 40: блочный 80 и строчный 80 — ровно половина, правило действует.
        assert!(ROW.holds(70.0, 0.0, 80.0, Some(80.0)));
        // Точка в зазоре перед рядом ряд не режет.
        assert!(!ROW.holds(41.0, 0.0, 80.0, Some(80.0)));
    }

    #[test]
    fn a_row_taller_than_half_of_either_size_is_freely_fragmentable() {
        assert!(!ROW.holds(70.0, 0.0, 79.0, Some(400.0)));
        assert!(!ROW.holds(70.0, 0.0, 400.0, Some(79.0)));
        assert!(!ROW.holds(70.0, 0.0, 400.0, None));
    }

    #[test]
    fn the_previous_row_must_fit_entirely_in_this_fragment() {
        // Предыдущий ряд — продолжение (начат выше `from`): ни один ряд целиком не влез.
        assert!(!ROW.holds(70.0, 20.0, 400.0, Some(400.0)));
        // Точка на конце ряда или выше границы — ряд влез либо не начат.
        assert!(!ROW.holds(82.0, 0.0, 400.0, Some(400.0)));
        assert!(!ROW.holds(40.0, 0.0, 400.0, Some(400.0)));
    }
}
