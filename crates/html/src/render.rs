//! Сборка дерева узлов в элементы GPUI.
//!
//! Блочные узлы становятся `div` со своим стилем; подряд идущие инлайн-узлы
//! собираются в один абзац (`inline.rs`). Списки, таблицы и картинки имеют
//! свои правила — они и описаны в доке отдельными разделами.

use crate::apply::{apply, apply_hover};
use crate::computed::{Align, Computed, Display, FlexDir};
use crate::dom::{Element, Node};
use crate::inline::{self};
use crate::value::Len;
use gpui::{
    AnyElement, IntoElement, ParentElement, SharedString, Styled, StyledImage, TextStyle, div, px,
};

/// Настройки отрисовки: то, что задаёт приложение, а не документ.
#[derive(Clone)]
pub struct RenderOpts {
    /// Базовый стиль текста — от него считаются прогоны и наследование.
    pub text: TextStyle,
    /// Размер окна в точках — от него считаются `vh` и `vw`.
    pub viewport: (f32, f32),
    /// Множитель строки при `line-height: normal`.
    ///
    /// Браузер берёт его из метрик шрифта — у интерфейсных это около 1.31
    /// кегля. Умолчание GPUI — золотое сечение (1.618), и без своего значения
    /// КАЖДЫЙ блок текста выходил на четверть выше браузерного, а разница
    /// копилась вниз по документу.
    pub normal_line_height: f32,
    /// Соль документа для буферов проб (`Document::key`).
    ///
    /// Номера узлов считаются с нуля в каждом документе: когда в одном
    /// потоке живут два документа сразу (стенд гонит пары параллельно),
    /// полоса фона одного забирала прямоугольники ячеек другого с тем же
    /// номером узла. Ноль допустим, пока документ один.
    pub doc_salt: u64,
}

impl RenderOpts {
    /// Цвет подложки выделения.
    ///
    /// Отдельного поля в настройках нет, чтобы не ломать вызывающих: берём
    /// цвет текста и делаем из него полупрозрачную подложку — она читается
    /// и на светлой, и на тёмной теме.
    fn selection_color(&self) -> gpui::Hsla {
        let mut c = self.text.color;
        c.a = 0.25;
        c
    }

    fn base_size(&self) -> f32 {
        f32::from(self.text.font_size.to_pixels(px(16.)))
    }

    /// Корневой стиль документа.
    ///
    /// Высота строки тут НЕ задаётся: `normal` по CSS — метрика шрифта, и
    /// считает её `normal_fraction` по семейству элемента. Пока корень
    /// навязывал постоянную долю, она наследовалась ВСЕМ, и замер шрифта не
    /// работал ни разу: коробка с `line-height: normal` выходила выше коробки
    /// с `line-height: 1em` при одном и том же шрифте.
    fn root_style(&self) -> Computed {
        Computed::default()
    }
}

/// Базовый стиль элемента плюс слой наведения и дорисовка того, чего в
/// `gpui::Style` нет: обводки, размытия подложки, разноцветных сторон рамки.
fn styled_div(e: &Element) -> gpui::Div {
    styled_div_with(e, &e.style)
}

/// То же, но со стилем, уже разрешённым по родителю.
///
/// Наследование даёт две вещи: текстовые свойства и разрешённые `em` — без
/// него отступ в `em` считался бы от базового кегля, а не от своего.
/// Слой фона, обрезанного внутренним краем коробки (`background-clip`).
///
/// Коробка в раскладке красит весь свой прямоугольник, включая рамку и поля,
/// а `padding-box`/`content-box` требуют красить меньше. Поэтому фон снимается
/// с коробки (см. `apply::apply_paint`) и рисуется отдельным слоем, вжатым
/// внутрь: на рамку, а для `content-box` ещё и на поля. Слой идёт ПЕРВЫМ
/// ребёнком — порядок детей задаёт порядок рисования, и содержимое остаётся
/// поверх фона.
///
/// `text` слоя не даёт вовсе: фон по форме глифов мы не рисуем, и закрасить
/// вместо него всю коробку — заметно хуже, чем не красить (тесты на него
/// прямо пишут «no red» про залитый прямоугольник).
fn clip_layer(c: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let clip = c.bg_clip?;
    if c.gradient.is_none() && c.background.is_none() {
        return None;
    }
    if clip == crate::computed::BgClip::Text {
        return None;
    }
    let size = own_size(c, opts);
    let family = c.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &family, size);
    let border = c.borders();
    let pad = |b: Option<Len>, p: Option<Len>| {
        px_of(b)
            + if clip == crate::computed::BgClip::ContentBox {
                px_of(p)
            } else {
                0.0
            }
    };
    let mut layer = div()
        .absolute()
        .top(px(pad(border.top, c.padding.top)))
        .right(px(pad(border.right, c.padding.right)))
        .bottom(px(pad(border.bottom, c.padding.bottom)))
        .left(px(pad(border.left, c.padding.left)));
    layer = match (&c.gradient, c.background) {
        (Some(g), _) => layer.bg(crate::apply::fill(g)),
        (None, Some(bg)) => layer.bg(gpui::Background::from(bg.to_hsla())),
        _ => return None,
    };
    // Скругление внутреннего края меньше внешнего ровно на толщину рамки
    // (css-backgrounds-3 §5.4): слой со скруглением коробки вылезал бы
    // уголками за неё.
    let inner = |r: Option<Len>, a: Option<Len>, b: Option<Len>| {
        let cut = px_of(a).max(px_of(b));
        (px_of(r) - cut).max(0.0)
    };
    layer = layer
        .rounded_tl(px(inner(c.radius.tl, border.top, border.left)))
        .rounded_tr(px(inner(c.radius.tr, border.top, border.right)))
        .rounded_br(px(inner(c.radius.br, border.bottom, border.right)))
        .rounded_bl(px(inner(c.radius.bl, border.bottom, border.left)));
    Some(layer.into_any_element())
}

fn styled_div_with(e: &Element, style: &Computed) -> gpui::Div {
    let c = style;
    let mut d = apply(div(), c);
    // `pointer-events: none` — элемент не реагирует на курсор, значит и слой
    // наведения к нему не применяется.
    if c.pointer_events_none != Some(true) {
        if let Some(h) = &e.hover {
            d = apply_hover(d, h);
        }
    }
    // Обрезка контейнера (css-overflow-3/4): точная точка среза приходит
    // из бюджета строк ПРОШЛОГО кадра (interact::ClampCut) — низ N-й
    // считаемой строки, поднятый к верху пересечённого блока. Пока точки
    // нет (первый кадр) — грубый потолок в N своих строк.
    // Обрезает только ВЛАДЕЛЕЦ line-clamp: свойство не наследуется, но
    // слитый стиль несёт его вниз для текст-ранов — потомки резали себя
    // тем же потолком и с чужими ключами бюджета.
    if e.style.clamp_auto == Some(true) && c.max_height.is_some() {
        if let Some(cut) = crate::interact::clamp_cut(e.node_id) {
            d = d.max_h(px(cut));
        }
        d = d.overflow_hidden();
    }
    if let Some(n) = e.style.clamp_lines() {
        d = d.line_clamp(n as usize);
        let font = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let line = match c.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * font,
            _ => 1.2 * font,
        };
        let cut = crate::interact::clamp_cut(e.node_id).unwrap_or(n as f32 * line);
        if std::env::var("HTML_CLAMP_DBG").is_ok() {
            eprintln!("CLAMP branch node={} n={} cut={}", e.node_id, n, cut);
        }
        d = d.max_h(px(cut)).overflow_hidden();
    }
    for extra in decorations(c) {
        d = d.child(extra);
    }
    d
}

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
fn decorations(c: &Computed) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];

    // РЕЗКАЯ тень (без размытия): примитив тени с нулевым размытием
    // вырождается в шейдере, поэтому она рисуется слоем-квадом, раздутым на
    // разлёт. Радиус фигуры — по спеке (css-backgrounds-3 §7.1): нулевой
    // остаётся острым, иначе растёт на разлёт; доля считается от размера
    // раздутой фигуры (известные ширина и высота).
    for sh in &c.shadows {
        if sh.blur > 0.0 || sh.color.a <= 0.0 {
            continue;
        }
        let spread = sh.spread;
        let radius = match c.radius.tl {
            Some(Len::Px(v)) if v > 0.0 => v + spread,
            Some(Len::Pct(k)) => match (c.width, c.height) {
                (Some(Len::Px(w)), Some(Len::Px(h))) => {
                    k * (w + 2.0 * spread).min(h + 2.0 * spread)
                }
                _ => 0.0,
            },
            _ => 0.0,
        };
        // Слой живёт СРЕДИ детей и рисовался бы поверх фона коробки, а тень
        // обязана быть ПОД ней — поэтому центр слоя прозрачный: краску несёт
        // РАМКА толщиной в разлёт (со сдвигом по смещению тени). Точная
        // фигура «раздутое минус коробка» этим покрыта при |смещении| не
        // больше разлёта — обычный случай резкой тени.
        let widths = [
            (spread - sh.y).max(0.0),
            (spread + sh.x).max(0.0),
            (spread + sh.y).max(0.0),
            (spread - sh.x).max(0.0),
        ];
        out.push(
            div()
                .absolute()
                .top(px(sh.y - spread))
                .left(px(sh.x - spread))
                .right(px(-sh.x - spread))
                .bottom(px(-sh.y - spread))
                .rounded(px(radius))
                .border_t(px(widths[0]))
                .border_r(px(widths[1]))
                .border_b(px(widths[2]))
                .border_l(px(widths[3]))
                .border_color(sh.color.to_hsla())
                .into_any_element(),
        );
    }

    // Фоновая картинка идёт первой: она поверх цвета фона и под всем
    // остальным — тот же порядок, что в браузере.
    if let Some(layer) = crate::background::layer(c) {
        out.push(layer);
    } else if c.gradient_as_tile() {
        // Градиент с размером/повтором/позицией — той же механикой плитки:
        // источник понимает записи `linear-gradient(...)`.
        let mut tiled = c.clone();
        tiled.bg_image = tiled.gradient_raw.clone();
        if let Some(layer) = crate::background::layer(&tiled) {
            out.push(layer);
        }
    }

    // Рамка-картинка рисуется ПОВЕРХ фона и заменяет обычную рамку.
    if let Some(layer) = crate::border_image::layer(c) {
        out.push(layer);
    }

    // `backdrop-filter: blur(N)`: размывает то, что под элементом. Рисуется
    // проходом рендера (патч gpui), поэтому это канвас, а не стиль.
    if let Some(radius) = c.backdrop_blur {
        let corner = match c.radius.tl {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        out.push(
            gpui::canvas(
                |_, _, _| {},
                move |bounds, _, window, _| {
                    window.paint_backdrop_blur_radius(
                        bounds,
                        gpui::Corners::all(px(corner)),
                        radius,
                    );
                },
            )
            .absolute()
            .top_0()
            .left_0()
            .size_full()
            .into_any_element(),
        );
    }

    // Градиент из пяти и более стопов: заливка несёт четыре (патч GPUI), а
    // дальше осевой градиент по-прежнему рисуется полосами — по слою на пару
    // соседних стопов. Наклонный полосами не выразить.
    if let Some(g) = &c.gradient {
        let vertical = matches!(g.angle_deg as i32, 0 | 180);
        let horizontal = matches!(g.angle_deg as i32, 90 | 270);
        let reverse = matches!(g.angle_deg as i32, 0 | 270);
        // Стопы в точках рисуются полосами точной ширины: доля от них не
        // считается, длина оси известна только коробке. Отсчёт полос — от
        // верха/лева; обратное направление (0/270deg) идёт от низа/права.
        if !g.stops_px.is_empty() && !g.radial && (vertical || horizontal) {
            // Полосы в точках могут выйти за коробку (стопы длиннее оси) —
            // фон обрезается её краем, поэтому все полосы живут в общем
            // обрезающем слое на всю коробку.
            let mut bands: Vec<AnyElement> = vec![];
            for pair in g.stops_px.windows(2) {
                let (a, b) = (pair[0], pair[1]);
                let (p0, p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (from, to) = (a.0, b.0);
                let band = crate::computed::Gradient {
                    angle_deg: if vertical { 180.0 } else { 90.0 },
                    radial: false,
                    circle: false,
                    from: if reverse { to } else { from },
                    to: if reverse { from } else { to },
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                };
                let layer = div().absolute().bg(crate::apply::fill(&band));
                bands.push(
                    match (vertical, reverse) {
                        (true, false) => layer.left_0().right_0().top(px(p0)).h(px(p1 - p0)),
                        (true, true) => layer.left_0().right_0().bottom(px(p0)).h(px(p1 - p0)),
                        (false, false) => layer.top_0().bottom_0().left(px(p0)).w(px(p1 - p0)),
                        (false, true) => layer.top_0().bottom_0().right(px(p0)).w(px(p1 - p0)),
                    }
                    .into_any_element(),
                );
            }
            out.push(
                div()
                    .absolute()
                    .top_0()
                    .left_0()
                    .right_0()
                    .bottom_0()
                    .overflow_hidden()
                    .children(bands)
                    .into_any_element(),
            );
        } else if g.stops.len() > 4 && !g.radial && (vertical || horizontal) {
            // Полоса перекрывает фон родителя целиком, поэтому скругление
            // приходится повторять на крайних полосах: иначе углы блока
            // становятся прямыми.
            let corner = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let last_band = g.stops.len() - 2;
            for (idx, pair) in g.stops.windows(2).enumerate() {
                let (a, b) = (pair[0], pair[1]);
                let (mut p0, mut p1) = (a.1, b.1);
                if p1 <= p0 {
                    continue;
                }
                let (mut from, mut to) = (a.0, b.0);
                if reverse {
                    // Отсчёт полос всегда сверху/слева, поэтому обратное
                    // направление разворачивает и порядок, и цвета.
                    (p0, p1) = (1.0 - p1, 1.0 - p0);
                    (from, to) = (to, from);
                }
                let band = crate::computed::Gradient {
                    angle_deg: g.angle_deg,
                    radial: false,
                    circle: false,
                    from,
                    to,
                    stops: vec![(from, 0.0), (to, 1.0)],
                    stops_px: vec![],
                    stops_raw: vec![],
                };
                let mut layer = div().absolute().bg(crate::apply::fill(&band));
                // «Первая» полоса по направлению отрисовки, а не по списку:
                // при обратном направлении список развёрнут.
                let first_edge = if reverse { idx == last_band } else { idx == 0 };
                let last_edge = if reverse { idx == 0 } else { idx == last_band };
                if vertical {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_tr(px(corner(c.radius.tr)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_bl(px(corner(c.radius.bl)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                } else {
                    if first_edge {
                        layer = layer
                            .rounded_tl(px(corner(c.radius.tl)))
                            .rounded_bl(px(corner(c.radius.bl)));
                    }
                    if last_edge {
                        layer = layer
                            .rounded_tr(px(corner(c.radius.tr)))
                            .rounded_br(px(corner(c.radius.br)));
                    }
                }
                out.push(
                    if vertical {
                        layer
                            .left_0()
                            .right_0()
                            .top(gpui::relative(p0))
                            .h(gpui::relative(p1 - p0))
                    } else {
                        layer
                            .top_0()
                            .bottom_0()
                            .left(gpui::relative(p0))
                            .w(gpui::relative(p1 - p0))
                    }
                    .into_any_element(),
                );
            }
        }
    }

    // `outline`: рамка ВНЕ коробки и без влияния на раскладку — отдельный
    // абсолютный слой с отрицательным отступом ровно на её толщину.
    if let Some(o) = c.outline {
        let w = match o.width {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        // Обводка без своего цвета берёт цвет текста — так решает CSS.
        if let (true, Some(colour)) = (w > 0.0, o.color.or(c.color)) {
            let off = match o.offset {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let corner = match c.radius.tl {
                Some(Len::Px(v)) => v + off + w,
                _ => 0.0,
            };
            out.push(
                div()
                    .absolute()
                    .top(px(-(off + w)))
                    .left(px(-(off + w)))
                    .right(px(-(off + w)))
                    .bottom(px(-(off + w)))
                    .border(px(w))
                    .border_color(colour.to_hsla())
                    .rounded(px(corner))
                    .into_any_element(),
            );
        }
    }

    // Разные цвета сторон рамки: у GPUI цвет рамки один на элемент, поэтому
    // несовпадающие стороны дорисовываются полосами поверх.
    let sides: Vec<_> = c.border_colors.iter().flatten().collect();
    let uniform = sides.len() == 4 && sides.iter().all(|s| *s == sides[0]);
    if !sides.is_empty() && !uniform {
        let side_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let bw = c.borders();
        let (t, r, b, l) = (
            side_px(bw.top),
            side_px(bw.right),
            side_px(bw.bottom),
            side_px(bw.left),
        );
        for (i, colour) in c.border_colors.iter().enumerate() {
            let Some(colour) = colour else { continue };
            let w = [t, r, b, l][i];
            if w <= 0.0 {
                continue;
            }
            // Абсолютный ребёнок считается от ВНУТРЕННЕГО края рамки,
            // поэтому кольцо накрывается отрицательными отступами ровно на
            // толщину сторон — иначе полоса ложится внутрь содержимого.
            let strip = div().absolute().bg(colour.to_hsla());
            out.push(
                match i {
                    0 => strip.top(px(-t)).left(px(-l)).right(px(-r)).h(px(w)),
                    1 => strip.top(px(-t)).right(px(-r)).bottom(px(-b)).w(px(w)),
                    2 => strip.bottom(px(-b)).left(px(-l)).right(px(-r)).h(px(w)),
                    _ => strip.top(px(-t)).left(px(-l)).bottom(px(-b)).w(px(w)),
                }
                .into_any_element(),
            );
        }
    }
    out
}

/// Отрисовать корневые узлы документа.
///
/// Годится для короткого документа — виджета, ответа модели. Длинный документ
/// рисуйте по блокам (`render_block`): раскладка в GPUI считается заново
/// каждый кадр, поэтому стоимость кадра обязана зависеть от видимой части, а
/// не от размера документа.
pub fn render(nodes: &[Node], opts: &RenderOpts) -> Vec<AnyElement> {
    let root = opts.root_style();
    blocks(nodes, &root, opts)
}

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let node = nodes.get(index)?;
    let root = opts.root_style();
    blocks(std::slice::from_ref(node), &root, opts)
        .into_iter()
        .next()
}

/// Разбор списка детей на блоки: инлайн-подряд склеивается в абзац.
/// Абзац с пробой бюджета строк: если строится внутри clamp-контейнера,
/// рядом с абзацем едет проба его границ и высоты строки.
fn paragraph_probed(taken: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let para = paragraph(taken, inherited, opts);
    let para = with_text_shadow(para, inherited, taken);
    if let Some((key, skip)) = crate::interact::clamp_context() {
        div()
            .relative()
            .child(para)
            .child(crate::interact::clamp_probe(
                crate::interact::clamp_lines_for(key),
                line_height_px(inherited, opts),
                skip,
                false,
            ))
            .into_any_element()
    } else {
        para
    }
}

fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    // `order` в CSS работает ТОЛЬКО внутри гибкого контейнера и сетки; в
    // обычном потоке он не значит ничего. Раньше сортировались дети любого
    // родителя — блоки меняли порядок там, где браузер их не трогает.
    let ordered_context = matches!(
        inherited.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            // Поток лунок — сеточный контекст: схлопывания отступов нет
            // (css-grid-3), и `order` действует.
            | Some(Display::GridLanes)
    );
    // Схлопывание вертикальных отступов есть ТОЛЬКО в обычном потоке: в
    // гибком контейнере и сетке CSS его запрещает, а мы схлопывали везде —
    // элементы ряда съезжали друг к другу против браузера.
    let collapsed = if ordered_context {
        reorder(nodes.to_vec())
    } else {
        collapse_margins(nodes)
    };
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed: Vec<Node> = if ordered_context {
        collapsed
            .into_iter()
            // `visibility: collapse` на элементе гибкого контейнера убирает
            // его из строки целиком: перенос считается так, будто элемента
            // нет. Прятать его на месте — значит переносить строки не там.
            .filter(|n| !matches!(n, Node::Element(e) if e.style.collapsed == Some(true)))
            .map(|n| match n {
                Node::Element(mut e) => {
                    e.style.float = None;
                    e.style.clear = None;
                    e.style.vertical_align = None;
                    // `flex-basis: content` — основа по содержимому, и
                    // заданный ГЛАВНЫЙ размер при ней не действует. Какая ось
                    // главная, знает только родитель, поэтому размер снимается
                    // здесь, а не в стиле самого элемента.
                    // У ЗАМЕЩАЕМОГО элемента содержимое — он сам, и его
                    // размер задаёт собственный пиксель или атрибут: снимать
                    // его нельзя, иначе `<canvas width=20>` схлопывается в
                    // ноль (`flexbox-flex-basis-content-001a`).
                    let own = matches!(
                        e.tag.as_str(),
                        "img" | "canvas" | "embed" | "iframe" | "video" | "object" | "svg"
                    );
                    if e.style.basis_content == Some(true) {
                        match inherited.flex_dir {
                            Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                                e.style.height = own.then_some(e.style.attr_height).flatten();
                            }
                            _ => e.style.width = own.then_some(e.style.attr_width).flatten(),
                        }
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
    };
    let collapsed = by_layer(wrap_floats(collapsed));
    // Блок мы изображаем гибкой колонкой, а её дети по умолчанию сжимаются —
    // в обычном потоке этого нет: ребёнок выше родителя обязан вылезти, а не
    // ужаться. Поэтому в потоке сжатие детям выключается, если разметка не
    // просила обратного.
    let flex_context = matches!(
        inherited.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    );
    let collapsed: Vec<Node> = if ordered_context {
        // Элемент гибкого контейнера сжимается по умолчанию — это его
        // начальное значение в CSS. Проставляем его явно, потому что
        // `display: inline-block` в другом месте выключает сжатие: строчная
        // коробка В СТРОКЕ и правда не жмётся, а тот же элемент В РЯДУ —
        // обязан. Без этого ряд из `<span>`-ов держал свою ширину и не
        // ужимался до минимального размера содержимого.
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) if flex_context => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(1.0);
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    } else {
        collapsed
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e) if e.style.flex_shrink.is_none() => {
                    e.style.flex_shrink = Some(0.0);
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    let mut out = vec![];
    // Липкому ребёнку нужны две вещи, которых он сам не видит: коробка
    // родителя и видимая часть ленты. Их снимает распорка — она идёт первой,
    // потому что готовит замер до отрисовки детей.
    let sticky = collapsed.iter().any(|n| match n {
        Node::Element(e) => e.style.position == Some(crate::computed::Position::Sticky),
        _ => false,
    });
    let frame: crate::interact::StickyCell = Default::default();
    if sticky {
        out.push(sticky_probe(frame.clone()));
    }
    let mut pending: Vec<Node> = vec![];
    // Слой верхней отрисовки этого контейнера: позиционированные элементы
    // складывают сюда содержимое, а забирается оно последними детьми.
    crate::interact::late_open();
    let nodes = collapsed.as_slice();
    for n in nodes {
        let is_inline = match n {
            // Пробельный узел между инлайн-соседями — часть строки, а не
            // разрыв: `<button>A</button> <button>B</button>` в разметке с
            // переносами давал два абзаца, и кнопки вставали столбиком.
            // Под `white-space: pre*` пробельный узел — содержимое: узел из
            // одного перевода строки это ПУСТАЯ СТРОКА перед `</pre>`
            // (block-plaintext-006), отбрасывание съедало её высоту.
            Node::Text(t) => {
                inherited.preserve_newlines == Some(true)
                    || !blank_text(t)
                    || (!pending.is_empty() && t.contains(' '))
            }
            // Элемент с ЗАДАННЫМИ краями строчным не бывает: края он считает
            // от позиционированного предка, а не от строки. Куском абзаца он
            // получал содержащим блоком сам абзац — и `inset: 0` растягивал
            // его на одну строку вместо всей коробки родителя. На этом стоит
            // приём эталонов WPT: `::after` с `content: ""` и `inset: 0`
            // накрывает красное зелёным (`overflow-wrap-anywhere-001`).
            Node::Element(e)
                if matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) && !at_static_position(&e.style)
                    // Поле формы и заменяемый элемент строит СВОЙ путь
                    // (`forms::element`, картинка), и краями он распоряжается
                    // сам. Выведенный из строки, он терял свою коробку —
                    // `<button>` с четырьмя краями переставал растягиваться
                    // (`position-absolute-semi-replaced-stretch-button`).
                    && !matches!(
                        e.tag.as_str(),
                        "input" | "textarea" | "select" | "button" | "img" | "svg" | "canvas"
                    ) =>
            {
                false
            }
            Node::Element(e) => match e.style.display {
                // Явно заявленная инлайновая коробка остаётся в строке даже у
                // блочного по природе тега — но НЕ внутри гибкого контейнера
                // или сетки: там каждый ребёнок сам себе элемент раскладки
                // («блокирование» из CSS). Иначе колонка из таких коробок
                // выкладывалась рядом: они склеивались в один абзац.
                Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineTable) => !ordered_context,
                Some(_) => false,
                // Дети гибкого контейнера и сетки блокируются по CSS: каждый
                // сам себе элемент раскладки. Без оговорки `<span>` без
                // объявленного `display` оставался строчным, склеивался с
                // соседями в ОДИН абзац, и четыре элемента раскладки
                // превращались в один.
                //
                // Плавающий кусок строчным не бывает: `float` вынимает элемент
                // из строки и делает блоком (CSS 2.1 §9.7). Пока картинка с
                // `float: right` оставалась куском абзаца, до неё не доходило
                // поле родителя, и она вылезала за край страницы.
                // Перевод строки коробки не создаёт: в гибком контейнере и
                // сетке он остаётся ВНУТРИ безымянного элемента раскладки
                // вместе с соседним текстом, а не становится своим элементом
                // (`position-absolute-root-element-flex`: два предложения,
                // разделённые `<br><br>`, вставали бок о бок и переносились
                // раньше времени).
                None => {
                    e.inline
                        && (!ordered_context || e.tag == "br")
                        && !e.style.float.is_some_and(|f| f != 0)
                }
            },
        };
        if is_inline {
            pending.push(n.clone());
            continue;
        }
        if !pending.is_empty() {
            let taken = std::mem::take(&mut pending);
            out.push(paragraph_probed(&taken, inherited, opts));
        }
        if let Node::Element(e) = n {
            // Слой разрешён, только если ни один предок сам не отложен:
            // вложенная отложенная отрисовка в GPUI запрещена.
            let layer_ok = !inside_deferred();
            let _deferred_guard = DeferGuard::enter(defers(&e.style));
            // Ряд обтекания: текст рядом с плавающим блоком и остаток под ним.
            if e.tag == "kamin-float" {
                out.push(float_flow(e, inherited, opts));
                continue;
            }
            if let Some(el) = scrollable(e, inherited, opts) {
                out.push(layered(el, &e.style, layer_ok));
                continue;
            }
            if let Some(el) = resizable(e, inherited, opts) {
                out.push(el);
                continue;
            }
            if let Some(el) = transitioned(e, inherited, opts) {
                // Наложение считается и для узла с переходом: раньше ветка
                // уходила мимо, и `z-index` у него пропадал.
                out.push(layered(el, &e.style, layer_ok));
                continue;
            }
            // `display: contents` — своей коробки у элемента нет: дети
            // становятся детьми родителя, и стиль самого элемента исчезает.
            if e.style.display == Some(Display::Contents) {
                let merged = inline::inherit(inherited, &e.style);
                out.extend(blocks(&e.children, &merged, opts));
                continue;
            }
            // Обёртка `content_sized` — сетка, а дорожка сетки НЕ считает
            // боковые поля ребёнка: коробка `width: max-content` с полем
            // теряла его и уезжала (`pre-wrap-017`: зелёный блок пропадал
            // вовсе). Поэтому элемент строится БЕЗ боковых полей, а поля
            // берёт на себя обёртка.
            // Переносится только ОТРИЦАТЕЛЬНОЕ поле: положительное внутри
            // дорожки работает как надо, а отрицательное дорожка съедает —
            // коробка `width: max-content` с `margin-left: -1em` пропадала
            // вовсе (`pre-wrap-017`).
            let negative = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::Px(v) | Len::Em(v) | Len::Ch(v) | Len::Ex(v)) if v < 0.0
                )
            };
            let hoist_margins = content_sized_wraps(&e.style)
                && (negative(e.style.margin.left) || negative(e.style.margin.right));
            let stripped;
            let e = if hoist_margins {
                let mut copy = e.clone();
                copy.style.margin.left = None;
                copy.style.margin.right = None;
                stripped = copy;
                &stripped
            } else {
                e
            };
            // Анимация оборачивает ЛЮБОЙ элемент: таблицу, список, картинку —
            // раньше она доставалась только простому блоку.
            // Фон КАНВАСА (CSS 2.2 §14.2): фон корневого html — а без него
            // фон body — красит всю область просмотра, включая место за
            // полями. Слой absolute от родителя-корня растягивается на всё
            // окно, с самой коробки краска снимается (иначе двойная альфа).
            let canvas_paint = e.style.canvas_bg;
            let canvas_stripped;
            let e = if canvas_paint {
                let mut layer = div().absolute().top_0().left_0().right_0().bottom_0();
                if let Some(g) = &e.style.gradient {
                    layer = layer.bg(crate::apply::fill(g));
                } else if let Some(bg) = e.style.background {
                    layer = layer.bg(bg.to_hsla());
                }
                out.push(layer.into_any_element());
                let mut copy = e.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                canvas_stripped = copy;
                &canvas_stripped
            } else {
                e
            };
            let built = grouped(
                transformed(animated(e, inherited, opts), &e.style),
                &e.style,
            );
            let built = vertical_hug(built, e, inherited);
            let built = sticky_wrap(built, &e.style, &frame, layer_ok);
            // Абсолютный блок без заданных краёв стоит на СТАТИЧЕСКОЙ позиции —
            // там, где он оказался бы в потоке, а не в углу содержащего блока.
            // Пустышка нулевой высоты держит это место в потоке, элемент висит
            // от её угла. Без неё такой блок уезжал к началу родителя и
            // накрывал собой всё, что стояло выше.
            // Только в обычном потоке: в сетке и гибком контейнере пустышка
            // стала бы ЯЧЕЙКОЙ и сдвинула соседей, а по CSS абсолютный
            // ребёнок из раскладки родителя выключен.
            if !ordered_context && at_static_position(&e.style) {
                // Позиционированный элемент рисуется ПОВЕРХ обычного
                // содержимого (CSS 2.1 §9.9, шаг 8) и без заданного `z-index`:
                // без верхнего слоя следующий за ним сосед закрашивал его
                // собой — блок стоял на месте, но был не виден (проба:
                // абсолютный кусок между «AA» и «BB» пропадал целиком, хотя
                // один в блоке рисовался верно).
                //
                // Позиционированный элемент рисуется ПОВЕРХ обычного
                // содержимого (CSS 2.1 §9.9, шаг 8), а порядок отрисовки у нас
                // — порядок детей. Отложенная отрисовка тут не работает ни в
                // каком виде (пробовали трижды: css-position 31 → 0, css-text
                // 966 → 810, падение процесса), поэтому содержимое уходит
                // ПОСЛЕДНИМ ребёнком родителя, а на своём месте остаётся
                // нулевая распорка с холстом-щупом. Разницу их положений
                // элемент забирает отрицательным полем — так он оказывается
                // там же, где был, но рисуется последним.
                // Отрицательный `z-index` рисуется ПОД содержимым потока
                // (CSS 2.1 §9.9, шаг 3), поэтому в верхний слой он не идёт:
                // там его место — поверх всего.
                let below = e.style.z_index.is_some_and(|z| z < 0);
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    ..Default::default()
                });
                let probe = crate::interact::spot_probe(spot.clone(), true);
                let taken = if below {
                    Some(built)
                } else {
                    crate::interact::late_push(spot, built)
                };
                match taken {
                    None => out.push(probe),
                    Some(kept) => out.push(
                        div()
                            .relative()
                            .w_full()
                            .h_0()
                            .flex_shrink_0()
                            .child(kept)
                            .into_any_element(),
                    ),
                }
                continue;
            }
            let _ = hoist_margins;
            let mut done = content_sized(layered(built, &e.style, layer_ok), &e.style);
            // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
            // flow): свой анкор-ряд вокруг ОДНОГО узла — соседей не трогает.
            // Корню с фоном-картинкой не ставится (гасил canvas-слой).
            if matches!(e.tag.as_str(), "html" | "body")
                && e.style.vertical_rl == Some(true)
                && e.style.bg_image.is_none()
            {
                done = div()
                    .w_full()
                    .flex()
                    .justify_end()
                    .child(done)
                    .into_any_element();
            }
            // Релятивный элемент с отрицательным `z-index`: место в потоке —
            // своё, краска — под содержимым до него (CSS 2.1 §9.9, шаг 3).
            if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
            {
                done = crate::interact::Underlay::new(done).into_any_element();
            }
            out.push(done);
        }
    }
    if !pending.is_empty() {
        out.push(paragraph_probed(&pending, inherited, opts));
    }
    // Верхний слой: то, что обязано рисоваться поверх соседей, идёт последним
    // и возвращается на своё место замеренным сдвигом.
    out.extend(crate::interact::late_close());
    out
}

/// `order`: визуальный порядок в гибкой строке.
///
/// Раскладка под нами это свойство не знает, поэтому детей переставляем сами.
/// Сортировка устойчивая — элементы с равным `order` сохраняют порядок
/// разметки, как того требует CSS.
fn reorder(mut nodes: Vec<Node>) -> Vec<Node> {
    let ordered = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.order.is_some(),
        Node::Text(_) => false,
    });
    if !ordered {
        return nodes;
    }
    nodes.sort_by_key(|n| match n {
        Node::Element(e) => e.style.order.unwrap_or(0),
        Node::Text(_) => 0,
    });
    nodes
}

