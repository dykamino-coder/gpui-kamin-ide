//! Computed::resolve_em: перевод длин в единицах шрифта (em/ex/ch/…) в точки по кеглю элемента и
//! родителя.

use super::*;
use crate::values::ValueCx;

impl Computed {
    /// Перевести `em` в точки по размеру шрифта.
    ///
    /// Размер шрифта известен только после каскада, поэтому длины в `em`
    /// доживают до этого места неразрешёнными. Для самого `font-size` база —
    /// РОДИТЕЛЬСКИЙ размер, для остального — свой собственный.
    pub fn resolve_em(&mut self, parent_font_px: f32, vcx: ValueCx<'_>) {
        // Сначала свой размер шрифта: от него считается всё остальное. Для
        // него самого единицы шрифта считаются от РОДИТЕЛЬСКОГО кегля.
        // Родовое `monospace` имени семейства не даёт, а меряться должно по
        // тому шрифту, которым текст в самом деле наберётся.
        let family = self.inh.font.font_family.clone().unwrap_or_else(|| {
            if self.inh.font.monospace == Some(true) {
                vcx.doc
                    .font_snapshot()
                    .mono_family(self.inh.text_flow.lang.as_deref())
                    .to_string()
            } else {
                String::new()
            }
        });
        // Свой кегль — первым: от него считается всё остальное (одна функция
        // с фазой шрифта корня, `computed_font_px`). Вычисленный кегль —
        // абсолютная длина, и шаг `larger`/`smaller` в нём уже учтён.
        if let Some(px) = self.computed_font_px(FontCx::new(&family, parent_font_px), vcx) {
            self.inh.font.font_size = Some(Len::Px(px));
            self.reset_cascade.font_size_step = 0;
        }
        self.compute_font_adjust(&family, vcx);
        if self.inh.font.font_size.is_none()
            && (self.inh.font.font_size_adjust.is_some()
                || self
                    .used_font_for(&family, parent_font_px, vcx)
                    .descriptor_scale
                    != 1.0)
        {
            self.inh.font.font_size = Some(Len::Px(parent_font_px));
        }
        // Дорожки сетки в единицах шрифта: считаются от СВОЕГО кегля.
        let own_px = match self.inh.font.font_size {
            Some(Len::Px(v)) => v,
            _ => parent_font_px,
        };
        // Длины в единицах шрифта внутри transform/transform-origin: свой
        // кегль известен только теперь.
        if self.reset_transforms.transform_raw.is_some()
            || self.reset_transforms.transform_origin_raw.is_some()
            || self.reset_effects.shadow_raw.is_some()
            || self.reset_text_box.text_shadow_raw.is_some()
        {
            let own_font = own_px;
            let [_, ch, ex, _] = self.glyph_units(&family, own_font, vcx);
            if let Some(raw) = self.reset_transforms.transform_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform", &px, vcx);
            }
            if let Some(raw) = self.reset_transforms.transform_origin_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform-origin", &px, vcx);
            }
            if let Some(raw) = self.reset_effects.shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("box-shadow", &px, vcx);
            }
            // `text-shadow` наследуется ВЫЧИСЛЕННЫМ значением: `em` решается
            // кеглем того элемента, где тень объявлена, а потомки получают уже
            // точки (сырая запись есть только у своего стиля).
            if let Some(raw) = self.reset_text_box.text_shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("text-shadow", &px, vcx);
            }
        }
        let grid_font = FontCx {
            family: &family,
            font_px: own_px,
            metric_px: self.used_font_for(&family, own_px, vcx).size_px,
            line_px: self.numeric_line_px(own_px),
            units: self
                .font_adjustment()
                .map(|_| self.glyph_units(&family, own_px, vcx)),
        };
        for list in {
            let grid = &mut *self.reset_grid;
            [grid.grid_tracks.as_mut(), grid.grid_rows.as_mut()]
        }
        .into_iter()
        .flatten()
        {
            for t in list.iter_mut() {
                t.resolve_font(grid_font, vcx);
            }
        }
        let base = match self.inh.font.font_size {
            Some(Len::Px(px)) => px,
            _ => parent_font_px,
        };
        // `ch` и `ex` меряются по ГЛИФАМ семейства, а не по кеглю: у Ahem
        // нуль занимает целый кегль, у текстового шрифта — около половины.
        // Семейство здесь уже унаследовано, поэтому замер возможен только на
        // этом шаге, вместе с `em`.
        let used = self.used_font_for(&family, base, vcx).size_px;
        let [cap, mut ch, ex, ic] = self.glyph_units(&family, base, vcx);
        // `ch` — продвижение нуля вдоль оси строки. При стоящих глифах в
        // вертикальном письме строка идёт сверху вниз, и продвижение равно
        // кеглю, а не ширине глифа (CSS Writing Modes §7.4).
        if self.wm().vertical() && self.wm().upright() {
            ch = used;
        }
        // `ic` меряется по тому же семейству и тем же шагом, что `ch` и `ex`.
        // `cap` — высота прописной того же лица (css-values-4 §6.1.4). Щуп
        // вертикальных метрик её уже отдаёт третьим числом (по нему
        // `text-box-trim` считает срез `cap`), своего замера не нужно.
        let to_px = move |l: &mut Option<Len>| match *l {
            Some(Len::Em(k)) => *l = Some(Len::Px(k * base)),
            Some(Len::EmPx(k, add)) => *l = Some(Len::Px(k * base + add)),
            Some(Len::Ch(k)) => *l = Some(Len::Px(k * ch)),
            Some(Len::Ex(k)) => *l = Some(Len::Px(k * ex)),
            Some(Len::Ic(k)) => *l = Some(Len::Px(k * ic)),
            // Смешанный calc: шрифтовые слагаемые складываются здесь — база
            // и метрики известны; остаток сворачивается заново.
            Some(Len::Calc(i)) => {
                let mut s = crate::style::values::value::calc_get(i, vcx);
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
                    *l = s.collapse_mixed(vcx);
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
        for l in {
            let group_0 = &mut *self.reset_box_layout;
            let group_1 = &mut *self.reset_fragmentation;
            [
                &mut group_0.width,
                &mut group_0.height,
                &mut group_0.min_width,
                &mut group_0.min_height,
                &mut group_0.max_width,
                &mut group_0.max_height,
                &mut self.reset_flex.flex_basis,
                &mut group_1.column_width,
                &mut group_1.column_height,
                &mut group_1.column_gap,
                &mut self.reset_svg_math.svg_path_length,
            ]
        } {
            fix(l);
        }
        // Наследуемые длины — в общей группе: она копируется, только если
        // есть что переводить.
        let unresolved = |l: &Option<Len>| {
            matches!(
                l,
                Some(
                    Len::Em(_)
                        | Len::EmPx(..)
                        | Len::Ch(_)
                        | Len::Ex(_)
                        | Len::Ic(_)
                        | Len::Calc(_)
                )
            )
        };
        if [
            &self.inh.text_flow.letter_spacing,
            &self.inh.text_flow.word_spacing,
            &self.inh.text_flow.text_indent,
            &self.inh.font.line_height,
        ]
        .into_iter()
        .any(unresolved)
        {
            let inh = &mut *self.inh;
            for l in {
                let group_0 = &mut *inh.text_flow;
                [
                    &mut group_0.letter_spacing,
                    &mut group_0.word_spacing,
                    &mut group_0.text_indent,
                    &mut inh.font.line_height,
                ]
            } {
                fix(l);
            }
        }
        sides(&mut self.reset_box_layout.padding);
        sides(&mut self.reset_box_layout.snap.padding);
        sides(&mut self.reset_box_layout.snap.margin);
        sides(&mut self.reset_box_layout.margin);
        sides(&mut self.reset_border.border_width);
        sides(&mut self.reset_positioned.inset);
        self.resolve_radius_lengths(fix);
        if let Some((row, col)) = self.reset_flex.gap.as_mut() {
            fix(row);
            fix(col);
        }
        if let Some(o) = self.reset_border.outline.as_mut() {
            fix(&mut o.width);
            fix(&mut o.offset);
        }
        self.resolve_decor_inset_font(fix, &family, base, vcx);
        self.resolve_gradient_lengths(fix, vcx);
        self.resolve_image_lengths(vcx);
    }
}
