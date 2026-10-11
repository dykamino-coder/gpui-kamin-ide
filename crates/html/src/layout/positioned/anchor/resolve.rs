//! Выбранная запасная позиция и `anchor-size()` в стиле коробки.

use super::settle::{DefaultAnchor, default_anchor_of};
use super::{AREA_LAST, AnchorRec, CELL_LAST, CHOSEN, LAST_IMPLICIT, LAST_NAMED, USED_LAST};
use crate::style::computed::{Align, Computed, Position};
use crate::style::values::value::{AnchorSize, Len, anchor_get};
use std::rc::Rc;

/// Выбранный на прошлом кадре вариант `position-try-fallbacks` (`CHOSEN`) —
/// в стиль ДО раскладки (зовёт `render::element` сразу после ключей):
/// объявления его `@position-try`-правила (размеры, вставки) и
/// `<position-area>` накладываются на `Computed`, база уезжает в `try_base`
/// для сборки остальных кандидатов (`place`). Тактики раскладке не нужны:
/// сдвиг считается абсолютно на подготовке.
pub fn apply_chosen(c: &mut Computed) {
    if c.position_try_fallbacks.is_empty()
        || !matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed))
    {
        return;
    }
    let base = Rc::new(c.clone());
    let chosen = CHOSEN
        .with(|m| m.borrow().get(&c.anchor_key).copied())
        .unwrap_or(0);
    if chosen > 0
        && let Some(fb) = base.position_try_fallbacks.get(chosen - 1)
    {
        if let Some(n) = &fb.name
            && let Some(decls) = crate::style::css::try_rule(n)
        {
            c.apply_decls(&decls);
        }
        if let Some(area) = fb.area {
            c.position_area = Some(area);
        }
    }
    c.try_base = Some(base);
}

/// Якорь из реестра ПРОШЛОГО кадра: по имени — последняя запись с этим
/// именем, собранная РАНЬШЕ коробки (`seq`), — «the last element in tree
/// order» среди «laid out strictly before» (§target anchor element);
/// неявный — по `node_id` хозяина.
fn last_lookup(name: Option<&str>, default: &Option<DefaultAnchor>, seq: u32) -> Option<AnchorRec> {
    match name {
        Some(n) => last_named(n, seq),
        None => match default {
            Some(DefaultAnchor::Named(n)) => last_named(n, seq),
            Some(DefaultAnchor::Implicit(id)) => {
                LAST_IMPLICIT.with(|m| m.borrow().get(id).copied())
            }
            None => None,
        },
    }
}

/// Последняя запись прошлого кадра с этим именем, собранная раньше `seq`.
pub(super) fn last_named(n: &str, seq: u32) -> Option<AnchorRec> {
    LAST_NAMED.with(|v| {
        v.borrow()
            .iter()
            .filter(|(k, s, _)| k == n && *s < seq)
            .max_by_key(|(_, s, _)| *s)
            .map(|(_, _, r)| *r)
    })
}