/// Схлопнуть отступы соседей вдоль горизонтальной оси потока.
///
/// Между двумя блоками остаётся больший из смежных отступов, а не их сумма.
/// Раскладка их складывает, поэтому у второго и следующих соседей ведущий
/// отступ уменьшается на уже занятый предыдущим.
/// Ортогональный поток: горизонтальный блок внутри вертикального контейнера.
///
/// Доля полей считается от СТРОЧНОЙ оси контейнера — при вертикальном письме
/// это его высота (css-writing-modes-3 §7.3, `sizing-orthogonal-percentage-
/// margin-*`). Авто-ширина такого блока не безгранична: она зажимается
/// доступным местом — физической шириной контейнера за вычетом боковых полей
/// (§7.3 auto-sizing). Без зажима строка мерялась по содержимому и вылезала
/// на сотни точек.
fn orthogonal_children(children: Vec<Node>, container: &Computed) -> Vec<Node> {
    let mut out = children;
    let inline_size = match container.height {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(false) {
            continue;
        }
        // Внепоточные не зажимаются: абсолютный элемент меряется от своего
        // содержащего блока, а не от потока (available-size-003: зажатый
        // абсолютный маркер вылезал красным).
        if !in_flow(&ch.style) {
            continue;
        }
        if let Some(il) = inline_size {
            for side in [
                &mut ch.style.margin.top,
                &mut ch.style.margin.right,
                &mut ch.style.margin.bottom,
                &mut ch.style.margin.left,
            ] {
                if let Some(Len::Pct(k)) = side {
                    *side = Some(Len::Px(*k * il));
                }
            }
        }
        if ch.style.width.is_none() && ch.style.max_width.is_none() {
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let margins = side(ch.style.margin.left) + side(ch.style.margin.right);
            ch.style.max_width = Some(match container.width {
                Some(Len::Px(w)) => Len::Px((w - margins).max(0.0)),
                _ => Len::Pct(1.0),
            });
        }
    }
    out
}

/// Зеркальный ортогональный случай: ВЕРТИКАЛЬНЫЙ блок внутри горизонтального
/// контейнера. Его строчная ось — высота, и авто-размер по ней зажимается
/// высотой контейнера за вычетом вертикальных полей (css-writing-modes-3
/// §7.3). Доля полей здесь обычная — от ширины контейнера, её решает
/// раскладка сама.
fn orthogonal_vertical_children(children: Vec<Node>, container: &Computed) -> Vec<Node> {
    let has_vertical = children.iter().any(|n| match n {
        Node::Element(ch) => !ch.inline && ch.style.vertical == Some(true),
        _ => false,
    });
    if !has_vertical {
        return children;
    }
    let mut out = children;
    for node in out.iter_mut() {
        let Node::Element(ch) = node else { continue };
        if ch.inline || ch.style.vertical != Some(true) {
            continue;
        }
        if !in_flow(&ch.style) {
            continue;
        }
        // Корень с vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
        // flow). Прижим самим стилем корня (align-self) — контейнеры-колонки
        // его уважают; для корня с ФОНОМ-КАРТИНКОЙ якорь ранее гасил
        // canvas-слой — тем страницам якорь не ставится (замерено).
        if matches!(ch.tag.as_str(), "html" | "body")
            && ch.style.vertical_rl == Some(true)
            && ch.style.align_self.is_none()
            && ch.style.bg_image.is_none()
        {
            ch.style.align_self = Some(crate::computed::Align::End);
        }
        if ch.style.height.is_some() || ch.style.max_height.is_some() {
            continue;
        }
        let margin = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(k)) => match container.width {
                Some(Len::Px(w)) => k * w,
                _ => 0.0,
            },
            _ => 0.0,
        };
        let margins = margin(ch.style.margin.top) + margin(ch.style.margin.bottom);
        ch.style.max_height = Some(match container.height {
            Some(Len::Px(h)) => Len::Px((h - margins).max(0.0)),
            _ => Len::Pct(1.0),
        });
    }
    out
}

fn collapse_flow_margins(children: Vec<Node>, reverse: bool) -> Vec<Node> {
    // Поле контейнера схлопывается С КРАЙНИМ flow-ребёнком через пустую
    // границу (CSS 2.1 §8.3.1): у `<body>` без рамки и паддинга хвостовое
    // поле — max(своё, block-end последнего ребёнка), рекурсивно. Без этого
    // `html::after` за body отъезжал на сумму полей (wm-propagation-body-042:
    // 16 у последнего `<p>` + 8 у body складывались вместо max).
    // Поглощение: поле крайнего ребёнка ОБНУЛЯЕТСЯ и уезжает на контейнер
    // (иначе оно распирало бы его коробку изнутри и зазор снаружи удваивался).
    fn absorb_margin(e: &mut Element, tail_side: bool, reverse: bool) -> f32 {
        let own = if tail_side == reverse {
            margin_px(e.style.margin.left, &e.style)
        } else {
            margin_px(e.style.margin.right, &e.style)
        }
        .unwrap_or(0.0);
        // Контейнер с ГОРИЗОНТАЛЬНЫМ письмом в вертикальном потоке —
        // ортогональный: его внутренний поток идёт по другой оси, и полей
        // на этой границе не отдаёт (available-size-020..023).
        if e.style.vertical == Some(false) {
            return own;
        }
        let b = e.style.borders();
        let (border, pad) = if tail_side == reverse {
            (b.left, e.style.padding.left)
        } else {
            (b.right, e.style.padding.right)
        };
        let sealed = margin_px(border, &e.style).unwrap_or(0.0) > 0.0
            || margin_px(pad, &e.style).unwrap_or(0.0) > 0.0;
        if sealed {
            return own;
        }
        let edge_child = {
            let mut it = e.children.iter_mut().filter_map(|n| match n {
                Node::Element(c)
                    if !matches!(
                        c.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    ) && c.style.display.is_none()
                        // Схлопка живёт в ОДНОМ потоке: ребёнок со своим
                        // письмом заводит другой и границу запечатывает.
                        && c.style.vertical.is_none()
                        && c.style.vertical_rl.is_none() =>
                {
                    Some(c)
                }
                _ => None,
            });
            if tail_side { it.last() } else { it.next() }
        };
        match edge_child {
            Some(c) => {
                let inner = absorb_margin(c, tail_side, reverse);
                // Поглощать есть что только при ненулевом внутреннем поле;
                // иначе стили НЕ переписываются: заморозка `Em` в точки
                // до разрешения кегля портила поле (`font-size: 5em` у
                // text-combine-upright-value-*).
                if inner <= 0.0 {
                    return own;
                }
                if tail_side == reverse {
                    c.style.margin.left = Some(Len::Px(0.0));
                } else {
                    c.style.margin.right = Some(Len::Px(0.0));
                }
                let total = own.max(inner);
                if tail_side == reverse {
                    e.style.margin.left = Some(Len::Px(total));
                } else {
                    e.style.margin.right = Some(Len::Px(total));
                }
                total
            }
            None => own,
        }
    }
    let mut out = children;
    let mut trailing: Option<f32> = None;
    for node in out.iter_mut() {
        let Node::Element(child) = node else { continue };
        // В обратном потоке ведущая сторона — правая. Ведущий край НЕ
        // поглощается: замерено — available-size-022/023 0.00 -> 2.66 при
        // нуле выигрышей; хватает хвостового (042/049/054).
        let lead = if reverse {
            child.style.margin.right
        } else {
            child.style.margin.left
        };
        // Доли кегля разрешаются здесь же: голый разбор точек считал `1em`
        // нулём и ЗАПИСЫВАЛ ноль — поле абзаца вдоль вертикального потока
        // пропадало вовсе (wm-propagation-body-*).
        let lead_px = margin_px(lead, &child.style).unwrap_or(0.0);
        if let Some(prev) = trailing {
            let kept = (lead_px - prev).max(0.0);
            if reverse {
                child.style.margin.right = Some(Len::Px(kept));
            } else {
                child.style.margin.left = Some(Len::Px(kept));
            }
        }
        trailing = Some(absorb_margin(child, true, reverse));
    }
    out
}

/// Блок вертикального письма занимает по горизонтали столько, сколько просит
/// содержимое, а не всю строку родителя.
///
/// Горизонтальная ось для него — ось ПОТОКА, а не строки: в Chrome контейнер
/// из трёх полос шириной 22 с отступами 16 занял 130 точек, а не всю ширину
/// окна. Блочная раскладка растягивает детей по ширине и слушать `align-self`
/// не обязана, поэтому обёртка-ряд: сам ряд занимает строку, а блок внутри
/// него жмётся к содержимому. Без этого колонки `vertical-rl` уезжали к
/// правому краю окна.
fn vertical_hug(el: AnyElement, e: &Element, inherited: &Computed) -> AnyElement {
    let starts_here = e.style.vertical == Some(true) && inherited.vertical != Some(true);
    if !starts_here || e.style.width.is_some() {
        return el;
    }
    // Письмо, заданное на `body` (или `html`), — ГЛАВНОЕ письмо страницы: оно
    // управляет окном целиком, и содержимое `vertical-rl` начинается от
    // правого края окна, а не от края сжатой коробки.
    if matches!(e.tag.as_str(), "body" | "html") {
        return el;
    }
    // ЗАМЕРЕНО (не гипотеза): по этим тестам размер по оси строки — не корень
    // зла. Пробовал и растяжение на высоту родителя, и свой элемент-измеритель
    // (`fit-content` с зажимом по доступному) — на двенадцати тестах
    // ортогональных потоков сдвиг в пределах полупроцента. Настоящая поломка
    // видна замером против Chrome на `sizing-orthogonal-percentage-margin-001`:
    // элемент рисуется ПОЛОСОЙ 25×417 у левого края страницы, а должен быть
    // 100×100 внутри контейнера с полями 50. То есть вертикальный блок уходит
    // из коробки родителя — вот что чинить дальше.
    div().flex().flex_row().child(el).into_any_element()
}

thread_local! {
    /// Глубина вложенности отложенной отрисовки на время построения дерева.
    ///
    /// GPUI запрещает откладывать рисование изнутри уже отложенного —
    /// `position: fixed` внутри `position: fixed` роняло окно
    /// (`cannot call defer_draw during deferred drawing`). Отложен только
    /// внешний слой, вложенные рисуются на месте: порядок наложения внутри
    /// одного слоя всё равно задаётся порядком разметки.
    static DEFERRED_DEPTH: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Строим ли мы сейчас поддерево отложенного элемента.
fn inside_deferred() -> bool {
    DEFERRED_DEPTH.with(|d| d.get()) > 0
}

/// Будет ли элемент с таким стилем отложен.
fn defers(c: &Computed) -> bool {
    matches!(
        c.position,
        Some(crate::computed::Position::Fixed) | Some(crate::computed::Position::Sticky)
    ) || c.z_index.is_some_and(|z| z > 0)
}

/// Счётчик глубины на время построения детей элемента.
struct DeferGuard(bool);

impl DeferGuard {
    fn enter(deferred: bool) -> Self {
        if deferred {
            DEFERRED_DEPTH.with(|d| d.set(d.get() + 1));
        }
        Self(deferred)
    }
}

impl Drop for DeferGuard {
    fn drop(&mut self) {
        if self.0 {
            DEFERRED_DEPTH.with(|d| d.set(d.get().saturating_sub(1)));
        }
    }
}

/// Текущая глубина — её запоминают поддеревья, которые строятся не сейчас.
fn defer_depth() -> usize {
    DEFERRED_DEPTH.with(|d| d.get())
}

/// Вернуть запомненную глубину на время отложенного построения поддерева.
///
/// Лента прокрутки, переход и ручка размера строят детей на ОТРИСОВКЕ, а не
/// при сборке дерева: к тому времени счётчик уже обнулён, и вложенный
/// `position: fixed` внутри прокручиваемого `position: fixed` снова просился
/// в отложенный слой — окно падало.
struct DepthScope(usize);

impl DepthScope {
    fn enter(depth: usize) -> Self {
        Self(DEFERRED_DEPTH.with(|d| d.replace(depth)))
    }
}

impl Drop for DepthScope {
    fn drop(&mut self) {
        DEFERRED_DEPTH.with(|d| d.set(self.0));
    }
}

/// `z-index`: порядок наложения.
///
/// Слоёв в GPUI нет, зато есть отложенная отрисовка с приоритетом — она и
/// задаёт, что рисуется поверх. Отрицательный `z-index` (под потоком) так не
/// выражается, поэтому применяем только положительный.
///
/// `allowed` — снаружи ли мы отложенного поддерева: внутри откладывать нельзя.
fn layered(el: AnyElement, c: &Computed, allowed: bool) -> AnyElement {
    if !allowed {
        // Внутри отложенного поддерева `position: fixed` отсчитывается от
        // ближайшего отложенного предка, а не от окна: своей системы
        // координат ему взять неоткуда.
        if c.position == Some(crate::computed::Position::Fixed) {
            return div()
                .absolute()
                .top_0()
                .left_0()
                .size_full()
                .child(el)
                .into_any_element();
        }
        return el;
    }
    // `position: fixed` — отсчёт от ОКНА: отложенная отрисовка выносит
    // элемент из потока родителя, а размер окна задаёт его систему координат.
    if c.position == Some(crate::computed::Position::Fixed) {
        let priority = c.z_index.unwrap_or(0).max(0) as usize;
        return gpui::deferred(div().absolute().top_0().left_0().size_full().child(el))
            .with_priority(priority)
            .into_any_element();
    }
    match c.z_index {
        Some(z) if z > 0 => gpui::deferred(el)
            .with_priority(z as usize)
            .into_any_element(),
        // ПРОБОВАЛИ И ОТКАТИЛИ: откладывать ЛЮБОЙ абсолютный элемент, чтобы
        // он рисовался поверх соседей (CSS 2.1 §9.9, шаг 8). На пробе помогло
        // — блок стал виден, — но на наборе обрушило всё: css-position 31 → 0,
        // css-text 967 → 428. Вложенная отложенная отрисовка в GPUI запрещена,
        // а абсолютные элементы вложены сплошь и рядом. Делать только с
        // проверкой глубины и по одному месту, а не общим правилом.
        _ => el,
    }
}

/// Обтекание: плавающий блок и следующие за ним встают в один ряд.
///
/// Своего обтекания в раскладке нет и быть не может — оно определено через
/// строчный контекст, которого taffy не знает. Но ровно то, ради чего его
/// пишут — «картинка слева, текст справа» — выражается рядом из двух колонок
/// точно. Отличие от браузера одно: текст не заворачивается ПОД плавающий
/// блок, когда тот кончился. `clear` закрывает ряд и начинает новый.
fn wrap_floats(nodes: Vec<Node>) -> Vec<Node> {
    let floated = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.float.is_some_and(|f| f != 0),
        Node::Text(_) => false,
    });
    if !floated {
        return nodes;
    }
    let mut out: Vec<Node> = vec![];
    let mut i = 0usize;
    while i < nodes.len() {
        let Node::Element(e) = &nodes[i] else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        let Some(side) = e.style.float.filter(|f| *f != 0) else {
            out.push(nodes[i].clone());
            i += 1;
            continue;
        };
        // Подряд идущие плавающие блоки стоят в ОДНОМ ряду, а не каждый в
        // своём: `float: left` у четырёх соседей выстраивает их бок о бок.
        // Прежде каждый начинал свой ряд, и они вставали столбиком.
        let mut floaters: Vec<Element> = vec![];
        let mut j = i;
        while j < nodes.len() {
            if is_blank(&nodes[j]) {
                j += 1;
                continue;
            }
            let Node::Element(next) = &nodes[j] else {
                break;
            };
            let Some(next_side) = next.style.float.filter(|f| *f != 0) else {
                break;
            };
            if next_side != side {
                break;
            }
            // `clear` у соседа обрывает ряд: он обязан начать свой. Так
            // написаны эталоны WPT — колонка из `float: right` + `clear: both`.
            if j > i && next.style.clear == Some(true) {
                break;
            }
            let mut floater = next.clone();
            floater.style.float = None;
            // ПРОБОВАЛИ И ОТКАТИЛИ: помечать плавающий кусок блочным
            // (`display: block` + `inline = false`), как велит CSS 2.1 §9.7.
            // Замер: css-text 1003 → 998, flexbox 318 → 319 — итог в минус.
            // Строчная природа картинки нужна ряду обтекания: как блок она
            // перестаёт участвовать в общей строке текста рядом с собой.
            // Плавающий блок не растягивается и не сжимается — он занимает
            // свою ширину, остальное достаётся соседям.
            floater.style.flex_shrink = Some(0.0);
            // Плавающий блок сжимается ДО СОДЕРЖИМОГО, но не шире доступного
            // места. Ключевым словом `fit-content` это писалось раньше, и
            // выходило дороже: слово заворачивает коробку в сетку, а дорожка
            // сетки не считает БОКОВЫЕ ПОЛЯ ребёнка — `margin: 1px` съедал два
            // пикселя ширины, текст переставал помещаться и рвался посреди
            // слова (`word-space-transform-010`, где эталон — 21 одинаковая
            // коробка). Поэтому коробке С ПОЛЯМИ ширина не задаётся вовсе, а
            // потолком служит родитель.
            //
            // Всем остальным остаётся `fit-content`: потолок в родителя не
            // равен ему по смыслу. В родителе НУЛЕВОЙ ширины он обнуляет
            // коробку, тогда как по CSS плавающая коробка не уже минимального
            // содержимого и просто вылезает наружу
            // (`white-space-intrinsic-size-001`).
            let side_margin = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)));
            if floater.style.width.is_none() {
                if side_margin(&floater.style.margin.left)
                    || side_margin(&floater.style.margin.right)
                {
                    floater.style.max_width = floater.style.max_width.or(Some(Len::Pct(1.0)));
                } else {
                    floater.style.width = Some(Len::FitContent);
                }
            }
            floaters.push(floater);
            j += 1;
        }
        // Соседи до ближайшего `clear` — они и обтекают.
        let mut rest: Vec<Node> = vec![];
        while j < nodes.len() {
            if let Node::Element(next) = &nodes[j]
                && (next.style.clear == Some(true) || next.style.float.is_some_and(|f| f != 0))
            {
                break;
            }
            rest.push(nodes[j].clone());
            j += 1;
        }
        // Плавающий блок, рядом с которым НЕЧЕМУ обтекать, рядом не нуждается:
        // он остаётся обычным блоком потока. Ряд в этом случае только вредил —
        // ширину внутри него раскладка мерила по самому узкому слову.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: распускать ряд, когда обтекать нечем, для
        // ЛЮБОГО числа плавающих (а не только одного) — css-text 1018 → 1019,
        // но flexbox 320 → **306**. Ряд соседних плавающих блоков нужен: без
        // него они встают столбиком.
        if rest.iter().all(is_blank) && floaters.len() == 1 {
            {
                let mut lone = floaters.remove(0);
                lone.style.flex_shrink = None;
            // Обтекать нечем — но сторону блок обязан держать: `float: right`
            // без соседей всё равно стоит У ПРАВОГО края. Ряда тут нет, и
            // сторону задаёт выравнивание себя в колонке родителя. Оно
            // действует только на ЭЛЕМЕНТ раскладки, поэтому строчный по
            // природе тег (картинка) здесь же делается блочным: иначе он
            // уходит в абзац, и выравнивание достаётся абзацу, а не ему.
                lone.style.align_self =
                    Some(if side < 0 { Align::Start } else { Align::End });
                out.push(Node::Element(lone));
            }
            out.extend(rest);
            i = j;
            continue;
        }
        let mut column = Element {
            node_id: 0,
            anim: None,
            tag: "div".into(),
            style: Computed {
                flex_grow: Some(1.0),
                ..Computed::default()
            },
            hover: None,
            first_letter: None,
            first_line: None,
            children: rest,
            attrs: vec![],
            inline: false,
        };
        column.style.display = Some(Display::Block);
        let mut row_children: Vec<Node> = vec![];
        if side < 0 {
            row_children.extend(floaters.into_iter().map(Node::Element));
            row_children.push(Node::Element(column));
        } else {
            row_children.push(Node::Element(column));
            // Прижатые вправо идут справа налево в порядке разметки.
            row_children.extend(floaters.into_iter().rev().map(Node::Element));
        }
        out.push(Node::Element(Element {
            node_id: 0,
            anim: None,
            // Метка для сборщика дерева: у ряда обтекания текст ещё режется
            // по нижнему краю плавающего блока (см. `float_flow`).
            tag: "kamin-float".into(),
            style: Computed {
                display: Some(Display::Flex),
                flex_dir: Some(FlexDir::Row),
                align_items: Some(Align::Start),
                // Плавающие блоки, которым не хватило ширины, уходят НИЖЕ
                // (CSS 2.1 §9.5.1): ряд обязан переносить.
                flex_wrap: Some(true),
                ..Computed::default()
            },
            hover: None,
            first_letter: None,
            first_line: None,
            children: row_children,
            attrs: vec![],
            inline: false,
        }));
        i = j;
    }
    out
}

/// Многоколоночный поток из сплошного текста.
///
/// Годится, когда всё содержимое — строчное: тогда режется сам текст. Если
/// внутри блоки, колонки набираются из них сеткой, как и раньше.
/// Начертание, которым НАБИРАЕТСЯ текст этого места.
///
/// Разрез на колонки и обтекание считают, сколько текста влезает в строку.
/// Меряли базовым шрифтом окна — и на любом документе со своей типографикой
/// (`body { font: 13px system-ui }`) разрез уезжал: мерилось одно, рисовалось
/// другое.
fn measure_font(c: &Computed, opts: &RenderOpts) -> gpui::Font {
    let mut font = opts.text.font();
    if let Some(family) = &c.font_family {
        font.family = crate::fonts::alias(family)
            .unwrap_or_else(|| family.clone())
            .into();
    } else if c.monospace == Some(true) {
        font.family = crate::metrics::mono_family().into();
    }
    if let Some(w) = c.font_weight {
        font.weight = gpui::FontWeight(w as f32);
    }
    if c.italic == Some(true) {
        font.style = gpui::FontStyle::Italic;
    }
    font
}

fn column_flow(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    count: usize,
) -> Option<AnyElement> {
    let all_inline = e.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(child) => child.inline && child.style.display.is_none(),
    });
    if !all_inline {
        // Один-единственный блок с текстом — это тот же поток, только в своей
        // коробке: колонки режут его строки, а не обходят стороной. Разметка
        // теста колонок почти всегда такая (`<div class=multicol><div>…`).
        let mut blocks = e.children.iter().filter(|n| !is_blank(n));
        let (Some(Node::Element(only)), None) = (blocks.next(), blocks.next()) else {
            return None;
        };
        if only.style.position.is_some() || only.style.float.is_some_and(|f| f != 0) {
            return None;
        }
        let inside = inline::inherit(inherited, &only.style);
        return column_flow(only, &inside, opts, count);
    }
    let mut plain = String::new();
    gather_text(&e.children, &mut plain);
    let plain = normalize_for_shadow(&plain).trim().to_string();
    if plain.is_empty() {
        return None;
    }
    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => size * k,
        _ => size * normal_fraction(inherited, opts),
    };
    let gap = match e.style.column_gap {
        Some(Len::Px(v)) => v,
        _ => size,
    };
    let nodes = e.children.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let depth = defer_depth();
    let build: std::rc::Rc<dyn Fn(&[usize], gpui::Pixels) -> AnyElement> =
        std::rc::Rc::new(move |cuts: &[usize], width: gpui::Pixels| {
            let _depth = DepthScope::enter(depth);
            // Куски текста по местам разрезов: каждый — своя колонка.
            let mut parts: Vec<Vec<Node>> = vec![];
            let mut rest = nodes.clone();
            let mut base = 0usize;
            for cut in cuts {
                let (head, tail) = split_nodes(&rest, cut.saturating_sub(base));
                parts.push(head);
                base = *cut;
                rest = tail;
            }
            parts.push(rest);
            let inner =
                (f32::from(width) - gap * (count.saturating_sub(1)) as f32) / count.max(1) as f32;
            let mut row = div().flex().flex_row().gap_x(px(gap)).w(width);
            for part in parts {
                row = row.child(div().w(px(inner)).flex().flex_col().children(blocks(
                    &part,
                    &inherited_owned,
                    &opts_owned,
                )));
            }
            row.into_any_element()
        });
    Some(
        crate::float::ColumnFlow::new(
            build,
            SharedString::from(plain),
            count,
            gap,
            measure_font(inherited, opts),
            size,
            line,
        )
        .into_any_element(),
    )
}

/// Ряд обтекания: рядом с плавающим блоком столько текста, сколько помещается
/// в его высоту, остальное — под ним на всю ширину.
///
/// Место разреза считает `FloatFlow`: ширина контейнера известна только на
/// замере, а без неё непонятно, сколько текста влезает сбоку. Если размеры
/// плавающего блока не заданы явно, резать нечем — тогда ряд остаётся прежним:
/// две колонки до конца абзаца.
fn float_flow(row: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Ряд без разреза — прежнее поведение: две колонки до конца абзаца.
    // Стиль берётся СЛИТЫЙ: у ряда своя раскладка, и без неё дети встают
    // друг под другом вместо колонок.
    let merged = inline::inherit(inherited, &row.style);
    let plain_row = |nodes: &[Node]| -> AnyElement {
        styled_div_with(row, &merged)
            .children(blocks(nodes, &merged, opts))
            .into_any_element()
    };
    // Плавающий блок в этой паре всегда первый, текстовая колонка — вторая.
    // Раньше здесь стоял `match`, у которого ОБЕ ветви давали `(0, 1)`:
    // условие вычислялось и ни на что не влияло.
    let (float_ix, text_ix) = (0, 1);
    let (Some(Node::Element(floater)), Some(Node::Element(column))) =
        (row.children.get(float_ix), row.children.get(text_ix))
    else {
        return plain_row(&row.children);
    };
    // Колонка текста — синтетическая, у плавающего блока её роли нет.
    let (floater, column) = if column.style.flex_grow == Some(1.0) {
        (floater, column)
    } else if let Some(Node::Element(other)) = row.children.get(1) {
        (other, floater)
    } else {
        return plain_row(&row.children);
    };
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let (Some(fw), Some(fh)) = (px_of(floater.style.width), px_of(floater.style.height)) else {
        return plain_row(&row.children);
    };
    let mut plain = String::new();
    gather_text(&column.children, &mut plain);
    if plain.trim().is_empty() {
        return plain_row(&row.children);
    }

    let left = matches!(row.children.first(), Some(Node::Element(e)) if std::ptr::eq(e, floater));
    let floater = floater.clone();
    let rest = column.children.clone();
    let column_style = column.style.clone();
    let row_node = row.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let depth = defer_depth();
    let build: crate::float::Split = std::rc::Rc::new(move |split: usize, width: gpui::Pixels| {
        let _depth = DepthScope::enter(depth);
        let (beside, below) = split_nodes(&rest, split);
        let mut side = column_style.clone();
        side.display = Some(Display::Block);
        let column_el = Element {
            node_id: 0,
            anim: None,
            tag: "div".into(),
            style: side,
            hover: None,
            first_letter: None,
            first_line: None,
            children: beside,
            attrs: vec![],
            inline: false,
        };
        let row_children = if left {
            vec![Node::Element(floater.clone()), Node::Element(column_el)]
        } else {
            vec![Node::Element(column_el), Node::Element(floater.clone())]
        };
        let mut top = row_node.clone();
        top.tag = "div".into();
        top.children = row_children;
        let mut all = vec![Node::Element(top)];
        all.extend(below);
        // Ширина коробки задаётся явно: дерево раскладывается отдельным
        // корнем, и без неё текст считает себя свободным и не переносится.
        div()
            .flex()
            .flex_col()
            .w(width)
            .children(blocks(&all, &inherited_owned, &opts_owned))
            .into_any_element()
    });

    let size = match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => size * k,
        _ => size * normal_fraction(inherited, opts),
    };
    crate::float::FloatFlow::new(
        build,
        SharedString::from(plain),
        (fw, fh),
        measure_font(inherited, opts),
        size,
        line,
    )
    .into_any_element()
}

