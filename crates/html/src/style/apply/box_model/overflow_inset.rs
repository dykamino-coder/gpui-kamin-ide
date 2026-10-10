//! apply_box, хвостовые этапы: обрезка overflow, полосы прокрутки, ранний выход для непозиционированных; края inset, избыточные края и translate.

use super::*;

fn box_inset(mut d: Div, c: &Computed) -> Div {
    // §9.4.3: у относительно сдвинутой коробки с ОБОИМИ горизонтальными
    // краями один из них избыточен — «if neither is auto, one of them must be
    // ignored: for direction ltr `right`, for rtl `left`». Раскладка под нами
    // всегда берёт начальный край, поэтому при rtl левый край снимается здесь.
    let set = |l: Option<Len>| matches!(l, Some(x) if x != Len::Auto);
    // У АБСОЛЮТНОЙ коробки то же правило §10.3.7: избыточен один из краёв,
    // и при rtl это левый. Но избыток возникает, только когда заданы все три
    // величины — при `width: auto` края решают ширину, и отбрасывать нечего.
    let over = match c.position {
        Some(Position::Relative) => true,
        Some(Position::Absolute) | Some(Position::Fixed) => set(c.width),
        _ => false,
    };
    // В ВЕРТИКАЛЬНОМ содержащем блоке оси меняются ролями
    // (css-writing-modes-4 §7.1: правила §10.3 горизонтали действуют по
    // вертикали). Горизонталь — БЛОЧНАЯ ось: отбрасывается её конец (§10.6.4
    // «ignore the value for 'bottom'»), а он у `vertical-rl` слева, у
    // `vertical-lr` справа, и от `direction` не зависит. Вертикаль — СТРОЧНАЯ:
    // по §10.3.7 при `rtl` отбрасывается её конец, то есть `top` (у
    // `sideways-lr` строка идёт снизу вверх, и конец — верх уже при `ltr`).
    // Прежнее правило по `cb_rtl` в вертикали угадывало только `vrl`+`rtl`:
    // `abs-pos-non-replaced-vrl-214/220` (ltr) и `vlr-223/227/229` (rtl)
    // уезжали на 80, `vlr-093/097`, `vrl-092/096` (rtl, `top`+`bottom`+
    // `height`) — тоже. Ровно так и в корпусе: `dynamic-offset-vrl-002`
    // (`left … /* ignored */`), `dynamic-offset-vrl-rtl-002` (`top …
    // /* ignored */`).
    // Относительный сдвиг решает переопределённую ось по тем же сторонам
    // (§9.4.3 в логических осях, Blink `relative_utils.cc:71-88`):
    // `overconstrained-rel-pos-*-vrl-*`, `-rtl-*-vlr-*`.
    let vertical_cb = matches!(
        c.position,
        Some(Position::Absolute) | Some(Position::Fixed) | Some(Position::Relative)
    ) && c.cb_vertical;
    let drop_left = over
        && set(c.inset.left)
        && set(c.inset.right)
        && if vertical_cb {
            c.cb_vertical_rl
        } else {
            c.cb_rtl
        };
    // У относительного сдвига переопределение от размера не зависит (`over`
    // для него всегда истинно) — только у абсолюта нужен `height`.
    let drop_top = vertical_cb
        && (c.position == Some(Position::Relative) || set(c.height))
        && set(c.inset.top)
        && set(c.inset.bottom)
        && c.cb_rtl != (c.cb_sideways && !c.cb_vertical_rl);
    for (val, f) in [
        (if drop_top { None } else { c.inset.top }, 0u8),
        (c.inset.right, 1),
        (c.inset.bottom, 2),
        (if drop_left { None } else { c.inset.left }, 3),
    ] {
        let Some(l) = val else { continue };
        if l == Len::Auto {
            continue;
        }
        // Доля края по оси БЛОКА у относительно сдвинутой коробки считается
        // от высоты содержащего блока, а когда та не задана — край
        // вычисляется в `auto`, то есть в ноль (CSS2 §9.3.2: «If the height
        // of the containing block is not specified explicitly … the value
        // computes to auto»; Blink `relative_utils.cc::ResolveInset` отдаёт
        // `nullopt` при неопределённом размере).
        if inset_percent::is_auto(c, l, f) {
            continue;
        }
        let g = len_to_gpui(l);
        d = match f {
            0 => d.top(g),
            1 => d.right(g),
            2 => d.bottom(g),
            _ => d.left(g),
        };
    }

    // `translate` двигает элемент ВИЗУАЛЬНО, не трогая раскладку. Считается
    // ПОСЛЕ краёв и складывается с ними: раньше цикл краёв затирал сдвиг, и
    // у абсолютного элемента с `left` он пропадал.
    if let Some((x, y)) = c.translate {
        let shift = |base: Option<Len>, delta: Len| -> Option<Len> {
            match (base, delta) {
                (Some(Len::Px(b)), Len::Px(v)) => Some(Len::Px(b + v)),
                (None, Len::Px(v)) if v != 0.0 => Some(Len::Px(v)),
                (base, _) => base,
            }
        };
        if c.position.is_none() {
            d = d.relative();
        }
        if let Some(l) = shift(c.inset.left, x) {
            d = d.left(len_to_gpui(l));
        }
        if let Some(t) = shift(c.inset.top, y) {
            d = d.top(len_to_gpui(t));
        }
    }

    d
}

