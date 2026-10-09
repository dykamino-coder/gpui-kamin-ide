//! Коробка: поля, отступы, рамки по сторонам, скругления (apply_box).

use crate::style::apply::*;

pub(crate) fn apply_box(mut d: Div, c: &Computed) -> Div {
    contained_intrinsic::apply(&mut d, c);
    // `contain: size`: коробка меряется как пустая — рост от содержимого
    // подменяется `contain-intrinsic-size` (или нулём). Подмена касается
    // размера ПО СОДЕРЖИМОМУ: высота auto считается от содержимого — её и
    // задаём; ширина блока в потоке и так не от содержимого, её не трогаем.
    // Явное `height: auto` — та же высота от содержимого: подмена нужна и
    // ему (`contain-size-replaced-003*` пишут auto буквально).
    // `contain-intrinsic-size` — ВНУТРЕННИЙ размер (css-sizing-4): отступы
    // и рамка прибавляются к нему независимо от `box-sizing`. Раньше
    // ставилась голая величина, и taffy подпирал её суммой отступов —
    // выходило max(ci, pad) вместо ci + pad (`cis-007`, `cis-008`).
    // CSS Containment 2 §3.1: intrinsic keywords also size the box as empty.
    if c.contains_height() && matches!(c.height,
        None | Some(Len::Auto | Len::MinContent | Len::MaxContent | Len::FitContent)) {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.top) + side(c.padding.bottom) + side(b.top) + side(b.bottom);
        let ci = px(c
            .contain_intrinsic
            .1
            .unwrap_or_else(|| empty_contained_size(c, false))
            + pad);
        // `contain-intrinsic-size` — ВНУТРЕННИЙ размер (css-sizing-4
        // §intrinsic-size-override), а не использованный: у растянутого
        // строкой элемента ряда высоту даёт строка (css-flexbox-1 §9.4 п.11).
        // Явная высота глушила растяжку (`contain-intrinsic-size-010/016`:
        // 13 точек вместо 100); нижней гранью подмена держит строку
        // авто-высоты от схлопывания в ноль.
        d = if c.cross_stretched { d.min_h(ci) } else { d.h(ci) };
    }
    // По строчной оси то же самое, но только когда ширина ЯВНО названа
    // размером по содержимому: обычная блочная ширина и так берётся от
    // родителя, а не от содержимого.
    // Ширина от содержимого бывает не только по ключевому слову: строчный
    // контейнер, плавающий и позиционированный ужимаются по нему сами
    // (shrink-to-fit). Под обособлением содержимого у них нет — ширина
    // становится `contain-intrinsic-size`.
    let shrink_to_fit = matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) || c.float.is_some()
        || matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        );
    if c.contains_width()
        && (matches!(
            c.width,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        ) || (shrink_to_fit && matches!(c.width, None | Some(Len::Auto))))
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.left) + side(c.padding.right) + side(b.left) + side(b.right);
        d = d.w(px(c
            .contain_intrinsic
            .0
            .unwrap_or_else(|| empty_contained_size(c, true))
            + pad));
    }
    // Вклад обособленной коробки в измеряющего родителя — тоже
    // `contain-intrinsic-size`: он же перебивает автоминимум элемента ряда
    // или сетки (`min-width: auto` = размер по содержимому, а содержимого
    // здесь нет). При ЯВНОЙ ширине вклад не нужен — она и есть ответ, а
    // подпорка снизу растягивала коробку против написанного.
    if c.contains_width()
        && matches!(c.min_width, None | Some(Len::Auto))
        && matches!(c.width, None | Some(Len::Auto))
        && !shrink_to_fit
    {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = c.borders();
        let pad = side(c.padding.left) + side(c.padding.right) + side(b.left) + side(b.right);
        d = d.min_w(px(c.contain_intrinsic.0.unwrap_or(0.0) + pad));
    }
    d = apply_sides(d, &c.padding, SideKind::Padding);
    d = apply_sides(d, &c.margin, SideKind::Margin);
    d = apply_sides(d, &c.borders(), SideKind::Border);
    d = apply_radius(d, c);
    // `clip-path: circle()` — обрезка содержимого по кругу. Прямоугольная
    // обрезка со скруглением — единственная в конвейере, но для круга и
    // эллипса она точна.
    if let Some(round) = c.clip_round.filter(|_| !crate::render::rounded_rect_clip(c)) {
        let base = match (c.width, c.height) {
            (Some(Len::Px(w)), Some(Len::Px(h))) => w.min(h),
            (Some(Len::Px(w)), _) => w,
            (_, Some(Len::Px(h))) => h,
            _ => 0.0,
        };
        // Доля без известного размера — это «половина стороны», то есть
        // заведомо большое значение: растеризатор обрежет его сам. Раньше
        // 0.5 понималось как полпикселя, и круг выходил квадратом.
        let radius = if round <= 1.0 {
            if base > 0.0 {
                round * base
            } else {
                9999.0 * round
            }
        } else {
            round
        };
        // Обрезка НЕ отменяет собственное скругление: берётся более сильное
        // из двух, иначе `border-radius` рядом с `clip-path` пропадал.
        let own = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        d = d.rounded(px(radius.max(own))).overflow_hidden();
    }
    // `contain: paint` — содержимое не выходит за коробку.
    // `contain: layout` (и `strict`/`content`, которые раскрываются в него):
    // коробка «is treated as having no baseline» (css-contain-2 §3.2 п.7).
    // Родитель — строка, flex, grid — синтезирует её от края коробки. Прежде
    // базовая линия текста внутри уходила наружу: `inline-block` с «a»
    // вставал выше пустого соседа (`contain-layout-baseline-001..003`).
    if c.contain_layout == Some(true) {
        d.style().hides_baseline = Some(true);
    }
    if c.contain_paint == Some(true) {
        // Обрезка по краю БЕЗ контейнера прокрутки (css-contain-2 §3.3
        // paint containment: «contents … clipped to the overflow clip
        // edge»; вычисленный `overflow` остаётся `visible`) — это `clip`, а
        // не `hidden`. `hidden` в taffy — контейнер прокрутки, а у него
        // базовой линии нет (`compute/block.rs` `hides_baseline`), и
        // `inline-block` с `contain: paint` садился на строку низом полей
        // (`contain-paint-independent-formatting-context-002`). Ось с уже
        // заданным `overflow` не трогается.
        let o = &mut d.style().overflow;
        if o.x.is_none_or(|x| x == gpui::Overflow::Visible) {
            o.x = Some(gpui::Overflow::Clip);
        }
        if o.y.is_none_or(|y| y == gpui::Overflow::Visible) {
            o.y = Some(gpui::Overflow::Clip);
        }
    }

    match c.position {
        // Слой окна для `fixed` создаёт сборщик дерева; внутри него элемент
        // размещается так же, как абсолютный.
        Some(Position::Fixed) | Some(Position::Absolute) => {
            d = d.absolute();
            // Абсолютный элемент, у которого задан только один край, не имеет
            // определённой ширины — раскладка сжимает его до самого узкого
            // содержимого, и текст встаёт столбиком по букве. В браузере такой
            // элемент занимает ширину содержимого без переносов; повторяем это.
            // Явный `auto` краем не считается (CSS 2.1 §9.3.2).
            let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
            let horizontal = edge(c.inset.left) && edge(c.inset.right);
            if !horizontal && c.width.is_none() {
                d = d.flex_shrink_0().whitespace_nowrap();
            }
        }
        Some(Position::Relative) => d = d.relative(),
        // Липкий остаётся в потоке: край для него — порог прилипания, а не
        // сдвиг, поэтому вставки ниже к нему не применяются.
        Some(Position::Sticky) => return d.relative(),
        // `static` в GPUI недостижим: элемент всегда участвует в потоке
        // относительно родителя, что соответствует `relative`.
        _ => {}
    }
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
    if !shaped && (c.overflow_x == Some(Overflow::Hidden) || c.overflow_x == Some(Overflow::Scroll)) {
        d = d.overflow_x_hidden();
    }
    if !shaped && (c.overflow_y == Some(Overflow::Hidden) || c.overflow_y == Some(Overflow::Scroll)) {
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
    let scroller = matches!(c.overflow_x, Some(Overflow::Hidden) | Some(Overflow::Scroll))
        || matches!(c.overflow_y, Some(Overflow::Hidden) | Some(Overflow::Scroll));
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
            let (mut dx, mut dy) = c.translate.map_or((0.0, 0.0), |(x, y)| (px_of(x), px_of(y)));
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

/// Наружные отступы отдельно от остального стиля.
///
/// Нужно ленте прокрутки: её видимая область — это коробка БЕЗ наружных
/// отступов, и когда отступ оставался на прокручиваемом узле, лента считала
/// его своей высотой и показывала лишнее.
pub fn margins(d: Div, s: &Sides) -> Div {
    apply_sides(d, s, SideKind::Margin)
}

pub(crate) enum SideKind {
    Padding,
    Margin,
    Border,
}

pub(crate) fn apply_sides(mut d: Div, s: &Sides, kind: SideKind) -> Div {
    for (val, side) in [(s.top, 0u8), (s.right, 1), (s.bottom, 2), (s.left, 3)] {
        let Some(l) = val else { continue };
        // `margin: auto` — это центрирование блока, а не «нет значения».
        // Отступы и рамки с `auto` смысла не имеют, их пропускаем.
        if l == Len::Auto {
            if matches!(kind, SideKind::Margin) {
                d = match side {
                    0 => d.mt(gpui::Length::Auto),
                    1 => d.mr(gpui::Length::Auto),
                    2 => d.mb(gpui::Length::Auto),
                    _ => d.ml(gpui::Length::Auto),
                };
            }
            continue;
        }
        let g = len_to_gpui(l);
        d = match (&kind, side) {
            (SideKind::Padding, 0) => d.pt(g),
            (SideKind::Padding, 1) => d.pr(g),
            (SideKind::Padding, 2) => d.pb(g),
            (SideKind::Padding, _) => d.pl(g),
            (SideKind::Margin, 0) => d.mt(g),
            (SideKind::Margin, 1) => d.mr(g),
            (SideKind::Margin, 2) => d.mb(g),
            (SideKind::Margin, _) => d.ml(g),
            // Толщина рамки в GPUI задаётся только абсолютной длиной.
            (SideKind::Border, side) => match (l, side) {
                (Len::Px(v), 0) => d.border_t_1().border_t(px(v)),
                (Len::Px(v), 1) => d.border_r_1().border_r(px(v)),
                (Len::Px(v), 2) => d.border_b_1().border_b(px(v)),
                (Len::Px(v), _) => d.border_l_1().border_l(px(v)),
                _ => d,
            },
        };
    }
    d
}

/// Скругление углов.
///
/// Доля считается от размера коробки: `border-radius: 50%` — это круглый
/// аватар, самая частая запись после пикселей. GPUI принимает только
/// абсолютную длину, поэтому долю разрешаем сами по заданному размеру, а без
/// него берём заведомо большое значение — растеризатор обрежет его половиной
/// меньшей стороны, что и даёт круг.
/// Радиус угла в точках: доля — от BORDER-BOX (css-backgrounds-3 §5.1), а
/// `c.width` — содержимое, поэтому отбивки и рамка прибавляются. Проба
/// `probe-bg-radiuspct` (`width:20; padding:20; border:20;
/// border-radius:100% 0 0 0`) давала радиус 20 вместо 100. Без заданного
/// размера берётся заведомо большое значение — растеризатор обрежет его
/// половиной меньшей стороны, что и даёт круг.
pub(crate) fn radius_px(c: &Computed, l: Option<Len>) -> Option<f32> {
    let px_len = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    let extra_w = px_len(c.padding.left) + px_len(c.padding.right) + px_len(b.left) + px_len(b.right);
    let extra_h = px_len(c.padding.top) + px_len(c.padding.bottom) + px_len(b.top) + px_len(b.bottom);
    let base = match (c.width, c.height) {
        (Some(Len::Px(w)), Some(Len::Px(h))) => (w + extra_w).min(h + extra_h),
        (Some(Len::Px(w)), _) => w + extra_w,
        (_, Some(Len::Px(h))) => h + extra_h,
        _ => f32::NAN,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Pct(p) if base.is_nan() => Some(9999.0 * p.min(1.0)),
        Len::Pct(p) => Some(base * p),
        // Шрифтовые единицы — от запасного кегля, единой точкой.
        l => crate::metrics::fallback_len_px(l, "", 16.0),
    }
}

pub(crate) fn apply_radius(mut d: Div, c: &Computed) -> Div {
    // Эллиптические углы и большой неоднородный радиус режет альфа-маска
    // буфера группы; круглое скругление сверху обрезало бы форму вторым
    // лезвием (см. `Computed::radius_masked`).
    // `border-shape` не совместим с `border-radius`: радиус — «as if it was
    // set to 0» (css-borders-4 §border-shape-radius-interaction).
    if c.radius_masked() || c.border_shape.is_some() {
        return d;
    }
    let r = &c.radius;
    let resolve = |l: Option<Len>| radius_px(c, l);
    for (val, corner) in [(r.tl, 0u8), (r.tr, 1), (r.br, 2), (r.bl, 3)] {
        let Some(v) = resolve(val) else { continue };
        d = match corner {
            0 => d.rounded_tl(px(v)),
            1 => d.rounded_tr(px(v)),
            2 => d.rounded_br(px(v)),
            _ => d.rounded_bl(px(v)),
        };
    }
    d
}
