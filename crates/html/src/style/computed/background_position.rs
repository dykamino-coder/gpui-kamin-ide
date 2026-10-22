//! Вычисленный стиль: `background-position` с функциями сравнения
//! (`min()`/`max()`/`clamp()` с долями). Такое выражение не сводится к линейной
//! `Len`, поэтому хранится сырым (`bg_position_math`) с заглушкой для разбора
//! оси/края, а используемое значение считается при краске от конкретного
//! свободного места образа (CSS Values 4 §10.2/10.10; Blink `ResolveXPosition`
//! передаёт доступную ширину, в том числе отрицательную, в вычислитель длин).

use crate::document::state::ValueCx;
mod expressions;
mod value;
use super::{Computed, parse_pos_words};
use crate::style::values::value::Len;
use expressions::{capture, math};
pub(crate) use value::PositionExpression;

impl Computed {
    pub(crate) fn background_position_length(raw: &str, vcx: ValueCx<'_>) -> bool {
        Len::parse_mixed(raw, vcx).is_some_and(|l| {
            !matches!(
                l,
                Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent
            )
        }) || math(raw, vcx).is_some()
    }

    /// Ось положения верхнего слоя (`background-position-x/-y`,
    /// css-backgrounds-4 §3.6): значение оси дополняется краем другой оси до
    /// `<bg-position>` и разбирается тем же разбором, что положение целиком;
    /// пишется только своя ось. Начальное `0%` хранится отсутствием значения
    /// (вычисленное значение то же).
    pub(crate) fn set_background_position_axis(
        &mut self,
        axis: usize,
        raw: &str,
        vcx: ValueCx<'_>,
    ) {
        let full = match axis {
            0 => format!("{raw} top"),
            _ => format!("left {raw}"),
        };
        let (pos, mut math) = position_of(&full, vcx);
        let expression = math[axis].take();
        let value = [pos.x, pos.y][axis].filter(|v| *v != Len::Pct(0.0) || expression.is_some());
        if axis == 0 {
            self.bg_pos.x = value;
        } else {
            self.bg_pos.y = value;
        }
        self.bg_position_math[axis] = expression;
        self.accept_declaration();
    }

    /// CSS Values 4 §10.2/10.10 compares resolved lengths, not percentage
    /// coefficients. Blink ResolveXPosition passes available width (which
    /// can be negative) into the calculated length evaluator.
    pub(crate) fn background_position_used(
        &self,
        fallback: (f32, f32),
        free: (f32, f32),
        _vcx: ValueCx<'_>,
    ) -> (f32, f32) {
        if self.bg_position_math.iter().all(Option::is_none) {
            return fallback;
        }
        let mut used = [fallback.0, fallback.1];
        for (i, expression) in self.bg_position_math.iter().enumerate() {
            if let Some(expression) = expression
                && let Some(value) = expression.evaluate([free.0, free.1][i])
            {
                used[i] = value;
            }
        }
        (used[0], used[1])
    }
}

/// Положение (верхний слой) и выражения сравнения по осям. Смешанное
/// сравнение ещё не линейная `Len`: для разбора осей и краёв стоит заглушка,
/// используемое значение даёт сохранённое выражение.
fn position_of(raw: &str, vcx: ValueCx<'_>) -> (super::BgPos, [Option<PositionExpression>; 2]) {
    let first = crate::style::css::split_args(raw)
        .into_iter()
        .next()
        .unwrap_or("");
    let words: Vec<String> = super::split_outside_parens(first)
        .into_iter()
        .map(|v| {
            if Len::parse_mixed(&v, vcx).is_none() && math(&v, vcx).is_some() {
                "0px".into()
            } else {
                v
            }
        })
        .collect();
    (parse_pos_words(&words.join(" "), vcx), capture(raw, vcx))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn negative_free_space_changes_comparison_order_and_axis_overrides() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed::default();
        c.apply_one("background-position", "min(0%, 100%) max(0%, 100%)", vcx);
        assert_eq!(
            c.background_position_used((123.0, 123.0), (-50.0, -50.0), vcx),
            (-50.0, 0.0)
        );
        assert_eq!(
            c.background_position_used((123.0, 123.0), (50.0, 50.0), vcx),
            (0.0, 50.0)
        );
        c.apply_one("background-position-x", "5px", vcx);
        assert_eq!(
            c.background_position_used((5.0, 123.0), (-50.0, -50.0), vcx),
            (5.0, 0.0)
        );
        c.apply_one(
            "background-position",
            "right min(100%, 10px) bottom max(100%, 10px)",
            vcx,
        );
        assert_eq!(
            c.background_position_used((0.0, 0.0), (-50.0, -50.0), vcx),
            (0.0, -60.0)
        );
    }

    #[test]
    fn computed_relative_lengths_survive_a_different_inherited_font() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut parent = Computed {
            font_size: Some(Len::Px(20.0)),
            line_height: Some(Len::Pct(2.0)),
            ..Computed::default()
        };
        parent.apply_one("background-position", "min(100%, 1em) max(100%, 1lh)", vcx);
        parent.resolve_image_lengths(vcx);
        let mut child = parent.clone();
        child.font_size = Some(Len::Px(80.0));
        child.line_height = Some(Len::Px(200.0));
        assert_eq!(
            child.background_position_used((0.0, 0.0), (50.0, 50.0), vcx),
            (20.0, 50.0)
        );
    }

    #[test]
    fn layer_position_expressions_resolve_the_owning_line_height() {
        let doc = crate::document::state::DocState::new(crate::style::css::Media::default());
        let vcx = doc.values();
        let mut c = Computed {
            font_size: Some(Len::Px(30.0)),
            line_height: Some(Len::Px(80.0)),
            ..Computed::default()
        };
        c.apply_one("background-image", "url(a), url(b)", vcx);
        c.apply_one(
            "background-position",
            "min(100%, 1lh) 0px, max(100%, 2lh) 0px",
            vcx,
        );
        c.resolve_em(16.0, vcx);
        let layers = c.bg_layers(vcx).unwrap();
        assert_eq!(
            layers[0].background_position_used((0.0, 0.0), (100.0, 100.0), vcx),
            (80.0, 0.0)
        );
        assert_eq!(
            layers[1].background_position_used((0.0, 0.0), (100.0, 100.0), vcx),
            (160.0, 0.0)
        );
    }
}