/// Разрезать список узлов по смещению в их общем тексте.
///
/// Смещение считается по тому же тексту, что уходит в переносчик, поэтому
/// элементы режутся вместе с ним: `<b>` на границе разреза становится двумя.
fn split_nodes(nodes: &[Node], at: usize) -> (Vec<Node>, Vec<Node>) {
    let mut before = vec![];
    let mut after = vec![];
    let mut seen = 0usize;
    for node in nodes {
        if seen >= at {
            after.push(node.clone());
            continue;
        }
        match node {
            Node::Text(t) => {
                let len = t.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    // Режем по границе символа, ближайшей к месту разреза.
                    let mut cut = at - seen;
                    while cut < t.len() && !t.is_char_boundary(cut) {
                        cut += 1;
                    }
                    if cut > 0 {
                        before.push(Node::Text(t[..cut].to_string()));
                    }
                    if cut < t.len() {
                        after.push(Node::Text(t[cut..].to_string()));
                    }
                }
                seen += len;
            }
            Node::Element(e) => {
                let mut text = String::new();
                gather_text(&e.children, &mut text);
                let len = text.len();
                if seen + len <= at {
                    before.push(node.clone());
                } else {
                    let (head, tail) = split_nodes(&e.children, at - seen);
                    let mut a = e.clone();
                    a.children = head;
                    let mut b = e.clone();
                    b.children = tail;
                    before.push(Node::Element(a));
                    after.push(Node::Element(b));
                }
                seen += len;
            }
        }
    }
    (before, after)
}

/// Устойчивый номер абзаца для памяти выделения.
///
/// Абзац не элемент документа, своего номера у него нет; берём отпечаток его
/// текста — от кадра к кадру он не меняется, а разные абзацы почти всегда
/// различаются.
fn text_id(text: &str) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::collections::hash_map::DefaultHasher::new();
    text.hash(&mut h);
    h.finish()
}

/// Римская запись номера — для `list-style-type: lower-roman`.
fn roman(mut n: usize) -> String {
    const TABLE: &[(usize, &str)] = &[
        (1000, "m"),
        (900, "cm"),
        (500, "d"),
        (400, "cd"),
        (100, "c"),
        (90, "xc"),
        (50, "l"),
        (40, "xl"),
        (10, "x"),
        (9, "ix"),
        (5, "v"),
        (4, "iv"),
        (1, "i"),
    ];
    let mut out = String::new();
    for (value, sign) in TABLE {
        while n >= *value {
            out.push_str(sign);
            n -= value;
        }
    }
    out
}

/// Внешний отступ на обёртке: то же, что делает `apply`, но только поля.
fn apply_margin(d: gpui::Div, c: &Computed) -> gpui::Div {
    let mut d = d;
    for (val, side) in [
        (c.margin.top, 0u8),
        (c.margin.right, 1),
        (c.margin.bottom, 2),
        (c.margin.left, 3),
    ] {
        let Some(Len::Px(v)) = val else { continue };
        d = match side {
            0 => d.mt(px(v)),
            1 => d.mr(px(v)),
            2 => d.mb(px(v)),
            _ => d.ml(px(v)),
        };
    }
    d
}

/// Пробельный текстовый узел: в подсчёте детей он не участвует.
fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}

/// Порядок наложения внутри одного родителя.
///
/// Отрицательный `z-index` кладёт элемент ПОД поток: отложенной отрисовкой это
/// не выражается — она всегда рисует поверх. Зато порядок детей мы задаём
/// сами: такие элементы уходят в начало списка и рисуются раньше.
fn by_layer(mut nodes: Vec<Node>) -> Vec<Node> {
    // Элемент на статической позиции переставлять НЕЛЬЗЯ: место в потоке и
    // есть его координата. `z-index` меняет только порядок отрисовки, а
    // перестановка меняла и раскладку — абсолютный блок с `z-index: -1`
    // уезжал к началу родителя и накрывал собой абзац над собой.
    // Двигать можно только ВНЕПОТОЧНЫЙ элемент: у релятивного слот в потоке и
    // есть его координата, перестановка меняла раскладку всего родителя
    // (красная полоса `overlapped-red` уезжала в начало страницы). Релятивный
    // с отрицательным `z-index` остаётся на месте и рисуется подложкой.
    let movable = |e: &Element| {
        e.style.z_index.is_some_and(|z| z < 0)
            && matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            )
            && !at_static_position(&e.style)
    };
    let has_negative = nodes.iter().any(|n| match n {
        Node::Element(e) => movable(e),
        Node::Text(_) => false,
    });
    if !has_negative {
        return nodes;
    }
    nodes.sort_by_key(|n| match n {
        Node::Element(e) if movable(e) => e.style.z_index.unwrap_or(0).min(0),
        _ => 0,
    });
    nodes
}

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
fn collapse_margins(nodes: &[Node]) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    for node in out.iter_mut() {
        let Node::Element(e) = node else { continue };
        if e.inline {
            continue;
        }
        // Отступ не протекает наружу и через край блока с собственным
        // контекстом: прокрутка, обрезка, гибкая раскладка, сетка,
        // позиционирование. Раньше учитывались только рамка и внутренний
        // отступ, и содержимое прокручиваемой панели вставало на 6-8 точек
        // выше браузерного.
        let own_context = !matches!(
            e.style.overflow_y,
            None | Some(crate::computed::Overflow::Visible)
        ) || !matches!(
            e.style.overflow_x,
            None | Some(crate::computed::Overflow::Visible)
        ) || matches!(
            e.style.display,
            Some(Display::Flex)
                | Some(Display::InlineFlex)
                | Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::InlineBlock)
                | Some(Display::Table)
                | Some(Display::InlineTable)
        ) || matches!(
            e.style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) || e.style.float.is_some()
            || e.style.contain_paint == Some(true);
        // Отсечка по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и
        // `border: 0` схлопыванию не мешают (CSS 2.1 §8.3.1).
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
        let separated = !zero(e.style.padding.top) || !zero(e.style.borders().top) || own_context;
        if separated {
            continue;
        }
        // Именно ПЕРВЫЙ блок в потоке: отступ второго и последующих
        // схлопывается с соседом, а не выносится наружу.
        let child_top = e
            .children
            .iter()
            .find(|c| match c {
                Node::Element(ch) => !ch.inline,
                Node::Text(t) => !blank_text(t),
            })
            .and_then(|c| match c {
                Node::Element(ch) if in_flow(&ch.style) => {
                    margin_px(ch.style.margin.top, &ch.style).filter(|v| *v > 0.0)
                }
                _ => None,
            });
        if let Some(v) = child_top {
            let own = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
            e.style.margin.top = Some(Len::Px(own.max(v)));
            for c in e.children.iter_mut() {
                if let Node::Element(ch) = c
                    && !ch.inline
                    && in_flow(&ch.style)
                    && margin_px(ch.style.margin.top, &ch.style).is_some_and(|t| t > 0.0)
                {
                    ch.style.margin.top = Some(Len::Px(0.0));
                    break;
                }
            }
        }
        // То же СНИЗУ: отступ последнего ребёнка протекает наружу, если
        // родителя от него не отделяют ни рамка, ни внутренний отступ, ни
        // заданная высота. Иначе следующий за родителем блок отодвигался на
        // сумму двух отступов вместо большего из них
        // (`text-align-end-015`: вторая коробка стояла на 20 точек ниже).
        let closed = !zero(e.style.padding.bottom)
            || !zero(e.style.borders().bottom)
            || !matches!(e.style.height, None | Some(Len::Auto))
            || !zero(e.style.min_height)
            || own_context;
        if closed {
            continue;
        }
        let child_bottom = e
            .children
            .iter()
            .rev()
            .find(|c| match c {
                Node::Element(ch) => !ch.inline,
                Node::Text(t) => !blank_text(t),
            })
            .and_then(|c| match c {
                Node::Element(ch) if in_flow(&ch.style) => {
                    margin_px(ch.style.margin.bottom, &ch.style).filter(|v| *v > 0.0)
                }
                _ => None,
            });
        if let Some(v) = child_bottom {
            let own = margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0);
            e.style.margin.bottom = Some(Len::Px(own.max(v)));
            for c in e.children.iter_mut().rev() {
                if let Node::Element(ch) = c
                    && !ch.inline
                    && in_flow(&ch.style)
                    && margin_px(ch.style.margin.bottom, &ch.style).is_some_and(|t| t > 0.0)
                {
                    ch.style.margin.bottom = Some(Len::Px(0.0));
                    break;
                }
            }
        }
    }
    let mut prev_bottom: Option<f32> = None;
    for node in out.iter_mut() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу (поймано сравнением с
            // Chrome — документ уезжал вниз).
            if matches!(node, Node::Text(t) if blank_text(t)) {
                continue;
            }
            prev_bottom = None;
            continue;
        };
        // Отступы схлопываются только у блоков в обычном потоке: плавающий и
        // абсолютный не схлопываются ни с соседями, ни через себя — соседний
        // блок видит того, кто был ДО них. Раньше ряд из четырёх плавающих
        // блоков с полем в кегль терял поле у всех, кроме первого.
        // Плавающий и абсолютный цепочку не рвут: сосед видит того, кто был
        // ДО них. А вот СТРОЧНЫЙ элемент с содержимым порождает строчную
        // коробку, и отступы блоков через неё уже не примыкают
        // (CSS 2.1 §8.3.1).
        if e.inline {
            if !e.children.is_empty() {
                prev_bottom = None;
            }
            continue;
        }
        if !in_flow(&e.style) {
            continue;
        }
        let top = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
        if let Some(bottom) = prev_bottom {
            // Слитый отступ по CSS 2.1 §8.3.1 — это БОЛЬШИЙ из положительных
            // плюс МЕНЬШИЙ из отрицательных. Нижний отступ соседа раскладка уже
            // поставила, поэтому верхнему достаётся разница. Прежняя формула
            // `(top - bottom).max(0)` считала только положительный случай:
            // при `margin-bottom: -10px` соседи расходились на 30 вместо 10.
            let pos = bottom.max(0.0).max(top.max(0.0));
            let neg = bottom.min(0.0).min(top.min(0.0));
            e.style.margin.top = Some(Len::Px(pos + neg - bottom));
        }
        prev_bottom = Some(margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0));
    }
    out
}

/// Схлопываются ли отступы этого элемента с соседями и родителем.
///
/// Схлопывание — свойство БЛОЧНОГО потока. Не схлопываются: плавающий блок,
/// абсолютный и всё строчного уровня (`inline-block` и родня) — у них поля
/// стоят как написаны. Без этой проверки поле плавающего ребёнка «протекало»
/// наружу и поднимало родителя, а ряд строчных коробок терял поля у всех,
/// кроме первой.
fn in_flow(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
        && !matches!(
            c.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}

/// Отступ в точках для схлопывания.
///
/// Схлопывание идёт ДО каскада размеров шрифта, а разметка пишет `margin: 1em 0`
/// не реже, чем в точках: без перевода правило не срабатывало ни разу на таких
/// отступах, и блоки уезжали вниз на целый отступ. Кегль берётся свой, если
/// элемент его задал, иначе базовый — унаследованного здесь ещё нет.
/// Проценты не переводятся: они считаются от ширины родителя, а её тут никто
/// не знает, и выдуманное число было бы хуже пропуска.
fn margin_px(l: Option<Len>, style: &Computed) -> Option<f32> {
    let base = match style.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * base),
        _ => None,
    }
}

/// Схлопывание пробелов для тени — той же формы, что и в абзаце.
fn normalize_for_shadow(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len());
    let mut prev_space = false;
    for ch in raw.chars() {
        // Схлопываются только четыре знака CSS: идеографический и неразрывный
        // пробелы — обычные знаки со своей шириной (см. `inline.rs`).
        let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
        if is_space {
            if !prev_space {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
        prev_space = is_space;
    }
    out
}

/// Тень текста: та же строка под основной, размытая в своём буфере.
///
/// Тень в GPUI есть у коробки, у глифов — нет. Копия строки цветом тени
/// рисуется ПОД основной и уходит в отдельный буфер, где её размывает тот же
/// проход, что и `filter: blur`. Прежний обходной путь набирал размытие
/// четырьмя копиями по кругу: на близком расстоянии копии читались по
/// отдельности, а широкая тень не получалась вовсе.
fn text_shadow_layers(text: &str, sh: &crate::computed::Shadow) -> Vec<AnyElement> {
    let copy = div()
        .text_color(sh.color.to_hsla())
        .child(SharedString::from(text.to_string()))
        .into_any_element();
    // Смещение живёт на обёртке, а не внутри группы: у буфера группы размер
    // берётся из коробки ребёнка, и абсолютный ребёнок оставил бы её пустой —
    // размытая тень оказалась бы срезана маской композита.
    let placed = |child: AnyElement| {
        div()
            .absolute()
            .left(px(sh.x))
            .top(px(sh.y))
            .child(child)
            .into_any_element()
    };
    if sh.blur <= 0.5 {
        return vec![placed(copy)];
    }
    // Радиус тени в CSS — это диаметр размытия, то есть вдвое больше сигмы.
    let mut group = crate::interact::Grouped::new(copy);
    group.blur = sh.blur * 0.5;
    vec![placed(group.into_any_element())]
}

/// Абзац: одна строка текста с прогонами либо гибкая строка из кусков.
/// Абзац для тех, кто собирает текст сам — содержимое поля ввода.
pub fn paragraph_public(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    paragraph(nodes, inherited, opts)
}

fn paragraph(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Вертикальное письмо: строка идёт сверху вниз. Поворачивается только
    // текст — коробки блоков уже выстроены по горизонтальной оси потока.
    //
    // ПРОБОВАЛИ И ОТКАТИЛИ ТРИЖДЫ: отдать ось строки самому абзацу
    // (`Paragraph::vertical`, машинерия на месте и работает). Третий раз —
    // уже ПОСЛЕ того, как оси контейнеров стали логическими (гибкий ряд,
    // сетка, таблица), то есть предполагаемая причина двух прежних откатов
    // была снята. Всё равно минус: writing-modes 199 → 194, и ломается ровно
    // то, что абзац раньше чинил (`available-size-022/023` 0.00 → 9.15,
    // `three-levels-of-orthogonal-flows` 0.00 → 4.06), плюс всё семейство
    // `text-combine-upright-*`. Значит дело НЕ только в осях контейнеров:
    // ортогональный поток требует, чтобы родитель отдавал ребёнку
    // ограничение по своей ОСИ ПОТОКА, а не по физической высоте, — а это
    // ещё одна точка, в замере не найденная.
    // Вертикальное письмо: строка идёт сверху вниз. Поворачивается только
    // текст — коробки блоков уже выстроены по горизонтальной оси потока.
    //
    // Отдать ось строки самому абзацу (`Paragraph::vertical`) НЕЛЬЗЯ, пока в
    // нём нет вертикальной ОТРИСОВКИ. Флаг влияет только на замер: предел
    // переноса берётся по высоте. Сам проход рисования кладёт строку вдоль X
    // (`shape_line` в точку `origin.x + dx, y`) и шагает по Y — при
    // вертикальном письме строки от этого наступают друг на друга.
    // Проверено пробой `target/vt.html` (десять знаков в коробке 100px против
    // эталона с явным разрывом): одна плотная колонка вместо двух.
    // Прежний комментарий «машинерия на месте и работает» был неверен.
    // Порядок работ: сперва вертикальная отрисовка до совпадения на пробе,
    // потом включение. Ограничение §7.3 и логические оси уже сделаны.
    if inherited.vertical == Some(true) {
        // `text-orientation: upright`: глифы СТОЯТ и идут сверху вниз —
        // никакого поворота. Это обычный горизонтальный абзац шириной в один
        // кегль (продвижение стоячего глифа = кегль, §7.4) с резкой по
        // знакам: каждый знак — своя строка, стопка растёт вниз.
        // У `sideways-*` ориентация текста ИГНОРИРУЕТСЯ (css-writing-modes-4
        // §text-orientation): глифы всегда лежат, стопка не строится.
        // `text-orientation` наследуется и действует на ТЕКСТ (§4.1): когда
        // все куски абзаца несут `upright` сами (`html::after { upright }`),
        // стопка обязана строиться так же, как при флаге на контейнере.
        let kids_upright = !nodes.is_empty()
            && nodes.iter().all(|n| match n {
                Node::Element(e) => e.style.upright == Some(true),
                Node::Text(t) => t.trim().is_empty(),
            });
        let upright = inherited.upright == Some(true) || kids_upright;
        if upright && inherited.sideways != Some(true) {
            let mut stack = inherited.clone();
            stack.vertical = None;
            stack.break_word = Some(true);
            let em = match stack.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            // Каждый стоячий глиф продвигает строку РОВНО на кегль (§7.4):
            // шаг стопки — кегль, а не своя высота строки; полоса переноса
            // уже одного глифа — в строку ложится ровно один знак (два узких
            // нуля вставали рядом, и стопка выходила короче).
            stack.line_height = Some(Len::Px(em));
            // Толщина вертикальной строки — LINE-HEIGHT, как у горизонтальной
            // (стопка глифов стоит в полосе высоты строки, повернутой набок).
            let lane = match stack.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * em,
                // До этой точки `ch` мог не разрешиться: стоячий ноль
                // продвигается на кегль (§7.4) — считаем сами.
                Some(Len::Ch(k)) => k * em,
                _ => em,
            };
            let inner = paragraph(nodes, &stack, opts);
            return div()
                .w(px(lane.max(em)))
                .flex_shrink_0()
                .flex()
                .justify_center()
                .child(div().w(px(em * 0.9)).flex_shrink_0().child(inner))
                .into_any_element();
        }
        let mut horizontal = inherited.clone();
        horizontal.vertical = None;
        // Пометка для `text-combine-upright`: кускам внутри повёрнутого
        // абзаца нужен контр-поворот (см. atom-ветку ниже).
        horizontal.rotated_line = Some(true);
        // Поворот — приём отрисовки ТЕКСТА. Замещаемое содержимое (картинка,
        // элемент формы) вертикальное письмо не поворачивает никогда: абзац
        // из одной картинки обязан выглядеть так же, как в горизонтальном
        // письме (`wm-propagation-body-*`: подпись теста — рисунок, и он
        // ложился боком).
        let mut plain = String::new();
        gather_text(nodes, &mut plain);
        // Неразрывный пробел — ТЕКСТ: он даёт строку и её толщину
        // (`<td>&nbsp;</td>` в вертикальном ряду, ch-units-vrl-005), а
        // `trim()` съедал его как юникод-пробел, и абзац уходил
        // горизонтальным путём шириной в один пробел.
        if plain.trim().is_empty() && !plain.contains('\u{a0}') {
            return paragraph(nodes, &horizontal, opts);
        }
        let inner = paragraph(nodes, &horizontal, opts);
        // Спросить размер у родителя обход не может (замер внутри чужого
        // замера падает — см. `VerticalText::request_layout`). Зато предел
        // ортогонального потока уже принесён вниз стилем: задаём его ШИРИНОЙ
        // до поворота, и после поворота он становится высотой коробки — то
        // есть перенос считается по той оси, по которой идёт строка.
        let limit = inherited.ortho_limit.unwrap_or(opts.viewport.1);
        let inner = div().w(px(limit)).child(inner).into_any_element();
        // Высота заявляется только под ортогональным зажимом (max-height от
        // §7.3) и только при ПОЛНОМ зажиме — иначе коробка без высоты
        // схлопывалась в ноль (даже фон пропадал), а заявка без зажима
        // делала её бесконечной (замерено: wm 118 → 104).
        let vt = crate::interact::VerticalText::new(inner);
        let vt = if let Some(Len::Px(cap)) = inherited.max_height {
            vt.claiming_height(px(cap))
        } else {
            vt
        };
        return vt.into_any_element();
    }
    // Первая строка со своим стилем: где она кончается, известно только после
    // переноса, поэтому абзац собирается замером (см. `float::FirstLine`).
    if let Some(first) = inherited.first_line.clone() {
        let mut base = inherited.clone();
        base.first_line = None;
        let nodes_owned = nodes.to_vec();
        let opts_owned = opts.clone();
        let mut plain = String::new();
        gather_until_break(nodes, &mut plain);
        let plain = crate::inline::transform_case(&normalize_for_shadow(&plain), inherited);
        if !plain.trim().is_empty() {
            let size = match base.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * opts.base_size(),
                _ => opts.base_size(),
            };
            let line = match base.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Pct(k)) => size * k,
                _ => size * normal_fraction(&base, opts),
            };
            let for_build = first.clone();
            let depth = defer_depth();
            let build: crate::float::Split = std::rc::Rc::new(move |at, width| {
                let _depth = DepthScope::enter(depth);
                let mut styled = base.clone();
                styled.first_line = None;
                let mut para = paragraph_pieces(&nodes_owned, &styled, &opts_owned, at, &for_build);
                para = div().w(width).child(para).into_any_element();
                para
            });
            // Мерить надо ТЕМ начертанием, каким строка и будет набрана:
            // жирная первая строка занимает больше места, и разрез по
            // обычному шрифту не помещался бы в неё целиком.
            let mut font = opts.text.font();
            if let Some(w) = first.font_weight {
                font.weight = gpui::FontWeight(w as f32);
            }
            if first.italic == Some(true) {
                font.style = gpui::FontStyle::Italic;
            }
            if let Some(family) = &first.font_family {
                font.family = family.clone().into();
            }
            let measure_size = match first.font_size {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) => k * size,
                _ => size,
            };
            return crate::float::FirstLine::new(
                build,
                SharedString::from(plain.trim().to_string()),
                font,
                measure_size,
                line,
            )
            .into_any_element();
        }
    }
    paragraph_pieces(nodes, inherited, opts, 0, &Computed::default())
}

/// Свой кегль абзаца в точках — точка отсчёта для строки-опоры и для долей.
///
/// Отсчёт идёт от СВОЕГО кегля, а не от базового кегля документа: у коробки с
/// `font-size: 10px` строка обязана быть в 10 точек, а базовый (16) держал её
/// вдвое выше.
fn own_size(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) | Some(Len::Pct(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    }
}

/// Абзац с готовым разрезом первой строки: `at` — сколько байт в неё вошло.
fn paragraph_pieces(
    nodes: &[Node],
    inherited: &Computed,
    opts: &RenderOpts,
    first_line_at: usize,
    first_line: &Computed,
) -> AnyElement {
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: брать поперечное выравнивание ряда с самих
    // кусков, когда абзац своего не задал (`vertical-align: bottom` у
    // картинки). Ни это, ни `align-self` на самой картинке высоту строки не
    // меняют — строка всё равно выходит около 90 точек вместо 60, и картинка
    // просто прижимается к её низу. Дело не в выравнивании.
    let mut atom = |e: &Element| -> Option<inline::Piece> {
        // Абсолютный элемент на статической позиции ВНУТРИ строки — кусок вне
        // потока: место в строке он не занимает, поэтому абзац остаётся
        // текстовым и не теряет пробелы (`line-breaking-018`).
        // Только для обычного письма слева направо: место знака в строке
        // абзац отдаёт по ЛОГИЧЕСКОМУ порядку, а при RTL и вертикали оно
        // считается по-другому — там работает прежний обход через распорку.
        let plain_flow = inherited.rtl != Some(true) && inherited.vertical != Some(true);
        if plain_flow && at_static_position(&e.style) && inline_level(e) {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.position = None;
            let inner = styled_div_with(e, &merged)
                .children(blocks(&e.children, &merged, opts))
                .into_any_element();
            return Some(inline::Piece::Overlay(inner));
        }
        // Стоячая коробка в повёрнутом абзаце: `inline-block` с явным
        // ГОРИЗОНТАЛЬНЫМ письмом контр-поворачивается — его содержимое
        // обязано стоять прямо (эмуляция tcy в эталонах compression-*).
        if inherited.rotated_line == Some(true)
            && e.style.display == Some(Display::InlineBlock)
            && e.style.vertical == Some(false)
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.rotated_line = None;
            let em = match merged.width {
                Some(Len::Px(v)) => v,
                _ => match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                },
            };
            let inner = styled_div_with(e, &merged)
                .children(blocks(&e.children, &merged, opts))
                .into_any_element();
            return Some(inline::Piece::Atom(
                crate::interact::CombinedUpright::upright_box(inner, em).into_any_element(),
            ));
        }
        // `text-combine-upright` в повёрнутом абзаце: подходящий кусок
        // (цифры не длиннее N или любой при `all`) — атом-квадрат кегля с
        // контр-поворотом и ужатием (css-writing-modes-3 §9.1).
        if inherited.rotated_line == Some(true)
            && e.style.display.is_none()
            && let Some(n) = inline::inherit(inherited, &e.style).combine_upright
        {
            let mut plain = String::new();
            gather_text(&e.children, &mut plain);
            let text = plain.trim().to_string();
            let fits = !text.is_empty()
                && (n == 0
                    || (text.chars().all(|c| c.is_ascii_digit())
                        && text.chars().count() <= n as usize));
            if fits {
                let mut merged = inline::inherit(inherited, &e.style);
                merged.combine_upright = None;
                let em = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let inner = paragraph(&e.children, &merged, opts);
                return Some(inline::Piece::Atom(
                    crate::interact::CombinedUpright::new(inner, em).into_any_element(),
                ));
            }
        }
        atom_element(e, inherited, opts).map(|el| {
            // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
            // ряд строит базовую линию, а кускам с top/middle/bottom нужен
            // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
            // в строке с квадратом прижата к верху, у нас висела на базовой).
            use crate::computed::Align;
            let self_align = match e.style.vertical_align {
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            let el = match self_align {
                Some(a) => {
                    let mut w = div().flex_shrink_0();
                    w.style().align_self = Some(a);
                    w.child(el).into_any_element()
                }
                None => el,
            };
            inline::Piece::Atom(el)
        })
    };
    let mut pieces = inline::collect(nodes, inherited, &mut atom);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    // Точка переноса показывается знаком по СОСЕДЯМ, а они лежат в других
    // кусках — проход идёт по всему абзацу сразу.
    // Слогораздел идёт ПЕРВЫМ: он меняет сам текст кусков, а всё дальше
    // считает по готовому тексту байтовые смещения.
    inline::hyphenate_pieces(&mut pieces);
    inline::space_transform_pieces(&mut pieces);
    let mut pieces = pieces;
    inline::trim_edge_spaces(&mut pieces);
    // Свой `unicode-bidi` у самого абзаца знаками не обрамлялся: их ставит
    // сборка КУСКОВ, а корень абзаца куском не бывает. Из-за этого
    // `bidi-override` на блоке не действовал вовсе (`pre-wrap-align-*-003`:
    // строки шли в исходном порядке вместо перевёрнутого).
    // Только ОТМЕНА и ИЗОЛЯЦИЯ: своё направление письма абзац и так знает —
    // оно уходит в основной уровень разбора двунаправленности.
    let own_bidi = inherited.bidi_override == Some(true) || inherited.bidi_isolate == Some(true);
    let marks = if own_bidi {
        inline::bidi_marks(inherited, inherited)
    } else {
        (None, None)
    };
    if let (Some(open), Some(close)) = marks {
        pieces.insert(
            0,
            inline::Piece::Text {
                text: open.to_string(),
                style: inherited.clone(),
            },
        );
        pieces.push(inline::Piece::Text {
            text: close.to_string(),
            style: inherited.clone(),
        });
        // Жёсткий разрыв ЗАКАНЧИВАЕТ абзац разбора двунаправленности, и знак
        // отмены за ним уже не действует: его приходится ставить заново на
        // каждой строке (`pre-wrap-align-*-003`: перевёрнутой выходила только
        // первая строка).
        let again = format!("{close}\n{open}");
        for piece in pieces.iter_mut() {
            if let inline::Piece::Text { text, .. } = piece
                && text.contains('\n')
            {
                *text = text.replace('\n', &again);
            }
        }
    }
    // Буквица: первая буква абзаца — свой кусок со своим стилем. Кегль куска
    // доезжает до прогона (патч GPUI), поэтому она может быть крупнее строки.
    if let Some(first) = inherited.first_letter.as_deref() {
        pieces = inline::split_first_letter(pieces, first);
    }
    if first_line_at > 0 {
        pieces = inline::style_first_line(pieces, first_line_at, first_line);
    }
    // Межсловный интервал и отступ первой строки требуют строки из слов, а
    // единый текстовый блок их не умеет — поэтому решение принимается ДО
    // сборки блока, иначе оба свойства молча пропадали.
    let word = match inherited.word_spacing {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Отступ первой строки. Абсолютную часть разбор уже свёл к точкам, доля
    // же берётся от ширины содержащего блока и здесь ещё неизвестна — её
    // считает раскладка строк, когда ширина решена.
    let indent = crate::lines::Indent {
        px: match inherited.text_indent {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        },
        pct: match inherited.text_indent {
            Some(Len::Pct(k)) => k,
            _ => 0.0,
        },
        each_line: inherited.text_indent_each_line == Some(true),
        hanging: inherited.text_indent_hanging == Some(true),
    };
    // Межсловный интервал считает своя раскладка строк: ряд из слов ломает
    // выключку, висящие пробелы и перенос. Ряд остаётся только под отступ
    // первой строки, который своей раскладке пока неизвестен.
    // Отступ первой строки умеет своя раскладка строк: она одна знает, где
    // строка кончается, и отрицательный отступ ей не помеха. Ряд из слов
    // остаётся запасным путём — на нём отступ становится распоркой, а она
    // отрицательной ширины не бывает.
    let spaced = indent != crate::lines::Indent::default() && crate::lines::rules(inherited).is_none();
    if !spaced
        && inline::single_block(&pieces, opts.base_size())
        && let Some((text, runs)) = inline::text_and_runs(&pieces, &opts.text)
    {
        // `word-space-transform` смотрит на СОСЕДЕЙ точки переноса, а они
        // сплошь и рядом лежат в других кусках (`あ<wbr>い` — это три куска:
        // текст, точка, текст). По кускам преобразование их не видит, поэтому
        // идёт по собранному тексту абзаца. Замена знак-в-знак: нулевой
        // пробел и идеографический занимают в UTF-8 одинаково, поэтому
        // прогоны не съезжают.
        // Строка растёт под самый крупный кусок — как коробка строки в CSS.
        // Иначе крупный `<span>` вылезал бы на соседние строки.
        let biggest = inline::max_font_size(&pieces, own_size(inherited, opts), opts.base_size());
        let mut opts = opts.clone();
        if biggest != opts.base_size() {
            opts.text.line_height = gpui::px(biggest * normal_fraction(inherited, &opts)).into();
        }
        let opts = &opts;
        // `user-select: none` — абзац рисуется обычным текстом: ни области
        // попадания, ни обработчиков мыши он тогда не создаёт.
        // `pointer-events: none` снимает и выделение: элемент не должен
        // ловить курсор ничем.
        if inherited.no_select == Some(true) || inherited.pointer_events_none == Some(true) {
            return gpui::StyledText::new(SharedString::from(text))
                .with_runs(runs)
                .into_any_element();
        }
        // Своя строчная раскладка нужна там, где ширина строки и точка
        // разрыва связаны: висящие пробелы, `break-spaces`, разрыв где угодно.
        // В остальных случаях остаётся выделяемый текст движка — он умеет
        // выделение мышью, а своя раскладка пока нет.
        if let Some(wrap) = crate::lines::rules(inherited) {
            // Кегль абзаца — самый крупный кусок в нём: строка растёт под него,
            // и от него же считается высота строки в долях.
            let line = match inherited.line_height {
                Some(Len::Px(v)) => gpui::px(v),
                Some(Len::Pct(k)) => gpui::px(k * biggest),
                Some(Len::Em(k)) => gpui::px(k * biggest),
                _ => gpui::px(biggest * normal_fraction(inherited, opts)),
            };
            if std::env::var("PLAIN_DBG").is_ok() && inherited.preserve_newlines == Some(true) {
                eprintln!("PLAIN pre text={:?}", text);
            }
            let id = gpui::ElementId::Integer(text_id(&text));
            let family = inherited.font_family.clone().unwrap_or_default();
            let para = crate::lines::Paragraph::new(
                SharedString::from(text),
                runs,
                gpui::px(biggest),
                line,
                crate::lines::align_for(inherited),
                wrap,
            )
            // Правила переноса вложенных кусков: `word-break` на `<span>`
            // действует только на его знаки, а не на абзац целиком.
            // `unicode-bidi: plaintext` (в том числе `dir="auto"`): сторона
            // письма и логическая выключка решаются построчно.
            .plaintext(
                inherited
                    .bidi_plaintext
                    .unwrap_or(false)
                    .then(|| inherited.text_align.unwrap_or(crate::computed::TextAlign::Start))
                    .filter(|a| {
                        matches!(
                            a,
                            crate::computed::TextAlign::Start | crate::computed::TextAlign::End
                        )
                    }),
            )
            .spans(inline::wrap_spans(&pieces, inherited))
            .word_spans(inline::word_spans(&pieces, biggest))
            // Автозазоры идут ПЕРВЫМИ: поиск диапазона берёт первое
            // попадание, и зазор обязан перебить трекинг всего куска.
            .letter_spans(
                [
                    inline::autospace_spans(&pieces, biggest),
                    inline::letter_spans(&pieces, biggest),
                ]
                .concat(),
            )
            .shift_spans(inline::shift_spans(&pieces, biggest))
            .align_last(
                inherited
                    .text_align_last
                    .map(|a| a.physical(inherited.rtl == Some(true)))
                    .map(crate::lines::align_of_value),
            )
            .letter_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.letter_spacing,
                &family,
                biggest,
            )))
            .word_spacing(gpui::px(crate::metrics::spacing_px(
                inherited.word_spacing,
                &family,
                biggest,
            )))
            .hanging(inherited.hanging)
            .indent(indent)
            .spacers(inline::spacers(&pieces))
            .line_clamp(inherited.clamp_lines().map(|n| n as usize))
            .text_ellipsis(
                inherited.ellipsis == Some(true)
                    && inherited
                        .overflow_x
                        .is_some_and(|o| o != crate::computed::Overflow::Visible),
            )
            .text_fit(inherited.text_fit)
            .hyphen_char(inherited.hyphen_char.clone())
            .tab_stop(gpui::px(match inherited.tab_size_len {
                // Длина задаёт шаг НАПРЯМУЮ, ширина знака к ней не примешана.
                Some(Len::Px(v)) if v > 0.0 => v,
                _ => {
                    inherited.tab_size.unwrap_or(8).max(1) as f32
                        * crate::metrics::ch_ex_px(&family, biggest).0
                }
            }))
            .overlays(inline::overlays(pieces))
            .selectable(id, opts.selection_color());
            return para.into_any_element();
        }
        let id = gpui::ElementId::Integer(text_id(&text));
        return crate::select::Selectable::new(
            id,
            SharedString::from(text),
            runs,
            opts.selection_color(),
        )
        .into_any_element();
    }
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        // На кусок текста идут ТОЛЬКО текстовые свойства: фон, отступы и
        // рамка принадлежат абзацу целиком, а не каждому его слову.
        apply(div(), &style.text_only())
            .child(SharedString::from(t))
            .into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = inherited
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(inherited.rtl == Some(true));
    if spaced {
        return inline::as_word_row(
            pieces,
            word,
            indent.px,
            inherited.vertical_align,
            Some(align),
            &mut render_text,
        );
    }
    inline::as_wrapped_row(pieces, inherited.vertical_align, Some(align), &mut render_text)
}

