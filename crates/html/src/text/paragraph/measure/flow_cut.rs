//! Вырез строки обтекаемыми флоатами: `flow_cut` — сколько слева и справа отбирают формы
//! `shape-outside` на высоте готовой строки номер `line_no`, `candidate_cut` — то же для
//! строки-кандидата по её настоящей коробке с надбавками и сдвигом под флоат; и базовая линия
//! струта от верха строки (`root_base`) — по замеренной коробке струта, иначе по метрикам
//! первого прогона.

use crate::text::paragraph::*;

impl Paragraph {
    pub(crate) fn flow_cut(&self, line_no: usize) -> (f32, f32) {
        if self.flow.0.is_empty() && self.flow.1.is_empty() {
            return (0.0, 0.0);
        }
        let pads = self.padding_for(&self.lines);
        let lh = f32::from(self.line_height);
        let y0: f32 = pads.iter().take(line_no).map(|(a, b)| lh + a + b).sum();
        let (a, b) = pads.get(line_no).copied().unwrap_or((0., 0.));
        let clearance = self
            .lines
            .get(line_no)
            .map_or(0., |l| f32::from(l.clearance));
        self.cut_band(y0 + clearance, y0 + lh + a + b)
    }

    /// Query the candidate's real line box before choosing its break.
    pub(crate) fn candidate_cut(
        &self,
        before: &[Line],
        range: std::ops::Range<usize>,
        clearance: gpui::Pixels,
    ) -> (f32, f32) {
        if self.flow.0.is_empty() && self.flow.1.is_empty() {
            return (0., 0.);
        }
        let mut lines = before.to_vec();
        lines.push(Line {
            range,
            clearance,
            width: gpui::px(0.),
            ellipsis: false,
            clamped: false,
            vis_cut: None,
            hyphen: false,
            indent: gpui::px(0.),
        });
        let pads = self.padding_for(&lines);
        let lh = f32::from(self.line_height);
        let y0: f32 = pads[..before.len()].iter().map(|(a, b)| lh + a + b).sum();
        let (a, b) = pads[before.len()];
        self.cut_band(y0 + f32::from(clearance), y0 + lh + a + b)
    }

    fn cut_band(&self, y0: f32, y1: f32) -> (f32, f32) {
        let l = self
            .flow
            .0
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        let r = self
            .flow
            .1
            .iter()
            .map(|f| f.cut(y0, y1))
            .fold(0.0f32, f32::max);
        (l, r)
    }
}

impl Paragraph {
    /// Базовая линия струта от верха строки: по замеренной коробке струта
    /// (`strut_box`), иначе по `strut`; без замера — `fallback`.
    pub(super) fn root_base(&self, fallback: f32) -> f32 {
        let lh = f32::from(self.line_height);
        let (sa, sd) = self.strut_box;
        if sa + sd > 0.0 {
            return (lh - (sa + sd)) / 2.0 + sa;
        }
        if self.strut.0 + self.strut.1 > 0.0 {
            return self.strut_base();
        }
        fallback
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::float::shapes::FloatShape;
    use gpui::px;

    #[test]
    fn zero_strut_image_uses_its_height_and_clearance_is_not_a_line() {
        let mut p = Paragraph::new(
            "x".into(),
            Vec::new(),
            px(16.),
            px(0.),
            Align::Left,
            Wrap::default(),
        );
        p.atom_boxes.push(AtomBox {
            at: 0,
            h: 100.,
            base: 100.,
            align: AtomAlign::Shift(0.),
            over: 0.,
            under: 0.,
            shift: 0.,
        });
        p.flow = std::sync::Arc::new((
            vec![FloatShape::Band {
                top: 0.,
                h: 200.,
                w: 100.,
            }],
            vec![FloatShape::Band {
                top: 0.,
                h: 100.,
                w: 100.,
            }],
        ));
        assert_eq!(p.candidate_cut(&[], 0..1, px(0.)), (100., 100.));
        assert_eq!(p.candidate_cut(&[], 0..1, px(100.)), (100., 0.));
        p.lines.push(Line {
            range: 0..1,
            clearance: px(100.),
            width: px(100.),
            ellipsis: false,
            clamped: false,
            vis_cut: None,
            hyphen: false,
            indent: px(100.),
        });
        let pads = p.line_padding();
        assert_eq!(p.lines.len(), 1);
        assert_eq!(pads[0].0 + pads[0].1, 200.);
        assert_eq!(p.flow_cut(0), (100., 0.));
        let key = p.split_key(Some(px(250.)));
        p.atom_boxes[0].h = 90.;
        assert_ne!(p.split_key(Some(px(250.))), key);
    }
}
