//! ★ ДОЛГ (08.09, v158, замер второго прохода `settle`): полный свод +45 в
//! `css/motion`, но три пары ушли в минус и по бисекту (v163: в дереве только
//! этот патч) принадлежат ему: `anchor-position-inline-005` 0.05 → 0.67,
//! `-006` 0.08 → 1.00, `disclosure-styles` 0.19 → 0.62. Ни у одной нет
//! `offset-*` — задет либо снятый вызов из разбора стиля (`dom.rs: walk`),
//! либо порядок `settle` после `anchor::settle_static`. Разобрать отдельно.
//! `offset-path`/`offset-distance`/`offset-rotate`/`offset-anchor`/
//! `offset-position` (motion-1).
//!
//! Offset-трансформ по §«Calculating The Offset Transform» — это сдвиг,
//! совмещающий ТОЧКУ ПРИВЯЗКИ коробки (`offset-anchor`) с ТОЧКОЙ ПУТИ
//! (`offset-path` + `offset-distance`), и поворот (`offset-rotate`).
//! Отдельного конвейера под него нет: строка `transform` синтезируется и
//! уходит в тот же разборщик (`Computed::apply_one`), а матрицы складываются
//! в порядке слоения — сначала offset, поверх него авторский `transform`.
//!
//! В записи `transform`, где точка отсчёта — `transform-origin` `O`, а точка
//! привязки — `A`, это ровно
//!
//! ```text
//! translate(P) translate(-O) rotate(angle) translate(O - A)
//! ```
//!
//! `P` — точка пути В СИСТЕМЕ САМОЙ КОРОБКИ. При `offset-anchor: auto`
//! (начальное) `A == O`, хвост нулевой и не пишется вовсе.
//!
//! Считается это ВТОРЫМ ПРОХОДОМ по собранному дереву (`settle`), а не на
//! разборе стиля: §offset-path говорит «In CSS contexts, the boxes being
//! referenced are from the element that establishes the containing block for
//! this element» — опорную коробку `<coord-box>`, длину `ray()` (`<ray-size>`)
//! и начало `at <position>` даёт СОДЕРЖАЩИЙ БЛОК, которого на разборе стиля
//! ещё нет. Где содержащий блок статически не выводится (доля или `auto` у
//! его ширины/высоты; начальный содержащий блок — его размер знает только
//! отрисовка), проход отдаёт `None`, и работают ровно те ветки, что работали
//! раньше: `path()` и `ray()` с пиксельным `offset-distance`.

use crate::style::computed::{Computed, Position};
use crate::dom::Node;
use crate::style::values::value::Len;

mod ellipse_path;

/// Ломаная контура: точки в системе координат коробки и признак замыкания.
struct Poly {
    pts: Vec<(f32, f32)>,
    closed: bool,
}

/// Опорные коробки СОДЕРЖАЩЕГО БЛОКА в его собственной системе (начало —
/// левый верхний угол его border-коробки) плюс место коробки элемента в ней.
/// Всё в css-точках.
#[derive(Clone, Copy, Debug, Default)]
pub struct Cb {
    /// `<coord-box>` по номеру: 0 margin, 1 border, 2 padding, 3 content —
    /// каждая как (x, y, w, h).
    pub boxes: [(f32, f32, f32, f32); 4],
    /// Радиусы border-коробки содержащего блока (tl, tr, br, bl),
    /// эллиптические. Нужны голому `<coord-box>`: §offset-path «If
    /// `<offset-path>` is omitted, it defaults to `inset(0 round X)`, where X
    /// is the value of `border-radius` on the element that establishes the
    /// containing block for this element».
    pub radius: [(f32, f32); 4],
    /// Левый верхний угол border-коробки ЭЛЕМЕНТА в той же системе.
    pub self_off: (f32, f32),
    /// Размер border-коробки элемента — его требует ключ `contain`.
    pub self_size: (f32, f32),
}

/// Второй проход по дереву коробок: каждому элементу с `offset-path`
/// считается offset-трансформ по геометрии его СОДЕРЖАЩЕГО БЛОКА.
pub fn settle(nodes: &mut [Node]) {
    // `offset-path: url(#id)` ссылается на SVG-фигуру ЛЮБОГО места документа —
    // словарь эквивалентных путей собирается до обхода и живёт ровно один
    // проход (вложенный документ `<iframe>` зовёт `settle` со своим деревом).
    let mut shapes = std::collections::HashMap::new();
    collect_shapes(nodes, &mut shapes);
    SVG_SHAPES.with(|m| *m.borrow_mut() = shapes);
    walk(nodes, None);
    SVG_SHAPES.with(|m| m.borrow_mut().clear());
}