/// Обернуть готовый абзац тенью текста, если она задана.
fn with_text_shadow(el: AnyElement, style: &Computed, nodes: &[Node]) -> AnyElement {
    let Some(sh) = style.text_shadow else {
        return el;
    };
    let mut plain = String::new();
    gather_text(nodes, &mut plain);
    // Тень повторяет ТУ ЖЕ строку, что и абзац: пробелы схлопнуты, регистр
    // изменён. Иначе тень к `text-transform: uppercase` осталась бы строчной.
    let plain = crate::inline::transform_case(&normalize_for_shadow(&plain), style);
    if plain.trim().is_empty() {
        return el;
    }
    div()
        .relative()
        .children(text_shadow_layers(plain.trim(), &sh))
        .child(el)
        .into_any_element()
}

/// Строчный ли элемент по своему `display`.
fn inline_level(e: &Element) -> bool {
    match e.style.display {
        Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid) => true,
        Some(_) => false,
        None => e.inline,
    }
}

/// Не-текстовые инлайн-элементы, которые в поток встроить нельзя.
fn atom_element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    // Элементу формы нужен СЛИТЫЙ стиль: в своём у него единицы шрифта ещё не
    // разрешены (`width: 3ch` считался бы по базовому кеглю, а не по своему),
    // да и наследуемое до поля иначе не доходит.
    if let Some(el) = crate::forms::element(e, &inline::inherit(inherited, &e.style), opts) {
        return Some(el);
    }
    // Абсолютный элемент без заданных краёв стоит на СТАТИЧЕСКОЙ позиции — там,
    // где он оказался бы в потоке. Внутри строки это место знает только сама
    // строка, поэтому в неё встаёт пустышка нулевого размера, а элемент висит
    // от её угла. Без этого раскладка ставила его в угол ближайшего
    // позиционированного предка, и текст уезжал в начало абзаца.
    if at_static_position(&e.style) {
        let mut merged = inline::inherit(inherited, &e.style);
        // Позиционирование с внутренней коробки СНИМАЕТСЯ. Содержащим блоком
        // ей стала бы нулевая пустышка, а ширина у неё «по содержимому» —
        // в нулевом блоке это ноль, и элемент пропадал вовсе. Без
        // позиционирования она меряется своим содержимым и висит от угла
        // пустышки, то есть ровно от статической позиции.
        merged.position = None;
        // Замещаемый элемент строит своя ветка: голая коробка со стилем
        // теряла содержимое (сломанная картинка с alt-подписью пропадала,
        // `abs-pos-vlr-border-001`). Позиция снимается копией — та же
        // причина, что и у merged ниже.
        let replaced: Option<AnyElement> = match e.tag.as_str() {
            "img" | "svg" => {
                let mut copy = e.clone();
                copy.style.position = None;
                Some(match copy.tag.as_str() {
                    "svg" => crate::svg::element(&copy).unwrap_or_else(|| image(&copy)),
                    _ => image(&copy),
                })
            }
            _ => None,
        };
        let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
        // Пустышка прижимается к ВЕРХУ строки: иначе она садится на базовую
        // линию, и содержимое уезжает под неё — абсолютный блок оказывался
        // ниже своей строки, а не на её месте (видно на `static-position`:
        // зелёный блок висел под коробкой, красное проступало).
        // Рисуется элемент ПОВЕРХ соседей по строке, поэтому содержимое
        // уходит в верхний слой блока-контейнера, а в строке остаётся щуп: он
        // и держит место, и сообщает, куда потом вернуть содержимое.
        let spot: crate::interact::SpotCell = Default::default();
        let below = e.style.z_index.is_some_and(|z| z < 0);
        // Блочный элемент встал бы на НОВУЮ строку — там его статическая
        // позиция и находится: левый край содержимого родителя, верх — низ
        // текущей строки. Строчный остаётся точкой в самой строке.
        let inline_level = match e.style.display {
            Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable) => true,
            Some(_) => false,
            None => e.inline,
        };
        // Флаги направления нужны и СТРОЧНОМУ атому: в rtl-строке статическая
        // позиция — правый край, заместитель вешается правым краем на точку
        // распорки. Начало новой строки — только у блочного.
        spot.set(crate::interact::Spot {
            hole: None,
            next_line: (!inline_level).then(|| line_height_px(inherited, opts)),
            rtl: inherited.rtl == Some(true),
            vertical: inherited.vertical == Some(true),
            vertical_rl: inherited.vertical_rl == Some(true),
            ..Default::default()
        });
        let probe = crate::interact::spot_probe(spot.clone(), false);
        let inner = match replaced {
            Some(el) => el,
            None => inner.into_any_element(),
        };
        let taken = if below {
            Some(inner)
        } else {
            crate::interact::late_push(spot, inner)
        };
        return match taken {
            None => Some(probe),
            Some(kept) => {
                let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                Some(hole.child(kept).into_any_element())
            }
        };
    }
    // Абсолютный элемент С заданными краями считается от ближайшего
    // позиционированного предка, а не от строки. Места в строке он не
    // занимает вовсе — потому и пустышка нулевая, и БЕЗ `relative`: иначе
    // содержащим блоком стала бы она сама, и края отсчитывались бы от неё.
    // Пока он был обычной коробкой куска, строка росла под его высоту.
    if matches!(
        e.style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        let merged = inline::inherit(inherited, &e.style);
        // ПРОБОВАЛИ И ОТКАТИЛИ: отдавать элемент без пустышки, чтобы `inset: 0`
        // считался от позиционированного ПРЕДКА. В раскладке под нами
        // содержащим блоком служит ЛЮБОЙ родитель, поэтому вынос ничего не
        // меняет: css-position 35 → 34, зелёный прямоугольник
        // `position-absolute-in-inline-005` так и не появился.
        // Пустышка нулевого размера сама становится содержащим блоком, и
        // края, заданные с ОБЕИХ сторон оси, схлопываются в ничто. Такому
        // элементу коробка нужна настоящая, поэтому он идёт без пустышки.
        // Остальным она нужна: без неё сдвигаются соседи (замерено на
        // `position-sticky-contained-by-display-table`).
        let stretched = (e.style.inset.left.is_some() && e.style.inset.right.is_some())
            || (e.style.inset.top.is_some() && e.style.inset.bottom.is_some());
        let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
        if stretched {
            return Some(inner.into_any_element());
        }
        return Some(
            div()
                .w_0()
                .h_0()
                .flex_shrink_0()
                .child(inner)
                .into_any_element(),
        );
    }
    // Таблица в строке — атомарная коробка со своей табличной раскладкой:
    // путь блока строил бы детей-ряды как обычные блоки, без решётки.
    if e.style.display == Some(Display::InlineTable) {
        let built = table(e, &inline::inherit(inherited, &e.style), opts);
        // `vertical-align` коробки в строке: низ/верх/середина СТРОКИ, а не
        // базовая линия. Строка — гибкий ряд, и место коробки задаёт её
        // собственный `align-self`.
        let self_align = match e.style.vertical_align {
            Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
            Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
            Some(Align::Center) => Some(gpui::AlignItems::Center),
            _ => None,
        };
        if let Some(a) = self_align {
            let mut wrap = div().flex_shrink_0();
            wrap.style().align_self = Some(a);
            return Some(wrap.child(built).into_any_element());
        }
        return Some(built);
    }
    match e.tag.as_str() {
        "img" => Some(image_with(e, Some(atom_base_font(inherited, opts)))),
        "iframe" => {
            if let Some(el) = iframe(e, opts) {
                return Some(el);
            }
            None
        }
        "svg" => crate::svg::element(e)
            .or_else(|| Some(image_with(e, Some(atom_base_font(inherited, opts))))),
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(&e.style) => {
            let merged = inline::inherit(inherited, &e.style);
            let mut box_ = styled_div_with(e, &merged);
            // Строчная коробка БЕЗ содержимого всё равно высотой в строку:
            // рамка и фон рисуются по кеглю, а не по тексту. Без этого
            // `<span style="border-left:30px solid green">  </span>` выходил
            // нулевой высоты и не рисовался вовсе
            // (`line-edge-white-space-collapse-001`).
            if merged.height.is_none() && !has_text(&e.children) {
                box_ = box_.h(px(line_height_px(&merged, opts)));
            }
            // `vertical-align` коробки в строке: верх/низ/середина СТРОКИ
            // (CSS 2.1 §10.8.1) — как у строчной таблицы выше. Без этого
            // `inline-block` с `vertical-align: top` сидел на базовой линии
            // и в высокой строке уезжал вниз.
            let self_align = match e.style.vertical_align {
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            if let Some(a) = self_align {
                box_.style().align_self = Some(a);
            }
            Some(
                box_.children(blocks(&e.children, &merged, opts))
                    .into_any_element(),
            )
        }
        _ => None,
    }
}

/// Размер по содержимому (`width: min-content`/`max-content`).
///
/// У коробки такого размера раскладка под нами не знает — зато знает такую
/// ДОРОЖКУ СЕТКИ. Элемент заворачивается в сетку из одной дорожки нужного
/// вида: ширину она посчитает по содержимому и отдаст элементу. Обёртка
/// прижата к началу строки, иначе сетка растянула бы её саму на всю ширину
/// родителя и смысл потерялся.
/// Завернёт ли `content_sized` этот элемент в свою обёртку.
///
/// Отдельный предикат нужен вызывающей стороне: она обязана снять с элемента
/// боковые поля ДО сборки — обёртка их не пропускает.
fn content_sized_wraps(c: &Computed) -> bool {
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    (keyword(c.width) || keyword(c.height))
        && !matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
}

fn content_sized(el: AnyElement, c: &Computed) -> AnyElement {
    let track = |l: Option<Len>| match l {
        Some(Len::MinContent) => Some(gpui::GridTrack::MinContent),
        Some(Len::MaxContent) => Some(gpui::GridTrack::MaxContent),
        // `fit-content` — дорожка `auto`: она и есть «по содержимому, но не
        // шире доступного».
        Some(Len::FitContent) => Some(gpui::GridTrack::Auto),
        _ => None,
    };
    let (col, row) = (track(c.width), track(c.height));
    if col.is_none() && row.is_none() {
        return el;
    }
    // Позиционированный элемент заворачивать нельзя: обёртка стала бы его
    // содержащим блоком, и края отсчитывались бы от неё. Он и так не
    // растягивается — размер по содержимому получается сам.
    if matches!(
        c.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) {
        return el;
    }
    let mut wrap = div().grid();
    // Боковые поля сняты с элемента вызывающей стороной (дорожка сетки их не
    // считает) — здесь они ставятся на саму обёртку, без лишней коробки:
    // отдельный держатель менял раскладку соседей
    // (`text-transform-fullwidth-008`).
    let side = |l: Option<Len>| -> Option<f32> {
        let size = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let family = c.font_family.clone().unwrap_or_default();
        match l {
            Some(Len::Px(_) | Len::Em(_) | Len::Ch(_) | Len::Ex(_)) => {
                let v = crate::metrics::spacing_px(l, &family, size);
                (v < 0.0).then_some(v)
            }
            _ => None,
        }
    };
    if let Some(v) = side(c.margin.left) {
        wrap = wrap.ml(px(v));
    }
    if let Some(v) = side(c.margin.right) {
        wrap = wrap.mr(px(v));
    }
    // ★ ЗАМЕРЕНО ДВАЖДЫ И ОТКАЧЕНО: растягивать элемент на дорожку
    // (`justify_items: Stretch`) — всем подряд css-text 1027 → 1023, только
    // под `max-content` 1027 → 1026. Чинит `pre-wrap-017` (коробка шириной в
    // дорожку), ломает `white-space-intrinsic-size-024/025` (обводка обязана
    // облегать глифы). Значит дорожка местами шире содержимого, и сперва надо
    // разобраться с НЕЙ, а не с выравниванием в ней.
    wrap.style().justify_items = Some(gpui::AlignItems::FlexStart);
    if let Some(col) = col {
        wrap = wrap.grid_template_cols(vec![col]);
    }
    if let Some(row) = row {
        wrap = wrap.grid_template_rows(vec![row]);
    }
    wrap.child(el).into_any_element()
}

/// Абсолютный элемент, которому не задан ни один край.
///
/// Такой элемент по CSS остаётся на статической позиции — той, что была бы у
/// него в обычном потоке. Как только задан хотя бы один край, отсчёт идёт от
/// содержащего блока, и пустышка в строке уже не нужна.
fn at_static_position(c: &Computed) -> bool {
    // Доля считается от СОДЕРЖАЩЕГО БЛОКА, а пустышка нулевая: элемент с
    // `height: 100%` внутри неё схлопнулся бы в ноль. Такому оставляем прежнее
    // размещение — размер важнее точки отсчёта, его видно всегда.
    let relative_size = matches!(c.width, Some(Len::Pct(_)))
        || matches!(c.height, Some(Len::Pct(_)))
        || matches!(c.min_width, Some(Len::Pct(_)))
        || matches!(c.min_height, Some(Len::Pct(_)))
        || matches!(c.max_width, Some(Len::Pct(_)))
        || matches!(c.max_height, Some(Len::Pct(_)));
    // Только `absolute`: у `fixed` содержащий блок — окно, и слой ему строит
    // сборщик дерева; пустышка в потоке ломала бы этот слой.
    c.position == Some(crate::computed::Position::Absolute)
        && !relative_size
        && c.inset.top.is_none()
        && c.inset.right.is_none()
        && c.inset.bottom.is_none()
        && c.inset.left.is_none()
}

/// Есть ли у инлайнового куска собственная коробка.
///
/// Прогон текста не умеет рисовать вокруг себя ничего: ни рамку, ни тень, ни
/// отступ. Раньше проверялись только верх и лево, поэтому `padding-right`,
/// боковая рамка, тень, прозрачность и три угла из четырёх у `<span>` молча
/// пропадали. У настоящего `display: inline` размеры не проверяются: CSS их
/// такому элементу и не даёт.
fn has_own_box(c: &Computed) -> bool {
    // Нулевая величина коробки не создаёт: `padding: 0` и `border: 0` пишут
    // в стиль ноль, и по одному лишь «задано» кусок вынимался из строки —
    // а вынутый кусок рвёт соединение букв и общий перенос по словам.
    let set = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let any =
        |s: &crate::computed::Sides| set(&s.top) || set(&s.right) || set(&s.bottom) || set(&s.left);
    // Атомарная строчная коробка — коробка по определению: у неё свои ширина,
    // высота и вертикальные поля, а прогон текста не умеет ни одного из трёх.
    // Подсветка прогона её не заменяет: `display: inline-block; width: 10em;
    // height: 6em` — это прямоугольник, а не слово с фоном. Признаком служит
    // заданный размер: у настоящего `display: inline` его не бывает.
    //
    // Вертикальные поля признаком НЕ служат: строку они не двигают (замерено
    // на `flexbox_inline`, где `margin-top: -20em` обязан пройти впустую), и
    // по ним коробка заводилась бы только затем, чтобы уехать за экран.
    let atomic = matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && (c.width.is_some() || c.height.is_some());
    // Позиционированный кусок — тем же порядком: его коробку двигают края, а
    // краёв у прогона нет.
    // ПРОБОВАЛИ И ОТКАТИЛИ: считать коробкой и `position: relative`, чтобы
    // относительный `<span>` служил содержащим блоком абсолютным потомкам
    // (по CSS это так). Выигрыш нулевой во всех разделах, css-grid 389 → 386.
    // Возвращать вместе с настоящей коробкой строчного фрагмента.
    let positioned = matches!(
        c.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    );
    if atomic || positioned {
        return true;
    }
    // Сплошной фон коробки не требует: его несёт прогон текста, и тогда
    // подсветка переносится вместе со строкой. Раньше `<span>` с фоном
    // становился отдельным блоком, и его слова вставали столбиком.
    // ПРОЗРАЧНАЯ рамка не рисует ничего, поэтому и коробки не требует: место
    // под неё держит знак-распорка строки. Пока `border: solid transparent`
    // заводил коробку, `<span>` с такой рамкой рвал абзац на части
    // (`word-space-transform-010`, где рамка ровно для того и прозрачная,
    // чтобы проверить одни только отступы).
    // РОВНУЮ рамку рисует прогон текста по кускам строк, коробка ей не нужна
    // (`inline::uniform_border`): в коробке текст перестаёт переноситься
    // вместе с абзацем, и `<span>` с рамкой уезжал одной строкой за край
    // (`hanging-punctuation-inline-bound-001`).
    // …и только у СТРОЧНОГО уровня: у блока рамка принадлежит его коробке, и
    // без неё он теряет и фон, и поля (`flexbox_first-line`: `li` с рамкой в
    // 1px разъезжался на четверть страницы).
    let inline_level = c.display.is_none();
    if inline_level
        && (crate::inline::uniform_border(c).is_some()
            || crate::inline::sided_border(c).is_some())
    {
        return false;
    }
    let visible = |col: &Option<crate::value::Color>| col.is_some_and(|x| x.a > 0.0);
    let colored = c.border_color.is_some() || c.border_colors.iter().any(Option::is_some);
    let border_paints = visible(&c.border_color)
        || c.border_colors.iter().any(visible)
        // Цвет не задан вовсе — рамка красится цветом текста, то есть видна.
        || (!colored && any(&c.borders()));
    c.gradient.is_some()
        || c.bg_image.is_some()
        || border_paints
        // Отступ и поле СТРОЧНОЙ коробки рисуют пустоту: место под них
        // держит знак-распорка внутри строки (`inline_sides`). Пока они
        // заводили коробку, строка рвалась по краю `<span>` — иероглифы
        // расходились по разным строкам, а коробки выходили разной ширины
        // (`word-space-transform-010`).
        // Скругление подсветки рисует прогон вместе с её фоном.
        || (c.background.is_none()
            && (set(&c.radius.tl)
                || set(&c.radius.tr)
                || set(&c.radius.br)
                || set(&c.radius.bl)))
        || !c.shadows.is_empty()
        || c.opacity.is_some()
        // Контур на раскладку не влияет ВООБЩЕ (css-ui §2): он рисуется за
        // краем коробки и места не занимает. Строчному куску коробку он
        // поэтому не заводит — иначе `<span>` с контуром переставал
        // переноситься вместе с абзацем и уезжал одной строкой за край
        // (`text-autospace-break-001`). Рисует его прогон строки, как и
        // ровную рамку (см. `inline::uniform_border`).
        || (!inline_level && c.outline.is_some())
}

/// Обернуть элемент преобразованием, если оно задано.
///
/// Сюда же попадает вертикальное письмо: строки, идущие сверху вниз, — это
/// повёрнутый на четверть оборота блок. Глифы при этом ложатся боком, как в
/// браузере для латиницы (`text-orientation: mixed`).
/// Распорка, снимающая коробку родителя и видимую часть ленты.
///
/// Абсолютная и во весь родитель: так её собственный прямоугольник и есть
/// коробка родителя, а обрезка на замере — это видимая часть прокрутки.
fn sticky_probe(frame: crate::interact::StickyCell) -> AnyElement {
    gpui::canvas(
        move |bounds, window, _| {
            frame.set(crate::interact::StickyFrame {
                container: Some(bounds),
                viewport: Some(window.content_mask().bounds),
            });
        },
        |_, _, _, _| {},
    )
    .absolute()
    .top_0()
    .left_0()
    .size_full()
    .into_any_element()
}

/// Обернуть элемент липкой рамкой, если `position: sticky`.
///
/// Отложенный проход нужен из-за порядка: прилипший заголовок рисуется до
/// содержимого, которое под ним проезжает, и без переноса в конец кадра это
/// содержимое его закрашивало бы.
fn sticky_wrap(
    el: AnyElement,
    c: &Computed,
    frame: &crate::interact::StickyCell,
    allowed: bool,
) -> AnyElement {
    if c.position != Some(crate::computed::Position::Sticky) {
        return el;
    }
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        Some(Len::Auto) | None => None,
        // Проценты у порога считаются от видимой части; её размер известен
        // только на отрисовке, поэтому берём ноль — как `top: 0`.
        Some(_) => Some(0.0),
    };
    let mut wrapper = crate::interact::Sticky::new(el, frame.clone());
    wrapper.top = side(c.inset.top);
    wrapper.bottom = side(c.inset.bottom);
    wrapper.left = side(c.inset.left);
    wrapper.right = side(c.inset.right);
    // Внутри отложенного поддерева липкий элемент рисуется на месте:
    // откладывать повторно нельзя.
    if !allowed {
        return wrapper.into_any_element();
    }
    gpui::deferred(wrapper)
        .with_priority(c.z_index.unwrap_or(0).max(0) as usize)
        .into_any_element()
}

/// Отрисовать поддерево в отдельный буфер, когда эффекту нужна готовая
/// картинка целиком.
///
/// Таких случаев три: размытие поддерева (`filter: blur`), смешивание с
/// кадром по формулам CSS (`mix-blend-mode`) и изоляция (`isolation`), где
/// поддерево обязано сложиться отдельно, прежде чем попасть в кадр.
fn grouped(el: AnyElement, c: &Computed) -> AnyElement {
    let blur = c.filter.map_or(0.0, |f| f.blur);
    let blend = c.blend.unwrap_or(0);
    let polygon = c.clip_polygon.as_deref().unwrap_or(&[]);
    if blur <= 0.0 && blend == 0 && polygon.is_empty() && c.isolate != Some(true) {
        return el;
    }
    let mut wrapper = crate::interact::Grouped::new(el);
    wrapper.blur = blur;
    wrapper.blend = u32::from(blend);
    // Доли коробки: проценты — как есть, точки перевести нечем до отрисовки,
    // поэтому берутся долей от стороны в сто точек — так их и пишут в CSS.
    wrapper.polygon = polygon
        .iter()
        .map(|(x, y)| {
            let frac = |l: Len| match l {
                Len::Pct(p) => p,
                Len::Px(v) => v / 100.0,
                _ => 0.0,
            };
            (frac(*x), frac(*y))
        })
        .collect();
    wrapper.into_any_element()
}

fn transformed(el: AnyElement, c: &Computed) -> AnyElement {
    let Some(t) = c.transform else {
        return el;
    };
    let mut wrapper = crate::interact::Transformed::new(el);
    wrapper.rotate = t.rotate_rad;
    wrapper.skew = t.skew_rad;
    wrapper.scale = t.scale;
    wrapper.translate = t.translate;
    wrapper.translate_pct = t.translate_pct;
    if let Some(o) = c.transform_origin {
        wrapper.origin = o;
    }
    wrapper.origin_px = c.transform_origin_px;
    wrapper.into_any_element()
}

/// Развернуть именованные области сетки в номера линий.
///
/// `grid-template-areas` — способ разложить макет именами вместо цифр. Ни
/// GPUI, ни taffy имён не знают, но знают номера: имя ищется в раскладке
/// контейнера, и ребёнок получает готовый прямоугольник линий.
fn place_named_areas(areas: &[Vec<String>], children: Vec<Node>) -> Vec<Node> {
    use crate::computed::Placement;
    children
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) => {
                let Some(name) = e.style.grid_area_name.clone() else {
                    return Node::Element(e);
                };
                // Прямоугольник имени: первая и последняя строка, первый и
                // последний столбец, где оно встречается.
                let (mut r0, mut r1, mut c0, mut c1) = (usize::MAX, 0usize, usize::MAX, 0usize);
                for (row, cells) in areas.iter().enumerate() {
                    for (col, cell) in cells.iter().enumerate() {
                        if *cell == name {
                            r0 = r0.min(row);
                            r1 = r1.max(row + 1);
                            c0 = c0.min(col);
                            c1 = c1.max(col + 1);
                        }
                    }
                }
                if r0 != usize::MAX {
                    // Линии в CSS считаются с единицы.
                    e.style.grid_row = Some((
                        Placement::Line(r0 as i16 + 1),
                        Placement::Line(r1 as i16 + 1),
                    ));
                    e.style.grid_col = Some((
                        Placement::Line(c0 as i16 + 1),
                        Placement::Line(c1 as i16 + 1),
                    ));
                }
                Node::Element(e)
            }
            other => other,
        })
        .collect()
}

/// Обернуть элемент лентой прокрутки, если `overflow` её просит.
///
/// `auto` и `scroll` в CSS означают именно ленту; обрезка без прокрутки —
/// это `hidden`, и подменять одно другим значило терять содержимое.
fn scrollable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    use crate::computed::Overflow;
    let horizontal = e.style.overflow_x == Some(Overflow::Scroll);
    let vertical = e.style.overflow_y == Some(Overflow::Scroll);
    if !horizontal && !vertical {
        return None;
    }
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(
        move |handle: &gpui::ScrollHandle, h: bool, v: bool| -> AnyElement {
            let _depth = DepthScope::enter(depth);
            // Внутренний узел рисуется без прокрутки: ею занимается лента.
            let mut inner = node.clone();
            inner.style.overflow_x = None;
            inner.style.overflow_y = None;
            // Наружный отступ принадлежит коробке, а не видимой области:
            // оставленный внутри, он увеличивал ленту на свою величину, и
            // содержимое было видно ниже края панели.
            let outer_margin = inner.style.margin;
            inner.style.margin = Default::default();
            use gpui::{InteractiveElement, StatefulInteractiveElement};
            let mut d = crate::apply::margins(div(), &outer_margin)
                .id(gpui::ElementId::Integer(node.node_id as u64 + 1))
                .track_scroll(handle)
                .child(element(&inner, &inherited, &opts));
            if h {
                d = d.overflow_x_scroll();
            }
            if v {
                d = d.overflow_y_scroll();
            }
            d.into_any_element()
        },
    );
    Some(
        crate::interact::ScrollArea::new(
            gpui::ElementId::Integer(e.node_id as u64),
            horizontal,
            vertical,
            build,
        )
        .into_any_element(),
    )
}

/// Обернуть элемент ручкой изменения размера, если `resize` разрешает.
fn resizable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    if e.style.pointer_events_none == Some(true) {
        return None;
    }
    let (horizontal, vertical) = e.style.resize?;
    let axis = match (horizontal, vertical) {
        (true, true) => crate::interact::ResizeAxis::Both,
        (true, false) => crate::interact::ResizeAxis::Horizontal,
        _ => crate::interact::ResizeAxis::Vertical,
    };
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |w: Option<f32>, h: Option<f32>| {
        let _depth = DepthScope::enter(depth);
        // Заданный мышью размер побеждает разметку — как и в браузере, где
        // он пишется в инлайн-стиль элемента.
        let mut mixed = node.clone();
        mixed.style.resize = None;
        if let Some(w) = w {
            mixed.style.width = Some(Len::Px(w));
        }
        if let Some(h) = h {
            mixed.style.height = Some(Len::Px(h));
        }
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::interact::Resizable::new(gpui::ElementId::Integer(e.node_id as u64), axis, build)
            .into_any_element(),
    )
}