pub(super) fn box_overflow(mut d: Div, c: &Computed) -> Div {
    // Обрезка — ДО разбора краёв: ниже стоит ранний выход для непозиционированных,
    // и всё, что после него, у обычного блока не выполнялось вовсе. Из-за этого
    // `overflow: hidden` не обрезал ничего (проверено пробой: коробка с ним и
    // без него рисовались одинаково).
    //
    // Прокрутка: в GPUI скролл требует своего состояния и обработчика, поэтому
    // на уровне стиля выражается только обрезка. Прокручиваемый контейнер
    // собирается вызывающим (см. доку, раздел «Прокрутка»).
    // Обрезка с ПОЛЕМ снимается с коробки: раскладка режет ровно по её краю,
    // а поле требует резать дальше наружу. Маску ставит свой слой
    // (`interact::ClipMargin`), его заводит сборщик дерева.
    // `border-shape`: переполнение режет ВНУТРЕННИЙ контур фигуры
    // (css-borders-4 §border-shape-overflow-interaction) — маска группы
    // (`render::grouped`), а не прямоугольник padding-box: тот срезал бы и
    // кольцо рамки (абсолютный слой с выносом), и углы фигуры шире него
    // (border-shape-overflow-child-clip, -replaced-img/-iframe).
    let shaped = c.border_shape.is_some();
    if !shaped && (c.overflow_x == Some(Overflow::Hidden) || c.overflow_x == Some(Overflow::Scroll))
    {
        d = d.overflow_x_hidden();
    }
    if !shaped && (c.overflow_y == Some(Overflow::Hidden) || c.overflow_y == Some(Overflow::Scroll))
    {
        d = d.overflow_y_hidden();
    }
    // `clip` режет краску, но НЕ создаёт скролл-контейнер: авто-минимум
    // flex/grid-элемента остаётся по содержимому — у gpui/taffy для этого
    // отдельный вариант (CSSWG #7714; min-size-auto-overflow-clip).
    if !shaped && c.overflow_x == Some(Overflow::Clip) {
        d.style().overflow.x = Some(gpui::Overflow::Clip);
    }
    if !shaped && c.overflow_y == Some(Overflow::Clip) {
        d.style().overflow.y = Some(gpui::Overflow::Clip);
    }
    // Поле обрезки: край отодвигается от коробки отсчёта (css-overflow-3 §5).
    // Сдвиг несёт РОДНАЯ маска (патч GPUI): рамка и фон самой коробки
    // рисуются вне маски, режется только содержимое — обёртка снаружи резала
    // и рамку.
    // У СКРОЛЛЕРА (`hidden`/`scroll`) коробка `border-box` игнорируется
    // ВМЕСТЕ со сдвигом (css-overflow-3 §overflow-clip-margin; названия
    // `overflow-clip-margin-021/022`: «border-box is ignored on a scroller,
    // including the offset»): край остаётся на padding-box. `content-box` и
    // отрицательный сдвиг у скроллера действуют — `-018/-019/-020` зелёные.
    let scroller = matches!(
        c.overflow_x,
        Some(Overflow::Hidden) | Some(Overflow::Scroll)
    ) || matches!(
        c.overflow_y,
        Some(Overflow::Hidden) | Some(Overflow::Scroll)
    );
    if let Some(m) = c
        .clip_margin
        .filter(|_| !(scroller && c.clip_margin_box == Some(2)))
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let border = c.borders();
        let b = [
            side(border.top),
            side(border.right),
            side(border.bottom),
            side(border.left),
        ];
        let pd = [
            side(c.padding.top),
            side(c.padding.right),
            side(c.padding.bottom),
            side(c.padding.left),
        ];
        let arr = match c.clip_margin_box {
            Some(2) => [b[0] + m, b[1] + m, b[2] + m, b[3] + m],
            Some(0) => [m - pd[0], m - pd[1], m - pd[2], m - pd[3]],
            _ => [m; 4],
        };
        d.style().overflow_clip_offset = Some(arr);
    }
    // Края двигают только позиционированный элемент. У обычного (`static`)
    // браузер их игнорирует, а мы сдвигали — блок с `top` в потоке уезжал.
    // `translate` (css-transforms-2 §individual-transforms) действует и на
    // СТАТИКЕ: он визуальный, как `transform`, и от `position` не зависит.
    // Ранний выход стоял ПЕРЕД блоком сдвига, поэтому ветка
    // `if c.position.is_none() { d.relative() }` была недостижима.
    if !matches!(
        c.position,
        Some(Position::Relative) | Some(Position::Absolute) | Some(Position::Fixed)
    ) {
        // Чистый px-сдвиг `transform` складывается сюда же
        // (`Computed::folded_shift`); матрицу обёртки он тогда не трогает.
        let folded = c.folded_shift();
        if c.translate.is_some() || folded.is_some() {
            let px_of = |l: Len| match l {
                Len::Px(v) => v,
                _ => 0.0,
            };
            let (mut dx, mut dy) = c
                .translate
                .map_or((0.0, 0.0), |(x, y)| (px_of(x), px_of(y)));
            if let Some((fx, fy)) = folded {
                dx += fx;
                dy += fy;
            }
            if dx != 0.0 || dy != 0.0 {
                d = d.relative();
                if dx != 0.0 {
                    d = d.left(gpui::px(dx));
                }
                if dy != 0.0 {
                    d = d.top(gpui::px(dy));
                }
            }
        }
        return d;
    }
    box_inset(d, c)
}
