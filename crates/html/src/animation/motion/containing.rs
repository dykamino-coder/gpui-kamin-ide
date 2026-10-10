//! Обход содержащих коробок и геометрия motion path.

use super::{Cb, apply_offset_transform};
use crate::dom::Node;
use crate::style::computed::{Computed, Position};
use crate::style::values::value::Len;

/// `parent` — геометрия родителя и признак «родитель сам устанавливает
/// содержащий блок для абсолютов» (нужен, чтобы не приписать абсолюту чужую
/// опорную коробку).
pub(super) fn walk(nodes: &mut [Node], parent: Option<(Cb, bool)>) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        // Геометрия снимается ДО правки стиля: `transform` на размеры коробки
        // не влияет, но порядок так честнее читается.
        let mine = cb_of(&e.style).map(|g| (g, crate::text::inline::establishes_cb(&e.style)));
        if super::motion_style(&e.style).offset_path.is_some() {
            let cb = parent
                .filter(|(_, est)| cb_is_parent(&e.style, *est))
                .map(|(g, _)| with_self(g, &e.style));
            apply_offset_transform(&mut e.style, cb.as_ref());
        }
        walk(&mut e.children, mine);
    }
}

/// Содержащий блок коробки — её родитель? У `static`/`relative`/`sticky` — да
/// (§10.1 п.2: ближайший блочный предок). У `absolute` — только если родитель
/// сам устанавливает содержащий блок (§10.1 п.4), у `fixed` — никогда (окно).
/// Во всех прочих случаях геометрию содержащего блока этот проход не знает и
/// работает без неё.
pub(super) fn cb_is_parent(c: &Computed, parent_establishes: bool) -> bool {
    match c.position {
        Some(Position::Absolute) => parent_establishes,
        Some(Position::Fixed) => false,
        _ => true,
    }
}

/// Опорные коробки элемента как СОДЕРЖАЩЕГО БЛОКА.
///
/// Размер выводится только из ЗАПИСАННОЙ пиксельной длины: доля и `auto`
/// требуют раскладки, а её на этом шаге нет. Без размера возвращается `None`,
/// и путь строится ровно теми ветками, которым содержащий блок не нужен.
pub(super) fn cb_of(p: &Computed) -> Option<Cb> {
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let (w, h) = match (p.width, p.height) {
        (Some(Len::Px(w)), Some(Len::Px(h))) => (w, h),
        _ => return None,
    };
    let b = p.borders();
    let (bt, br, bb, bl) = (px(b.top), px(b.right), px(b.bottom), px(b.left));
    let (pt, pr, pb, pl) = (
        px(p.padding.top),
        px(p.padding.right),
        px(p.padding.bottom),
        px(p.padding.left),
    );
    let (mt, mr, mb, ml) = (
        px(p.margin.top),
        px(p.margin.right),
        px(p.margin.bottom),
        px(p.margin.left),
    );
    // `box-sizing: border-box` — записанная длина уже включает рамку и поля
    // (css-sizing-3 §3.1).
    let (cw, ch) = if p.border_box == Some(true) {
        (
            (w - bl - br - pl - pr).max(0.0),
            (h - bt - bb - pt - pb).max(0.0),
        )
    } else {
        (w, h)
    };
    let (bw, bh) = (cw + pl + pr + bl + br, ch + pt + pb + bt + bb);
    // Доля радиуса — от СВОЕЙ оси (css-backgrounds-3 §5.5).
    let r = |l: Option<Len>, base: f32| match l {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => k * base,
        _ => 0.0,
    };
    let rad = [
        (r(p.radius.tl, bw), r(p.radius.tl, bh)),
        (r(p.radius.tr, bw), r(p.radius.tr, bh)),
        (r(p.radius.br, bw), r(p.radius.br, bh)),
        (r(p.radius.bl, bw), r(p.radius.bl, bh)),
    ];
    Some(Cb {
        boxes: [
            (-ml, -mt, bw + ml + mr, bh + mt + mb),
            (0.0, 0.0, bw, bh),
            (bl, bt, bw - bl - br, bh - bt - bb),
            (bl + pl, bt + pt, cw, ch),
        ],
        radius: rad,
        self_off: (bl + pl, bt + pt),
        self_size: (0.0, 0.0),
    })
}

/// Дописать в геометрию место и размер САМОЙ коробки.
///
/// Статическую позицию в потоке этот проход не знает — её даёт раскладка — и
/// берёт нулевой: коробка стоит в начале своей опорной области. Для всего
/// набора `css/motion` это верно, кроме `offset-path-ray-016`, где перед целью
/// стоит ещё один блок; там результат так и останется неверным, пока место в
/// потоке не будет прокинуто с раскладки.
pub(super) fn with_self(mut g: Cb, c: &Computed) -> Cb {
    let px = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let abs = matches!(c.position, Some(Position::Absolute));
    // Вставки абсолюта считаются от PADDING-коробки блока (§10.1 п.4), сдвиг
    // относительного — от статической позиции в content-коробке (§9.4.3).
    let base = if abs { g.boxes[2] } else { g.boxes[3] };
    let len = |l: Option<Len>, size: f32| match l {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => k * size,
        _ => 0.0,
    };
    g.self_off = (
        base.0 + len(c.inset.left, base.2),
        base.1 + len(c.inset.top, base.3),
    );
    let b = c.borders();
    g.self_size = if c.border_box == Some(true) {
        (px(c.width), px(c.height))
    } else {
        (
            px(c.width) + px(c.padding.left) + px(c.padding.right) + px(b.left) + px(b.right),
            px(c.height) + px(c.padding.top) + px(c.padding.bottom) + px(b.top) + px(b.bottom),
        )
    };
    g
}
