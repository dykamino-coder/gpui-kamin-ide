//! Computed::resolve_em: перевод длин в единицах шрифта (em/ex/ch/…) в точки по кеглю элемента и родителя.

use super::*;

impl Computed {
    pub fn resolve_em(&mut self, parent_font_px: f32) {
        // Сначала свой размер шрифта: от него считается всё остальное. Для
        // него самого единицы шрифта считаются от РОДИТЕЛЬСКОГО кегля.
        // Родовое `monospace` имени семейства не даёт, а меряться должно по
        // тому шрифту, которым текст в самом деле наберётся.
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                crate::text::metrics::mono_family_for(self.lang.as_deref()).to_string()
            } else {
                String::new()
            }
        });
        // Дорожки сетки в единицах шрифта: считаются от СВОЕГО кегля, он к
        // этому моменту уже разрешён вызывающим (см. ниже по функции).
        let own_px = match self.font_size {
            Some(Len::Px(v)) => v,
            _ => parent_font_px,
        };
        // Длины в единицах шрифта внутри transform/transform-origin: свой
        // кегль известен только теперь.
        if self.transform_raw.is_some()
            || self.transform_origin_raw.is_some()
            || self.shadow_raw.is_some()
            || self.text_shadow_raw.is_some()
        {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::text::metrics::ch_ex_px(&family, own_font);
            if let Some(raw) = self.transform_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform", &px);
            }
            if let Some(raw) = self.transform_origin_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform-origin", &px);
            }
            if let Some(raw) = self.shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("box-shadow", &px);
            }
            // `text-shadow` наследуется ВЫЧИСЛЕННЫМ значением: `em` решается
            // кеглем того элемента, где тень объявлена, а потомки получают уже
            // точки (сырая запись есть только у своего стиля).
            if let Some(raw) = self.text_shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("text-shadow", &px);
            }
        }
        if let Some(raw) = self.gradient_em.take() {
            let own_font = match self.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * parent_font_px,
                _ => parent_font_px,
            };
            let (ch, ex) = crate::text::metrics::ch_ex_px(&family, own_font);
            let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
            if let Some(g) = parse_gradient(&px) {
                self.gradient = Some(g);
                if self.gradient_raw.is_some() {
                    self.gradient_raw = Some(px);
                }
            }
        }
        for list in [self.grid_tracks.as_mut(), self.grid_rows.as_mut()]
            .into_iter()
            .flatten()
        {
            for t in list.iter_mut() {
                t.resolve_font(&family, own_px);
            }
        }
        match self.font_size {
            Some(Len::Em(k)) => self.font_size = Some(Len::Px(k * parent_font_px)),
            Some(Len::Ch(k)) => {
                let (ch, _) = crate::text::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ch));
            }
            Some(Len::Ex(k)) => {
                let (_, ex) = crate::text::metrics::ch_ex_px(&family, parent_font_px);
                self.font_size = Some(Len::Px(k * ex));
            }
            Some(Len::Ic(k)) => {
                self.font_size = Some(Len::Px(
                    k * crate::text::metrics::ic_px(&family, parent_font_px),
                ));
            }
            _ => {}
        }
        let base = match self.font_size {
            Some(Len::Px(px)) => px,
            _ => parent_font_px,
        };
        // `ch` и `ex` меряются по ГЛИФАМ семейства, а не по кеглю: у Ahem
        // нуль занимает целый кегль, у текстового шрифта — около половины.
        // Семейство здесь уже унаследовано, поэтому замер возможен только на
        // этом шаге, вместе с `em`.
        // Повторный вызов на уже подогнанном стиле: база — сохранённый
        // вычисленный кегль, а не подогнанный.
        let base = self.font_adjust_base.map_or(base, |b| b.0);
        // Используемый кегль (css-fonts-5 §font-size-adjust): «affects the
        // size of relative units that are based on font metrics such as ex
        // and ch but does not affect the size of em units». `em` и числовой
        // `line-height` остаются от `base`, метрики шрифта — от `used`, и сам
        // текст набирается `used` (`font_size`), а детям уходит `base`.
        let used = base * self.used_font_factor(&family);
        if used != base && self.font_adjust_base.is_none() {
            self.font_adjust_base = Some((base, self.line_height));
            if let Some(Len::Pct(m)) = self.line_height {
                self.line_height = Some(Len::Px(m * base));
            }
            self.font_size = Some(Len::Px(used.max(0.01)));
        }
        let (mut ch, ex) = crate::text::metrics::ch_ex_px(&family, used);
        // `ch` — продвижение нуля вдоль оси строки. При стоящих глифах в
        // вертикальном письме строка идёт сверху вниз, и продвижение равно
        // кеглю, а не ширине глифа (CSS Writing Modes §7.4).
        if self.vertical == Some(true) && self.upright == Some(true) {
            ch = used;
        }
        // `ic` меряется по тому же семейству и тем же шагом, что `ch` и `ex`.
        let ic = crate::text::metrics::ic_px(&family, used);
        // `cap` — высота прописной того же лица (css-values-4 §6.1.4). Щуп
        // вертикальных метрик её уже отдаёт третьим числом (по нему
        // `text-box-trim` считает срез `cap`), своего замера не нужно.
        let cap = crate::text::metrics::vmetrics_px(&family, used).2;
        let to_px = move |l: &mut Option<Len>| match *l {
            Some(Len::Em(k)) => *l = Some(Len::Px(k * base)),
            Some(Len::EmPx(k, add)) => *l = Some(Len::Px(k * base + add)),
            Some(Len::Ch(k)) => *l = Some(Len::Px(k * ch)),
            Some(Len::Ex(k)) => *l = Some(Len::Px(k * ex)),
            Some(Len::Ic(k)) => *l = Some(Len::Px(k * ic)),
            // Смешанный calc: шрифтовые слагаемые складываются здесь — база
            // и метрики известны; остаток сворачивается заново.
            Some(Len::Calc(i)) => {
                let mut s = crate::style::values::value::calc_get(i);
                // Без шрифтовых слагаемых складывать нечего — индекс остаётся
                // прежним: арена append-only, а `resolve_em` идёт на каждом
                // наследовании, и повторное хранение раздувало бы её впустую.
                if s.em != 0.0 || s.ch != 0.0 || s.ex != 0.0 || s.ic != 0.0 || s.cap != 0.0 {
                    s.px += s.em * base + s.ch * ch + s.ex * ex + s.ic * ic + s.cap * cap;
                    s.em = 0.0;
                    s.ch = 0.0;
                    s.ex = 0.0;
                    s.ic = 0.0;
                    s.cap = 0.0;
                    // Процентная смесь обязана ДОЖИТЬ: `collapse` вернул бы
                    // `None` и стёр `text-indent: calc(1em + 50%)`. Для всего,
                    // что пришло из `Len::parse`, `pct == 0`, и обе свёртки
                    // совпадают.
                    *l = s.collapse_mixed();
                }
            }
            _ => {}
        };
        let fix = to_px;
        let sides = |s: &mut Sides| {
            for one in [&mut s.top, &mut s.right, &mut s.bottom, &mut s.left] {
                to_px(one);
            }
        };
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.letter_spacing,
            &mut self.word_spacing,
            &mut self.text_indent,
            &mut self.line_height,
            &mut self.column_width,
            &mut self.column_height,
            &mut self.column_gap,
        ] {
            fix(l);
        }
        sides(&mut self.padding);
        sides(&mut self.margin);
        sides(&mut self.border_width);
        sides(&mut self.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.gap.as_mut() {
            fix(row);
            fix(col);
        }
        if let Some(o) = self.outline.as_mut() {
            fix(&mut o.width);
            fix(&mut o.offset);
        }
    }
}
