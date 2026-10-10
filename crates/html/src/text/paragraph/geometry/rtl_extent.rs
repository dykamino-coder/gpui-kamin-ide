//! Rtl extent for geometry; split out to keep the owning module within 250 lines.

use crate::text::paragraph::*;
use gpui::{Bounds, Pixels, Point, point, px};

impl Paragraph {
    /// Визуальный отрезок знаков `a..b` строки rtl-абзаца от её ЛЕВОГО края:
    /// прогоны UAX#9 в визуальном порядке (L2), как у `visual_x_rtl`, внутри
    /// rtl-прогона знаки идут справа налево. Пустой отрезок — точка `a`.
    pub(crate) fn visual_extent_rtl(
        &self,
        segs: &[Seg],
        a: usize,
        b: usize,
        line: &Line,
    ) -> (Pixels, Pixels) {
        let start = line.range.start;
        let end = start + trim_hanging(&self.text[line.range.clone()]);
        let a = a.clamp(start, end);
        let b = b.clamp(a, end);
        let info = unicode_bidi::BidiInfo::new(&self.text, Some(unicode_bidi::Level::rtl()));
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= start && start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return (px(0.), px(0.));
        };
        if a == b {
            let x = self.visual_x_rtl(segs, a, line);
            return (x, x);
        }
        let (levels, runs) = info.visual_runs(para, start..end);
        let mut x = px(0.);
        let mut lo: Option<Pixels> = None;
        let mut hi: Option<Pixels> = None;
        for run in runs {
            let w = self.span(segs, run.start, run.end);
            let (s, e) = (a.max(run.start), b.min(run.end));
            if s < e {
                let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
                let off = if rtl {
                    self.span(segs, e, run.end)
                } else {
                    self.span(segs, run.start, s)
                };
                let part = self.span(segs, s, e);
                lo = Some(lo.map_or(x + off, |v: Pixels| v.min(x + off)));
                hi = Some(hi.map_or(x + off + part, |v: Pixels| v.max(x + off + part)));
            }
            x += w;
        }
        (lo.unwrap_or(px(0.)), hi.unwrap_or(px(0.)))
    }
}

impl Paragraph {
    /// Визуальное продвижение места `at` от ЛЕВОГО края rtl-строки.
    ///
    /// На место куска ставится нейтральный U+FFFC (так UAX#9 видит
    /// замещаемый объект), строка разбирается с базой rtl, и прогоны идут в
    /// ВИЗУАЛЬНОМ порядке (L2), как у отрисовки (`paint_line`): слева
    /// складываются ширины прогонов до прогона метки, внутри него — знаки
    /// до метки (ltr-прогон) или после неё (rtl-прогон). Пример
    /// `abs-pos-non-replaced-vrl-008`: строка «34» + абсолют — метка уровня 1
    /// после числа уровня 2 встаёт ЛЕВЕЕ числа, и коробка висит от левого
    /// края строки, а не от правого.
    pub(crate) fn visual_x_rtl(&self, segs: &[Seg], at: usize, line: &Line) -> Pixels {
        let start = line.range.start;
        let end = start + trim_hanging(&self.text[line.range.clone()]);
        let at = at.clamp(start, end);
        const MARK: usize = 3; // U+FFFC в UTF-8
        let mut probe = String::with_capacity(self.text.len() + MARK);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let info = unicode_bidi::BidiInfo::new(&probe, Some(unicode_bidi::Level::rtl()));
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= start && start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return px(0.);
        };
        let (levels, runs) = info.visual_runs(para, start..end + MARK);
        // Отрезок метки-строки обратно в отрезок исходного текста.
        let orig = |p: usize| if p <= at { p } else { p - MARK };
        let width = |a: usize, b: usize| self.span(segs, orig(a), orig(b));
        let mut x = px(0.);
        for run in runs {
            let mark_in = run.start <= at && at < run.end;
            if !mark_in {
                x += width(run.start, run.end);
                continue;
            }
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            x += if rtl {
                width(at + MARK, run.end)
            } else {
                width(run.start, at)
            };
            break;
        }
        x
    }
}

impl Paragraph {
    /// Уровень bidi у места куска вне потока — справа налево ли? Сам кусок
    /// в тексте знака не имеет, поэтому на его место ставится нейтральный
    /// U+FFFC (так UAX#9 видит замещаемый объект): его уровень решают
    /// соседи по правилам N1/N2.
    pub(crate) fn rtl_level_at(&self, at: usize) -> bool {
        let at = at.min(self.text.len());
        let mut probe = String::with_capacity(self.text.len() + 3);
        probe.push_str(&self.text[..at]);
        probe.push('\u{fffc}');
        probe.push_str(&self.text[at..]);
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        let forced = if self.plaintext.is_some() {
            None
        } else {
            Some(base)
        };
        let info = unicode_bidi::BidiInfo::new(&probe, forced);
        info.levels.get(at).map_or(self.wrap.rtl, |l| l.is_rtl())
    }
}

impl Paragraph {
    /// Статическая позиция БЛОЧНОГО куска вне потока: строчное начало —
    /// край содержимого (без `text-indent`: отступ — свойство первой
    /// СТРОКИ, а гипотетическая коробка — блок), блочное — начало строки,
    /// следующей за той, где кусок стоит в тексте (CSS 2.1 §10.6.4 «if
    /// position had been static»: блок в строчном содержимом рвёт строку и
    /// встаёт после неё). Кусок в самом начале строки ничего перед собой не
    /// имеет — строка рвётся ДО него, и место — верх этой же строки.
    /// Порядок строк на экране при `lines_reversed` зеркален (см. `point_of`).
    pub(crate) fn next_line_point(&self, at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let count = self.lines.len();
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(count.saturating_sub(1));
        // Есть ли перед куском в его строке настоящее содержимое (служебные
        // распорки `SPACER`/`ZWSP` места не занимают).
        let after = self.lines.get(row).is_some_and(|l| {
            let end = at.clamp(l.range.start, l.range.end);
            self.text
                .get(l.range.start..end)
                .is_some_and(|t| t.chars().any(|c| !matches!(c, '\u{feff}' | '\u{200b}')))
        });
        let visual = if self.lines_reversed {
            count.saturating_sub(1).saturating_sub(row) as f32
        } else {
            row as f32
        };
        // Следующая строка: при обратном порядке она ВЫШЕ на экране.
        let step = match (after, self.lines_reversed) {
            (false, _) => 0.0,
            (true, false) => 1.0,
            (true, true) => -1.0,
        };
        point(
            bounds.origin.x,
            bounds.origin.y + self.line_height * (visual + step),
        )
    }
}