thread_local! {
    /// `id` → эквивалентный путь (SVG 2 §shapes, «equivalent path») фигур
    /// документа — для `offset-path: url(#id)`.
    static SVG_SHAPES: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Собрать фигуры с `id`. По `getElementById` побеждает ПЕРВАЯ.
fn collect_shapes(nodes: &[Node], out: &mut std::collections::HashMap<String, String>) {
    for n in nodes {
        let Node::Element(e) = n else { continue };
        if let Some(d) = shape_d(e)
            && let Some(id) = e.attr("id")
        {
            out.entry(id.to_string()).or_insert(d);
        }
        collect_shapes(&e.children, out);
    }
}

/// Эквивалентный путь SVG-фигуры в её пользовательских единицах (SVG 2
/// §9.x «equivalent path»): прямоугольник — от левого верхнего угла по
/// часовой, круг и эллипс — от самой правой точки по часовой, линия и
/// ломаная — от первой точки. `None` — не фигура (тогда `url()` ведёт себя
/// как `path("m 0 0")`, motion-1 §offset-path: `offset-path-url-011`).
fn shape_d(e: &crate::dom::Element) -> Option<String> {
    let n = |k: &str| {
        e.attr(k)
            .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            .unwrap_or(0.0)
    };
    let arcs = |cx: f32, cy: f32, rx: f32, ry: f32| {
        format!(
            "M{} {cy} A{rx} {ry} 0 0 1 {cx} {} A{rx} {ry} 0 0 1 {} {cy} A{rx} {ry} 0 0 1 {cx} {} A{rx} {ry} 0 0 1 {} {cy} Z",
            cx + rx,
            cy + ry,
            cx - rx,
            cy - ry,
            cx + rx
        )
    };
    let pts = |closed: bool| -> Option<String> {
        let nums: Vec<f32> = e
            .attr("points")?
            .split(|ch: char| ch == ',' || ch.is_whitespace())
            .filter_map(|t| t.parse().ok())
            .collect();
        let mut d = String::new();
        for (i, p) in nums.chunks_exact(2).enumerate() {
            d.push_str(&format!("{}{} {} ", if i == 0 { 'M' } else { 'L' }, p[0], p[1]));
        }
        if closed && !d.is_empty() {
            d.push('Z');
        }
        (!d.is_empty()).then_some(d)
    };
    Some(match e.tag.as_str() {
        "path" => e.attr("d")?.to_string(),
        "rect" => {
            let (x, y) = (n("x"), n("y"));
            format!("M{x} {y} H{} V{} H{x} Z", x + n("width"), y + n("height"))
        }
        "circle" => {
            let r = n("r");
            arcs(n("cx"), n("cy"), r, r)
        }
        "ellipse" => arcs(n("cx"), n("cy"), n("rx"), n("ry")),
        "line" => format!("M{} {} L{} {}", n("x1"), n("y1"), n("x2"), n("y2")),
        "polyline" => pts(false)?,
        "polygon" => pts(true)?,
        _ => return None,
    })
}

/// `parent` — геометрия родителя и признак «родитель сам устанавливает
/// содержащий блок для абсолютов» (нужен, чтобы не приписать абсолюту чужую
/// опорную коробку).
fn walk(nodes: &mut [Node], parent: Option<(Cb, bool)>) {
    for n in nodes.iter_mut() {
        let Node::Element(e) = n else { continue };
        // Геометрия снимается ДО правки стиля: `transform` на размеры коробки
        // не влияет, но порядок так честнее читается.
        let mine = cb_of(&e.style).map(|g| (g, crate::text::inline::establishes_cb(&e.style)));
        if e.style.offset_path.is_some() {
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
fn cb_is_parent(c: &Computed, parent_establishes: bool) -> bool {
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
fn cb_of(p: &Computed) -> Option<Cb> {
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
fn with_self(mut g: Cb, c: &Computed) -> Cb {
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

/// Приставить offset-трансформ к авторскому.
pub fn apply_offset_transform(c: &mut Computed, cb: Option<&Cb>) {
    let Some(css) = offset_transform_css(c, cb) else {
        return;
    };
    let author = c.transform.take();
    c.apply_one("transform", &css);
    if let (Some(off), Some(a)) = (c.transform, author) {
        c.transform = Some(off.then(&a));
    }
}

fn offset_transform_css(c: &Computed, cb: Option<&Cb>) -> Option<String> {
    let raw = c.offset_path.as_deref()?.trim();
    let (func, kind) = split_coord_box(raw);
    // `path()` — единственная запись в СВОЕЙ системе координат: опорной
    // коробки у неё нет, начало — левый верхний угол самой коробки. Это не
    // домысел: `offset-distance-001` даёт `path('m 0 0 h 200 v 150 z')` и
    // `offset-distance: 20%`, а эталон — `translateX(120px)`, то есть отсчёт
    // от места ЭЛЕМЕНТА (коробка стоит на (8,8) от начального блока).
    if let Some(inner) = func.strip_prefix("path(") {
        let d = inner
            .trim_end_matches(')')
            .trim()
            .trim_matches('\'')
            .trim_matches('"')
            .to_string();
        return path_css(c, &d, (0.0, 0.0));
    }
    if let Some(args) = func.strip_prefix("ray(") {
        return ray_css(c, args.trim_end_matches(')'), cb, cb.map(|g| g.boxes[kind]));
    }
    // `url(#id)` (motion-1 §offset-path): путь — эквивалентный путь SVG-фигуры,
    // а «The <coord-box> defines the viewport and user coordinate system for
    // the shape element, with the origin … at the top left corner, and units
    // being 1px in size» — то есть та же опорная коробка содержащего блока,
    // что у `<basic-shape>`. Не фигура (или нет такого `id`) — `path("m 0 0")`
    // в своей системе (`offset-path-url-011`). Отрезается всё до `#`: разбор
    // значения мог дописать к ссылке адрес документа.
    if let Some(inner) = func.strip_prefix("url(") {
        let raw = inner
            .trim_end_matches(')')
            .trim()
            .trim_matches(|q: char| q == '"' || q == '\'');
        let id = raw.rsplit_once('#').map_or(raw, |(_, t)| t);
        let Some(d) = SVG_SHAPES.with(|m| m.borrow().get(id).cloned()) else {
            return path_css(c, "m 0 0", (0.0, 0.0));
        };
        let g = cb?;
        let rb = g.boxes[kind];
        return path_css(c, &d, (rb.0 - g.self_off.0, rb.1 - g.self_off.1));
    }
    // Всё прочее — `<basic-shape>` или голый `<coord-box>`: и то и другое
    // живёт в опорной коробке содержащего блока, без неё строить нечего.
    let g = cb?;
    let rb = g.boxes[kind];
    let shift = (rb.0 - g.self_off.0, rb.1 - g.self_off.1);
    let shape = shape_start(&func, c, g, rb);
    if shape.starts_with("circle(") || shape.starts_with("ellipse(") {
        return ellipse_path::css(c, &shape, rb.2, rb.3, shift);
    }
    let d = if func.is_empty() {
        // Голый `<coord-box>` = `inset(0 round X)` (§offset-path); слово
        // передаём любое — `rrect_of` берёт из него только размер и радиусы.
        crate::paint::background::motion_shape_d("border-box", rb.2, rb.3, g.radius)?
    } else {
        crate::paint::background::motion_shape_d(&shape, rb.2, rb.3, g.radius)?
    };
    // Путь строится в системе ОПОРНОЙ коробки, а `transform` живёт в системе
    // самой коробки: сдвигаем на разницу их начал.
    path_css(c, &d, shift)
}

/// Разделить `<offset-path> || <coord-box>` (§offset-path «Value: none |
/// `<offset-path>` || `<coord-box>`»): слово-коробка может стоять и до, и
/// после функции. Возвращает саму функцию (может быть пустой — это голый
/// `<coord-box>`) и номер коробки; умолчание — `border-box`.
fn split_coord_box(raw: &str) -> (String, usize) {
    // css-box-4 §coord-box: у коробок CSS `fill-box` ведёт себя как
    // content-box, `stroke-box` и `view-box` — как border-box.
    const BOXES: [(&str, usize); 7] = [
        ("margin-box", 0),
        ("border-box", 1),
        ("padding-box", 2),
        ("content-box", 3),
        ("fill-box", 3),
        ("stroke-box", 1),
        ("view-box", 1),
    ];
    let mut s = raw.trim().to_string();
    let mut kind = 1usize;
    for (word, k) in BOXES {
        let Some(at) = s.find(word) else { continue };
        // Слово ищем только ВНЕ скобок: внутри `inset(… round …)` его быть не
        // может, а `ray(…) content-box` — снаружи.
        if s[..at].matches('(').count() != s[..at].matches(')').count() {
            continue;
        }
        kind = k;
        s.replace_range(at..at + word.len(), "");
        break;
    }
    (s.trim().to_string(), kind)
}

/// Эквивалентный путь `<basic-shape>` в системе опорной коробки w×h
/// (§«Equivalent Paths For `<basic-shape>`»).
///
/// Круг и эллипс: «starts at the rightmost point … four circular arcs, each
/// comprising a quarter of the circle/ellipse, proceeding clockwise». Именно
/// поэтому здесь не годится `border_shape_path`: он пишет контур от ЛЕВОЙ
/// точки и против часовой — точка на 25 % оказалась бы в зеркальном месте.
/// Прямоугольники и многоугольник отдаёт `motion_shape_d`: `rrect_d` уже
/// начинает с левого конца верхней стороны и идёт по часовой.
/// Пропущенный `at <position>` у круга и эллипса: «if they accept an `at
/// <position>` argument but that argument is omitted, and the element defines
/// an offset starting position via 'offset-position', it uses the specified
/// offset starting position for that argument» (motion-1 §offset-path).
/// Начало даёт `start_of` в системе содержащего блока — в запись оно уходит
/// в системе опорной коробки (`offset-path-shape-circle-002`, `-ellipse-002`).
fn shape_start(func: &str, c: &Computed, g: &Cb, rb: (f32, f32, f32, f32)) -> String {
    let pos = c.offset_position.as_deref().map(str::trim).unwrap_or("normal");
    let round = func.starts_with("circle(") || func.starts_with("ellipse(");
    let Some(open) = func.find('(') else {
        return func.to_string();
    };
    let body = func[open + 1..].trim_end_matches(')').trim();
    if !round || pos == "normal" || body.split_whitespace().any(|t| t == "at") {
        return func.to_string();
    }
    let s = start_of(c, Some(g), Some(rb));
    let (x, y) = (s.0 - rb.0, s.1 - rb.1);
    let head = &func[..open + 1];
    if body.is_empty() {
        format!("{head}at {x}px {y}px)")
    } else {
        format!("{head}{body} at {x}px {y}px)")
    }
}

/// Точка на разложенном пути и готовая строка `transform`. `shift` — перенос
/// из системы пути в систему коробки.
fn path_css(c: &Computed, d: &str, shift: (f32, f32)) -> Option<String> {
    let poly = flatten(d)?;
    let total = length(&poly.pts);
    let want = match c.offset_distance {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(p)) => p * total,
        // Смесь долей и точек: доля — от длины пути (css-values-4 §10.9).
        Some(Len::Calc(i)) => crate::style::values::value::calc_get(i)
            .pct_px()
            .map_or(0.0, |(p, px)| p * total + px),
        _ => 0.0,
    };
    // §path-distance: замкнутый контур — по модулю длины («Modulo here uses
    // the traditional mathematical definition, where the output is always
    // non-negative»), открытый — зажимается нулём и длиной.
    let s = if poly.closed && total > 0.0 {
        want.rem_euclid(total)
    } else {
        want.clamp(0.0, total)
    };
    let (p, ang) = sample(&poly.pts, s);
    Some(origin_shift(
        c,
        (p.0 + shift.0, p.1 + shift.1),
        rotation(c, ang),
    ))
}

/// `offset-rotate: [ auto | reverse ] || <angle>` (§offset-rotate).
///
/// `auto` (начальное) — «the difference between the offset path's direction
/// at the offset position and the direction of the positive X axis»;
/// `reverse` — то же плюс пол-оборота; записанный угол ПРИБАВЛЯЕТСЯ к тому и
/// к другому («If specified with an `<angle>`, the angle is added to the
/// rotation component»), а сам по себе — просто поворот. Прежняя запись
/// прибавку к `reverse` не читала вовсе.
fn rotation(c: &Computed, tangent: f32) -> f32 {
    let v = c.offset_rotate.as_deref().map(str::trim).unwrap_or("auto");
    let (base, rest) = if let Some(r) = v.strip_prefix("auto") {
        (tangent, r)
    } else if let Some(r) = v.strip_prefix("reverse") {
        (tangent + std::f32::consts::PI, r)
    } else {
        (0.0, v)
    };
    base + rest
        .split_whitespace()
        .next()
        .and_then(angle_rad)
        .unwrap_or(0.0)
}

/// `<position>` в коробке `rb` (css-values-4 §position): слова, длины, доли.
/// Одно значение задаёт X, второе — Y; горизонтальное слово всегда идёт в X,
/// как бы ни стояло в записи (`at top left` — это `left top`).
fn position_in(toks: &[&str], rb: (f32, f32, f32, f32)) -> (f32, f32) {
    let axis = |t: &str, base: f32| -> f32 {
        match t {
            "left" | "top" => 0.0,
            "right" | "bottom" => base,
            "center" => base / 2.0,
            _ => match Len::parse(t) {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => k * base,
                _ => base / 2.0,
            },
        }
    };
    let horiz = |t: &str| matches!(t, "left" | "right");
    let vert = |t: &str| matches!(t, "top" | "bottom");
    let (a, b) = match (toks.first(), toks.get(1)) {
        (Some(a), Some(b)) if vert(a) || horiz(b) => (*b, *a),
        (Some(a), Some(b)) => (*a, *b),
        // Одно вертикальное слово — ось Y, X по центру (css-values-4 §position).
        (Some(a), None) if vert(a) => ("center", *a),
        (Some(a), None) => (*a, "center"),
        _ => ("center", "center"),
    };
    (rb.0 + axis(a, rb.2), rb.1 + axis(b, rb.3))
}

/// Начало пути, когда функция своего не задала: `offset-position`
/// (§offset-position). `auto` — «the top-left corner of the box», то есть
/// начало СВОЕЙ системы координат; `<position>` — точка в содержащем блоке;
/// `normal` (начальное) — начала нет, и `ray()` ведёт себя как `at center`
/// (§ray()/at). Без геометрии содержащего блока остаётся только `auto`.
fn start_of(c: &Computed, cb: Option<&Cb>, rb: Option<(f32, f32, f32, f32)>) -> (f32, f32) {
    let v = c
        .offset_position
        .as_deref()
        .map(str::trim)
        .unwrap_or("normal");
    let own = cb.map_or((0.0, 0.0), |g| g.self_off);
    match (v, rb) {
        ("auto", _) => own,
        ("normal", Some(rb)) => (rb.0 + rb.2 / 2.0, rb.1 + rb.3 / 2.0),
        (_, Some(rb)) => position_in(&v.split_whitespace().collect::<Vec<_>>(), rb),
        // Опорной коробки нет: точку в ней не выразить, остаётся своё начало.
        _ => own,
    }
}

/// Сборка строки `translate(P) translate(−O) rotate(A) translate(O − Anchor)`.
///
/// Поворот идёт вокруг точки отсчёта преобразования, поэтому вычитание `O`
/// пишется ОТДЕЛЬНЫМИ звеньями: доля своего размера выражается процентным
/// `translate()`, а `calc()` разборщик `transform` не знает.
/// `offset-anchor: auto` (начальное) совпадает с точкой отсчёта (§offset-anchor:
/// «The anchor point is the same as the transform-origin») — хвостовой сдвиг
/// `O − Anchor` тогда нулевой и не пишется вовсе, и строка для сегодняшних
/// зелёных не меняется ни байтом.
fn origin_shift(c: &Computed, p: (f32, f32), rot: f32) -> String {
    let (ofx, ofy) = c.transform_origin.unwrap_or((0.5, 0.5));
    let (opx, opy) = c.transform_origin_px;
    let mut css = format!("translate({}px, {}px)", p.0, p.1);
    if opx.is_some() || opy.is_some() {
        css.push_str(&format!(
            " translate({}px, {}px)",
            -opx.unwrap_or(0.0),
            -opy.unwrap_or(0.0)
        ));
    }
    if opx.is_none() {
        css.push_str(&format!(" translateX({}%)", -ofx * 100.0));
    }
    if opy.is_none() {
        css.push_str(&format!(" translateY({}%)", -ofy * 100.0));
    }
    css.push_str(&format!(" rotate({}rad)", rot));
    // Хвост пишется В СИСТЕМЕ ПОВЁРНУТОЙ коробки — так же, как это делают
    // эталоны набора (`offset-path-shape-rect-001-ref`:
    // `translate(520px,142px) rotate(90deg) translate(40px,40px)`).
    if let Some(((axp, axc), (ayp, ayc))) = anchor_point(c) {
        let oxp = opx.unwrap_or(0.0);
        let oyp = opy.unwrap_or(0.0);
        let oxc = if opx.is_none() { ofx * 100.0 } else { 0.0 };
        let oyc = if opy.is_none() { ofy * 100.0 } else { 0.0 };
        if oxp != axp || oyp != ayp {
            css.push_str(&format!(" translate({}px, {}px)", oxp - axp, oyp - ayp));
        }
        if oxc != axc {
            css.push_str(&format!(" translateX({}%)", oxc - axc));
        }
        if oyc != ayc {
            css.push_str(&format!(" translateY({}%)", oyc - ayc));
        }
    }
    css
}

/// Точка привязки (`offset-anchor`) как пара «точки, проценты» по каждой оси.
/// `None` — `auto` (начальное) или не задано: привязка равна `transform-origin`.
///
/// Проценты НЕ сворачиваются в пиксели намеренно: §offset-anchor меряет их «to
/// the width and the height of the element's reference box», а собственный
/// размер коробки известен только на отрисовке — там же, где `transform`
/// решает `translateX(%)`.
fn anchor_point(c: &Computed) -> Option<((f32, f32), (f32, f32))> {
    let v = c.offset_anchor.as_deref().map(str::trim)?;
    if v.is_empty() || v == "auto" {
        return None;
    }
    let toks: Vec<&str> = v.split_whitespace().collect();
    let axis = |t: &str| -> (f32, f32) {
        match t {
            "left" | "top" => (0.0, 0.0),
            "right" | "bottom" => (0.0, 100.0),
            "center" => (0.0, 50.0),
            _ => match Len::parse(t) {
                Some(Len::Px(x)) => (x, 0.0),
                Some(Len::Pct(k)) => (0.0, k * 100.0),
                _ => (0.0, 50.0),
            },
        }
    };
    let horiz = |t: &str| matches!(t, "left" | "right");
    let vert = |t: &str| matches!(t, "top" | "bottom");
    let (a, b) = match (toks.first(), toks.get(1)) {
        (Some(a), Some(b)) if vert(a) || horiz(b) => (*b, *a),
        (Some(a), Some(b)) => (*a, *b),
        // `offset-anchor: top` — это `center top` (css-values-4 §position).
        (Some(a), None) if vert(a) => ("center", *a),
        (Some(a), None) => (*a, "center"),
        _ => ("center", "center"),
    };
    Some((axis(a), axis(b)))
}

/// `ray(<angle> && <ray-size>? && contain? && [at <position>]?)` (§ray()).
///
/// Луч — не контур, а отрезок из начала: ломаной по нему не строят, длину
/// задаёт `<ray-size>` от содержащего блока. Без опорной коробки (`rb == None`)
/// работает только пиксельный `offset-distance` — как и раньше.
fn ray_css(
    c: &Computed,
    args: &str,
    cb: Option<&Cb>,
    rb: Option<(f32, f32, f32, f32)>,
) -> Option<String> {
    let toks: Vec<&str> = args.split_whitespace().collect();
    let bearing = toks.iter().find_map(|t| angle_rad(t))?;
    // Компасный угол: 0deg смотрит ВВЕРХ, положительные — по часовой
    // («`<angle>` values are interpreted as bearing angles, with 0deg pointing
    // up and positive angles representing clockwise rotation»). В экранных
    // осях (x вправо, y вниз) это (sin a, -cos a).
    let dir = (bearing.sin(), -bearing.cos());
    // Начало: `at <position>` сильнее всего, иначе `offset-position`, иначе
    // центр («If the element doesn't have an offset starting position either,
    // it behaves as `at center`»).
    let start = match (toks.iter().position(|t| *t == "at"), rb) {
        (Some(i), Some(rb)) => position_in(&toks[i + 1..], rb),
        _ => start_of(c, cb, rb),
    };
    let len = match (c.offset_distance, rb) {
        (Some(Len::Px(v)), _) => v,
        // Доля меряется длиной луча, а та — опорной коробкой: без неё
        // по-прежнему не рисуем ничего, чтобы не встать заведомо не туда.
        (Some(Len::Pct(k)), Some(rb)) => {
            let kind = toks.iter().find_map(|t| ray_size(t)).unwrap_or(0);
            let mut full = ray_len(kind, start, rb, dir);
            // `contain`: «the path's length is reduced by half the width or
            // half the height of the element's border box, whichever is
            // larger, and floored at zero».
            if toks.contains(&"contain") {
                let (sw, sh) = cb.map_or((0.0, 0.0), |g| g.self_size);
                full = (full - sw.max(sh) / 2.0).max(0.0);
            }
            k * full
        }
        (Some(Len::Pct(_)), None) => return None,
        _ => 0.0,
    };
    let own = cb.map_or((0.0, 0.0), |g| g.self_off);
    let p = (
        start.0 + dir.0 * len - own.0,
        start.1 + dir.1 * len - own.1,
    );
    // Касательная луча относительно оси X — это компасный угол минус
    // четверть оборота; `reverse`/прибавку читает `rotation`.
    Some(origin_shift(
        c,
        p,
        rotation(c, bearing - std::f32::consts::FRAC_PI_2),
    ))
}

/// `<ray-size> = <radial-extent> | sides`: 0 closest-side, 1 closest-corner,
/// 2 farthest-side, 3 farthest-corner, 4 sides. Умолчание — `closest-side`.
fn ray_size(t: &str) -> Option<u8> {
    Some(match t {
        "closest-side" => 0,
        "closest-corner" => 1,
        "farthest-side" => 2,
        "farthest-corner" => 3,
        "sides" => 4,
        _ => return None,
    })
}

/// Длина луча от начала `s` до края опорной коробки `rb` (§`<ray-size>`).
///
/// У сторон края считаются ПРЯМЫМИ, продолженными в бесконечность («if the
/// ray's starting point is outside the containing block entirely, the edges of
/// the containing block are considered to extend out to infinity»), поэтому
/// берутся модули расстояний до четырёх линий, а не до отрезков.
fn ray_len(kind: u8, s: (f32, f32), rb: (f32, f32, f32, f32), dir: (f32, f32)) -> f32 {
    let (x0, y0, w, h) = rb;
    let (x1, y1) = (x0 + w, y0 + h);
    let dx = [(s.0 - x0).abs(), (x1 - s.0).abs()];
    let dy = [(s.1 - y0).abs(), (y1 - s.1).abs()];
    let corner = |cx: f32, cy: f32| ((cx - s.0).powi(2) + (cy - s.1).powi(2)).sqrt();
    match kind {
        0 => dx[0].min(dx[1]).min(dy[0]).min(dy[1]),
        1 => corner(x0, y0)
            .min(corner(x1, y0))
            .min(corner(x0, y1))
            .min(corner(x1, y1)),
        2 => dx[0].max(dx[1]).max(dy[0]).max(dy[1]),
        3 => corner(x0, y0)
            .max(corner(x1, y0))
            .max(corner(x0, y1))
            .max(corner(x1, y1)),
        // `sides` — до пересечения ЛУЧА с границей блока; начало на границе
        // или снаружи даёт ноль (§`<ray-size>`/sides).
        _ => {
            if s.0 < x0 || s.0 > x1 || s.1 < y0 || s.1 > y1 {
                return 0.0;
            }
            let t = |num: f32, den: f32| {
                if den.abs() < 1e-6 {
                    return f32::INFINITY;
                }
                let t = num / den;
                // Сторона, на которой начало уже стоит (t = 0), не пересечение:
                // из угла (0,0) под 90deg луч идёт до ПРАВОЙ стороны
                // (`offset-path-ray-019`: `translateX(100px)` и `200px`).
                // Луч наружу со стороны не находит ни одного t > 0 — длина 0,
                // как и велит §ray() для начала на границе.
                if t > 1e-4 { t } else { f32::INFINITY }
            };
            let hit = t(x0 - s.0, dir.0)
                .min(t(x1 - s.0, dir.0))
                .min(t(y0 - s.1, dir.1))
                .min(t(y1 - s.1, dir.1));
            if hit.is_finite() { hit } else { 0.0 }
        }
    }
}

fn angle_rad(t: &str) -> Option<f32> {
    let v: f32 = t
        .trim_end_matches("deg")
        .trim_end_matches("grad")
        .trim_end_matches("turn")
        .trim_end_matches("rad")
        .parse()
        .ok()?;
    Some(if t.ends_with("grad") {
        v * std::f32::consts::PI / 200.0
    } else if t.ends_with("turn") {
        v * std::f32::consts::TAU
    } else if t.ends_with("rad") {
        v
    } else {
        v.to_radians()
    })
}

/// Разложить `d` в ломаную. Кроме прямых читаются кубики/квадратики (`C`,
/// `S`, `Q`, `T` — их даёт `shape()` и авторский `path()`) и дуги (`A` — их
/// даёт эквивалентный путь круга, эллипса и скруглённого прямоугольника).
/// Дробление фиксированное: 64 отрезка на сегмент — при радиусе 210px
/// (`offset-path-coord-box-001`) хорда отходит от дуги на 0.016px, а длина
/// занижается на четверть промилле; порог стенда — 0.5 % площади.
fn flatten(d: &str) -> Option<Poly> {
    let mut pts: Vec<(f32, f32)> = Vec::new();
    let mut closed = false;
    let (mut cx, mut cy) = (0.0f32, 0.0f32);
    let (mut sx, mut sy) = (0.0f32, 0.0f32);
    // Отражение последней контрольной точки — для `S`/`T`
    // (SVG 2 §PathDataCubicBezierCommands: «the reflection of the second
    // control point on the previous command relative to the current point»).
    let (mut rfx, mut rfy) = (0.0f32, 0.0f32);
    let mut prev_curve = ' ';
    let toks = tokens(d);
    let mut i = 0usize;
    let mut cmd = ' ';
    while i < toks.len() {
        if let Some(ch) = toks[i].chars().next().filter(|c| c.is_ascii_alphabetic()) {
            cmd = ch;
            i += 1;
        }
        let num = |k: usize| -> f32 { toks.get(k).and_then(|t| t.parse().ok()).unwrap_or(0.0) };
        let rel = cmd.is_ascii_lowercase();
        // Снимок текущей точки: относительные команды отсчитываются от точки
        // НА НАЧАЛЕ команды (SVG 2 §PathData), а `cx`/`cy` по ходу разбора
        // переписываются — замыкание не должно держать их взаймы.
        let (bx, by) = (cx, cy);
        let ax = |v: f32| if rel { bx + v } else { v };
        let ay = |v: f32| if rel { by + v } else { v };
        let lower = cmd.to_ascii_lowercase();
        match lower {
            'm' => {
                cx = ax(num(i));
                cy = ay(num(i + 1));
                sx = cx;
                sy = cy;
                pts.push((cx, cy));
                i += 2;
                // Следующая пара после `m` — уже линия (SVG 2 §PathData).
                cmd = if rel { 'l' } else { 'L' };
            }
            'l' => {
                cx = ax(num(i));
                cy = ay(num(i + 1));
                pts.push((cx, cy));
                i += 2;
            }
            'h' => {
                cx = ax(num(i));
                pts.push((cx, cy));
                i += 1;
            }
            'v' => {
                cy = ay(num(i));
                pts.push((cx, cy));
                i += 1;
            }
            'c' | 's' | 'q' | 't' => {
                let p0 = (cx, cy);
                // Контрольные точки: у `S`/`T` первая — отражение прошлой,
                // у первой команды подряд отражать нечего, и она равна началу.
                let mirror = if matches!(prev_curve, 'c' | 's' | 'q' | 't') {
                    (2.0 * cx - rfx, 2.0 * cy - rfy)
                } else {
                    p0
                };
                let (c1, c2, end, n) = match lower {
                    'c' => (
                        (ax(num(i)), ay(num(i + 1))),
                        (ax(num(i + 2)), ay(num(i + 3))),
                        (ax(num(i + 4)), ay(num(i + 5))),
                        6,
                    ),
                    's' => (
                        mirror,
                        (ax(num(i)), ay(num(i + 1))),
                        (ax(num(i + 2)), ay(num(i + 3))),
                        4,
                    ),
                    'q' => {
                        let q = (ax(num(i)), ay(num(i + 1)));
                        let e = (ax(num(i + 2)), ay(num(i + 3)));
                        (quad_c1(p0, q), quad_c2(e, q), e, 4)
                    }
                    _ => {
                        let e = (ax(num(i)), ay(num(i + 1)));
                        (quad_c1(p0, mirror), quad_c2(e, mirror), e, 2)
                    }
                };
                cubic_pts(p0, c1, c2, end, &mut pts);
                // Отражать положено ВТОРУЮ контрольную точку кубика; у
                // квадратика — его единственную, поэтому её и запоминаем.
                (rfx, rfy) = if matches!(lower, 'q' | 't') {
                    if lower == 'q' {
                        (ax(num(i)), ay(num(i + 1)))
                    } else {
                        mirror
                    }
                } else {
                    c2
                };
                cx = end.0;
                cy = end.1;
                i += n;
            }
            'a' => {
                let p0 = (cx, cy);
                let (rx, ry) = (num(i), num(i + 1));
                let phi = num(i + 2).to_radians();
                let large = num(i + 3) != 0.0;
                let sweep = num(i + 4) != 0.0;
                let end = (ax(num(i + 5)), ay(num(i + 6)));
                arc_pts(p0, rx, ry, phi, large, sweep, end, &mut pts);
                cx = end.0;
                cy = end.1;
                i += 7;
            }
            'z' => {
                pts.push((sx, sy));
                cx = sx;
                cy = sy;
                closed = true;
                i += 1;
            }
            _ => break,
        }
        prev_curve = lower;
    }
    (!pts.is_empty()).then_some(Poly { pts, closed })
}

/// Квадратик в кубик (SVG 2 §PathDataQuadraticBezierCommands, «two thirds of
/// the way»): первая контрольная точка.
fn quad_c1(p0: (f32, f32), q: (f32, f32)) -> (f32, f32) {
    (
        p0.0 + 2.0 / 3.0 * (q.0 - p0.0),
        p0.1 + 2.0 / 3.0 * (q.1 - p0.1),
    )
}

/// Квадратик в кубик: вторая контрольная точка.
fn quad_c2(p1: (f32, f32), q: (f32, f32)) -> (f32, f32) {
    (
        p1.0 + 2.0 / 3.0 * (q.0 - p1.0),
        p1.1 + 2.0 / 3.0 * (q.1 - p1.1),
    )
}

/// Кубик в точки (начало НЕ пишется — оно уже в ломаной).
fn cubic_pts(
    p0: (f32, f32),
    c1: (f32, f32),
    c2: (f32, f32),
    p1: (f32, f32),
    out: &mut Vec<(f32, f32)>,
) {
    const N: usize = 64;
    for k in 1..=N {
        let t = k as f32 / N as f32;
        let u = 1.0 - t;
        let b = |a: f32, b1: f32, b2: f32, d: f32| {
            u * u * u * a + 3.0 * u * u * t * b1 + 3.0 * u * t * t * b2 + t * t * t * d
        };
        out.push((b(p0.0, c1.0, c2.0, p1.0), b(p0.1, c1.1, c2.1, p1.1)));
    }
}

/// Дуга `A rx ry φ large sweep x y` в точки: перевод из КОНЦЕВОЙ записи в
/// центровую по SVG 2 §F.6.5 плюс раздутие недостаточных радиусов по §F.6.6.
fn arc_pts(
    p0: (f32, f32),
    rx: f32,
    ry: f32,
    phi: f32,
    large: bool,
    sweep: bool,
    p1: (f32, f32),
    out: &mut Vec<(f32, f32)>,
) {
    let (mut rx, mut ry) = (rx.abs(), ry.abs());
    // §F.6.2: нулевой радиус или совпавшие концы — обычный отрезок.
    if rx == 0.0 || ry == 0.0 || ((p0.0 - p1.0).abs() + (p0.1 - p1.1).abs()) < 1e-9 {
        out.push(p1);
        return;
    }
    let (cs, sn) = (phi.cos(), phi.sin());
    let dx2 = (p0.0 - p1.0) / 2.0;
    let dy2 = (p0.1 - p1.1) / 2.0;
    let x1 = cs * dx2 + sn * dy2;
    let y1 = -sn * dx2 + cs * dy2;
    let lam = x1 * x1 / (rx * rx) + y1 * y1 / (ry * ry);
    if lam > 1.0 {
        rx *= lam.sqrt();
        ry *= lam.sqrt();
    }
    let num = (rx * rx * ry * ry - rx * rx * y1 * y1 - ry * ry * x1 * x1).max(0.0);
    let den = rx * rx * y1 * y1 + ry * ry * x1 * x1;
    let mut co = if den > 0.0 { (num / den).sqrt() } else { 0.0 };
    if large == sweep {
        co = -co;
    }
    let cx1 = co * rx * y1 / ry;
    let cy1 = -co * ry * x1 / rx;
    let cx = cs * cx1 - sn * cy1 + (p0.0 + p1.0) / 2.0;
    let cy = sn * cx1 + cs * cy1 + (p0.1 + p1.1) / 2.0;
    let ang = |ux: f32, uy: f32, vx: f32, vy: f32| {
        let d = (ux * vx + uy * vy) / ((ux * ux + uy * uy).sqrt() * (vx * vx + vy * vy).sqrt());
        let a = d.clamp(-1.0, 1.0).acos();
        if ux * vy - uy * vx < 0.0 { -a } else { a }
    };
    let (ux, uy) = ((x1 - cx1) / rx, (y1 - cy1) / ry);
    let (vx, vy) = ((-x1 - cx1) / rx, (-y1 - cy1) / ry);
    let t0 = ang(1.0, 0.0, ux, uy);
    let mut dt = ang(ux, uy, vx, vy);
    if !sweep && dt > 0.0 {
        dt -= std::f32::consts::TAU;
    }
    if sweep && dt < 0.0 {
        dt += std::f32::consts::TAU;
    }
    const N: usize = 64;
    for k in 1..=N {
        let t = t0 + dt * (k as f32 / N as f32);
        let (xa, ya) = (rx * t.cos(), ry * t.sin());
        out.push((cx + cs * xa - sn * ya, cy + sn * xa + cs * ya));
    }
}

fn tokens(d: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut cur = String::new();
    for ch in d.chars() {
        if ch.is_ascii_alphabetic() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
            out.push(ch.to_string());
        } else if ch == ',' || ch.is_whitespace() {
            if !cur.is_empty() {
                out.push(std::mem::take(&mut cur));
            }
        } else if ch == '-' && !cur.is_empty() && !cur.ends_with('e') {
            out.push(std::mem::take(&mut cur));
            cur.push(ch);
        } else {
            cur.push(ch);
        }
    }
    if !cur.is_empty() {
        out.push(cur);
    }
    out
}

fn length(p: &[(f32, f32)]) -> f32 {
    p.windows(2).map(|w| dist(w[0], w[1])).sum()
}

fn dist(a: (f32, f32), b: (f32, f32)) -> f32 {
    ((b.0 - a.0).powi(2) + (b.1 - a.1).powi(2)).sqrt()
}

/// Точка и угол касательной на расстоянии `s` от начала.
///
/// Вырожденный путь (`m 120 0 h 0 v 0 z`) даёт одну точку и нулевой угол —
/// именно этого ждут `offset-distance-004..006`.
fn sample(p: &[(f32, f32)], s: f32) -> ((f32, f32), f32) {
    if p.len() < 2 {
        return (*p.first().unwrap_or(&(0.0, 0.0)), 0.0);
    }
    let mut left = s;
    for w in p.windows(2) {
        let seg = dist(w[0], w[1]);
        if seg <= 0.0 {
            continue;
        }
        if left <= seg {
            let t = left / seg;
            let pt = (
                w[0].0 + (w[1].0 - w[0].0) * t,
                w[0].1 + (w[1].1 - w[0].1) * t,
            );
            return (pt, (w[1].1 - w[0].1).atan2(w[1].0 - w[0].0));
        }
        left -= seg;
    }
    let last = p[p.len() - 1];
    let prev = p[p.len() - 2];
    (last, (last.1 - prev.1).atan2(last.0 - prev.0))
}
