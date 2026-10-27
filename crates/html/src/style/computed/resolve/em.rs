//! Computed::resolve_em: перевод длин в единицах шрифта (em/ex/ch/…) в точки по кеглю элемента и
//! родителя.

use super::*;
use crate::document::state::ValueCx;

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
        let family = self.font_family.clone().unwrap_or_else(|| {
            if self.monospace == Some(true) {
                vcx.doc
                    .font_snapshot()
                    .mono_family(self.lang.as_deref())
                    .to_string()
            } else {
                String::new()
            }
        });
        // Свой кегль — первым: от него считается всё остальное (одна функция
        // с фазой шрифта корня, `computed_font_px`). Вычисленный кегль —
        // абсолютная длина, и шаг `larger`/`smaller` в нём уже учтён.
        if let Some(px) = self.computed_font_px(FontCx::new(&family, parent_font_px), vcx) {
            self.font_size = Some(Len::Px(px));
            self.font_size_step = 0;
        }
        self.compute_font_adjust(&family, vcx);
        if self.font_size.is_none()
            && (self.font_size_adjust.is_some()
                || self
                    .used_font_for(&family, parent_font_px, vcx)
                    .descriptor_scale
                    != 1.0)
        {
            self.font_size = Some(Len::Px(parent_font_px));
        }
        // Дорожки сетки в единицах шрифта: считаются от СВОЕГО кегля.
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
            let own_font = own_px;
            let [_, ch, ex, _] = self.glyph_units(&family, own_font, vcx);
            if let Some(raw) = self.transform_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform", &px, vcx);
            }
            if let Some(raw) = self.transform_origin_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("transform-origin", &px, vcx);
            }
            if let Some(raw) = self.shadow_raw.take() {
                let px = font_lengths_to_px(&raw, own_font, 16.0, ex, ch);
                self.apply_one("box-shadow", &px, vcx);
            }
            // `text-shadow` наследуется ВЫЧИСЛЕННЫМ значением: `em` решается
            // кеглем того элемента, где тень объявлена, а потомки получают уже
            // точки (сырая запись есть только у своего стиля).
            if let Some(raw) = self.text_shadow_raw.take() {
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
                .font_size_adjust
                .map(|_| self.glyph_units(&family, own_px, vcx)),
        };
        for list in [self.grid_tracks.as_mut(), self.grid_rows.as_mut()]
            .into_iter()
            .flatten()
        {
            for t in list.iter_mut() {
                t.resolve_font(grid_font, vcx);
            }
        }
        let base = match self.font_size {
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
        for l in [
            &mut self.width,
            &mut self.height,
            &mut self.min_width,
            &mut self.min_height,
            &mut self.max_width,
            &mut self.max_height,
            &mut self.flex_basis,
            &mut self.column_width,
            &mut self.column_height,
            &mut self.column_gap,
            &mut self.svg_path_length,
        ] {
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
            &self.letter_spacing,
            &self.word_spacing,
            &self.text_indent,
            &self.line_height,
        ]
        .into_iter()
        .any(unresolved)
        {
            let inh = &mut **self;
            for l in [
                &mut inh.letter_spacing,
                &mut inh.word_spacing,
                &mut inh.text_indent,
                &mut inh.line_height,
            ] {
                fix(l);
            }
        }
        sides(&mut self.padding);
        sides(&mut self.snap.padding);
        sides(&mut self.snap.margin);
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
        self.resolve_decor_inset_font(fix, &family, base, vcx);
        self.resolve_gradient_lengths(fix, vcx);
        self.resolve_image_lengths(vcx);
    }
}