/// Обернуть элемент плавным переходом, если он задан.
///
/// Поддерево пересобирается по доле перехода — иначе смешанный стиль некуда
/// применить: у собранного элемента стиль уже зафиксирован.
fn transitioned(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    let seconds = e.style.transition?;
    let hover = e.hover.clone()?;
    let node = e.clone();
    let inherited = inherited.clone();
    let opts = opts.clone();
    let depth = defer_depth();
    let build = std::rc::Rc::new(move |k: f32| {
        let _depth = DepthScope::enter(depth);
        let mut mixed = node.clone();
        mixed.style = node.style.blend(&hover, k);
        // Слой наведения снят: его роль уже сыграла доля перехода, иначе
        // стиль прыгнул бы поверх плавного.
        mixed.hover = None;
        mixed.style.transition = None;
        element(&mixed, &inherited, &opts)
    });
    Some(
        crate::transition::Transition::new(
            gpui::ElementId::Integer(e.node_id as u64),
            seconds,
            build,
        )
        .into_any_element(),
    )
}

/// Интерполяция стиля между кадрами анимации.
///
/// Интерполируются те свойства, которые анимируют на практике и которые можно
/// подменить у уже собранного элемента: прозрачность, заливка, цвет текста,
/// сдвиг и размеры. Всё остальное берётся с ближайшего кадра — перестраивать
/// поддерево каждый кадр нельзя, это стоило бы дороже самой анимации.
fn frame_at(frames: &[(f32, Computed)], t: f32) -> Computed {
    let t = t.clamp(0.0, 1.0);
    let mut prev = &frames[0];
    let mut next = &frames[frames.len() - 1];
    for pair in frames.windows(2) {
        if t >= pair[0].0 && t <= pair[1].0 {
            prev = &pair[0];
            next = &pair[1];
            break;
        }
    }
    let span = (next.0 - prev.0).max(0.0001);
    let k = ((t - prev.0) / span).clamp(0.0, 1.0);
    let mut out = prev.1.clone();
    let lerp = |a: f32, b: f32| a + (b - a) * k;
    if let (Some(a), Some(b)) = (prev.1.opacity, next.1.opacity) {
        out.opacity = Some(lerp(a, b));
    }
    let mix = |a: crate::value::Color, b: crate::value::Color| crate::value::Color {
        r: lerp(a.r, b.r),
        g: lerp(a.g, b.g),
        b: lerp(a.b, b.b),
        a: lerp(a.a, b.a),
    };
    if let (Some(a), Some(b)) = (prev.1.background, next.1.background) {
        out.background = Some(mix(a, b));
    }
    if let (Some(a), Some(b)) = (prev.1.color, next.1.color) {
        out.color = Some(mix(a, b));
    }
    let len = |a: Option<Len>, b: Option<Len>| -> Option<Len> {
        match (a?, b?) {
            (Len::Px(x), Len::Px(y)) => Some(Len::Px(lerp(x, y))),
            (Len::Pct(x), Len::Pct(y)) => Some(Len::Pct(lerp(x, y))),
            (x, _) => Some(x),
        }
    };
    out.width = len(prev.1.width, next.1.width).or(out.width);
    out.height = len(prev.1.height, next.1.height).or(out.height);
    if let (Some(a), Some(b)) = (prev.1.translate, next.1.translate) {
        out.translate = Some((
            len(Some(a.0), Some(b.0)).unwrap_or(a.0),
            len(Some(a.1), Some(b.1)).unwrap_or(a.1),
        ));
    }
    out
}

/// Обернуть элемент анимацией, если она задана.
fn animated(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let (Some(frames), Some(spec)) = (e.anim.clone(), e.style.animation.clone()) else {
        return element(e, inherited, opts);
    };
    // Анимируется обёртка: у готового элемента стиль уже зафиксирован, а
    // менять надо ровно те свойства, которые перечислены в кадрах.
    //
    // Внешний отступ переезжает НА обёртку: оставшись внутри, он переставал
    // раздвигать соседей — блоки слипались против браузера.
    let mut inner = e.clone();
    inner.style.margin = crate::computed::Sides::default();
    let el = element(&inner, inherited, opts);
    let d = apply_margin(div(), &e.style).child(el);
    let seconds = spec.seconds.max(0.05);
    let mut anim = gpui::Animation::new(std::time::Duration::from_secs_f32(seconds));
    if spec.infinite {
        anim = anim.repeat();
    }
    if spec.alternate {
        // Обратный ход через раз — это ровно «туда-сюда» по времени.
        anim = anim.with_easing(gpui::pulsating_between(0.0, 1.0));
    }
    gpui::AnimationExt::with_animation(
        d,
        gpui::ElementId::Integer(e.node_id as u64),
        anim,
        move |d, delta| {
            let c = frame_at(&frames, delta);
            let mut d = d;
            if let Some(o) = c.opacity {
                d = d.opacity(o);
            }
            if let Some(bg) = c.background {
                d = d.bg(bg.to_hsla());
            }
            if let Some(col) = c.color {
                d = d.text_color(col.to_hsla());
            }
            if let Some(Len::Px(w)) = c.width {
                d = d.w(px(w));
            }
            if let Some(Len::Px(h)) = c.height {
                d = d.h(px(h));
            }
            if let Some((x, y)) = c.translate {
                if let Len::Px(v) = x {
                    d = d.left(px(v));
                }
                if let Len::Px(v) = y {
                    d = d.top(px(v));
                }
            }
            d
        },
    )
    .into_any_element()
}

