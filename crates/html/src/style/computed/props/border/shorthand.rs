//! Стиль и сокращения рамки: set_border_style, set_visible, apply_border_shorthand (`border: 1px solid red` и стороны).

use super::*;

impl Computed {
    /// `border-left-style: solid` — рисунок одной стороны. Без рисунка её
    /// толщина не считается, поэтому видимость помним по сторонам.
    pub(crate) fn set_border_style(&mut self, v: &str, side: Option<usize>) {
        let on = border_style(v);
        if let Some(r) = border_style_rank(v) {
            match side {
                None => self.border_side_styles = [Some(r); 4],
                Some(i) => self.border_side_styles[i] = Some(r),
            }
        }
        self.set_visible(side, on);
        if on {
            let w = match side {
                Some(0) => &mut self.border_width.top,
                Some(1) => &mut self.border_width.right,
                Some(2) => &mut self.border_width.bottom,
                _ => &mut self.border_width.left,
            };
            if w.is_none() {
                *w = Some(Len::Px(3.0));
            }
        }
    }

    pub(crate) fn set_visible(&mut self, side: Option<usize>, on: bool) {
        match side {
            None => self.border_visible = [Some(on); 4],
            Some(i) => self.border_visible[i] = Some(on),
        }
    }

    /// `border: 1px solid #333` — ширина и цвет; стиль линии GPUI различает
    /// только solid/dashed на весь элемент, поэтому его не разбираем.
    pub(crate) fn apply_border_shorthand(&mut self, v: &str, side: Option<usize>) {
        // Негодная часть роняет ВСЁ объявление (§4.2), а не пропускается:
        // `border: -1px solid red` не даёт ни рамки `medium`, ни красного
        // цвета. Проверка отдельным проходом — применение ниже правит поля по
        // ходу разбора, и откатить его на середине уже нельзя.
        let known = |token: &str| {
            token == "none"
                || token == "hidden"
                || border_style(token)
                || line_width(token).is_some()
                || Color::parse(token).is_some()
        };
        if !split_outside_parens(v)
            .iter()
            .all(|t| known(t.as_str().trim()))
        {
            return;
        }
        // Каждая часть встречается не больше ОДНОГО раза (§8.5.4: сокращение
        // это `<border-width> || <border-style> || <border-color>`).
        // `border: 1px solid red green` негодно целиком, а прежде вторая
        // краска просто побеждала первую.
        {
            let (mut w, mut st, mut c) = (0usize, 0usize, 0usize);
            for token in split_outside_parens(v) {
                let t = token.as_str().trim();
                if t == "none" || t == "hidden" || border_style(t) {
                    st += 1;
                } else if line_width(t).is_some() {
                    w += 1;
                } else if Color::parse(t).is_some() {
                    c += 1;
                }
            }
            if w > 1 || st > 1 || c > 1 {
                return;
            }
        }
        let mut width = None;
        let mut color = None;
        let mut visible_style = false;
        // Скобки не разрываем: `border: 1px solid rgba(0, 0, 0, .2)`.
        for token in split_outside_parens(v) {
            let token = token.as_str();
            if token == "none" || token == "hidden" {
                width = Some(Len::Px(0.0));
                self.set_visible(side, false);
                let r = border_style_rank(token);
                match side {
                    None => self.border_side_styles = [r; 4],
                    Some(i) => self.border_side_styles[i] = r,
                }
            } else if border_style(token) {
                visible_style = true;
                self.set_visible(side, true);
                if let Some(r) = border_style_rank(token) {
                    match side {
                        None => self.border_side_styles = [Some(r); 4],
                        Some(i) => self.border_side_styles[i] = Some(r),
                    }
                }
                self.border_dashed = Some(token == "dashed");
                self.border_dotted = Some(token == "dotted");
            } else if let Some(l) = line_width(token) {
                width = Some(l);
            } else if let Some(c) = Color::parse(token) {
                color = Some(c);
            }
        }
        // Толщина в сокращении необязательна: `border: orange solid` — это
        // рамка `medium`, то есть 3px (css-backgrounds-3 §4.2). Раньше такая
        // запись оставляла коробку БЕЗ рамки, и её содержимое считалось по
        // другой ширине (`word-break-break-all-062`).
        if width.is_none() && visible_style {
            width = Some(Len::Px(3.0));
        }
        // Цвет из БОКОВОГО сокращения принадлежит своей стороне: раньше он
        // писался в общий цвет, и `border-bottom: 2px solid red` красил все
        // четыре стороны.
        // Опущенная часть сокращения возвращается к НАЧАЛЬНОМУ значению
        // (§1.4.2, §8.5.4): у цвета это `currentColor`, то есть пусто — цвет
        // решает отрисовка. Прежде запись `border: solid 1em` оставляла цвет
        // от менее специфичного правила.
        match (color, side) {
            (Some(c), None) => {
                self.border_color = Some(c);
                self.border_colors = [Some(c); 4];
            }
            (Some(c), Some(i)) => self.border_colors[i] = Some(c),
            (None, None) => {
                self.border_color = None;
                self.border_colors = [None; 4];
            }
            // Боковое сокращение без цвета даёт стороне `currentColor`
            // (§8.5.4), и он обязан перебить общий цвет менее специфичного
            // правила: `div { border-color: red }` + `.test { border-top: solid
            // 1em }` — верх цвета ТЕКСТА (`border-shorthands-003`). Пустой слот
            // стороны значит «взять общий», поэтому сторона помечается.
            (None, Some(i)) => {
                self.border_colors[i] = None;
                self.border_side_current[i] = true;
            }
        }
        let Some(w) = width else { return };
        match side {
            None => {
                self.border_width = Sides {
                    top: Some(w),
                    right: Some(w),
                    bottom: Some(w),
                    left: Some(w),
                }
            }
            Some(0) => self.border_width.top = Some(w),
            Some(1) => self.border_width.right = Some(w),
            Some(2) => self.border_width.bottom = Some(w),
            Some(3) => self.border_width.left = Some(w),
            _ => {}
        }
    }
}