/// Размеры абсолюта, зависящие от якоря, — В ТОЧКИ до раскладки (зовёт
/// `render::element` сразу после слияния стилей):
/// * `anchor-size()` в `width/height/min-*/max-*` (§anchor-size-fn) — из
///   реестра прошлого кадра; неразрешимая — запасное значение, без него
///   `auto` («invalid at computed-value time»);
/// * `place-self: stretch` в клетке `position-area` — размер IMCB прошлого
///   кадра за вычетом полей, рамки и отбивок (`width` у нас — содержимое).
///
/// Первый кадр отдаёт запасные значения, второй — верные; стенд ждёт
/// устоявшихся кадров.
pub fn resolve_sizes(c: &mut Computed, inherited: &Computed) {
    if !matches!(c.position, Some(Position::Absolute) | Some(Position::Fixed)) {
        return;
    }
    let seq = c.anchor_seq;
    let default_anchor = default_anchor_of(c);
    let cb_vertical = inherited.vertical == Some(true);
    let own_vertical = c.vertical == Some(true);
    let one = |l: Option<Len>, y_axis: bool| -> Option<Len> {
        let Some(Len::Anchor(i)) = l else { return l };
        USED_LAST.with(|u| u.set(true));
        let mut f = anchor_get(i);
        for _ in 0..4 {
            let Some(cur) = f.take() else { break };
            // `anchor()` в размере негодна (§anchor-fn: только вставки).
            let kind = cur.size?;
            let hit = last_lookup(cur.name.as_deref(), &default_anchor, seq).map(|r| {
                // Размер решается ДО раскладки, стека трансформов коробки ещё
                // нет: под трансформированным предком считаем цепочку общей
                // (до-трансформная рамка), без него — отображённую
                // (`transform-002/009`: `anchor-size(width)` = 100, а не 50).
                let b = if r.tf_top != 0
                    && !(inherited.transform_ancestor || inherited.transform.is_some())
                {
                    r.rect_tf
                } else {
                    r.rect
                };
                let width = match kind {
                    AnchorSize::Width => true,
                    AnchorSize::Height => false,
                    AnchorSize::Implicit => !y_axis,
                    AnchorSize::Block => cb_vertical,
                    AnchorSize::Inline => !cb_vertical,
                    AnchorSize::SelfBlock => own_vertical,
                    AnchorSize::SelfInline => !own_vertical,
                };
                let v = if width { b.size.width } else { b.size.height };
                f32::from(v) + cur.add
            });
            match (hit, cur.fallback) {
                (Some(v), _) => return Some(Len::Px(v)),
                (None, Some(Len::Anchor(j))) => f = anchor_get(j),
                (None, fb) => return fb,
            }
        }
        None
    };
    // §position-area: клетка «makes that the box's containing block», и
    // доли размеров, полей и отбивок решаются от неё, а не от исходного
    // содержащего блока (заметка спеки: «like max-height: 100%»). Раскладка
    // клетки не знает — доли переводятся в точки здесь, по клетке прошлого
    // кадра, ДО растяжки ниже (она вычитает уже решённые поля и отбивки).
    // Поля и отбивки — от строчного размера клетки по письму содержащего
    // блока (`position-area-percents-001`: 5% от 80 в горизонтальном, от 40
    // в `vertical-rl`); размеры — по своей оси (`transform-001`:
    // `width: 100%` = ширина якоря, а не 800 окна).
    if c.position_area.is_some()
        && default_anchor.is_some()
        && let Some((cw, ch)) = CELL_LAST.with(|m| m.borrow().get(&c.anchor_key).copied())
    {
        USED_LAST.with(|u| u.set(true));
        let of = |l: Option<Len>, base: f32| match l {
            Some(Len::Pct(p)) => Some(Len::Px(p * base)),
            other => other,
        };
        let inline = if cb_vertical { ch } else { cw };
        c.width = of(c.width, cw);
        c.min_width = of(c.min_width, cw);
        c.max_width = of(c.max_width, cw);
        c.height = of(c.height, ch);
        c.min_height = of(c.min_height, ch);
        c.max_height = of(c.max_height, ch);
        c.margin.top = of(c.margin.top, inline);
        c.margin.right = of(c.margin.right, inline);
        c.margin.bottom = of(c.margin.bottom, inline);
        c.margin.left = of(c.margin.left, inline);
        c.padding.top = of(c.padding.top, inline);
        c.padding.right = of(c.padding.right, inline);
        c.padding.bottom = of(c.padding.bottom, inline);
        c.padding.left = of(c.padding.left, inline);
    }
    c.width = one(c.width, false);
    c.min_width = one(c.min_width, false);
    c.max_width = one(c.max_width, false);
    c.height = one(c.height, true);
    c.min_height = one(c.min_height, true);
    c.max_height = one(c.max_height, true);
    if c.position_area.is_none() || default_anchor.is_none() {
        return;
    }
    if c.align_self == Some(Align::Stretch) || c.justify_self == Some(Align::Stretch) {
        USED_LAST.with(|u| u.set(true));
    }
    let Some((w, h)) = AREA_LAST.with(|m| m.borrow().get(&c.anchor_key).copied()) else {
        return;
    };
    let pxv = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let block_stretch = c.align_self == Some(Align::Stretch);
    let inline_stretch = c.justify_self == Some(Align::Stretch);
    let (x_stretch, y_stretch) = if cb_vertical {
        (block_stretch, inline_stretch)
    } else {
        (inline_stretch, block_stretch)
    };
    let b = c.borders();
    if x_stretch {
        let extra = pxv(c.margin.left)
            + pxv(c.margin.right)
            + pxv(b.left)
            + pxv(b.right)
            + pxv(c.padding.left)
            + pxv(c.padding.right);
        c.width = Some(Len::Px((w - extra).max(0.0)));
    }
    if y_stretch {
        let extra = pxv(c.margin.top)
            + pxv(c.margin.bottom)
            + pxv(b.top)
            + pxv(b.bottom)
            + pxv(c.padding.top)
            + pxv(c.padding.bottom);
        c.height = Some(Len::Px((h - extra).max(0.0)));
    }
}
