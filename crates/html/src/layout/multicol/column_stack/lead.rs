//! Ведущая неделимая протяжённость вложенной стопки колонок для рекордера объемлющей копии.

use super::ColumnStack;
use crate::layout::fragment::paginate::watch::Watched;
use crate::layout::fragment::types::StackChild;
use crate::tolerance::COORD_EPS;
use std::rc::Rc;

/// Рекордер копии, внутри которой раскладывается стопка, и id коробки стопки.
pub(crate) type LeadReport = (Rc<Watched>, u64);

impl ColumnStack {
    /// Рекордер объемлющей копии (вложенный многоколоночник).
    pub(crate) fn with_lead_report(mut self, report: Option<LeadReport>) -> Self {
        self.lead_report = report;
        self
    }

    /// Сообщить рекордеру объемлющей копии, сколько содержимого от верха стопки
    /// не делится: первый ряд колонок начинается с этого содержимого целиком
    /// (css-break-3 §4.4). Зовётся после раскладки детей.
    pub(crate) fn report_lead(&self) {
        if let Some((watched, id)) = &self.lead_report {
            let lead = leading_unbreakable(&self.children);
            if lead > COORD_EPS {
                watched.report_lead(*id, lead);
            }
        }
    }
}

/// Протяжённость от верха стопки до первой точки разрыва внутри первого
/// содержательного ребёнка (`leading`).
pub(crate) fn leading_unbreakable(children: &[StackChild]) -> f32 {
    leading(children.iter().map(|c| LeadKid {
        span: c.span,
        h: c.h,
        monolith: c.monolith,
        solid: &c.solid,
        mt: c.mt,
    }))
}

/// Геометрия ребёнка стопки, нужная ведущей протяжённости.
struct LeadKid<'a> {
    span: bool,
    h: f32,
    monolith: bool,
    solid: &'a [(f32, f32)],
    mt: f32,
}

/// Монолит целиком, иначе сплошной диапазон с верха ребёнка. Нулевые дети (щупы
/// статической позиции) пропускаются; спаннер начинает ряд сам.
fn leading<'a>(kids: impl IntoIterator<Item = LeadKid<'a>>) -> f32 {
    for c in kids {
        if c.span {
            return 0.0;
        }
        if c.h <= COORD_EPS && !c.monolith {
            continue;
        }
        let inner = if c.monolith {
            c.h
        } else {
            c.solid
                .iter()
                .filter(|&&(a, _)| a <= COORD_EPS)
                .map(|&(_, b)| b)
                .fold(0.0, f32::max)
        };
        return if inner > COORD_EPS { c.mt + inner } else { 0.0 };
    }
    0.0
}

#[cfg(test)]
mod tests {
    use super::{LeadKid, leading};

    fn kid(h: f32, monolith: bool, solid: &[(f32, f32)], mt: f32) -> LeadKid<'_> {
        LeadKid {
            span: false,
            h,
            monolith,
            solid,
            mt,
        }
    }

    #[test]
    fn lead_is_the_first_real_kids_unbreakable_start() {
        let probe = kid(0.0, false, &[], 0.0);
        assert_eq!(leading([probe, kid(20.0, true, &[], 5.0)]), 25.0);
        assert_eq!(
            leading([kid(80.0, false, &[(0.0, 18.0), (30.0, 40.0)], 0.0)]),
            18.0
        );
        assert_eq!(leading([kid(80.0, false, &[(10.0, 18.0)], 0.0)]), 0.0);
        let span = LeadKid {
            span: true,
            ..kid(20.0, true, &[], 0.0)
        };
        assert_eq!(leading([span]), 0.0);
    }
}