/// Блочный элемент.
fn element(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let mut merged = inline::inherit(inherited, &e.style);
    // `dir="auto"` — сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку содержимого.
    // Разбор двунаправленности выберет её сам при наборе, но выключка и
    // прижим текста читают `rtl` из стиля, и без этого шага блок с арабским
    // текстом прижимался влево.
    if e.attr("dir") == Some("auto") && e.style.rtl.is_none() {
        let mut text = String::new();
        gather_text(&e.children, &mut text);
        let strong = text.chars().find_map(|ch| {
            use unicode_bidi::BidiClass::*;
            match unicode_bidi::bidi_class(ch) {
                L => Some(false),
                R | AL => Some(true),
                _ => None,
            }
        });
        if let Some(rtl) = strong {
            merged.rtl = Some(rtl);
        }
    }
    // Предел ОРТОГОНАЛЬНОГО потока для строк вертикального письма внутри
    // (CSS Writing Modes §7.3). Искать его надо вверх по дереву, поэтому он
    // несётся вниз наследуемым полем. Ближе всего собственная определённая
    // высота элемента; за ней — высота ближайшего контейнера прокрутки с
    // наложенным на неё `max-height`; в самом конце — окно (его подставляет
    // потребитель в `paragraph`).
    {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let scrolls = !matches!(
            e.style.overflow_y,
            None | Some(crate::computed::Overflow::Visible)
        );
        if let Some(mut h) = px_of(e.style.height) {
            if let Some(max) = px_of(e.style.max_height) {
                h = h.min(max);
            }
            merged.ortho_limit = Some(h);
        } else if scrolls {
            if let Some(mut h) = px_of(e.style.min_height) {
                if let Some(max) = px_of(e.style.max_height) {
                    h = h.min(max);
                }
                merged.ortho_limit = Some(h);
            }
        }
    }
    // Псевдоэлементы принадлежат ЭТОМУ узлу: они едут в стиль его детей на
    // один уровень, а глубже слитый стиль их уже не несёт.
    // Кегль слоя приводится к точкам ЗДЕСЬ: слой применяется мимо
    // наследования, и `font-size: 200%` доезжал до набора неразрешённым —
    // первая строка оставалась обычного размера
    // (`text-autospace-first-line-001`). Доля считается от кегля самого блока.
    let resolved = |layer: &Computed| {
        let mut c = layer.clone();
        if let Some(Len::Px(base)) = merged.font_size {
            // ТОЛЬКО доля: `em` у слоя разрешает набор строк, и он считает
            // её от кегля РОДИТЕЛЯ абзаца (`inline::max_font_size`). Перевод
            // здесь давал второе умножение — буквица уезжала в четыре кегля
            // вместо одного (`text-transform-shaping-001`).
            c.font_size = match c.font_size {
                Some(Len::Pct(k)) => Some(Len::Px(k * base)),
                other => other,
            };
        }
        Box::new(c)
    };
    merged.first_letter = e.first_letter.as_ref().map(&resolved);
    merged.first_line = e.first_line.as_ref().map(&resolved);
    // Единицы окна разрешаются здесь: размер окна знает только сборщик.
    merged.resolve_viewport(opts.viewport);
    // Элементы форм рисуются своим набором: без него поле ввода — пустой
    // прямоугольник, что выглядит поломкой разметки.
    if let Some(el) = crate::forms::element(e, &merged, opts) {
        return el;
    }
    match e.tag.as_str() {
        "img" => image(e),
        "iframe" if iframe(e, opts).is_some() => iframe(e, opts).unwrap(),
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => crate::svg::element(e).unwrap_or_else(|| {
            styled_div_with(e, &merged)
                .child(SharedString::from("[рисунок]"))
                .into_any_element()
        }),
        "hr" => styled_div_with(e, &merged).w_full().into_any_element(),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        _ if merged.display == Some(Display::GridLanes) => {
            // Корневой поток лунок: html/body ростом с видимую область, как
            // и обычный корень (общий минимум главного пути в лунковую
            // ветку не проходил, и body в квирк-режиме не заполнял вьюпорт).
            if matches!(e.tag.as_str(), "html" | "body")
                && merged.vertical != Some(true)
                && e.style.height.is_none()
                && e.style.min_height.is_none()
            {
                let mut rooted = merged.clone();
                rooted.min_height = Some(Len::Px(opts.viewport.1));
                lanes(e, &rooted, opts)
            } else {
                lanes(e, &merged, opts)
            }
        }
        _ if matches!(
            e.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        ) =>
        {
            table(e, &merged, opts)
        }
        // Ряд, группа рядов или ячейка ВНЕ таблицы получают анонимную
        // таблицу-обёртку (css-tables-3 §3.1): иначе ячейки складывались
        // столбиком обычных блоков.
        _ if matches!(
            e.style.display,
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
        ) =>
        {
            let wrapper = anon_element("table", vec![Node::Element(e.clone())]);
            table(&wrapper, &merged, opts)
        }
        "table" => table(e, &merged, opts),
        // Список с заданной раскладкой — это уже не список, а контейнер:
        // на `ul` верстают навигацию и наборы чипов.
        "ul" | "ol" if e.style.display.is_none() => list(e, &merged, opts),
        // `white-space: pre*` значим не меньше тега: переводы строк сохраняет
        // именно он, и на `<div style="white-space: pre">` разметка обязана
        // вести себя так же, как на `<pre>`.
        // Преформат отдельным рисователем — ТОЛЬКО для непереносящегося
        // `white-space: pre`. Переводы строк хранят четыре режима, и три из
        // них переносят строки: `pre-wrap`, `pre-line`, `break-spaces`. Пока
        // сюда уходили все четыре, эти три шли мимо нашей строчной раскладки,
        // где и живут висящие пробелы, разрыв после сохранённого пробела и
        // правила куска. Отсюда же `break-spaces` был неотличим от `pre-wrap`.
        // Внутри преформата может стоять кусок со СВОИМ `white-space`, и он
        // переносится, хотя абзац — нет. Отдельный рисователь преформата
        // правил куска не знает, поэтому такой случай уходит в обычную
        // строчную раскладку (`white-space-pre-031`).
        _ => {
            let mut d = styled_div_with(e, &merged);
            // Корневая коробка документа — начальный содержащий блок, а он
            // ростом с видимую область (CSS 2.1 §10.1). Абсолютный потомок
            // статического `body` в браузере считается именно от неё; в нашей
            // раскладке содержащим блоком служит любой родитель, поэтому без
            // этой высоты `top: 0; bottom: 0` схлопывалось в ноль, и страница
            // выходила ПУСТОЙ (`background-attachment-margin-root-001` и вся
            // родня в css-backgrounds и css-position).
            // Только при горизонтальном письме: у вертикального ось блока —
            // горизонтальная, высота там задаёт ДЛИНУ строки, и навязанный
            // минимум ломает подбор размера ортогонального потока
            // (`available-size-022`).
            if matches!(e.tag.as_str(), "html" | "body")
                && merged.vertical != Some(true)
                && e.style.height.is_none()
                && e.style.min_height.is_none()
            {
                d = d.min_h(px(opts.viewport.1));
            }
            // Многоколоночный поток. Своей многоколоночной раскладки нет, но
            // сетка даёт то же расположение: число рядов считаем по числу
            // детей, а заполнение идёт по колонкам — тогда порядок совпадает
            // с браузерным (сверху вниз, затем в следующую колонку).
            // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
            // целых колонок этой ширины влезает в коробку, столько их и будет
            // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
            // считать не от чего, и остаётся прежняя дорожечная раскладка.
            let by_width = match (e.style.column_count, e.style.column_width, e.style.width) {
                (None, Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
                    let gap = match e.style.column_gap {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    Some((((box_w + gap) / (w + gap)).floor().max(1.0)) as u16)
                }
                _ => None,
            };
            if let Some(cols) = e.style.column_count.or(by_width).filter(|n| *n > 1) {
                // Сплошной текст режется на колонки по строкам, а не по детям:
                // один длинный абзац иначе оставался в первой колонке целиком.
                if let Some(el) = column_flow(e, &merged, opts, cols as usize) {
                    // Коробка элемента остаётся своей: отступы и фон
                    // принадлежат ей, поток живёт внутри.
                    return d.child(el).into_any_element();
                }
                let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
                let rows = count.div_ceil(cols as usize).max(1) as u16;
                // Умолчание `column-gap: normal` — один кегль текста.
                let gap = match e.style.column_gap {
                    Some(Len::Px(v)) => v,
                    _ => match e.style.font_size {
                        Some(Len::Px(size)) => size,
                        _ => opts.base_size(),
                    },
                };
                d = d
                    .grid()
                    .grid_template_cols((0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect())
                    .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
                    .gap_x(px(gap));
                d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
            } else if let Some(Len::Px(w)) = e.style.column_width {
                // Ширина колонки без их числа — это «сколько влезет»: ровно
                // то, что умеет короткая форма дорожек в GPUI.
                d = d.grid().grid_cols_min(px(w));
            } else if merged.vertical == Some(true) && e.style.display.is_none() {
                // Вертикальное письмо: ось блочного потока — горизонтальная.
                // Дети идут слева направо (`vertical-lr`) или справа налево
                // (`vertical-rl`).
                d = d.flex();
                d = if merged.vertical_rl == Some(true) {
                    d.flex_row_reverse()
                } else {
                    d.flex_row()
                };
                // `sideways-lr`: строка идёт снизу вверх — начало строчной
                // оси у НИЖНЕГО края (css-writing-modes-4 §block-flow).
                // Только ГЛАВНОЕ письмо страницы: у вложенных контейнеров
                // строчную ось ведёт абзац, и прижим коробки к низу расходился
                // с ним (abs-pos-border-offset-002).
                // Прижим — на ТЕЛЕ: у `<html>` бывают свои `::before`/`::after`
                // с собственным письмом, и прижим корня уводил их вниз
                // (wm-propagation-body-047).
                if merged.sideways == Some(true)
                    && merged.vertical_rl != Some(true)
                    && e.tag == "body"
                {
                    d = d.items_end();
                }
                if e.style.width.is_none() {
                    d = d.flex_shrink_0();
                }
            } else if e.style.display.is_none() {
                // Блок без явного `display` — блочная раскладка taffy, а не
                // гибкая колонка. Колонка навязывала детям сжатие: ребёнок
                // выше родителя ужимался, тогда как браузер даёт ему вылезти.
                // Схлопывание вертикальных отступов при этом делает сама
                // раскладка — включая протекание через пустой блок.
                d = d.flex().flex_col();
                // Письмо справа налево: начало строчной оси — правый край,
                // переполняющий БЛОК вылезает влево (csswg-drafts#5572).
                if merged.rtl == Some(true) {
                    d = d.items_end();
                }
            }
            // Ряд по умолчанию — но не тогда, когда письмо справа налево:
            // там ряд обязан идти в обратную сторону, и общая ветка его
            // разворот отменяла.
            if e.style.display == Some(Display::Flex)
                && e.style.flex_dir.is_none()
                && e.style.rtl != Some(true)
            {
                // При вертикальном письме умолчание `row` — это ось строки, а
                // она идёт сверху вниз.
                d = if merged.vertical == Some(true) {
                    d.flex_col()
                } else {
                    d.flex_row()
                };
            }
            // Имена областей разворачиваются здесь: контейнер и его дети
            // видны одновременно только на этом уровне.
            let children = match &e.style.grid_areas {
                Some(areas) => place_named_areas(areas, e.children.clone()),
                None => e.children.clone(),
            };
            // Отступы вдоль оси потока схлопываются — при вертикальном письме
            // это ГОРИЗОНТАЛЬНЫЕ отступы соседей. В Chrome три полосы с
            // `margin: 0 16px` стоят через 16, а не через 32.
            let children = if merged.vertical == Some(true) {
                orthogonal_children(
                    collapse_flow_margins(children, merged.vertical_rl == Some(true)),
                    &merged,
                )
            } else {
                orthogonal_vertical_children(children, &merged)
            };
            let mut kids: Vec<AnyElement> = Vec::new();
            kids.extend(clip_layer(&merged, opts));
            // Бюджет строк обрезки: сторожа контекста живут, пока строится
            // поддерево — пробы детей пишут строки в буфер контейнера.
            let is_clamp = e.style.clamp_lines().is_some()
                || (e.style.clamp_auto == Some(true) && merged.max_height.is_some());
            let _clamp_guard = is_clamp.then(|| crate::interact::ClampGuard::enter(e.node_id));
            let makes_bfc = matches!(
                merged.overflow_x,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || matches!(
                merged.overflow_y,
                Some(crate::computed::Overflow::Hidden) | Some(crate::computed::Overflow::Scroll)
            ) || merged.float.is_some();
            let _bfc_guard = (!is_clamp
                && makes_bfc
                && crate::interact::clamp_context().is_some())
            .then(crate::interact::ClampGuard::enter_bfc);
            if let Some((key, skip)) = crate::interact::clamp_context() {
                // Строки дают пробы абзацев (paragraph_probed); здесь — только
                // коробка с краской: блок прячется целиком, если срез внутри.
                if !is_clamp && has_box_style_probe(&e.style) {
                    kids.push(crate::interact::clamp_probe(
                        crate::interact::clamp_lines_for(key),
                        0.0,
                        skip,
                        e.style.height.is_some() || e.style.min_height.is_some(),
                    ));
                }
            }
            kids.extend(blocks(&children, &merged, opts));
            if is_clamp {
                let max_h = match merged.max_height {
                    Some(Len::Px(v)) => Some(v),
                    _ => None,
                };
                kids.push(
                    crate::interact::ClampCut::new(
                        e.node_id,
                        crate::interact::clamp_lines_for(e.node_id),
                        e.style.clamp_lines(),
                        max_h,
                    )
                    .into_any_element(),
                );
            }
            d.children(kids).into_any_element()
        }
    }
}

/// Картинка: `src` с `data:`-URI или путь. Внешние URL не грузим — документ
/// рисуется в чате, где сеть запрещена по тем же причинам, что и в вебвью.
/// Приклеить базовую папку к относительным `url(...)` вложенного документа.
fn resolve_embedded_urls(html: &str, dir: &std::path::Path) -> String {
    let mut out = String::with_capacity(html.len());
    let mut rest = html;
    while let Some(at) = rest.find("url(") {
        out.push_str(&rest[..at + 4]);
        rest = &rest[at + 4..];
        let Some(end) = rest.find(')') else { break };
        let raw = &rest[..end];
        let inner = raw.trim().trim_matches('"').trim_matches('\'');
        if inner.starts_with("data:") || inner.contains("://") || inner.starts_with('/') {
            out.push_str(raw);
        } else {
            let abs = format!("file:///{}", dir.join(inner).display()).replace('\\', "/");
            out.push('"');
            out.push_str(&abs);
            out.push('"');
        }
        out.push(')');
        rest = &rest[end + 1..];
    }
    out.push_str(rest);
    out
}

thread_local! {
    /// Глубина вложенных документов — от циклических iframe.
    static IFRAME_DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `<iframe>`: вложенный документ со своими стилями и областью просмотра
/// размером с коробку. Содержимое читается с диска (стенд переписывает
/// `src` в `file:///...`); без файла остаётся запасной текст тега.
fn iframe(e: &Element, opts: &RenderOpts) -> Option<AnyElement> {
    let src = e.attr("src")?;
    let path = src
        .strip_prefix("file:///")
        .or_else(|| src.strip_prefix("file://"))?;
    if IFRAME_DEPTH.with(|d| d.get()) >= 3 {
        return None;
    }
    let html = std::fs::read_to_string(path).ok()?;
    // Относительные адреса ВНУТРИ вложенного документа считаются от его
    // папки: движок путей не разрешает, поэтому база приклеивается текстом.
    let html = match std::path::Path::new(path).parent() {
        Some(dir) => resolve_embedded_urls(&html, dir),
        None => html,
    };
    let attr_len = |k: &str| e.attr(k).and_then(|v| v.parse::<f32>().ok());
    // Размер: CSS сильнее атрибутов; умолчание — 300×150 (CSS 2.2 §замещаемые).
    let w = match e.style.width {
        Some(Len::Px(v)) => v,
        _ => attr_len("width").unwrap_or(300.0),
    };
    let h = match e.style.height {
        Some(Len::Px(v)) => v,
        _ => attr_len("height").unwrap_or(150.0),
    };
    let (nodes, salt) = crate::doc::parse_embedded(&html, crate::BROWSER_CSS);
    // Верхний уровень вложенного документа проходит те же ортогональные
    // поправки, что и дети контейнера.
    let nodes = orthogonal_vertical_children(nodes, &Computed::default());

    let mut sub = opts.clone();
    sub.viewport = (w, h);
    sub.doc_salt = salt;
    IFRAME_DEPTH.with(|d| d.set(d.get() + 1));
    let kids = blocks(&nodes, &sub.root_style(), &sub);
    IFRAME_DEPTH.with(|d| d.set(d.get() - 1));
    Some(
        styled_div(e)
            .w(px(w))
            .h(px(h))
            .overflow_hidden()
            .relative()
            .flex_shrink_0()
            .children(kids)
            .into_any_element(),
    )
}

/// Кегль для разрешения долей на атоме: свой размер шрифта уже разрешён в
/// слитом стиле, иначе — базовый.
fn atom_base_font(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    }
}

fn image(e: &Element) -> AnyElement {
    image_with(e, None)
}

/// То же, но с базовым кеглем для разрешения долей: атом строится от СЫРОГО
/// стиля, и `padding-right: 1em` без разрешения терялся вовсе
/// (wm-propagation-body-040: сосед вставал на 16 точек левее эталона).
fn image_with(e: &Element, base_font: Option<f32>) -> AnyElement {
    let src = e.attr("src").unwrap_or_default();
    // Размеры коробки ставит общий разбор стиля (`apply`): он же добавляет к
    // заданной ширине отступы и рамку, потому что раскладка под нами считает
    // размер по внешнему краю, а CSS по умолчанию — по содержимому. Ставить
    // ширину ЕЩЁ РАЗ отсюда нельзя: она затирала эту поправку, и картинка с
    // `padding-left` вылезала за край на величину отступа.
    let resolved;
    let e = if let Some(base) = base_font {
        let mut copy = e.clone();
        copy.style.resolve_em(base);
        resolved = copy;
        &resolved
    } else {
        e
    };
    let d = styled_div(e);
    // Замещаемый элемент в строке НЕ сжимается: браузер даёт строке
    // переполниться или перенести коробку целиком (CSS 2.1 §10.3.2, замер
    // wm-propagation-body-040: картинка 340px ужималась на 6-7%).
    let d = d.flex_shrink_0();
    if src.starts_with("data:") || src.starts_with("file:") || src.starts_with('/') {
        // Локальный файл отдаётся ПУТЁМ, а не строкой адреса. Строку со схемой
        // `file:` система разбирает как сетевой адрес и уходит его скачивать —
        // ничего не приходит, и картинка молча не рисуется вовсе. Ровно на
        // этом эталоны из одних картинок выходили пустой страницей.
        let local = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))
            .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
        // Растровый файл декодируется СРАЗУ, своим декодером: штатный путь
        // грузит асинхронно (кадр успевал сняться до загрузки — стенд мигал),
        // и не применяет вшитый цветовой профиль (css-color-4 §12).
        let own = crate::background::source(local.unwrap_or(src)).and_then(|s| match s {
            crate::background::Source::Raster(image) => Some(image),
            crate::background::Source::Vector { .. }
            | crate::background::Source::Gradient { .. } => None,
        });
        let mut image = match (own, local) {
            (Some(ready), _) => gpui::img(ready),
            (None, Some(path)) => gpui::img(std::path::PathBuf::from(path)),
            (None, None) => gpui::img(SharedString::from(src.to_string())),
        };
        // Картинку арифметикой над своими цветами не поправить — но
        // обесцвечивание у неё своё, встроенное.
        if e.style.filter.is_some_and(|f| f.grayscale > 0.5) {
            image = image.grayscale(true);
        }
        // Заданный размер коробки картинке надо ОТДАТЬ: сама она берёт свой
        // пиксель и рисуется им, сколько бы ни стояло в разметке. Замерено на
        // пробе: `<img width=100 height=100>` и `img { width: 300px }` давали
        // ровно один и тот же рисунок в 15 точек — то есть размер не работал
        // никогда. Отдаётся он, только когда заданы ОБЕ стороны: с одной
        // вторая считается по соотношению сторон, а его коробка не знает.
        // Размер ставится САМОЙ картинке, а не через «во весь родитель»: в
        // ряду обтекания родитель своего размера не имеет, и доля от него
        // схлопывала рисунок в ничто.
        if let (Some(Len::Px(w)), Some(Len::Px(h))) = (e.style.width, e.style.height) {
            image = image.w(px(w)).h(px(h));
        }
        // CSS-умолчание для замещаемого содержимого — заполнить коробку, но
        // держится оно на СОБСТВЕННОМ соотношении сторон картинки: заданная
        // одна сторона задаёт вторую. Соотношения мы до загрузки не знаем,
        // поэтому вторая сторона остаётся своей, и заполнение растягивало бы
        // рисунок в чужой прямоугольник (замерено: `flexbox-min-width-auto`
        // ушёл в минус шестью парами). Вписывание в этих условиях ближе.
        image = match e.style.object_fit.as_deref() {
            Some("cover") => image.object_fit(gpui::ObjectFit::Cover),
            Some("fill") => image.object_fit(gpui::ObjectFit::Fill),
            Some("scale-down") => image.object_fit(gpui::ObjectFit::ScaleDown),
            Some("none") => image.object_fit(gpui::ObjectFit::None),
            // Обе стороны заданы — умолчание CSS: ЗАПОЛНИТЬ коробку, даже с
            // искажением (`object-fit: fill`). Вписывание оставлено случаю с
            // одной стороной: там вторая держится на собственном соотношении
            // рисунка (см. замер выше).
            _ if e.style.width.is_some() && e.style.height.is_some() => {
                image.object_fit(gpui::ObjectFit::Fill)
            }
            _ => image.object_fit(gpui::ObjectFit::Contain),
        };
        return d.child(image).into_any_element();
    }
    // Пустая рамка вместо чужой картинки: молча ничего не показать хуже —
    // в разметке останется дыра без объяснения.
    d.child(SharedString::from(
        e.attr("alt")
            .map(str::to_string)
            .unwrap_or_else(|| "[изображение]".into()),
    ))
    .into_any_element()
}

/// Список: маркер рисуем сами — `list-style` в GPUI нет.
fn list(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let ordered = e.tag == "ol";
    let mut rows = vec![];
    let mut idx = 1usize;
    for child in &e.children {
        let Node::Element(li) = child else { continue };
        if li.tag != "li" {
            continue;
        }
        // Вид маркера задаёт документ; без указания — умолчание тега.
        let kind = li.style.marker.or(e.style.marker);
        let marker = match kind {
            Some(crate::computed::Marker::Circle) => "◦".to_string(),
            Some(crate::computed::Marker::Square) => "▪".to_string(),
            Some(crate::computed::Marker::Disc) => "•".to_string(),
            Some(crate::computed::Marker::Decimal) => format!("{idx}."),
            Some(crate::computed::Marker::LowerAlpha) => {
                format!("{}.", (b'a' + ((idx - 1) % 26) as u8) as char)
            }
            Some(crate::computed::Marker::UpperAlpha) => {
                format!("{}.", (b'A' + ((idx - 1) % 26) as u8) as char)
            }
            Some(crate::computed::Marker::LowerRoman) => format!("{}.", roman(idx)),
            None if ordered => format!("{idx}."),
            None => "•".to_string(),
        };
        idx += 1;
        // `list-style: none` — на списках верстают навигацию и наборы чипов,
        // и точки там лишние.
        let no_marker = e.style.no_marker == Some(true) || li.style.no_marker == Some(true);
        let merged = inline::inherit(inherited, &li.style);
        rows.push(
            div()
                .flex()
                .flex_row()
                .gap_x(px(6.))
                .items_start()
                .children((!no_marker).then(|| {
                    div()
                        .flex_shrink_0()
                        .min_w(px(14.))
                        .child(SharedString::from(marker))
                }))
                .child(
                    div()
                        .flex()
                        .flex_col()
                        .children(blocks(&li.children, &merged, opts)),
                )
                .into_any_element(),
        );
    }
    styled_div_with(e, inherited)
        .flex()
        .flex_col()
        .children(rows)
        .into_any_element()
}



/// Текст поддерева — нужен формам (`<textarea>`, `<option>`).
pub fn gather_text_public(nodes: &[Node], out: &mut String) {
    gather_text(nodes, out)
}

/// Текст ДО первого жёсткого разрыва: дальше первая строка не идёт никогда.
///
/// Замер первой строки ищет, сколько знаков влезет по ширине, и про `<br>` он
/// не знает — с широкой коробкой в первую строку попадал весь абзац, и её
/// начертание доставалось второй строке тоже
/// (`text-autospace-first-line-001`).
fn gather_until_break(nodes: &[Node], out: &mut String) -> bool {
    for n in nodes {
        match n {
            Node::Text(t) => {
                if let Some(cut) = t.find('\n') {
                    out.push_str(&t[..cut]);
                    return true;
                }
                out.push_str(t);
            }
            Node::Element(e) if e.tag == "br" => return true,
            Node::Element(e) => {
                if gather_until_break(&e.children, out) {
                    return true;
                }
            }
        }
    }
    false
}

fn gather_text(nodes: &[Node], out: &mut String) {
    for n in nodes {
        match n {
            Node::Text(t) => out.push_str(t),
            Node::Element(e) => gather_text(&e.children, out),
        }
    }
}

/// Таблица.
///
/// Колонки — по содержимому: каждая дорожка это `minmax(min-content, auto)`,
/// последняя забирает остаток (`1fr`). Так ведёт себя и настоящая табличная
/// раскладка: узкие колонки сжимаются до содержимого, широкая тянется.
///
/// Это стало возможно только вместе с патчем произвольных дорожек в GPUI —
/// короткая форма умела ровно «N равных колонок», и таблица из даты и длинного
/// текста разъезжалась пополам.
fn table(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    // Раздельные рамки — умолчание; при `collapse` зазора между ячейками нет.
    let spacing = match (e.style.border_collapse, e.style.border_spacing) {
        (Some(true), _) => (0.0, 0.0),
        (None, _) if e.attr("rules").is_some() => (0.0, 0.0),
        // Заданный `border-spacing` перекрывает умолчание браузера в 2px.
        (_, Some((x, y))) => (
            match x {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            },
            match y {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            },
        ),
        // Начальное значение `border-spacing` — НОЛЬ: два пикселя — это
        // умолчание браузера для ТЕГА `<table>`, и оно приходит сюда
        // каскадом из своего стилевого листа. `div` с `display: table`
        // зазора не имеет.
        _ => (0.0, 0.0),
    };
    // Дети таблицы ЧИНЯТСЯ перед сбором (css-tables-3 §3, fixup):
    // `display: contents` растворяется — его дети идут в таблицу со слитым
    // стилем, — а бесхозные ячейки и текст заворачиваются в анонимный ряд.
    // Без этого содержимое просто пропадало: сборщик рядов видел только
    // настоящие `<tr>` и группы.
    let fixed = fixup_table_children(&e.children);
    // `<thead>` встаёт первым, `<tfoot>` — последним (CSS 2.2 §17.5.3,
    // HTML §14.3.9) СТАБИЛЬНОЙ перестановкой ГРУПП: прочий порядок детей
    // не трогается. Прошлая попытка ломала css-position — она сдвигала
    // ряды и там, где группы уже стояли по порядку.
    let fixed = {
        let first_of = |tag: &str| -> Option<u64> {
            fixed.iter().find_map(|n| match n {
                Node::Element(g) if g.tag == tag => Some(g.node_id),
                _ => None,
            })
        };
        // Заголовочной и подвальной становится только ПЕРВАЯ группа
        // своего рода; последующие thead/tfoot — обычные группы рядов.
        let head = first_of("thead");
        let foot = first_of("tfoot");
        let key = |n: &Node| match n {
            Node::Element(g) if Some(g.node_id) == head => 0u8,
            Node::Element(g) if Some(g.node_id) == foot => 2,
            _ => 1,
        };
        let ordered = fixed.windows(2).all(|w| key(&w[0]) <= key(&w[1]));
        if ordered {
            fixed
        } else {
            let mut sorted = fixed;
            sorted.sort_by_key(key);
            sorted
        }
    };
    let mut rows: Vec<(&Element, RowCarry)> = vec![];
    collect_rows(&fixed, (0.0, 0.0, None, None), &mut rows);
    // Ширина таблицы — сумма ОБЪЕДИНЕНИЙ, а не число ячеек: строка из двух
    // ячеек с `colspan=2` даёт четыре колонки, и без этого содержимое
    // выталкивалось в неявные ряды.
    let cols = rows
        .iter()
        .map(|(r, _)| {
            r.children
                .iter()
                .filter_map(|c| match c {
                    Node::Element(e) if is_cell(e) => Some(
                        e.attr("colspan")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1),
                    ),
                    _ => None,
                })
                .sum::<usize>()
        })
        .max()
        .unwrap_or(1)
        .max(1) as u16;

    // ОДНА сетка на всю таблицу, а не по сетке на строку. Со строками-сетками
    // ширина колонки считалась внутри строки, и соседние строки расходились —
    // заголовок стоял над одним столбцом, значения под другим. Колонки общие
    // только если ячейки живут в общей сетке.
    let mut cells: Vec<AnyElement> = vec![];
    // Наследуемые свойства САМОЙ таблицы обязаны дойти до ячеек: `inherited`
    // — это стиль её РОДИТЕЛЯ, и всё объявленное на теге таблицы
    // (`white-space`, шрифт, цвет) шло мимо. Видно было по сохранённым
    // пробелам: в ячейке они схлопывались, хотя на таблице стоял
    // `white-space: break-spaces`.
    //
    // С первого раза правка была в минус (ломалась арабская вязь) — но ломал
    // её свой замер ширин, который мерил текст ячейки отдельно от раскладки.
    // Со снятым замером она проходит чисто.
    let row_elements: Vec<&Element> = rows.iter().map(|(r, _)| *r).collect();
    // Заявленные ширины ячеек по КОЛОНКАМ: ширина ячейки в таблице задаёт
    // колонку, а не свою коробку (CSS 2.1 §17.5.2.2) — колонка не уже
    // содержимого (пол min-content), процентная забирает долю остатка.
    let mut col_widths: Vec<(Option<f32>, Option<f32>)> = vec![(None, None); cols as usize];
    let table_font = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let table_family = inherited.font_family.clone().unwrap_or_default();
    let (from_cols, cols_collapsed) =
        col_element_widths(&e.children, table_font, &table_family);
    let mut busy: Vec<u16> = vec![0; cols as usize];
    for row in &row_elements {
        let mut ix = 0usize;
        for slot in busy.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        for c in &row.children {
            let Node::Element(cell) = c else { continue };
            if !is_cell(cell) {
                continue;
            }
            while ix < busy.len() && busy[ix] > 0 {
                ix += 1;
            }
            let span = cell
                .attr("colspan")
                .and_then(|v| v.parse::<usize>().ok())
                .unwrap_or(1)
                .max(1);
            let rspan: u16 = cell
                .attr("rowspan")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
            for c2 in ix..(ix + span).min(busy.len()) {
                busy[c2] = rspan;
            }
            if span == 1 && ix < col_widths.len() {
                // Дорожку задаёт размер ячейки вдоль ИНЛАЙН-ОСИ ТАБЛИЦЫ.
                // Вертикальная таблица: ось вертикальна — дорожка из ВЫСОТЫ
                // ячейки (width остаётся её коробке, table-cell-align-002).
                // Горизонтальная: из ширины; у ортогональной ячейки
                // (вертикальное письмо в htb-таблице) `block-size` лёг в
                // height (размеры при вертикали не переставляются, см.
                // resolve_logical) — он и задаёт колонку.
                let table_vertical =
                    e.style.vertical == Some(true) || inherited.vertical == Some(true);
                let orthogonal = cell.style.vertical == Some(true) && !table_vertical;
                let source = if table_vertical {
                    cell.style.height
                } else if orthogonal && cell.style.width.is_none() {
                    cell.style.height
                } else {
                    cell.style.width
                };
                match source {
                    Some(Len::Px(v)) => {
                        let slot = &mut col_widths[ix].0;
                        *slot = Some(slot.map_or(v, |old| old.max(v)));
                    }
                    Some(Len::Pct(k)) => {
                        let slot = &mut col_widths[ix].1;
                        *slot = Some(slot.map_or(k, |old| old.max(k)));
                    }
                    // Шрифтовые единицы решаются кеглем САМОЙ ячейки
                    // (наследование row -> table): `td { width: 2em }` при
                    // `table { font: 50px }` — колонка 100px, не пропуск.
                    Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) => {
                        let size = match cell
                            .style
                            .font_size
                            .or(row.style.font_size)
                            .or(inherited.font_size)
                        {
                            Some(Len::Px(v)) => v,
                            _ => table_font,
                        };
                        let family = cell
                            .style
                            .font_family
                            .clone()
                            .unwrap_or_else(|| table_family.clone());
                        let v = crate::metrics::spacing_px(Some(l), &family, size);
                        if v > 0.0 {
                            let slot = &mut col_widths[ix].0;
                            *slot = Some(slot.map_or(v, |old| old.max(v)));
                        }
                    }
                    _ => {}
                }
            }
            ix += span;
        }
    }
    // Фон КОЛОНКИ картинкой — той же полосой, что фон ряда: слой на площадь
    // колонки, обрезанный прямоугольниками её ячеек. Полосы колонок идут
    // ПЕРЕД рядами: колонка рисуется ниже ряда (css-tables-3 §layers).
    let col_els = col_elements(&e.children);
    let mut col_rects: Vec<Option<crate::interact::RowRects>> = vec![None; cols as usize];
    {
        let mut seen: Vec<u64> = vec![];
        for (i, el) in col_els.iter().enumerate() {
            let Some(el) = el else { continue };
            let picture = el.style.bg_image.is_some() || el.style.gradient_raw.is_some();
            // Колонка/группа с одним ЦВЕТОМ тоже красится полосой: своей
            // коробки у неё нет, фон рисуют её ячейки.
            if !(picture || el.style.background.is_some() || !el.style.shadows.is_empty()) {
                continue;
            }
            let rects = crate::interact::row_rects_for(el.node_id ^ opts.doc_salt);
            if i < col_rects.len() {
                col_rects[i] = Some(rects.clone());
            }
            if !seen.contains(&el.node_id) {
                seen.push(el.node_id);
                let mut band_style = el.style.clone();
                if band_style.bg_image.is_none() {
                    band_style.bg_image = band_style.gradient_raw.clone();
                }
                cells.push(
                    crate::interact::CellsClipped::new(rects, band_style).into_any_element(),
                );
            }
        }
    }
    // Ширины рамки самой таблицы: крайние ячейки расползаются фоном на её
    // половину в сросшейся модели.
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, "", 16.0);
    let table_border = e.style.borders();
    let bw = [
        px_of(table_border.top),
        px_of(table_border.right),
        px_of(table_border.bottom),
        px_of(table_border.left),
    ];
    let table_edges = crate::interact::cell_edges_for(e.node_id ^ opts.doc_salt);
    let collapse_cells = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    // Легаси-атрибут `rules` (HTML rendering §15.3.10): `groups` даёт
    // группам рядов тонкие кромки по умолчанию.
    let rules_groups = e
        .attr("rules")
        .is_some_and(|v| v.eq_ignore_ascii_case("groups"));
    // Границы ГРУПП РЯДОВ: первый/последний ряд группы несёт её кромку
    // (UA-хинт `rules=groups` — тонкая сплошная, если авторState не задал).
    let mut group_of: std::collections::HashMap<u64, (&Element, bool, bool)> =
        std::collections::HashMap::new();
    for child in &e.children {
        let Node::Element(g) = child else { continue };
        if !matches!(g.tag.as_str(), "thead" | "tbody" | "tfoot") {
            continue;
        }
        let trs: Vec<u64> = g
            .children
            .iter()
            .filter_map(|c| match c {
                Node::Element(r) if r.tag == "tr" => Some(r.node_id),
                _ => None,
            })
            .collect();
        for (i, id) in trs.iter().enumerate() {
            group_of.insert(*id, (g, i == 0, i + 1 == trs.len()));
        }
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ: подавать ряды в обратном порядке для vertical-rl
    // (ряды-колонки от правого края). Без обратных охватов rowspan (сетка
    // умеет спан только вперёд) -001 пары ушли 1.02 → 1.31; -003 выиграла
    // 1.18 → 0.82 — нетто минус. Возвращаться с ЯВНОЙ расстановкой клеток.
    let mut row_ix = 0i16;
    // Занятость колонок ячейками с rowspan из ПРЕДЫДУЩИХ рядов: без неё
    // номер колонки считался по порядку детей ряда и съезжал — рамки,
    // схлопнутые колонки и пробы фона приписывались не тем колонкам.
    // Алгоритм тот же, что у авторазмещения сетки: занятые клетки
    // пропускаются.
    let mut occupied: Vec<u16> = vec![0; cols as usize];
    for (row, carry) in rows {
        row_ix += 1;
        for slot in occupied.iter_mut() {
            *slot = slot.saturating_sub(1);
        }
        // Фон ряда КАРТИНКОЙ (css-tables-3 §drawing-backgrounds): рисуется в
        // ЯЧЕЙКАХ, непрерывно от начала ряда, зазоры остаются чистыми.
        // Полоса на весь ряд несёт слой фона, но обрезает его прямоугольниками
        // ячеек, снятыми пробами прошлого кадра.
        let row_rects: Option<crate::interact::RowRects> = (row.style.bg_image.is_some()
            || row.style.gradient_raw.is_some()
            || !row.style.shadows.is_empty())
        .then(|| crate::interact::row_rects_for(row.node_id ^ opts.doc_salt));
        if let Some(rects) = &row_rects {
            // Градиент ряда идёт слоем-картинкой: источник понимает записи
            // `linear-gradient(...)` и растрирует их сам.
            let mut band_style = row.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            cells.push(
                crate::interact::CellsClipped::new(rects.clone(), band_style).into_any_element(),
            );
        }
        let shift = (carry.0, carry.1);
        // Письмо к строкам НЕ ПРИМЕНЯЕТСЯ (раскладку ряда ведёт таблица,
        // CSS Writing Modes §3.1) — но ВЫЧИСЛЕННОЕ значение наследуется в
        // ячейки как у любого свойства: `tr { writing-mode; line-height: 5ch }`
        // обязан дать ячейке вертикальное содержимое (ch-units-vrl-*).
        // Ряд у нас и так не строит своей коробки — урезать нечего.
        let own = row.style.clone();
        // Слой ГРУППЫ строк между таблицей и рядом: наследуемое с `<tbody>`
        // течёт вниз, как у любого предка.
        let group_layer;
        let inherited = match carry.3 {
            Some(g) => {
                group_layer = inline::inherit(inherited, g);
                &group_layer
            }
            None => inherited,
        };
        // Направление на строке ОСТАЁТСЯ: замерено, что его обнуление сдвигает
        // ячейки в парах `position-relative-table-*-left` (29 → 25).
        // ПРОБОВАЛИ ТРИЖДЫ И ОТКАТИЛИ: доводить до ячеек наследуемые свойства
        // САМОЙ таблицы (`inline::inherit(inherited, &e.style)` как основа).
        // Дыра настоящая — `white-space` и шрифт с тега таблицы до ячейки не
        // доходят, — но цена: css-text −3 (`shaping-tatweel-002/003`,
        // `shaping-join-003`), а выигрыш НУЛЕВОЙ: семейство
        // `ws-break-spaces-applies-to` не двигается ни на пару. Значит
        // сохранённые пробелы в ячейке теряются НЕ здесь, и до того, как
        // найдено настоящее место, правка только вредит.
        let row_style = inline::inherit(&inline::inherit(inherited, &e.style), &own);
        let mut col_ix = 0usize;
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if !is_cell(cell) {
                continue;
            }
            while col_ix < occupied.len() && occupied[col_ix] > 0 {
                col_ix += 1;
            }
            let cm = inline::inherit(&row_style, &cell.style);
            // Объединение ячеек: без него ячейка занимала одну дорожку, и всё
            // правее неё съезжало на колонку влево.
            let span_cols: u16 = cell
                .attr("colspan")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
            let span_rows: u16 = cell
                .attr("rowspan")
                .and_then(|v| v.parse().ok())
                .unwrap_or(1)
                .max(1);
            // Фон и рамка строки переносятся на её ячейки: своей строки как
            // элемента больше нет, а зебра и разделители нужны.
            // Обрезка снимается С САМОЙ ячейки и вешается на её содержимое.
            // Причина: раскладка под нами, увидев `overflow: hidden`, снимает
            // с элемента автоминимум — по CSS так и надо, — а размера от
            // таблицы ячейка не получает, и вся она схлопывается в ноль
            // (замерено: `flexbox_rowspan-overflow` рисовал пустую страницу).
            // Коробка ячейки при этом обрезать содержимое не перестаёт.
            // Объединённая ячейка ЧЕРЕЗ схлопнутую колонку обрезается по
            // урезанной ширине (css-tables-3 §visibility-collapse-cell-
            // rendering): содержимое не расталкивает оставшиеся колонки.
            let spans_collapsed = span_cols > 1
                && (col_ix..col_ix + span_cols as usize)
                    .any(|i| cols_collapsed.get(i).copied().unwrap_or(false));
            let clipped = cell.style.overflow_x == Some(crate::computed::Overflow::Hidden)
                || cell.style.overflow_y == Some(crate::computed::Overflow::Hidden)
                || spans_collapsed;
            let mut cell = cell.clone();
            // Сросшиеся рамки (border-collapse): рамки С ЯЧЕЕК СНИМАЮТСЯ
            // целиком — их рисует отдельный слой кромок на линиях сетки
            // (см. interact::EdgePainter): кромка соседей ОДНА, рисуется
            // поверх фонов, и «шире побеждает» решается наложением.
            let cell_edge = if collapse_cells {
                let b = cell.style.borders();
                let widths = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                let black = crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
                let side_colour = |i: usize| {
                    cell.style.border_colors[i]
                        .or(cell.style.border_color)
                        .unwrap_or(black)
                };
                let colors = [side_colour(0), side_colour(1), side_colour(2), side_colour(3)];
                let side_style = |i: usize| {
                    cell.style.border_side_styles[i]
                        .unwrap_or(if widths[i] > 0.0 { 9 } else { 0 })
                };
                let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                cell.style.border_width = Default::default();
                cell.style.border_visible = [None; 4];
                (widths.iter().any(|w| *w > 0.0) || styles.contains(&1))
                    .then_some((widths, colors, styles, cell.node_id as u32))
            } else {
                None
            };
            if clipped {
                cell.style.overflow_x = None;
                cell.style.overflow_y = None;
            }
            // Ячейка схлопнутой колонки не рисуется: колонка выброшена
            // (css-tables-3 §visibility-collapse-cell-rendering), её дорожка
            // нулевая, а краска ячейки торчала бы поверх соседей.
            if (col_ix..col_ix + span_cols as usize)
                .all(|i| cols_collapsed.get(i).copied().unwrap_or(false))
            {
                cell.style.hidden = Some(true);
            }
            // Ширину ячейки несёт КОЛОНКА (см. col_widths): на коробке она
            // резала бы ячейку уже содержимого (`width: 0` прятал текст).
            // Снимается с ЛЮБОЙ ячейки: у объединённой (colspan) ширина на
            // коробке резала её до одной колонки, хотя место ей — весь охват.
            if matches!(cell.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
                cell.style.width = None;
            }
            // Письмо к рядам и группам рядов не применяется (css-writing-modes
            // §applies), а РАЗМЕЩЕНИЕ ячеек в решётке всегда ведёт письмо
    // таблицы — оно уже посчитано табличным кодом. Собственное письмо
            // ячейки остаётся: оно законно управляет её СОДЕРЖИМЫМ
            // (ортогональные ячейки, table-cell-align-002).
            // `ch` на высоте ячейки разрешается с письмом РЯДА: при
            // vertical + upright продвижение нуля — кегль (css-values-3,
            // ch-units-vrl-*). Только ch: полный resolve_em здесь двигал
            // em-высоты и был нетто-минусом (замерено, откат 8b59418-ядра).
            if let Some(Len::Ch(k)) = cell.style.height {
                // Флаги РЯДА, не свои: свой upright ячейки давал кегль там,
                // где эталон меряет лежачим нулём (ch-units-vrl-007/008 —
                // расхождение путей резолва div-эмуляции, вернуться при
                // унификации resolve_em).
                let upright = row.style.upright.or(inherited.upright) == Some(true);
                let vertical = row.style.vertical.or(inherited.vertical) == Some(true);
                let base = match inherited.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let ch = if vertical && upright {
                    base
                } else {
                    let family = inherited.font_family.clone().unwrap_or_default();
                    crate::metrics::ch_ex_px(&family, base).0
                };
                cell.style.height = Some(Len::Px(k * ch));
            }
            // Ортогональная ячейка (вертикальный контент от ряда) не уже
            // ТОЛЩИНЫ своей вертикальной строки — вклад стека в дорожку
            // сжимался до колонки в один глиф (ch-units-vrl-001: 19 вместо
            // line-height 100).
            if cell
                .style
                .vertical
                .or(row.style.vertical)
                .or(inherited.vertical)
                == Some(true)
                && cell.style.min_width.is_none()
                && cell.style.width.is_none()
            {
                let upright =
                    row.style.upright.or(inherited.upright) == Some(true);
                let base = match inherited.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let lh_raw = cell
                    .style
                    .line_height
                    .or(row.style.line_height)
                    .or(inherited.line_height);
                let lh = match lh_raw {
                    Some(Len::Px(v)) => Some(v),
                    Some(Len::Em(k)) => Some(k * base),
                    Some(Len::Ch(k)) => Some(if upright {
                        k * base
                    } else {
                        let family =
                            inherited.font_family.clone().unwrap_or_default();
                        k * crate::metrics::ch_ex_px(&family, base).0
                    }),
                    _ => None,
                };
                if let Some(w) = lh {
                    cell.style.min_width = Some(Len::Px(w));
                }
            }
            // Высота ячейки — МИНИМУМ (css-tables §3.6): содержимое выше
            // растит ячейку, а не режется. `height: 20px` с блоком в 300
            // прятал всё под обрезкой.
            if let Some(Len::Px(h)) = cell.style.height {
                // Процентная высота ПРЯМОГО ребёнка решается от ЗАДАННОЙ
                // высоты ячейки (CSS 2.1 §10.5): раскладка под нами при
                // auto-росте ячейки трактует долю как auto, и ребёнок с
                // overflow и height:100% раздувался содержимым вместо
                // прокрутки в заданных ста точках.
                for child in cell.children.iter_mut() {
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.height
                    {
                        el.style.height = Some(Len::Px(h * k));
                    }
                }
                cell.style.height = None;
                let floor = match cell.style.min_height {
                    Some(Len::Px(v)) => v.max(h),
                    _ => h,
                };
                cell.style.min_height = Some(Len::Px(floor));
            } else {
                // Высота ячейки НЕ задана: доля ребёнка решается от высоты
                // ряда, а вклад ряда меряется БЕЗ доли (двухпроходная
                // раздача css-tables-3 §height-distribution). Однопроходное
                // приближение: якорь — собственный min-height ребёнка,
                // прокрутка держит содержимое внутри него.
                for child in cell.children.iter_mut() {
                    if let Node::Element(el) = child
                        && let Some(Len::Pct(k)) = el.style.height
                        && el.style.overflow_y.is_some_and(|o| o != crate::computed::Overflow::Visible)
                        && let Some(Len::Px(m)) = el.style.min_height
                    {
                        el.style.height = Some(Len::Px(m * k));
                    }
                }
            }
            // Потолок высоты к ячейке не применяется вовсе (браузеры
            // игнорируют max-height на ячейках): содержимое выше — растит.
            if matches!(cell.style.max_height, Some(Len::Px(_))) {
                cell.style.max_height = None;
            }
            let cell = &cell;
            let mut d = styled_div(cell);
            // Заливка строки И ГРУППЫ строк: своей коробки у них в общей сетке
            // не остаётся, поэтому фон рисуют ячейки. Раньше бралась только
            // строка, и `<tbody style="background">` пропадал молча
            // (`position-relative-table-tbody-left`).
            if let Some(bg) = carry.2 {
                // Ряд с КАРТИНКОЙ красит и цвет САМ (см. CellsClipped) —
                // ячейка его не дублирует, иначе цвет ложится поверх
                // картинки. Ряду только с тенью цвет оставляют ячейки.
                let picture =
                    row.style.bg_image.is_some() || row.style.gradient_raw.is_some();
                if !picture {
                    d = d.bg(bg.to_hsla());
                }
            }
            // Сдвиг строки или её группы: собственного элемента у них нет,
            // поэтому край, заданный на `<tr>`/`<tbody>`, двигает ячейки.
            if shift != (0.0, 0.0) {
                d = d.relative().left(px(shift.0)).top(px(shift.1));
            }
            // Умолчание браузера для ячейки — `vertical-align: middle`: без
            // него полоса высотой 10px в строке 22px стояла на 6 точек выше.
            let mut d = d.flex().flex_col();
            if cm.vertical == Some(true) && e.style.vertical != Some(true) {
                // ОРТОГОНАЛЬНАЯ ячейка (вертикальный контент в горизонтальной
                // таблице): строчная ось вертикальна — `text-align` правит
                // ВЕРТИКАЛЬНОЕ положение строки (line-left = верх), а
                // `vertical-align` уходит на поперечную ось
                // (table-cell-align-005/006).
                use crate::computed::TextAlign;
                d = match cm.text_align {
                    Some(TextAlign::Right) => d.justify_end(),
                    Some(TextAlign::Center) => d.justify_center(),
                    _ => d.justify_start(),
                };
                d = match cm.vertical_align {
                    Some(Align::Start) => d.items_start(),
                    Some(Align::End) => d.items_end(),
                    _ => d.items_center(),
                };
            } else {
                d = match cm.vertical_align {
                    Some(Align::Start) => d.justify_start(),
                    Some(Align::End) => d.justify_end(),
                    _ => d.justify_center(),
                };
            }
            // Вертикальное письмо таблицы: ряды идут ПОПЕРЁК — охваты
            // меняются осями вместе с сеткой (css-writing-modes-3 §8).
            let (grid_cols, grid_rows) = if e.style.vertical == Some(true) {
                (span_rows as u16, span_cols as u16)
            } else {
                (span_cols, span_rows as u16)
            };
            if grid_cols > 1 {
                d = d.col_span(grid_cols);
            }
            if grid_rows > 1 {
                d = d.row_span(grid_rows);
            }
            // Явные координаты вместо авто-потока: у `vertical-rl` ряды идут
            // от ПРАВОГО края, а авто-поток умеет только вперёд — реверс
            // рядов ломал охваты (замерено: -001 1.02 → 1.31, откачено).
            if e.style.vertical == Some(true) {
                let n_rows = row_elements.len() as i16;
                let gc = if e.style.vertical_rl == Some(true) {
                    n_rows - row_ix - (span_rows as i16) + 2
                } else {
                    row_ix
                };
                // Строчная ось вертикальной таблицы: `dir=rtl` разворачивает
                // её (ячейки снизу вверх), `text-orientation: upright`
                // ФОРСИРУЕТ ltr (§5.1 — upright задаёт направление ltr), а у
                // `sideways-lr` базовое направление само снизу вверх —
                // разворот инвертируется.
                let rtl_line =
                    e.style.rtl == Some(true) && e.style.upright != Some(true);
                let base_up = e.style.sideways == Some(true)
                    && e.style.vertical_rl != Some(true);
                let gr = if rtl_line != base_up {
                    cols as i16 - col_ix as i16 - span_cols as i16 + 1
                } else {
                    col_ix as i16 + 1
                };
                d = d.col_start(gc.max(1)).row_start(gr.max(1));
            } else if e.style.rtl == Some(true) {
                // `dir=rtl` на таблице: колонки идут от ПРАВОГО края
                // (CSS 2.2 §17.2) — та же явная расстановка, зеркалом.
                let gc = cols as i16 - col_ix as i16 - span_cols as i16 + 1;
                d = d.col_start(gc.max(1)).row_start(row_ix);
            }
            for c in col_ix..(col_ix + span_cols as usize).min(occupied.len()) {
                occupied[c] = span_rows;
            }
            col_ix += span_cols as usize;
            let inside = blocks(&cell.children, &cm, opts);
            // Обрезанная ячейка не расталкивает колонки: её минимальный
            // вклад в дорожки НУЛЕВОЙ (css-sizing: automatic minimum при
            // overflow, отличном от visible, равен нулю) — иначе длинное
            // слово в обрезаемой объединённой ячейке раздавало ширину
            // колонкам, которых оно не должно касаться.
            if clipped {
                d = d.min_w(px(0.0));
            }
            let inside: Vec<AnyElement> = if spans_collapsed {
                // Ячейка через схлопнутую колонку: содержимое НЕ влияет на
                // ширины колонок вовсе (css-tables-3 §visibility-collapse) —
                // раскладка не должна его мерить, поэтому слой абсолютный.
                vec![
                    div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .size_full()
                        .overflow_hidden()
                        .children(inside)
                        .into_any_element(),
                ]
            } else if clipped {
                vec![
                    div()
                        .overflow_hidden()
                        .size_full()
                        .children(inside)
                        .into_any_element(),
                ]
            } else {
                inside
            };
            let mut d = d;
            // Полосы фонов рядов и колонок в сросшейся модели начинаются от
            // СЕРЕДИНЫ рамки таблицы (CSS 2.1 §17.6.2): пробы сдвинуты на
            // полкромки — сами ячейки остаются в потоке с полной рамкой.
            let shift = if collapse_cells {
                (-bw[3] / 2.0, -bw[0] / 2.0)
            } else {
                (0.0, 0.0)
            };
            if let Some((widths, colors, styles, doc_ix)) = cell_edge {
                d = d.child(crate::interact::edge_probe(
                    table_edges.clone(),
                    widths,
                    colors,
                    styles,
                    4,
                    doc_ix,
                    [0.0; 4],
                ));
            }
            if let Some(rects) = &row_rects {
                d = d.child(crate::interact::cell_rect_probe(rects.clone(), span_rows == 1, shift));
            }
            // Проба и для колонок ячейки: объединённая регистрируется в
            // каждой накрытой колонке — полоса колонки красит её целиком.
            let cell_cols = (col_ix - span_cols as usize)..col_ix;
            let mut probed: Vec<u64> = vec![];
            for i in cell_cols.clone() {
                if let (Some(rects), Some(el)) = (col_rects.get(i).and_then(|r| r.clone()), col_els.get(i).copied().flatten()) {
                    if !probed.contains(&el.node_id) {
                        probed.push(el.node_id);
                        d = d.child(crate::interact::cell_rect_probe(rects, span_cols == 1, shift));
                    }
                }
            }
            // Кромки РЯДА (border на <tr>) — участник разбора сросшихся
            // конфликтов (CSS 2.1 §17.6.2.1: ячейка > ряд > группа >
            // колонка > таблица); в раздельной модели рамки ряда не
            // действуют вовсе (§17.6.1) — сюда попадает только collapse.
            if collapse_cells {
                let b = row.style.borders();
                let rw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                let hidden_row = row.style.border_side_styles.contains(&Some(1));
                if rw.iter().any(|w| *w > 0.0) || hidden_row {
                    let start_col = col_ix - span_cols as usize;
                    let last_col = col_ix >= cols as usize;
                    let widths = [
                        rw[0],
                        if last_col { rw[1] } else { 0.0 },
                        rw[2],
                        if start_col == 0 { rw[3] } else { 0.0 },
                    ];
                    let black = crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
                    let side_colour = |k: usize| {
                        row.style.border_colors[k]
                            .or(row.style.border_color)
                            .unwrap_or(black)
                    };
                    let colors =
                        [side_colour(0), side_colour(1), side_colour(2), side_colour(3)];
                    let side_style = |k: usize| {
                        row.style.border_side_styles[k]
                            .unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
                    };
                    let styles =
                        [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        3,
                        row.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
            // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
            if collapse_cells
                && let Some((g, first, last)) = group_of.get(&row.node_id).copied()
            {
                let b = g.style.borders();
                let default_w = if rules_groups { 1.0 } else { 0.0 };
                let explicit_top = g.style.border_width.top.is_some()
                    || g.style.border_visible[0].is_some();
                let explicit_bottom = g.style.border_width.bottom.is_some()
                    || g.style.border_visible[2].is_some();
                let top_w = if explicit_top { px_of(b.top) } else { default_w };
                let bottom_w = if explicit_bottom { px_of(b.bottom) } else { default_w };
                // Боковые кромки группы несут крайние ячейки ряда.
                let start_col = col_ix - span_cols as usize;
                let last_col = col_ix >= cols as usize;
                let widths = [
                    if first { top_w } else { 0.0 },
                    if last_col { px_of(b.right) } else { 0.0 },
                    if last { bottom_w } else { 0.0 },
                    if start_col == 0 { px_of(b.left) } else { 0.0 },
                ];
                if widths.iter().any(|w| *w > 0.0) {
                    let black = crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
                    let side_colour = |k: usize| {
                        g.style.border_colors[k].or(g.style.border_color).unwrap_or(black)
                    };
                    let colors = [side_colour(0), side_colour(1), side_colour(2), side_colour(3)];
                    let side_style = |k: usize| {
                        g.style.border_side_styles[k]
                            .unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        2,
                        g.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки КОЛОНКИ (рамка <col>/<colgroup>) — участники разбора
            // сросшихся конфликтов (источник между ячейкой и таблицей):
            // ячейка колонки несёт её кромку на совпадающем со спаном
            // колонки краю; верх/низ — только крайние ряды.
            if collapse_cells {
                for i in cell_cols {
                    let Some(el) = col_els.get(i).copied().flatten() else { continue };
                    let b = el.style.borders();
                    let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                    let hidden = el.style.border_side_styles.contains(&Some(1));
                    if !(cw.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let same = |j: i64| -> bool {
                        j >= 0
                            && col_els
                                .get(j as usize)
                                .copied()
                                .flatten()
                                .is_some_and(|o| o.node_id == el.node_id)
                    };
                    let left_edge = !same(i as i64 - 1);
                    let right_edge = !same(i as i64 + 1);
                    let last_row = row_ix as usize >= row_elements.len();
                    let widths = [
                        if row_ix == 1 { cw[0] } else { 0.0 },
                        if right_edge { cw[1] } else { 0.0 },
                        if last_row { cw[2] } else { 0.0 },
                        if left_edge { cw[3] } else { 0.0 },
                    ];
                    if !(widths.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let black = crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
                    let side_colour = |k: usize| {
                        el.style.border_colors[k].or(el.style.border_color).unwrap_or(black)
                    };
                    let colors = [side_colour(0), side_colour(1), side_colour(2), side_colour(3)];
                    let side_style = |k: usize| {
                        el.style.border_side_styles[k]
                            .unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        1,
                        el.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            cells.push(d.children(inside).into_any_element());
        }
    }

    if e.style.border_collapse == Some(true) {
        cells.push(crate::interact::EdgePainter::new(table_edges.clone()).into_any_element());
    }
    // Заголовок таблицы живёт ВНЕ коробки таблицы (CSS 2.1 §17.4:
    // анонимная обёртка держит заголовок и коробку) — рамка и обрезка
    // таблицы его не трогают; `caption-side: bottom` ставит его под сетку.
    let mut caption: Option<AnyElement> = None;
    let mut caption_bottom = false;
    for c in &e.children {
        if let Node::Element(cap) = c
            && cap.tag == "caption"
        {
            let cm = inline::inherit(inherited, &cap.style);
            caption_bottom = cap.style.caption_bottom == Some(true);
            caption = Some(
                styled_div(cap)
                    .flex()
                    .flex_col()
                    .children(blocks(&cap.children, &cm, opts))
                    .into_any_element(),
            );
            break;
        }
    }

    // Оси таблицы ЛОГИЧЕСКИЕ, как и у сетки: колонки идут вдоль строки. При
    // вертикальном письме строка идёт сверху вниз, и дорожки колонок
    // становятся физическими рядами. Своей ветки у таблицы не было, и её
    // сетка строилась физической — мимо уже переставленных осей.
    // Ширины колонок фиксированной раскладки — из ПЕРВОГО ряда
    // (CSS 2.1 §17.5.2.1): ячейка с шириной держит её, остальные делят
    // остаток поровну.
    let first_row_widths: Vec<Option<f32>> = row_elements
        .first()
        .map(|row| {
            let mut out = vec![];
            for c in &row.children {
                if let Node::Element(cell) = c
                    && is_cell(cell)
                {
                    let span = cell
                        .attr("colspan")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    match cell.style.width {
                        Some(Len::Px(v)) if span == 1 => out.push(Some(v)),
                        _ => out.extend(std::iter::repeat_n(None, span)),
                    }
                }
            }
            out
        })
        .unwrap_or_default();
    // `<col>`-ширины старше ячеек первого ряда (§17.5.2.1) и действуют и в
    // авто-раскладке: колонка с шириной держит её (как ширина ячейки).
    for (i, w) in from_cols.iter().enumerate() {
        if let (Some(w), Some(slot)) = (w, col_widths.get_mut(i)) {
            slot.0 = Some(slot.0.map_or(*w, |old| old.max(*w)));
        }
    }
    let first_row_widths: Vec<Option<f32>> = (0..cols as usize)
        .map(|i| {
            from_cols
                .get(i)
                .copied()
                .flatten()
                .or_else(|| first_row_widths.get(i).copied().flatten())
        })
        .collect();
    if std::env::var("HTML_ROWBG").is_ok() {
        eprintln!("TABLE cols={} col_widths={:?} first_row={:?}", cols, col_widths, first_row_widths);
    }
    let tracks = track_list_collapsed(
        cols,
        e.style.table_fixed == Some(true),
        &first_row_widths,
        &col_widths,
        &cols_collapsed,
    );
    if std::env::var("HTML_ROWBG").is_ok() {
        eprintln!("TABLE tracks={:?}", tracks);
    }
    // Таблица ЗАДАННОЙ высоты раздаёт лишнее место рядам БЕЗ своей высоты
    // (CSS 2.1 §17.5.3): ряд с высотой (своей или ячеек) держит её, остальные
    // делят остаток. Без этого средний ряд решётки 64/auto/64 в таблице 224px
    // схлопывался по содержимому, и вся середина уезжала.
    // Считается и от min-height: минимум так же растягивает таблицу, и
    // остаток обязан достаться безвысотным рядам.
    let table_tall = matches!(e.style.height, Some(Len::Px(_)))
        || matches!(e.style.min_height, Some(Len::Px(_)));
    let row_tracks: Option<Vec<gpui::GridTrack>> = match table_tall {
        true if e.style.vertical != Some(true) => Some(
            row_elements
                .iter()
                .map(|row| {
                    let cell_h = |c: &Node| match c {
                        Node::Element(cell) if is_cell(cell) => match cell.style.height {
                            Some(Len::Px(v)) => Some(v),
                            _ => None,
                        },
                        _ => None,
                    };
                    let own = match row.style.height {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    };
                    match own.into_iter().chain(row.children.iter().filter_map(cell_h)).fold(None::<f32>, |a, v| Some(a.map_or(v, |x| x.max(v)))) {
                        Some(h) => gpui::GridTrack::Pixels(px(h)),
                        None => gpui::GridTrack::Fraction(1.0),
                    }
                })
                .collect(),
        ),
        _ => None,
    };
    let grid_box = if e.style.vertical == Some(true) {
        // Ряд таблицы — КОЛОНКА сетки: заполнение идёт сверху вниз, ряд за
        // рядом поперёк (css-writing-modes-3 §8, table-progression-*).
        let mut g = div().grid().grid_template_rows(tracks);
        g.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
        g
    } else {
        let mut g = div().grid().grid_template_cols(tracks);
        if let Some(rt) = row_tracks {
            // Сетка обязана занять ВСЮ высоту таблицы: доли рядов считаются
            // от её остатка, а auto-высота ребёнка гибкой колонки — ноль.
            g = g.grid_template_rows(rt).flex_grow();
        }
        g
    };
    // Сросшиеся рамки: у таблицы нет паддинга, а кромка между её рамкой и
    // краевыми ячейками одна — ячейки накрывают ВНУТРЕННЮЮ ПОЛОВИНУ рамки
    // (CSS 2.1 §17.6.2). Рамка при этом рисуется ПОВЕРХ фонов ячеек, как и
    // все сросшиеся кромки: обычная рамка коробки красится под детьми и
    // закрашивалась бы их фоном. Поэтому у самой коробки рамка снимается,
    // её место держит паддинг, сетка выезжает на его половину, а красит
    // рамку кольцевой квад ПОСЛЕ сетки.
    let collapse = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    // Минимумы таблицы меряются ПОЛНОЙ коробкой с рамкой и паддингом
    // (css-tables-3 §computing-the-table-height, CSSWG #5336): пороги
    // пересчитываются в контентные, компенсацию вернёт общий слой.
    let min_fix = |len: Option<Len>, edges: f32| match len {
        Some(Len::Px(v)) if e.style.border_box != Some(true) => {
            Some(Len::Px((v - edges).max(0.0)))
        }
        other => other,
    };
    let pad = &e.style.padding;
    let pad_px = [
        px_of(pad.top),
        px_of(pad.right),
        px_of(pad.bottom),
        px_of(pad.left),
    ];
    let min_h = min_fix(e.style.min_height, bw[0] + bw[2] + pad_px[0] + pad_px[2]);
    let min_w = min_fix(e.style.min_width, bw[1] + bw[3] + pad_px[1] + pad_px[3]);
    let needs_clone = collapse
        || min_h != e.style.min_height
        || min_w != e.style.min_width;
    let host_style;
    let mut outer = if needs_clone {
        let mut c = inherited.clone();
        if collapse {
            c.border_width = Default::default();
            c.border_visible = [None; 4];
            c.padding = crate::computed::Sides {
                top: Some(Len::Px(bw[0])),
                right: Some(Len::Px(bw[1])),
                bottom: Some(Len::Px(bw[2])),
                left: Some(Len::Px(bw[3])),
            };
        }
        c.min_height = min_h;
        c.min_width = min_w;
        host_style = c;
        styled_div_with(e, &host_style).flex().flex_col()
    } else {
        styled_div_with(e, inherited).flex().flex_col()
    };
    // Таблица без заданной ширины СЖИМАЕТСЯ по содержимому, а не растягивается
    // на родителя (CSS 2.1 §17.5.2, shrink-to-fit). Пока она растягивалась,
    // две короткие колонки разъезжались к противоположным краям — видно на
    // `shaping-tatweel-002`, где одинаковые знаки стояли по краям окна.
    if e.style.width.is_none() {
        outer.style().align_self = Some(gpui::AlignItems::FlexStart);
    }
    // КОРНЕВОЙ стол (`<html display: table>`): родитель — блок стенда, где
    // `align-self` не работает, и стол растягивался на всё окно. Гибкая
    // обёртка возвращает сжатие по содержимому и центрирование `margin: auto`.
    let root_table = matches!(e.tag.as_str(), "html" | "body") && e.style.width.is_none();
    let mut outer = outer
        .child(
            grid_box
                // `border-spacing: 2px` — умолчание браузера для таблицы с
                // раздельными рамками. Без него строки идут плотнее, и
                // расхождение копится вниз по таблице.
                .gap_x(px(spacing.0))
                .gap_y(px(spacing.1))
                // Зазор действует и МЕЖДУ краем таблицы и крайними ячейками
                // (CSS 2.1 §17.6.1), не только между ячейками. Эталоны
                // гасят его отрицательным полем на таблице.
                .px(px(spacing.0))
                .py(px(spacing.1))
                .children(cells)
                .into_any_element(),
        );
    if collapse && (bw.iter().any(|w| *w > 0.0) || e.style.border_side_styles.contains(&Some(1))) {
        // Рамка самой таблицы — участник разбора конфликтов: её кромки
        // уходят в тот же слой (EdgePainter), линии — внутренние края
        // рамочного места, победившая кромка рисуется наружу.
        let black = crate::value::Color { r: 0.0, g: 0.0, b: 0.0, a: 1.0 };
        let side_colour = |i: usize| {
            e.style.border_colors[i].or(e.style.border_color).unwrap_or(black)
        };
        let colors = [side_colour(0), side_colour(1), side_colour(2), side_colour(3)];
        let side_style =
            |i: usize| e.style.border_side_styles[i].unwrap_or(if bw[i] > 0.0 { 9 } else { 0 });
        let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
        outer = outer.child(crate::interact::edge_probe(
            table_edges.clone(),
            bw,
            colors,
            styles,
            0,
            e.node_id as u32,
            bw,
        ));
    }
    let outer = outer;
    // Обёртка «заголовок + коробка»: заголовок вне рамки и обрезки.
    let outer = if let Some(cap) = caption {
        let mut wrap = div().flex().flex_col();
        wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        if caption_bottom {
            wrap.child(outer).child(cap).into_any_element()
        } else {
            wrap.child(cap).child(outer).into_any_element()
        }
    } else {
        outer.into_any_element()
    };
    if root_table {
        return div()
            .flex()
            .flex_row()
            .w_full()
            .child(outer)
            .into_any_element();
    }
    outer.into_any_element()
}

/// Дорожки таблицы: все по содержимому, последняя забирает остаток строки.
///
/// Ширины колонок считают ИМЕННО дорожки: это внутренние размеры содержимого,
/// посчитанные раскладкой по настоящим правилам переноса. Прежде поверх них
/// работал свой замер (`MeasuredTable`), меривший ГОЛЫЙ текст ячейки — без
/// переносов, сохранённых пробелов и вложенных коробок; снят по замеру:
/// css-text +1, flexbox +1, css-grid +1, поломок нет.
/// `min-content` снизу не даёт колонке сжаться в ноль на узкой панели.
fn track_list_collapsed(
    cols: u16,
    fixed: bool,
    first_row: &[Option<f32>],
    col_widths: &[(Option<f32>, Option<f32>)],
    collapsed: &[bool],
) -> Vec<gpui::GridTrack> {
    let mut tracks = track_list(cols, fixed, first_row, col_widths);
    for (i, t) in tracks.iter_mut().enumerate() {
        if collapsed.get(i).copied().unwrap_or(false) {
            *t = gpui::GridTrack::Pixels(px(0.0));
        }
    }
    tracks
}

fn track_list(
    cols: u16,
    fixed: bool,
    first_row: &[Option<f32>],
    col_widths: &[(Option<f32>, Option<f32>)],
) -> Vec<gpui::GridTrack> {
    // `table-layout: fixed` — ширины из первого ряда, безразмерные колонки
    // делят остаток поровну; содержимое не меряется.
    if fixed {
        return (0..cols as usize)
            .map(|i| match first_row.get(i).copied().flatten() {
                Some(w) => gpui::GridTrack::Pixels(px(w)),
                None => gpui::GridTrack::Fraction(1.0),
            })
            .collect();
    }
    // Все колонки по содержимому. Остаток строки НЕ отдаётся последней:
    // раньше она забирала его целиком, и таблица из двух коротких ячеек
    // расползалась по краям окна (видно на `shaping-join-001`). Излишек
    // раздаёт раскладка между дорожками `auto` — это ближе к табличной
    // раздаче «пропорционально разнице полной и минимальной ширины».
    (0..cols as usize)
        .map(|i| match col_widths.get(i).copied().unwrap_or((None, None)) {
            // Заявленная ширина, но не уже содержимого: minmax с потолком
            // ниже пола отдаёт пол (правило сетки), то есть
            // max(min-content, ширина).
            (Some(w), _) => gpui::GridTrack::MinMax(Box::new((
                gpui::GridTrack::MinContent,
                gpui::GridTrack::Pixels(px(w)),
            ))),
            // Процентная колонка забирает долю ОСТАТКА: соседние колонки по
            // содержимому, свободное место делится по долям.
            (None, Some(k)) => gpui::GridTrack::Fraction(k),
            (None, None) => gpui::GridTrack::MinMax(Box::new((
                gpui::GridTrack::MinContent,
                gpui::GridTrack::Auto,
            ))),
        })
        .collect()
}

/// Сдвиг относительно позиционированной части таблицы.
///
/// Строка и группа строк у нас растворяются в общей сетке — своего элемента
/// у них не остаётся, и `position: relative` вместе с краями пропадал бы
/// молча. Сдвиг переносится на ЯЧЕЙКИ: строка целиком сдвигается ровно
/// настолько же, насколько каждая её ячейка.
fn relative_shift(e: &Element) -> (f32, f32) {
    if e.style.position != Some(crate::computed::Position::Relative) {
        return (0.0, 0.0);
    }
    let side = |a: Option<Len>, b: Option<Len>| match (a, b) {
        (Some(Len::Px(v)), _) => v,
        // Задан только противоположный край — сдвиг в обратную сторону.
        (_, Some(Len::Px(v))) => -v,
        _ => 0.0,
    };
    (
        side(e.style.inset.left, e.style.inset.right),
        side(e.style.inset.top, e.style.inset.bottom),
    )
}

/// Сдвиг, фон и СТИЛЬ ГРУППЫ строк: письмо/шрифт с `<tbody>` наследуются в
/// ряды и ячейки, хотя своей коробки у группы нет (ch-units-vrl-006).
type RowCarry<'a> = (f32, f32, Option<crate::value::Color>, Option<&'a Computed>);

fn collect_rows<'a>(
    nodes: &'a [Node],
    carry: RowCarry<'a>,
    out: &mut Vec<(&'a Element, RowCarry<'a>)>,
) {
    for n in nodes {
        if let Node::Element(e) = n {
            let (dx, dy) = relative_shift(e);
            // Фон группы строк рисуют ЯЧЕЙКИ: своей коробки у группы в общей
            // сетке не остаётся, и заливка пропадала молча
            // (`position-relative-table-tbody-left`: зелёная коробка не
            // рисовалась вовсе, из-под неё светило красное).
            let shift = (
                carry.0 + dx,
                carry.1 + dy,
                e.style.background.or(carry.2),
                carry.3,
            );
            // Роль задаётся тегом ИЛИ стилем: разметка на `div` с
            // `display: table-row` встречается не реже настоящих таблиц.
            if e.tag == "tr" || e.style.display == Some(Display::TableRow) {
                out.push((e, shift));
            } else if e.tag == "thead"
                || e.tag == "tbody"
                || e.tag == "tfoot"
                || e.style.display == Some(Display::TableRowGroup)
            {
                let deeper = (shift.0, shift.1, shift.2, Some(&e.style));
                collect_rows(&e.children, deeper, out);
            }
        }
    }
}

/// Ячейка ли это — по тегу или по стилю.
/// Безымянный элемент починки таблицы: пустой стиль, только тег и дети.
/// Красится ли коробка (фон или рамка) — такой блок в бюджете строк
/// прячется целиком, если точка среза попала внутрь него.
fn has_box_style_probe(c: &Computed) -> bool {
    c.background.is_some()
        || c.bg_image.is_some()
        || c.gradient_raw.is_some()
        || c.border_visible.contains(&Some(true))
}

fn anon_element(tag: &str, children: Vec<Node>) -> Element {
    Element {
        node_id: 0,
        anim: None,
        tag: tag.into(),
        style: Computed::default(),
        hover: None,
        first_letter: None,
        first_line: None,
        children,
        attrs: vec![],
        inline: false,
    }
}

/// Починка детей таблицы (css-tables-3 §3): `display: contents` растворить,
/// бесхозные ячейки и непустой текст завернуть в анонимный ряд.
/// Чинит СОДЕРЖИМОЕ ряда (css-tables-3 §fixup): `display: contents`
/// растворяется с наследованием, последовательные не-ячейки сливаются в
/// одну анонимную ячейку, а вложенный ряд выталкивается ОТДЕЛЬНЫМ рядом
/// после текущего.
fn fixup_row_children(row: &Element) -> Vec<Node> {
    fn walk(
        nodes: &[Node],
        donor: Option<&Computed>,
        cells: &mut Vec<Node>,
        run: &mut Vec<Node>,
        extra: &mut Vec<Node>,
    ) {
        for child in nodes {
            match child {
                Node::Element(el) if el.style.display == Some(Display::Contents) => {
                    // Дети растворённого получают его наследуемое (цвет,
                    // шрифт) — слитый стиль передаётся вниз донором.
                    let merged: Vec<Node> = el
                        .children
                        .iter()
                        .cloned()
                        .map(|n| match n {
                            Node::Element(mut ge) => {
                                ge.style = inline::inherit(&el.style, &ge.style);
                                Node::Element(ge)
                            }
                            // Голый текст стиля не несёт: наследуемое от
                            // растворённого доносит строчная обёртка.
                            Node::Text(t) if !t.trim().is_empty() => {
                                let mut span = anon_element("span", vec![Node::Text(t)]);
                                span.style = el.style.clone();
                                // Сам растворённый display не переносится —
                                // иначе обёртка растворилась бы следом.
                                span.style.display = None;
                                span.inline = true;
                                Node::Element(span)
                            }
                            other => other,
                        })
                        .collect();
                    walk(&merged, donor, cells, run, extra);
                }
                Node::Element(el)
                    if el.tag == "tr"
                        || matches!(
                            el.style.display,
                            Some(Display::TableRow) | Some(Display::TableRowGroup)
                        ) =>
                {
                    extra.extend(fixup_row_children(el));
                }
                Node::Element(el) if is_cell(el) => {
                    if !run.is_empty() {
                        cells.push(Node::Element(anon_element("td", std::mem::take(run))));
                    }
                    cells.push(child.clone());
                }
                Node::Text(t) if !t.trim().is_empty() => run.push(child.clone()),
                Node::Element(_) => run.push(child.clone()),
                _ => {}
            }
        }
        let _ = donor;
    }
    let needs_fix = row.children.iter().any(|c| match c {
        Node::Element(el) => {
            el.style.display == Some(Display::Contents)
                || el.tag == "tr"
                || matches!(
                    el.style.display,
                    Some(Display::TableRow) | Some(Display::TableRowGroup)
                )
                || !is_cell(el)
        }
        Node::Text(t) => !t.trim().is_empty(),
        _ => false,
    });
    if !needs_fix {
        return vec![Node::Element(row.clone())];
    }
    let (mut cells, mut run, mut extra) = (vec![], vec![], vec![]);
    walk(&row.children, None, &mut cells, &mut run, &mut extra);
    if !run.is_empty() {
        cells.push(Node::Element(anon_element("td", run)));
    }
    let mut fixed = row.clone();
    fixed.children = cells;
    let mut out = vec![Node::Element(fixed)];
    out.extend(extra);
    out
}

fn fixup_table_children(children: &[Node]) -> Vec<Node> {
    let mut out: Vec<Node> = vec![];
    let mut stray: Vec<Node> = vec![];
    fn flush(stray: &mut Vec<Node>, out: &mut Vec<Node>) {
        if stray.is_empty() {
            return;
        }
        // ПОСЛЕДОВАТЕЛЬНЫЕ не-ячейки сливаются в ОДНУ анонимную ячейку
        // (css-tables-3 §consecutive-boxes): два inline-block с текстом между
        // ними — одна ячейка с общей строкой, а не ячейка на каждого.
        let mut cells: Vec<Node> = vec![];
        let mut run: Vec<Node> = vec![];
        for n in std::mem::take(stray) {
            match n {
                Node::Element(e) if is_cell(&e) => {
                    if !run.is_empty() {
                        cells.push(Node::Element(anon_element("td", std::mem::take(&mut run))));
                    }
                    cells.push(Node::Element(e));
                }
                other => run.push(other),
            }
        }
        if !run.is_empty() {
            cells.push(Node::Element(anon_element("td", run)));
        }
        out.push(Node::Element(anon_element("tr", cells)));
    }
    for child in children {
        match child {
            Node::Element(el) if el.style.display == Some(Display::Contents) => {
                // Дети идут в таблицу со СЛИТЫМ стилем: наследуемое от
                // растворённого элемента (цвет, шрифт) обязано дойти.
                for grand in fixup_table_children(&el.children) {
                    match grand {
                        Node::Element(mut ge) => {
                            ge.style = inline::inherit(&el.style, &ge.style);
                            let row = ge.tag == "tr"
                                || matches!(
                                    ge.style.display,
                                    Some(Display::TableRow) | Some(Display::TableRowGroup)
                                )
                                || matches!(ge.tag.as_str(), "thead" | "tbody" | "tfoot");
                            if row {
                                flush(&mut stray, &mut out);
                                out.push(Node::Element(ge));
                            } else if is_cell(&ge) {
                                stray.push(Node::Element(ge));
                            } else {
                                stray.push(Node::Element(ge));
                            }
                        }
                        text => stray.push(text),
                    }
                }
            }
            Node::Element(el) => {
                // Колоночные элементы — не содержимое: их читают дорожки.
                if matches!(el.tag.as_str(), "col" | "colgroup") {
                    continue;
                }
                let group = el.style.display == Some(Display::TableRowGroup)
                    || matches!(el.tag.as_str(), "thead" | "tbody" | "tfoot");
                let row = el.tag == "tr"
                    || el.style.display == Some(Display::TableRow)
                    || el.tag == "caption";
                if group {
                    // Группа рядов чинится ИЗНУТРИ тоже: contents и бесхозное
                    // содержимое встречаются и там.
                    flush(&mut stray, &mut out);
                    let mut copy = el.clone();
                    copy.children = fixup_table_children(&el.children);
                    out.push(Node::Element(copy));
                } else if el.tag == "caption" {
                    flush(&mut stray, &mut out);
                    out.push(child.clone());
                } else if row {
                    flush(&mut stray, &mut out);
                    out.extend(fixup_row_children(el));
                } else {
                    stray.push(child.clone());
                }
            }
            Node::Text(t) if !t.trim().is_empty() => stray.push(child.clone()),
            _ => {}
        }
    }
    flush(&mut stray, &mut out);
    out
}

/// Ширины колонок из элементов `<col>`/`<colgroup>` (атрибут `span`
/// повторяет запись): при фиксированной раскладке они СТАРШЕ ячеек первого
/// ряда (CSS 2.1 §17.5.2.1).
/// Элементы `<col>` по индексам колонок (повтор на span): фон колонки
/// рисуется в её ячейках (css-tables-3 §drawing-backgrounds).
fn col_elements(children: &[Node]) -> Vec<Option<&Element>> {
    let mut out: Vec<Option<&Element>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match el.tag.as_str() {
            "col" => {
                let span = el
                    .attr("span")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                out.extend(std::iter::repeat_n(Some(el), span));
            }
            "colgroup" => {
                let inner = col_elements(&el.children);
                if inner.is_empty() {
                    // Группа без <col> внутри сама несёт свои колонки:
                    // её span и стиль (фон группы) ложатся на каждую.
                    let span = el
                        .attr("span")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    out.extend(std::iter::repeat_n(Some(el), span));
                } else {
                    out.extend(inner);
                }
            }
            _ => {}
        }
    }
    out
}

fn col_element_widths(
    children: &[Node],
    base_font: f32,
    family: &str,
) -> (Vec<Option<f32>>, Vec<bool>) {
    let mut widths = vec![];
    let mut collapsed = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match el.tag.as_str() {
            "col" => {
                let w = match el.style.width {
                    Some(Len::Px(v)) => Some(v),
                    // `ch` на колонке считается с ЕЁ письмом: стоячий ноль
                    // продвигается на кегль (ch-units-vrl-003/004). Кегль
                    // колонки — свой или умолчание: шрифт таблицы сюда не
                    // наследуется, а тесты задают его одинаковым.
                    // Только СТОЯЧИЕ (upright): там продвижение — кегль и
                    // сходится с эталоном. Лежачий `ch` (sideways) оставлен
                    // авто-колонке: явный глиф-замер уводил ширину
                    // (ch-units-vrl-007/008 были зелёными на авто).
                    Some(Len::Ch(k))
                        if el.style.upright == Some(true)
                            && el.style.vertical == Some(true) =>
                    {
                        let base = match el.style.font_size {
                            Some(Len::Px(v)) => v,
                            _ => base_font,
                        };
                        let _ = family;
                        Some(k * base)
                    }
                    _ => None,
                };
                // `visibility: collapse` на колонке — колонка ВЫБРОШЕНА:
                // нулевая дорожка, ячейки не рисуются (css-tables-3
                // §visibility-collapse-cell-rendering).
                let c = el.style.collapsed == Some(true);
                let span = el
                    .attr("span")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                widths.extend(std::iter::repeat_n(w, span));
                collapsed.extend(std::iter::repeat_n(c, span));
            }
            "colgroup" => {
                let (w, c) = col_element_widths(&el.children, base_font, family);
                if w.is_empty() {
                    let ww = match el.style.width {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    };
                    let cc = el.style.collapsed == Some(true);
                    let span = el
                        .attr("span")
                        .and_then(|v| v.parse::<usize>().ok())
                        .unwrap_or(1)
                        .max(1);
                    widths.extend(std::iter::repeat_n(ww, span));
                    collapsed.extend(std::iter::repeat_n(cc, span));
                } else {
                    widths.extend(w);
                    collapsed.extend(c);
                }
            }
            _ => {}
        }
    }
    (widths, collapsed)
}

fn is_cell(e: &Element) -> bool {
    e.tag == "td" || e.tag == "th" || e.style.display == Some(Display::TableCell)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dom::parse;

    fn find_class<'a>(nodes: &'a [Node], class: &str) -> Option<&'a Element> {
        for n in nodes {
            if let Node::Element(e) = n {
                if e.attr("class")
                    .is_some_and(|c| c.split_whitespace().any(|x| x == class))
                {
                    return Some(e);
                }
                if let Some(found) = find_class(&e.children, class) {
                    return Some(found);
                }
            }
        }
        None
    }

    /// Разворачивает обёртки документа до содержимого страницы.
    fn page_children(html: &str) -> Vec<Node> {
        fn dive(n: &[Node]) -> Vec<Node> {
            match n.first() {
                Some(Node::Element(e)) if e.tag == "html" || e.tag == "body" => dive(&e.children),
                _ => n.to_vec(),
            }
        }
        dive(&parse(html, ""))
    }

    #[test]
    fn margin_collapse_matches_the_browser_on_the_fixture_case() {
        // Ровно тот случай, на котором сравнение с Chrome показало сдвиг на
        // 10 точек: блок-обёртка без своего отступа сверху и ребёнок с ним.
        let page = page_children(
            "<div class=\"page\">\
               <div class=\"wrap\" style=\"margin: 0 0 10px\">w</div>\
               <div class=\"stack\" style=\"margin: 0 0 10px\">\
                 <div class=\"mt\" style=\"margin-top: 24px\">m</div>\
               </div>\
             </div>",
        );
        let children = match &page[0] {
            Node::Element(e) => collapse_margins(&e.children),
            _ => panic!("нет страницы"),
        };
        let stack = children
            .iter()
            .find_map(|n| match n {
                Node::Element(e) if e.attr("class") == Some("stack") => Some(e),
                _ => None,
            })
            .expect("нет обёртки");
        // Отступ ребёнка вынесен наружу (24) и уменьшен на уже отданные
        // предыдущим блоком 10 — суммарный зазор остаётся 24, как в браузере.
        assert_eq!(stack.style.margin.top, Some(Len::Px(14.0)), "у обёртки");
        let child_top = stack.children.iter().find_map(|n| match n {
            Node::Element(e) => Some(e.style.margin.top),
            _ => None,
        });
        assert_eq!(child_top, Some(Some(Len::Px(0.0))), "у ребёнка снят");
    }

    #[test]
    fn out_of_flow_neighbours_keep_their_margins() {
        // Плавающий блок в схлопывании не участвует: его поле стоит как
        // написано, и соседа он не обкрадывает.
        let nodes = parse(
            "<div style=\"margin: 16px; float: left\">a</div>\
             <div style=\"margin: 16px; float: left\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        for (i, n) in body.iter().enumerate() {
            let Node::Element(e) = n else { continue };
            assert_eq!(
                e.style.margin.top,
                Some(Len::Px(16.0)),
                "плавающий блок {i} потерял поле"
            );
        }
    }

    #[test]
    fn margins_in_em_collapse_too() {
        // `margin: 1em 0` — самая частая запись отступа в разметке: без
        // перевода в точки схлопывание не срабатывало вовсе.
        let nodes = parse(
            "<div style=\"margin-bottom: 1em\">a</div><div style=\"margin-top: 2em\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        let second = match &body[1] {
            Node::Element(e) => e.style.margin.top,
            _ => panic!("нет второго блока"),
        };
        // 32 всего, из них 16 уже дал нижний отступ предыдущего блока.
        assert_eq!(second, Some(Len::Px(16.0)), "получено {second:?}");
    }

    #[test]
    fn adjacent_margins_collapse_into_the_larger() {
        // В CSS нижний отступ одного блока и верхний отступ следующего не
        // складываются: остаётся больший. Иначе документ растёт сверху вниз.
        let nodes = parse(
            "<div style=\"margin-bottom: 10px\">a</div><div style=\"margin-top: 24px\">b</div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        let second = match &body[1] {
            Node::Element(e) => e.style.margin.top,
            _ => panic!("нет второго блока"),
        };
        // 24 всего, из них 10 уже дал нижний отступ предыдущего блока.
        assert_eq!(second, Some(Len::Px(14.0)), "получено {second:?}");
    }

    #[test]
    fn first_child_margin_leaks_through_a_borderless_parent() {
        // Отступ первого ребёнка в CSS — тот же отступ, что у родителя, если
        // между ними нет ни рамки, ни внутреннего отступа.
        let nodes = parse(
            "<div class=\"wrap\"><div class=\"in\" style=\"margin-top: 24px\">x</div></div>",
            "",
        );
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children),
            _ => panic!("нет body"),
        };
        let wrap = match &body[0] {
            Node::Element(e) => e,
            _ => panic!("нет обёртки"),
        };
        assert_eq!(
            wrap.style.margin.top,
            Some(Len::Px(24.0)),
            "отступ вынесен наружу"
        );
        let child_top =
            find_class(std::slice::from_ref(&body[0]), "in").and_then(|e| e.style.margin.top);
        assert_eq!(child_top, Some(Len::Px(0.0)), "у ребёнка отступ снят");
    }
}

/// Высота строки в точках — для статической позиции блочного элемента.
fn line_height_px(style: &Computed, opts: &RenderOpts) -> f32 {
    let size = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * opts.base_size(),
        _ => opts.base_size(),
    };
    match style.line_height {
        Some(Len::Px(v)) => v,
        // Голое число хранится долей: это множитель к кеглю.
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * size,
        _ => size * 1.2,
    }
}



/// Раскладка ЛУНКАМИ (`display: grid-lanes`, CSS Grid 3).
///
/// Решётки тут нет: дорожки задают только поперечную ось, а вдоль потока
/// каждый элемент встаёт в САМУЮ КОРОТКУЮ лунку — как кирпичная кладка. Ни
/// раскладка под нами, ни сетка такого не умеют, поэтому лунки собираются
/// сами: ряд из колонок, а раздача идёт по накопленной высоте.
///
/// Высота элемента берётся из его стиля: в наборе она почти всегда задана
/// явно. Незаданная считается нулём — тогда лунки заполняются по кругу, как
/// и было бы при равных высотах.
fn lanes(e: &Element, merged: &Computed, opts: &RenderOpts) -> AnyElement {
    use crate::computed::{Track, TrackSize};
    // `grid-lanes-direction: row` — лунки идут РЯДАМИ: дорожки задаёт
    // `grid-template-rows`, элементы укладываются вдоль строки, а роль
    // `align-items` играет `justify-items`.
    //
    // Без явного направления его выдаёт ТА ОСЬ, по которой объявлены дорожки:
    // `grid-template-rows: repeat(auto-fill, auto)` без колоночных дорожек —
    // это лунки рядами (`row-auto-repeat-*`: вся укладка шла столбиком,
    // потому что направление читалось только из свойства).
    let row_tracks =
        merged.grid_rows.is_some() || merged.auto_repeat_rows.is_some() || merged.grid_auto_fill_row.is_some();
    let col_tracks = merged.grid_tracks.is_some()
        || merged.auto_repeat_cols.is_some()
        || merged.grid_auto_fill_min.is_some();
    let row_dir = match merged.lanes_row {
        Some(explicit) => explicit,
        None => row_tracks && !col_tracks,
    };
    let tracks = if row_dir {
        merged.grid_rows.clone().unwrap_or_default()
    } else {
        merged.grid_tracks.clone().unwrap_or_default()
    };
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    // `repeat(auto-fill, 100px)` — «сколько влезет»: число лунок считается по
    // размеру контейнера ПОПЕРЁК потока. Без этого счёта вся раскладка
    // схлопывалась в одну лунку (`column-auto-repeat-001`).
    let fill = if row_dir {
        merged.grid_auto_fill_row.zip(px_of(merged.height))
    } else {
        merged.grid_auto_fill_min.zip(px_of(merged.width))
    };
    // Доля зазора считается от размера контейнера ПО ЭТОЙ ЖЕ оси: `gap: 20%`
    // в коробке шириной 300 — это 60 точек по горизонтали. Раньше доля молча
    // отбрасывалась, и зазора не было вовсе (`grid-lanes/gap/*-percentage-*`).
    let px_of_size = |l: Option<Len>| match l {
        Some(Len::Px(v)) => Some(v),
        _ => None,
    };
    let gap_of = |l: Option<Len>, along: Option<f32>| match l {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => along.map(|s| k * s).unwrap_or(0.0),
        _ => 0.0,
    };
    let (box_w, box_h) = (px_of_size(merged.width), px_of_size(merged.height));
    let (row_gap, col_gap) = match merged.gap {
        Some((row, col)) => (gap_of(row, box_h), gap_of(col, box_w)),
        None => (0.0, 0.0),
    };
    // Зазор ВДОЛЬ лунки и зазор МЕЖДУ лунками — разные оси.
    let (along_gap, cross_gap) = if row_dir {
        (col_gap, row_gap)
    } else {
        (row_gap, col_gap)
    };
    let repeat = if row_dir {
        merged.auto_repeat_rows
    } else {
        merged.auto_repeat_cols
    };
    let room = if row_dir {
        px_of(merged.height)
    } else {
        px_of(merged.width)
    };
    let mut tracks = tracks;
    // Повтор «сколько влезет» разворачивается в настоящий список дорожек: от
    // него зависят и размер лунки, и размер элемента на несколько лунок
    // (`column-auto-repeat-001`: коробка на две дорожки). Дорожка `auto`
    // меряется по САМОМУ БОЛЬШОМУ элементу: своей ширины у неё нет, а число
    // повторов всё равно считается по месту (`column-auto-repeat-auto-012`).
    if tracks.is_empty()
        && let (Some(repeat), Some(room)) = (repeat, room)
    {
        // Доля дорожки считается от места контейнера: `repeat(auto-fill,
        // 25%)` в трёхстах точках — это четыре дорожки по 75
        // (`column-auto-repeat-002`).
        let step = repeat
            .track
            .or_else(|| repeat.track_pct.map(|k| k * room))
            .unwrap_or_else(|| {
            e.children
                .iter()
                .filter_map(|n| match n {
                    Node::Element(item) => {
                        let size = if row_dir {
                            item_height(item, merged, opts)
                        } else {
                            item_width(item)
                        };
                        // Доля элемента считается от места контейнера:
                        // `width: 25%` в трёхстах шестидесяти — дорожка 90
                        // (`column-auto-repeat-auto-002`). Точечный замер её
                        // не видел, и дорожки не разворачивались вовсе.
                        let pct = match (row_dir, item.style.height, item.style.width) {
                            (true, Some(Len::Pct(k)), _) => Some(k * room),
                            (false, _, Some(Len::Pct(k))) => Some(k * room),
                            _ => None,
                        };
                        let size = pct.unwrap_or(size);
                        // Вклад элемента НА НЕСКОЛЬКО дорожек делится между
                        // ними (css-grid-2 §11.5.1): `width: 200px` при
                        // `span 2` — это две дорожки по сто, а не одна в
                        // двести. Пока считали целиком, число повторов
                        // выходило 300/200 = 1, и вся укладка шла столбиком
                        // (`column-auto-repeat-auto-001`).
                        let (_, span) = lane_span(item, usize::MAX, row_dir);
                        Some(size / span.max(1) as f32)
                    }
                    _ => None,
                })
                .fold(0.0f32, f32::max)
        });
        if step > 0.0 {
            let n = (((room + cross_gap) / (step + cross_gap)).floor() as usize).max(1);
            tracks = match repeat.track {
                Some(px) => vec![TrackSize::Single(Track::Px(px)); n],
                // Дорожка по содержимому делит место поровну: свой размер ей
                // назначать нельзя, иначе `auto-fit` не сможет отдать место
                // схлопнутых дорожек соседям.
                None => vec![TrackSize::Single(Track::Fr(1.0)); n],
            };
        }
    }
    let count = if !tracks.is_empty() {
        tracks.len()
    } else {
        merged.grid_cols.unwrap_or(1).max(1) as usize
    };
    // Реверсы направления (css-grid-3): `fill-reverse` выбирает при равной
    // высоте ПРАВУЮ лунку, `track-reverse` перечисляет сами дорожки задом
    // наперёд — их список просто зеркалится, вместе с ним встают и элементы.
    let fill_reverse = merged.lanes_fill_reverse;
    let mut tracks = tracks;
    if merged.lanes_track_reverse {
        tracks.reverse();
    }
    let extent = |item: &Element| -> f32 {
        if row_dir {
            item_width(item)
        } else {
            item_height(item, merged, opts)
        }
    };
    let along_free = |item: &Element| -> bool {
        if row_dir {
            item.style.width.is_none() && item.style.min_width.is_none()
        } else {
            item.style.height.is_none() && item.style.min_height.is_none()
        }
    };
    let along_align = |item: &Element| -> Option<Align> {
        if row_dir {
            item.style.justify_self.or(merged.justify_items)
        } else {
            item.style.align_self.or(merged.align_items)
        }
    };
    // Сколько места достаётся элементу БЕЗ высоты: он тянется до верха
    // СЛЕДУЮЩЕГО элемента своей лунки. Разрыв там появляется, когда следующий
    // занимает несколько лунок и его прижала вниз соседняя
    // (`column-align-items-003`: элемент без высоты обязан дорасти до верха
    // двухлуночного соседа). Когда разрыва нет, у элемента остаётся высота
    // содержимого (`column-align-items-001`).
    let dense = merged.lanes_dense;
    // Куда встаёт элемент по оси лунки. Плотная укладка (`grid-lanes-pack:
    // dense`) ищет САМОЕ ВЕРХНЕЕ свободное место, куда он влезает во всех
    // своих лунках, — то есть заполняет дыры от многолуночных соседей.
    // Обычная укладка ставит его под всем, что уже уложено.
    //
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: сборка лунок в два приёма (сперва места, потом
    // коробки) ради полной плотной упаковки. Плотным тестам +18, но семейству
    // `column-align-items-*`/`row-justify-items-*` −15: их вид зависит от
    // ПОРЯДКА узлов в лунке, а не только от координат. Поэтому дырами
    // заполняется лишь то, что не нарушает порядок: элемент может подняться до
    // верхней свободной отметки, но не выше последнего соседа своей лунки.
    let mut used: Vec<Vec<(f32, f32)>> = vec![vec![]; count];
    let free_top = |used: &[Vec<(f32, f32)>], at: usize, span: usize, height: f32| -> f32 {
        // Занятые отрезки хранятся БЕЗ зазора, поэтому к нижней границе он
        // прибавляется здесь: иначе элемент на несколько лунок встаёт вплотную
        // к соседу в чужой лунке (`column-align-items-003`).
        let floor = used[at..at + span]
            .iter()
            .flat_map(|iv| iv.iter().map(|(_, end)| *end + along_gap))
            .fold(0.0f32, f32::max);
        if !dense {
            return floor;
        }
        // Ниже последнего элемента СВОЕЙ лунки не поднимаемся: порядок узлов
        // в ней задаёт и вид, и выравнивание.
        let own = used[at]
            .iter()
            .map(|(_, end)| *end + along_gap)
            .fold(0.0f32, f32::max);
        let mut y = own;
        for _ in 0..=used.iter().map(Vec::len).sum::<usize>() {
            let mut moved = false;
            for lane in at..at + span {
                for (s, en) in &used[lane] {
                    if y < *en + along_gap && *s < y + height + along_gap {
                        y = *en + along_gap;
                        moved = true;
                    }
                }
            }
            if !moved {
                return y;
            }
        }
        floor
    };
    let mut reach: Vec<(usize, f32)> = vec![];
    {
        let mut probe: Vec<Vec<(f32, f32)>> = vec![vec![]; count];
        let mut placed: Vec<(usize, usize, usize, f32, f32)> = vec![];
        for (idx, child) in e.children.iter().enumerate() {
            let Node::Element(item) = child else { continue };
            if matches!(
                item.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) {
                continue;
            }
            let (fixed, span) = lane_span(item, count, row_dir);
            let span = span.clamp(1, count);
            let height = extent(item);
            let at = fixed
                .unwrap_or_else(|| shortest_lane_free(&probe, count, span, height, &free_top, fill_reverse))
                .min(count - span);
            let top = free_top(&probe, at, span, height);
            placed.push((idx, at, span, top, height));
            if along_free(item) && matches!(along_align(item), None | Some(Align::Stretch)) {
                reach.push((idx, top));
            }
            for lane in at..at + span {
                probe[lane].push((top, top + height));
            }
        }
        // Верх следующего элемента той же лунки — по ПОРЯДКУ РАЗМЕТКИ.
        reach = reach
            .into_iter()
            .filter_map(|(idx, top)| {
                let (_, at, span, ..) = *placed.iter().find(|p| p.0 == idx)?;
                let next = placed
                    .iter()
                    .find(|(j, a, sp, ..)| *j > idx && *a < at + span && at < *a + *sp)?;
                let height = next.3 - along_gap - top;
                (height > 0.0).then_some((idx, height))
            })
            .collect();
    }
    let mut filled = vec![0f32; count];
    let mut buckets: Vec<Vec<Node>> = vec![vec![]; count];
    for (idx, child) in e.children.iter().enumerate() {
        let Node::Element(item) = child else {
            continue;
        };
        // Позиционированный элемент лунок не занимает: он вне потока.
        if matches!(
            item.style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) {
            buckets[0].push(child.clone());
            continue;
        }
        // Заданные линии сильнее раздачи: элемент встаёт именно между ними и
        // может ЗАНЯТЬ НЕСКОЛЬКО лунок.
        let (fixed, span) = lane_span(item, count, row_dir);
        let span = span.clamp(1, count);
        let mut height = extent(item);
        let at = fixed
            .unwrap_or_else(|| shortest_lane_free(&used, count, span, height, &free_top, fill_reverse))
            .min(count - span);
        // Верх элемента — низ самой заполненной из перекрытых лунок; при
        // плотной укладке — верхняя свободная отметка, не выше своей лунки.
        let top = free_top(&used, at, span, height).max(filled[at]);
        // Выравнивание элемента ВДОЛЬ лунки: `stretch` растит его на остаток,
        // остальные значения оставляют ему свой размер.
        let free = along_free(item);
        let along = along_align(item);
        let grown = reach
            .iter()
            .find(|(j, _)| *j == idx)
            .map(|(_, h)| *h)
            .filter(|h| *h > height);
        if let Some(h) = grown {
            height = h;
        }
        for lane in at..at + span {
            if lane != at {
                let pad = top - filled[lane];
                if pad > 0.0 {
                    buckets[lane].push(spacer(pad, row_dir));
                }
                // Место, занятое чужим элементом: своей коробки тут нет, но
                // следующий элемент лунки обязан начаться ПОД ним.
                buckets[lane].push(spacer(height, row_dir));
            } else if top > filled[at] {
                buckets[at].push(spacer(top - filled[at], row_dir));
            }
            filled[lane] = top + height + along_gap;
            used[lane].push((top, top + height));
        }
        // Элемент без заданной высоты ТЯНЕТСЯ вдоль лунки до низа контейнера
        // (`align-items: stretch` по оси укладки).
        let mut item = item.clone();
        // ПРОБОВАЛИ И ОТКАТИЛИ ДВАЖДЫ: дорожки ребёнку-сабгриду. Свои дорожки
        // по числу перекрытых линий — subgrid-gap +10, auto-fill/baseline −17.
        // Настоящая передача Px-куска дорожек родителя — gap-семейству −4.
        // Оба прохода нетто-вредны; честный subgrid требует передачи ИМЕННО
        // разрешённых ширин (после укладки), а их на этом этапе ещё нет.
        if let Some(h) = grown {
            // Размер вдоль лунки известен — рост не нужен, иначе элемент
            // съест и остаток лунки.
            if row_dir {
                item.style.width = Some(Len::Px(h));
            } else {
                item.style.height = Some(Len::Px(h));
            }
        } else if free && matches!(along, None | Some(Align::Stretch)) {
            item.style.flex_grow = Some(1.0);
        }
        // Размер по числу занятых лунок: коробка выходит за свою лунку в
        // соседние, а их место держат распорки.
        if span > 1 {
            let width: f32 = (at..at + span)
                .filter_map(|i| match tracks.get(i) {
                    Some(TrackSize::Single(Track::Px(w))) => Some(*w),
                    _ => None,
                })
                .sum();
            if width > 0.0 {
                let size = Some(Len::Px(width + cross_gap * (span as f32 - 1.0)));
                if row_dir {
                    item.style.height = size;
                } else {
                    item.style.width = size;
                }
            }
        }
        // Тянется ТОЛЬКО последний элемент лунки: свободное место копится в
        // хвосте. Снимать рост надо со ВСЕХ прежних, а не с последнего узла:
        // последним там может стоять распорка от чужого многолуночного
        // элемента, и тогда рост оставался у настоящего элемента под ней
        // (`column-align-items-003`).
        for prev in buckets[at].iter_mut() {
            if let Node::Element(prev) = prev {
                prev.style.flex_grow = None;
            }
        }
        buckets[at].push(Node::Element(item));
    }
    // Свободное место лунки достаётся ПОСЛЕДНЕМУ её элементу: по CSS его
    // область тянется до конца контейнера, и `align-items` выравнивает его
    // внутри неё. Растяжка уже учтена ростом; остальные значения требуют
    // коробки на весь остаток (`column-align-items-001`).
    for bucket in buckets.iter_mut() {
        let Some(Node::Element(last)) = bucket.last() else {
            continue;
        };
        let Some(along @ (Align::Center | Align::End | Align::Start)) = along_align(last) else {
            continue;
        };
        let item = last.clone();
        bucket.pop();
        bucket.push(lane_align_box(item, along, row_dir));
    }
    // `auto-fit` схлопывает ПУСТЫЕ дорожки: место, которое им причиталось,
    // делят между собой непустые (`column-auto-repeat-auto-012`: две дорожки
    // по 150 вместо трёх по 100).
    let mut buckets = buckets;
    if repeat.is_some_and(|r| r.fit) && buckets.iter().any(|b| b.is_empty()) {
        let keep: Vec<bool> = buckets.iter().map(|b| !b.is_empty()).collect();
        if keep.iter().any(|k| *k) {
            let mut i = 0;
            tracks.retain(|_| {
                let k = keep.get(i).copied().unwrap_or(true);
                i += 1;
                k
            });
            buckets.retain(|b| !b.is_empty());
        }
    }
    let mut row = styled_div_with(e, merged).flex();
    row = if row_dir {
        row.flex_col().gap_y(gpui::px(cross_gap))
    } else {
        row.flex_row().gap_x(gpui::px(cross_gap))
    };
    // `align-items` в лунках — про САМ элемент внутри лунки, а не про лунки в
    // ряду. Пока значение доходило до ряда, `align-items: center` сдвигал
    // целые лунки вниз на половину остатка; сами лунки обязаны быть равной
    // высоты всегда, а выравнивание элемента уже учтено выше растяжкой.
    row.style().align_items = Some(gpui::AlignItems::Stretch);
    // Раздача ЛУНОК в ряду — это `justify-content` по оси лунок и
    // `align-content` вдоль потока. До ряда они не доходили вовсе, и лунки
    // всегда жались к началу (`grid-lanes/alignment/*-content-*`).
    let content = |j: Option<crate::computed::Justify>| {
        use crate::computed::Justify;
        match j? {
            Justify::Start => Some(gpui::JustifyContent::Start),
            Justify::End => Some(gpui::JustifyContent::End),
            Justify::Center => Some(gpui::JustifyContent::Center),
            Justify::Between => Some(gpui::JustifyContent::SpaceBetween),
            Justify::Around => Some(gpui::JustifyContent::SpaceAround),
            Justify::Evenly => Some(gpui::JustifyContent::SpaceEvenly),
            // `start`/`end` по стороне ПИСЬМА: в вертикальном письме и при
            // `rtl` начало ряда — другой край.
            Justify::WmStart | Justify::WmEnd => {
                let flip = merged.rtl == Some(true);
                let end = (j? == Justify::WmEnd) != flip;
                Some(if end { gpui::JustifyContent::End } else { gpui::JustifyContent::Start })
            }
            Justify::Stretch => None,
        }
    };
    let across = if row_dir {
        merged.align_content
    } else {
        merged.justify_content
    };
    if let Some(j) = content(across) {
        row.style().justify_content = Some(j);
    }
    for (i, items) in buckets.into_iter().enumerate() {
        let mut lane = div().flex();
        lane = if row_dir {
            lane.flex_row().min_h_0().gap_x(gpui::px(along_gap))
        } else {
            lane.flex_col().min_w_0().gap_y(gpui::px(along_gap))
        };
        match tracks.get(i) {
            Some(TrackSize::Single(Track::Px(w))) if row_dir => {
                lane = lane.h(gpui::px(*w)).flex_shrink_0()
            }
            Some(TrackSize::Single(Track::Pct(k))) if row_dir => {
                lane = lane.h(gpui::relative(*k)).flex_shrink_0()
            }
            Some(TrackSize::Single(Track::Px(w))) => lane = lane.w(gpui::px(*w)).flex_shrink_0(),
            Some(TrackSize::Single(Track::Pct(k))) => {
                lane = lane.w(gpui::relative(*k)).flex_shrink_0()
            }
            // Дорожка по содержимому шире содержимого не бывает: ширину ей
            // задаёт самый широкий элемент лунки, а не равная доля.
            Some(TrackSize::Single(Track::Auto | Track::MinContent | Track::MaxContent)) => {
                lane = lane.flex_shrink_0()
            }
            Some(TrackSize::Single(Track::Fr(f))) => {
                lane.style().flex_grow = Some(*f);
                lane = lane.flex_basis(px(0.));
            }
            _ => lane = lane.flex_1(),
        }
        row = row.child(lane.children(blocks(&items, merged, opts)));
    }
    row.into_any_element()
}

/// Между какими лунками стоит элемент: начало (если задано) и сколько занимает.
fn lane_span(e: &Element, count: usize, row_dir: bool) -> (Option<usize>, usize) {
    use crate::computed::Placement;
    let line = |n: i16| -> usize {
        if n > 0 {
            (n as usize - 1).min(count.saturating_sub(1))
        } else {
            // Отрицательная линия считается с конца: -1 — последний край.
            count.saturating_sub((-n) as usize)
        }
    };
    // Лунка — это КОЛОНКА при укладке колонками и РЯД при укладке рядами:
    // в первом случае её выбирает `grid-column`, во втором `grid-row`.
    let across = if row_dir {
        e.style.grid_row
    } else {
        e.style.grid_col
    };
    match across {
        Some((Placement::Line(a), Placement::Line(b))) => {
            let (s, t) = (line(a), line(b));
            (Some(s.min(t)), (t as i32 - s as i32).unsigned_abs() as usize)
        }
        Some((Placement::Line(a), Placement::Span(k))) => (Some(line(a)), k as usize),
        Some((Placement::Line(a), Placement::Auto)) => (Some(line(a)), 1),
        Some((Placement::Span(k), _)) => (None, k as usize),
        _ => (None, 1),
    }
}

/// Распорка в лунке: места чужого элемента и выравнивания верхов.
/// Коробка на весь остаток лунки, внутри которой элемент стоит по
/// выравниванию. Растянуть его самого нельзя — размер у него свой.
fn lane_align_box(item: Element, along: Align, row_dir: bool) -> Node {
    let mut style = Computed::default();
    style.display = Some(Display::Flex);
    style.flex_dir = Some(if row_dir {
        crate::computed::FlexDir::Row
    } else {
        crate::computed::FlexDir::Col
    });
    style.flex_grow = Some(1.0);
    style.justify_content = Some(match along {
        Align::Center => crate::computed::Justify::Center,
        Align::End => crate::computed::Justify::End,
        _ => crate::computed::Justify::Start,
    });
    Node::Element(Element {
        node_id: 0,
        anim: None,
        tag: "div".into(),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![Node::Element(item)],
        attrs: vec![],
        inline: false,
    })
}

fn spacer(size: f32, row_dir: bool) -> Node {
    let mut style = Computed::default();
    if row_dir {
        style.width = Some(Len::Px(size));
    } else {
        style.height = Some(Len::Px(size));
    }
    style.display = Some(Display::Block);
    Node::Element(Element {
        node_id: 0,
        anim: None,
        tag: "div".into(),
        style,
        hover: None,
        first_letter: None,
        first_line: None,
        children: vec![],
        attrs: vec![],
        inline: false,
    })
}

/// Лунка с самым высоким верхом для элемента шириной в `span` лунок.
/// Лунка для элемента без заданных линий при укладке по занятым отрезкам: та,
/// где он встанет ВЫШЕ всего.
fn shortest_lane_free(
    used: &[Vec<(f32, f32)>],
    count: usize,
    span: usize,
    height: f32,
    top_of: &dyn Fn(&[Vec<(f32, f32)>], usize, usize, f32) -> f32,
    reverse: bool,
) -> usize {
    // При равной высоте побеждает ПЕРВАЯ лунка в порядке обхода: обычно левая,
    // при `fill-reverse` — правая (css-grid-3, `grid-lanes-direction`).
    let pick = |it: &mut dyn Iterator<Item = usize>| {
        it.min_by(|a, b| top_of(used, *a, span, height).total_cmp(&top_of(used, *b, span, height)))
            .unwrap_or(0)
    };
    if reverse {
        pick(&mut (0..=count.saturating_sub(span)).rev())
    } else {
        pick(&mut (0..=count.saturating_sub(span)))
    }
}

/// Ширина элемента по его же стилю — для раздачи по лункам-рядам.
fn item_width(e: &Element) -> f32 {
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let declared = px_of(e.style.width).max(px_of(e.style.min_width));
    let box_extra = if e.style.border_box == Some(true) {
        0.0
    } else {
        px_of(e.style.padding.left)
            + px_of(e.style.padding.right)
            + px_of(e.style.borders().left)
            + px_of(e.style.borders().right)
    };
    declared + box_extra + px_of(e.style.margin.left) + px_of(e.style.margin.right)
}

/// Высота элемента по его же стилю — для раздачи по лункам.
///
/// Незаданная высота считается по СТРОКЕ текста: в наборе элемент лунки — это
/// цифра в коробке с полями, и без учёта строки раздача уезжает.
fn item_height(e: &Element, inherited: &Computed, opts: &RenderOpts) -> f32 {
    let px_of = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let pad = px_of(e.style.padding.top) + px_of(e.style.padding.bottom);
    let bw = e.style.borders();
    let border = px_of(bw.top) + px_of(bw.bottom);
    let margin = px_of(e.style.margin.top) + px_of(e.style.margin.bottom);
    let declared = px_of(e.style.height).max(px_of(e.style.min_height));
    let inner = if declared > 0.0 {
        declared
    } else if has_text(&e.children) {
        line_height_px(&crate::inline::inherit(inherited, &e.style), opts)
    } else {
        0.0
    };
    if e.style.border_box == Some(true) {
        inner + margin
    } else {
        inner + pad + border + margin
    }
}


/// Есть ли в поддереве непустой текст — по нему считается высота строки.
fn has_text(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(e) => has_text(&e.children),
    })
}

/// Доля кегля для `line-height: normal` — по метрикам шрифта элемента.
///
/// Постоянная доля неверна: у Ahem `normal` ровно кегль, у текстовых шрифтов
/// около 1.15–1.3. Из-за постоянной 1.31 коробка с `line-height: 1em` и
/// соседняя без него расходились по высоте строк (`pre-wrap-008`).
fn normal_fraction(style: &Computed, opts: &RenderOpts) -> f32 {
    let family = style.font_family.clone().unwrap_or_else(|| {
        if style.monospace == Some(true) {
            crate::metrics::mono_family().to_string()
        } else {
            String::new()
        }
    });
    let measured = crate::metrics::normal_line(&family);
    if measured > 0.0 {
        measured
    } else {
        opts.normal_line_height
    }
}


/// Пустой ли текстовый узел ПО CSS.
///
/// Схлопывается только `space`, `tab`, `CR`, `LF`. `str::trim` снимает весь
/// юникодный пробел, и узел из идеографических U+3000 (или неразрывных
/// U+00A0) считался пустым: строка из них пропадала целиком, а абзац рвался
/// там, где рваться не должен (`trailing-ideographic-space-017`).
fn blank_text(t: &str) -> bool {
    t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n'))
}
