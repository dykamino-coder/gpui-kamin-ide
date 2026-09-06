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
    // Абсолютный слой в раскладке отсчитывается уже от padding-box
    // (`vendor/taffy/src/compute/block.rs`, как в CSS 2.1 §10.1): рамку
    // вычитать второй раз нельзя — проба `probe-bg-clipinset` давала 60×60
    // вместо 100×100 при рамке 20px.
    let pad = |p: Option<Len>| {
        if clip == crate::computed::BgClip::ContentBox {
            px_of(p)
        } else {
            0.0
        }
    };
    let mut layer = div()
        .absolute()
        .top(px(pad(c.padding.top)))
        .right(px(pad(c.padding.right)))
        .bottom(px(pad(c.padding.bottom)))
        .left(px(pad(c.padding.left)));
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

/// Объявлен ли где-то в поддереве СВОЙ `visibility: visible`.
///
/// По §11.2 потомок скрытого элемента виден, если объявил видимость сам.
/// Читается собственное значение узла, до слияния: `Some(false)` приходит
/// только из авторского CSS — прочие места ставят лишь `Some(true)`.
fn shows_inside(nodes: &[Node]) -> bool {
    nodes.iter().any(|n| match n {
        Node::Text(_) => false,
        Node::Element(e) => e.style.hidden == Some(false) || shows_inside(&e.children),
    })
}

pub(crate) fn styled_div_with(e: &Element, style: &Computed) -> gpui::Div {
    // Скрытая коробка с видимым потомком не прячется целиком: раскладка та
    // же, гаснет только СВОЯ краска — иначе ранний возврат из отрисовки
    // уносит и потомка (`visufx/visibility-005`).
    let bare;
    let c = if style.hidden == Some(true) && shows_inside(&e.children) {
        bare = style.paint_off();
        &bare
    } else {
        style
    };
    let mut d = apply(div(), c);
    // Проба якоря (css-anchor-position-1 §anchor-name): канвас во всю
    // коробку пишет её рамку в реестр кадра на подготовке — позже по дереву
    // её прочтёт `anchor::AnchorPlace` позиционированной коробки. Ставится
    // здесь, потому что через `styled_div_with` проходят и блоки, и атомы
    // строки (`inline-block` из `anchor-position-005`), и держатели.
    if let Some(probe) = crate::anchor::probe_for(e) {
        d = d.child(probe);
    }
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
    // Срез меняет только АВТОМАТИЧЕСКУЮ высоту (css-overflow-4 §5.3):
    // заданная `height`/`min-height` остаётся, строки после среза прячет
    // бюджет абзаца (`line-clamp-010`, `webkit-line-clamp-040`).
    let sized = e.style.height.is_some() || e.style.min_height.is_some();
    // В многоколоночнике `continue: collapse` ведёт себя как `auto`
    // (css-overflow-4 §5.2): срез не действует (`line-clamp-039`).
    let multicol = multicol_container(&e.style);
    if !multicol && e.style.clamp_auto == Some(true) && c.max_height.is_some() {
        if let Some(cut) = crate::interact::clamp_cut(e.node_id).filter(|_| !sized) {
            d = d.max_h(px(cut));
        }
        d = d.overflow_hidden();
    }
    if let Some(n) = e.style.clamp_lines().filter(|_| !multicol) {
        d = d.line_clamp(n as usize);
        let font = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): считать долю `normal` по метрикам
        // шрифта (как `normal_fraction`) вместо жёстких 1.2 — срез из 31 пары
        // `line-clamp`/`text-wrap-balance` дал 20 → 20. Запасная формула тут не
        // берётся вовсе: высоту приносит замер (`clamp_cut`). Недостающие
        // 6-7 точек в `text-wrap-balance-line-clamp-*` — это разница САМОЙ
        // строки (наши 19.2 против ~21.3 у эталона), то есть вопрос к
        // `metrics::normal_line`, а не к обрезке.
        let line = match c.line_height {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * font,
            _ => 1.2 * font,
        };
        let cut = crate::interact::clamp_cut(e.node_id).unwrap_or(n as f32 * line);
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("HTML_CLAMP_DBG").is_ok());
            *ON
        } {
            eprintln!("CLAMP branch node={} n={} cut={}", e.node_id, n, cut);
        }
        if !sized {
            d = d.max_h(px(cut));
        }
        d = d.overflow_hidden();
    }
    let empty = !e.children.iter().any(|n| !is_blank(n));
    for extra in decorations(c, empty) {
        d = d.child(extra);
    }
    d
}

/// Слои, которые в GPUI выражаются только отдельным элементом.
///
/// Все — абсолютные и вне потока, поэтому на раскладку не влияют и могут
/// идти первыми детьми.
fn decorations(c: &Computed, empty: bool) -> Vec<AnyElement> {
    let mut out: Vec<AnyElement> = vec![];

    // РЕЗКАЯ тень (без размытия): примитив тени с нулевым размытием
    // вырождается в шейдере, поэтому она рисуется слоем-квадом, раздутым на
    // разлёт. Радиус фигуры — по спеке (css-backgrounds-3 §7.1): нулевой
    // остаётся острым, иначе растёт на разлёт; доля считается от размера
    // раздутой фигуры (известные ширина и высота).
    for sh in &c.shadows {
        // Тень без цвета помечена отрицательной альфой и берёт `color`
        // (css-backgrounds-3 §7.1) — как в `apply::shadow_colour`.
        if sh.blur > 0.0 || sh.color.a == 0.0 {
            continue;
        }
        let colour = if sh.color.a < 0.0 {
            c.color.unwrap_or(crate::value::Color {
                r: 0.0,
                g: 0.0,
                b: 0.0,
                a: 1.0,
            })
        } else {
            sh.color
        };
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
                .border_color(colour.to_hsla())
                .into_any_element(),
        );
    }

    // `filter: url(#id)` на HTML-элементе (filter-effects-1 §filter
    // region): дешёвый путь для коробки без содержимого — SVG с `<rect>`
    // цвета фона и этим фильтром растрируется resvg и ложится слоем поверх
    // коробки; область — по умолчанию −10 %/120 % от border-box, поэтому
    // холст вдвое шире, а слой сдвинут на половину коробки. Фильтр над
    // готовым буфером группы — отдельная задача.
    // Только у коробки БЕЗ содержимого: над содержимым слой лёг бы поверх
    // детей (filter-region-transformed-composited-child-001).
    if empty
        && let Some(id) = c.filter_ref.as_deref()
        && let Some(def) = mask_def(&format!("filter:{id}"))
    {
        let fill = match c.background {
            Some(col) if col.a > 0.0 => format!(
                "rgba({},{},{},{})",
                (col.r * 255.0).round(),
                (col.g * 255.0).round(),
                (col.b * 255.0).round(),
                col.a
            ),
            _ => "none".to_string(),
        };
        out.push(
            crate::interact::FilterLayer {
                def,
                id: id.to_string(),
                fill,
            }
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

    // Рамка при фигурных углах (`corner-shape`, css-borders-4): внешний край
    // — контур, внутренний — он же, сжатый на толщину сторон. Квад её не
    // красит (`apply::apply_paint`); слой — цветной растр кольца в
    // border-box, тем же растеризатором, что и маска группы, — контуры
    // совпадают попиксельно. Разные цвета сторон остаются полосами ниже.
    if c.corner_shaped() {
        let sides: Vec<_> = c.border_colors.iter().flatten().collect();
        let uniform = sides
            .first()
            .filter(|f| sides.iter().all(|s| s == *f))
            .map(|f| **f);
        let mixed = sides.len() > 1 && uniform.is_none();
        let side_px = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let bw = c.borders();
        let widths = [
            side_px(bw.top),
            side_px(bw.right),
            side_px(bw.bottom),
            side_px(bw.left),
        ];
        if !mixed && widths.iter().any(|w| *w > 0.0) {
            // Без цвета рамка красится цветом текста, без него — чёрным
            // (начальное `border-color: currentColor`).
            let colour = uniform
                .or(c.border_color)
                .or(c.color)
                .unwrap_or(crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                });
            let spec = crate::background::rrect_spec(c, Some(widths));
            let [t, r, b, l] = widths;
            out.push(
                gpui::canvas(
                    |_, _, _| {},
                    move |bounds, _, window, _| {
                        let sf = window.scale_factor();
                        let (w, h) = (f32::from(bounds.size.width), f32::from(bounds.size.height));
                        let (pw, ph) = (
                            (w * sf).round().max(1.0) as u32,
                            (h * sf).round().max(1.0) as u32,
                        );
                        let args = spec.trim_start_matches("rrect(").trim_end_matches(')');
                        if let Some(img) =
                            crate::background::rasterize_ring(args, pw, ph, sf, colour)
                        {
                            let _ = window.paint_image(bounds, gpui::Corners::default(), img, 0, false);
                        }
                    },
                )
                // Абсолютный ребёнок считается от padding-box — кольцо
                // накрывает рамку отрицательными отступами (как полосы ниже).
                .absolute()
                .top(px(-t))
                .left(px(-l))
                .right(px(-r))
                .bottom(px(-b))
                .into_any_element(),
            );
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
    if let Some(o) = c.outline.clone() {
        // Шрифтовые единицы ширины и сдвига решаются своим кеглем.
        let em = match c.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            Some(Len::Em(k)) => k * em,
            _ => 0.0,
        };
        let w = px_of(o.width);
        // `outline-style: none` гасит обводку независимо от ширины.
        let visible = o.style != Some(0);
        // Обводка без своего цвета берёт цвет текста — так решает CSS.
        if let (true, true, Some(colour)) = (visible, w > 0.0, o.color.or(c.color)) {
            // Отрицательный сдвиг вжимает контур внутрь коробки, но внешняя
            // сторона фигуры не может стать уже удвоенной толщины
            // (css-ui-4 §outline-offset; Blink `outline_painter.cc`
            // `AdjustedOutlineOffset`: `max(offset, -size/2)` по каждой оси
            // отдельно). Зажимается по ЗАДАННОМУ размеру коробки — иного на
            // сборке нет (`outline-013…016`).
            let off = {
                let raw = px_of(o.offset);
                let half = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => Some(v / 2.0),
                    Some(Len::Em(k)) => Some(k * em / 2.0),
                    _ => None,
                };
                match (half(c.width), half(c.height)) {
                    (Some(hw), Some(hh)) => raw.max(-hw.min(hh)),
                    (Some(hw), None) => raw.max(-hw),
                    (None, Some(hh)) => raw.max(-hh),
                    (None, None) => raw,
                }
            };
            // Угол контура повторяет угол коробки, раздвинутый сдвигом и
            // толщиной (css-ui-4 §outline): доля решается так же, как у
            // рамки, — прежде она читалась нулём (`outline-005`).
            let corner = crate::apply::radius_px(c, c.radius.tl)
                .filter(|v| *v > 0.0)
                .map_or(0.0, |v| v + off + w);
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
    crate::interact::frame_sanitize();
    // Пойманная паника кадра внутри рамки оставляла счётчик глубины
    // навсегда — три такие паники, и рамки исчезали до перезапуска.
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    // Слой начального содержащего блока: внепоточные элементы без
    // позиционированного предка дописываются последними детьми документа —
    // их края решает область просмотра (§10.1 п.4).
    crate::interact::icb_open();
    let mut out = blocks(nodes, &root, opts);
    out.extend(crate::interact::icb_close());
    out
}

/// Копий ребёнка в стопке страниц — потолок числа страниц, на которые может
/// растянуться один блок верхнего уровня (в `css-page` не больше шести).
const PAGE_COPIES: usize = 12;

/// Постраничная отрисовка (css-page-3): блоки верхнего уровня документа —
/// дети стопки страниц, page area каждой страницы — фрагментаинер.
///
/// Обёртки `html`/`body` снимаются здесь (сборщик оставляет их при
/// собственной коробке, `doc.rs::has_box_style`): их фон — канвас документа
/// (§painting, слой 2), боковые поля/рамки/отступы — сдвиг содержимого на
/// КАЖДОЙ странице, верхнее — только на первой: коробка тела режется по
/// страницам вместе с содержимым, а её `page` — умолчание имени для детей.
pub fn render_paged(
    nodes: &[Node],
    opts: &RenderOpts,
    mut geom: crate::flow::PageGeom,
) -> AnyElement {
    let mut root = opts.root_style();
    crate::interact::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let mut nodes: Vec<Node> = nodes.to_vec();
    let (mut left, mut right, mut top) = (0.0f32, 0.0f32, 0.0f32);
    let mut root_page = String::new();
    loop {
        let live: Vec<&Node> = nodes.iter().filter(|n| !is_blank(n)).collect();
        let [Node::Element(e)] = live.as_slice() else { break };
        if !matches!(e.tag.as_str(), "html" | "body") {
            break;
        }
        let e = (*e).clone();
        let side = |l: &Option<Len>| match l {
            Some(Len::Px(v)) => *v,
            _ => 0.0,
        };
        let b = e.style.borders();
        left += side(&e.style.margin.left) + side(&b.left) + side(&e.style.padding.left);
        right += side(&e.style.margin.right) + side(&b.right) + side(&e.style.padding.right);
        top += side(&e.style.margin.top) + side(&b.top) + side(&e.style.padding.top);
        if let Some(c) = e.style.background.filter(|c| c.a > 0.0) {
            geom.canvas = Some(c.to_hsla());
        }
        if let Some(p) = &e.style.page {
            root_page = p.clone();
        }
        root = inline::inherit(&root, &e.style);
        nodes = e.children;
    }
    crate::interact::icb_open();
    let mut kids: Vec<crate::flow::PageKid> = Vec::new();
    let mut prev_end: Option<String> = None;
    let mut first = true;
    for n in nodes.iter().filter(|n| !is_blank(n)) {
        if let Node::Element(e) = n
            && matches!(e.style.display, Some(Display::None))
        {
            continue;
        }
        let pad_top = if first { top } else { 0.0 };
        first = false;
        let build = || {
            div()
                .pl(px(left))
                .pr(px(right))
                .pt(px(pad_top))
                .children(blocks(std::slice::from_ref(n), &root, opts))
                .into_any_element()
        };
        let el = build();
        let frags: Vec<AnyElement> = (1..PAGE_COPIES).map(|_| build()).collect();
        // Анонимный блок вокруг текста/строчного — коробка в потоке со
        // значением `page` родителя; флоат и внепоточный в сравнении имён
        // не участвуют (свойство к ним не применяется, §named pages п.2).
        let (monolith, fb, fa, names) = match n {
            Node::Element(e) if !e.inline && class_a_box(e) => (
                page_monolith(e),
                e.style.break_before_force,
                e.style.break_after_force,
                Some(page_names(e, &root_page)),
            ),
            Node::Element(e) if !e.inline => (
                page_monolith(e),
                e.style.break_before_force,
                e.style.break_after_force,
                None,
            ),
            _ => (false, false, false, Some((root_page.clone(), root_page.clone()))),
        };
        let renamed = match (&prev_end, &names) {
            (Some(p), Some((start, _))) => p != start,
            _ => false,
        };
        if let Some((_, end)) = &names {
            prev_end = Some(end.clone());
        }
        // Мера поддерева — те же точки разреза, что у колонок. Обёртка
        // первого ребёнка несёт отбивку корня сверху (`pad_top`): все
        // смещения меры сдвигаются на неё, а высота растёт.
        let shape = match n {
            Node::Element(e) if !e.inline => shape_full(e, 4).map(
                |(h, _mt, _mb, mut cuts, mut forced, mut solid)| {
                    if pad_top > 0.0 {
                        for c in cuts.iter_mut() {
                            c.0 += pad_top;
                            c.1 += pad_top;
                        }
                        for f in forced.iter_mut() {
                            *f += pad_top;
                        }
                        for r in solid.iter_mut() {
                            r.0 += pad_top;
                            r.1 += pad_top;
                        }
                    }
                    (h + pad_top, cuts, forced, solid)
                },
            ),
            _ => None,
        };
        // Монолит выше листа решается в `fill` (правило «сначала перенос,
        // потом разрыв внутри»): здесь мера считается и для него — точки
        // класса A нужны, когда он окажется с верха страницы.
        kids.push(crate::flow::PageKid {
            el,
            frags,
            monolith,
            force_before: fb || renamed,
            force_after: fa,
            shape,
        });
    }
    let icb = crate::interact::icb_close();
    crate::flow::PageStack::new(kids, geom, icb).into_any_element()
}

// Мера блочного поддерева для укладки по фрагментаинерам — колонкам и
// страницам: высота, поля, точки законного разреза, принудительные разрывы
// и монолиты. Раньше жила внутри `element()`; локальных переменных не
// захватывала, вынесена ради `render_paged`.
fn has_float(n: &Element, depth: u8) -> bool {
    if depth == 0 {
        return false;
    }
    n.children.iter().any(|k| match k {
        Node::Element(e) => {
            e.style.float.unwrap_or(0) != 0 || has_float(e, depth - 1)
        }
        _ => false,
    })
}
/// Мера блочного ребёнка для укладки колонок: высота с
/// отбивками и рамками, поля и точки ЗАКОННОГО разреза
/// (css-break-3 §4.3, класс A) — границы вложенных
/// блочных детей, рекурсивно. Высота `auto` складывается
/// из тех же детей со схлопыванием полей (CSS 2.1
/// §8.3.1); строчное содержимое высоты не даёт — такой
/// ребёнок мерить нечем, и весь стек идёт другим путём.
fn shape(c: &Element, depth: u8) -> Option<(f32, f32, f32, Vec<(f32, f32)>)> {
    shape_full(c, depth).map(|s| (s.0, s.1, s.2, s.3))
}
/// Высота сетки по ЯВНЫМ дорожкам рядов: все дорожки в
/// точках, плюс зазоры между ними. `None` — дорожки
/// неизвестны или не все в точках.
fn grid_rows_px(c: &Computed) -> Option<f32> {
    use crate::computed::{Track, TrackSize};
    if !matches!(
        c.display,
        Some(Display::Grid) | Some(Display::InlineGrid)
    ) {
        return None;
    }
    let rows = c.grid_rows.as_ref()?;
    if rows.is_empty() {
        return None;
    }
    let mut total = 0.0f32;
    for t in rows {
        match t {
            TrackSize::Single(Track::Px(v)) => total += v,
            _ => return None,
        }
    }
    let gap = match c.gap {
        Some((Some(Len::Px(v)), _)) => v,
        _ => 0.0,
    };
    Some(total + gap * (rows.len() as f32 - 1.0))
}

/// Монолит по css-break-4 §4.1 (Blink `IsMonolithic`): замещаемый,
/// атомарный строчный, прокручиваемый, `break-inside: avoid`,
/// строчное содержимое (строк укладка не видит) — пустая
/// коробка монолитом НЕ является.
fn solid_box(k: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    let block_kid = |n: &Node| {
        matches!(n, Node::Element(x)
            if !x.inline || x.style.display == Some(Display::Block))
    };
    k.style.break_inside_avoid
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): считать монолитом
        // и `contain: size` (css-contain-2 §size
        // containment). Срез css-break + css-multicol
        // 1495 пар: 472 -> 467, приобретено 0, потеряно 5
        // (`single-line-{column,row}-flex-fragmentation-
        // 010/011/051/063` в «красное видно»,
        // `overflow-clip-012` 0.00 -> 0.52).
        || scrolls(k.style.overflow_x)
        || scrolls(k.style.overflow_y)
        || matches!(
            k.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        // Таблица и ячейка монолитами НЕ являются
        // (css-break-4 §4.1: монолитен замещаемый,
        // прокручиваемый и `break-inside: avoid`);
        // строка таблицы — да, но её не режет и укладка.
        || matches!(
            k.style.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
        )
        || (k.children.iter().any(|n| !is_blank(n))
            && !k.children.iter().any(block_kid))
}
/// То же плюс смещения принудительных разрывов и диапазоны
/// монолитов внутри.
type Shape = (f32, f32, f32, Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>);
fn shape_full(c: &Element, depth: u8) -> Option<Shape> {
    let px_or = |l: &Option<Len>, strict: bool| match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        Some(_) if !strict => Some(0.0),
        _ => None,
    };
    if has_float(c, 3) {
        return None;
    }
    let b = c.style.borders();
    let mt = px_or(&c.style.margin.top, false)?;
    let mb = px_or(&c.style.margin.bottom, false)?;
    let top = px_or(&c.style.padding.top, false)? + px_or(&b.top, false)?;
    let bot = px_or(&c.style.padding.bottom, false)? + px_or(&b.bottom, false)?;
    let kids: Vec<&Node> = c.children.iter().filter(|n| !is_blank(n)).collect();
    // Спуск — по физике контейнера. ★ ЗАМЕРЕНО (04.09,
    // срез 1498): без гейта 384, гейт «только блочный
    // поток» 376 (+15/−23) — flex-колонки, flex с
    // переносом и ВЛОЖЕННЫЙ многоколоночник без спуска
    // теряют высоту и вылетают из укладки целиком.
    // Ряд flex без переноса: дети рядом — высота ряда
    // равна наибольшему, точек разреза между ними нет.
    // Сетка и таблица: дети не стопкой, спуска нет.
    let is_flex = matches!(
        c.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex)
    ) || c.style.webkit_box == Some(true);
    let row_nowrap = is_flex
        && matches!(
            c.style.flex_dir,
            None
                | Some(crate::computed::FlexDir::Row)
                | Some(crate::computed::FlexDir::RowReverse)
        )
        && c.style.flex_wrap != Some(true)
        && c.style.webkit_box_vertical != Some(true);
    let no_descent = matches!(
        c.style.display,
        Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
            | Some(Display::Table)
            | Some(Display::InlineTable)
            | Some(Display::TableRow)
            | Some(Display::TableRowGroup)
    );
    // (высота, поля, точки, forced, монолиты, force_before,
    //  force_after, ДОТЯГ внепоточного)
    // Дотяг — насколько ниже собственного верха ребёнка
    // уходит низ его внепоточного потомка. В поток он не
    // добавляется (абсолют соседей не двигает), но
    // фрагментация обязана его видеть: css-position-3
    // §abspos-breaking — «The box may subsequently be
    // broken over several fragmentation containers».
    type KidShape = (
        f32,
        f32,
        f32,
        Vec<(f32, f32)>,
        Vec<f32>,
        Vec<(f32, f32)>,
        bool,
        bool,
        f32,
    );
    let inner: Option<Vec<KidShape>> = if depth == 0 || no_descent {
        None
    } else {
        kids.iter()
            .map(|n| match n {
                // Абсолют высоты стопке не даёт и разреза
                // не мешает: нулевая запись, а не отказ
                // от всей укладки (`out-of-flow-in-
                // multicolumn-*`, корень A2). Но НУЛЬ в
                // девятом поле означал бы, что его вовсе
                // нет во фрагментации, а css-position-3
                // §abspos-breaking требует обратного: «an
                // absolutely positioned box is positioned
                // relative to its containing block ignoring
                // any fragmentation breaks (as if the flow
                // were continuous). The box may
                // subsequently be broken over several
                // fragmentation containers». Значит
                // содержащий блок обязан ДОТЯНУТЬСЯ до его
                // низа — иначе колонок под него не
                // родится (Blink
                // `column_layout_algorithm.cc:1125`:
                // `actual_column_count +=
                // column_balancing_info.num_new_columns`).
                // Плавающий сюда не входит: он не
                // позиционированный, и содержащего блока
                // собой не задаёт.
                Node::Element(k) if out_of_flow(&k.style) => {
                    let abs = matches!(
                        k.style.position,
                        Some(crate::computed::Position::Absolute)
                    );
                    let reach = if abs {
                        let top = match k.style.inset.top {
                            Some(Len::Px(v)) => v,
                            _ => 0.0,
                        };
                        // Собственная высота абсолюта — той
                        // же мерой: она уже включает дотяг
                        // ЕГО внепоточных потомков, и
                        // цепочка `abs > abs` складывается
                        // сама (`out-of-flow-in-multicolumn-
                        // 022/025`).
                        let own =
                            shape_full(k, depth - 1).map(|s| s.0).unwrap_or(0.0);
                        (top + own).max(0.0)
                    } else {
                        0.0
                    };
                    Some((
                        0.0,
                        0.0,
                        0.0,
                        Vec::new(),
                        Vec::new(),
                        Vec::new(),
                        false,
                        false,
                        reach,
                    ))
                }
                Node::Element(k)
                    if !k.inline
                        && (k.style.position.is_none()
                            || k.style.position
                                == Some(crate::computed::Position::Relative))
                        && k.style.float.unwrap_or(0) == 0 =>
                {
                    shape_full(k, depth - 1).map(
                        |(h, mt, mb, cuts, forced, solid)| {
                            // Монолит-потомок — весь диапазон
                            // его высоты; иначе — его собственные
                            // монолиты.
                            let solid = if solid_box(k) {
                                vec![(0.0, h)]
                            } else {
                                solid
                            };
                            (
                                h,
                                mt,
                                mb,
                                cuts,
                                forced,
                                solid,
                                k.style.break_before_force,
                                k.style.break_after_force,
                                // Дотяг внепоточных ЭТОГО потомка в
                                // поток родителя не переходит: у
                                // него свой содержащий блок.
                                0.0,
                            )
                        },
                    )
                }
                _ => None,
            })
            .collect()
    };
    let mut cuts: Vec<(f32, f32)> = Vec::new();
    let mut forced: Vec<f32> = Vec::new();
    let mut solid: Vec<(f32, f32)> = Vec::new();
    // Рамка и отбивка самой коробки — без разрывов (Blink:
    // «Avoid breaking inside block-start border»).
    if top > 0.0 {
        solid.push((0.0, top));
    }
    // Стек вложенных: конец, поле первого, схлопнувшееся
    // сквозь верх без отбивки, поле последнего.
    let mut stacked: Option<(f32, f32, f32)> = None;
    // Самый нижний край внепоточных потомков, отсчитанный
    // от верха ЭТОЙ коробки. В поток не входит, высоту
    // соседей не двигает — нужен только фрагментации.
    let mut oof_reach = 0.0f32;
    if let Some(kids) = inner.filter(|k| !k.is_empty()) {
        let inner_h: Vec<f32> = kids.iter().map(|k| k.0).collect();
        // Ряд flex БЕЗ переноса: дети стоят бок о бок, и
        // каждый фрагментируется СВОИМИ точками (Blink
        // `flex_layout_algorithm.cc`: элементу строки
        // выдаётся своя доля фрагментаинера). Значит точки
        // ряда — объединение точек детей, а запрет разрыва
        // — объединение их монолитных диапазонов: рвать
        // нельзя там, где не даёт хоть один. Прежде ряд
        // объявлялся монолитом целиком, и разреза не было
        // никогда (`single-line-row-flex-fragmentation-*`).
        if row_nowrap {
            let tallest = inner_h.iter().copied().fold(0.0f32, f32::max);
            for k in &kids {
                let start = top;
                for (need, nf) in &k.3 {
                    cuts.push((start + need, start + nf));
                }
                for f in &k.4 {
                    forced.push(start + f);
                }
                for (a, b) in &k.5 {
                    solid.push((start + a, start + b));
                }
            }
            stacked = Some((top + tallest, 0.0, 0.0));
        } else {
        let mut y = top;
        let mut prev_mb = 0.0f32;
        let mut through = 0.0f32;
        let mut first = true;
        let mut force_next = false;
        for (h, kmt, kmb, kcuts, kforced, ksolid, fb, fa, kreach) in kids {
            let lead = if first {
                if top == 0.0 {
                    through = kmt;
                    0.0
                } else {
                    kmt
                }
            } else {
                prev_mb.max(kmt)
            };
            if !first {
                cuts.push((y, y + lead));
                // Принудительный разрыв на границе детей.
                if fb || force_next {
                    forced.push(y);
                }
            }
            force_next = fa;
            let start = y + lead;
            for (need, nf) in kcuts {
                cuts.push((start + need, start + nf));
            }
            for f in kforced {
                forced.push(start + f);
            }
            for (a, b) in ksolid {
                solid.push((start + a, start + b));
            }
            // Дотяг ребёнка — от ЕГО верха; переводим в
            // координаты этой коробки. `y` он не двигает:
            // внепоточный соседей не сдвигает
            // (CSS 2.1 §9.3.1).
            oof_reach = oof_reach.max(start + kreach);
            y = start + h;
            prev_mb = kmb;
            first = false;
        }
        stacked = Some((y, through, prev_mb));
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): ряд flex С ПЕРЕНОСОМ
        // как строки — жадная сборка по ширинам детей в точках
        // (css-flexbox-1 §9.3), разрез между строками, высота —
        // сумма строк: срез фрагментации 469 -> 469 (0/0) —
        // ширины элементов в тестах не в точках (`flex: 1`,
        // проценты), ветка не срабатывает. Нужна ширина из
        // раскладки, а не из стиля (корень R4 scout-flexfrag).
        }
    }
    let (h, mt, mb) = match c.style.height {
        Some(Len::Px(v)) => (v + top + bot, mt, mb),
        None => match stacked {
            Some((end, through, last_mb)) => (
                end + bot,
                mt.max(through),
                if bot == 0.0 { mb.max(last_mb) } else { mb },
            ),
            None if kids.is_empty() => (top + bot, mt, mb),
            // Сетка без заданной высоты: её высоту знают
            // ЯВНЫЕ дорожки рядов (`grid-template-rows:
            // 200px`) с зазорами между ними. Без этой
            // оценки укладка колонок отказывалась от всей
            // коробки, и многоколоночник с сеткой внутри
            // уходил в запасную сетку целиком
            // (`scout-break-2026-09b.md`, корень C1).
            None => match grid_rows_px(&c.style) {
                Some(v) => (v + top + bot, mt, mb),
                None => return None,
            },
        },
        Some(_) => return None,
    };
    // Содержащий блок обязан дотянуться до низа своих
    // внепоточных потомков — только тогда фрагментация
    // родит под них колонки, а балансировка их посчитает
    // (css-position-3 §abspos-breaking; Blink
    // `column_layout_algorithm.cc:1092-1131` прогоняет
    // `OutOfFlowLayoutPart` внутри цикла балансировки
    // именно ради этого). Если коробка содержащим блоком
    // НЕ является, дотяг принадлежит кому-то выше и здесь
    // не учитывается — он всплывёт там.
    let h = if crate::inline::establishes_cb(&c.style) {
        h.max(oof_reach + bot)
    } else {
        h
    };
    cuts.retain(|&(need, _)| need > 0.01 && need < h - 0.01);
    forced.retain(|&f| f > 0.01 && f < h - 0.01);
    if bot > 0.0 {
        solid.push((h - bot, h));
    }
    Some((h, mt, mb, cuts, forced, solid))
}

/// Коробка, дающая точку разрыва класса A (css-break-4 §possible-breaks):
/// блочная, в потоке, не плавающая.
fn class_a_box(e: &Element) -> bool {
    !e.inline
        && !out_of_flow(&e.style)
        && e.style.float.unwrap_or(0) == 0
        && !matches!(
            e.style.display,
            Some(Display::None) | Some(Display::Contents)
        )
}

/// Монолит стопки страниц (css-break-4 §4.1; Blink `IsMonolithic`):
/// замещаемый, прокручиваемый, `break-inside: avoid`, `contain: size`,
/// атомарный строчный. Сплошной строчный набор монолитом НЕ считается:
/// страница режет его по краю, а обе стороны пары режутся одинаково.
fn page_monolith(e: &Element) -> bool {
    let scrolls = |o: Option<crate::computed::Overflow>| {
        matches!(o, Some(crate::computed::Overflow::Scroll))
    };
    e.style.break_inside_avoid
        || e.style.contain_size == Some(true)
        || scrolls(e.style.overflow_x)
        || scrolls(e.style.overflow_y)
        || matches!(
            e.tag.as_str(),
            "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe"
        )
        || matches!(
            e.style.display,
            Some(Display::InlineBlock) | Some(Display::InlineFlex) | Some(Display::InlineGrid)
        )
}

/// Начальное и конечное значения 'page' коробки (css-page-3 §"Using named
/// pages", п. 1-2): `auto` берёт имя ближайшего предка; начальное — от
/// ПЕРВОЙ дочерней коробки, конечное — от ПОСЛЕДНЕЙ, рекурсивно, но
/// передаёт значение только коробка, к которой свойство применяется
/// (класс A); текст, строчный, флоат, абсолют — не передают, и тогда
/// берётся используемое значение самой коробки.
fn page_names(e: &Element, inherited: &str) -> (String, String) {
    let used = e.style.page.clone().unwrap_or_else(|| inherited.to_string());
    let boxes: Vec<&Node> = e
        .children
        .iter()
        .filter(|n| !is_blank(n))
        .filter(|n| !matches!(n, Node::Element(k) if matches!(k.style.display, Some(Display::None))))
        .collect();
    let via = |n: Option<&&Node>| match n {
        Some(Node::Element(k)) if class_a_box(k) => Some(page_names(k, &used)),
        _ => None,
    };
    let start = via(boxes.first()).map(|p| p.0).unwrap_or_else(|| used.clone());
    let end = via(boxes.last()).map(|p| p.1).unwrap_or_else(|| used.clone());
    (start, end)
}

thread_local! {
    /// Определения `<mask id>` / `<clipPath id>` документа: id — разметка
    /// содержимого. Ссылки `url(#id)` из `mask-image`/`clip-path` резолвятся
    /// при отрисовке (см. `interact::Grouped`).
    static MASK_DEFS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Чей документ собран: адрес среза узлов. Виртуализация рисует ПО
    /// БЛОКАМ (`render_block`) — сбор на каждый блок каждого кадра был бы
    /// расточительным, а документ между кадрами один и тот же.
    static MASK_DEFS_FOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Содержимое определения маски по имени (`#id` без решётки).
pub(crate) fn mask_def(id: &str) -> Option<String> {
    MASK_DEFS.with(|m| m.borrow().get(id).cloned())
}

thread_local! {
    /// Снимки определений на момент СБОРКИ дерева: отрисовка идёт позже, а
    /// документов в кадре может быть два (тест и эталон стенда) — реестр
    /// определений к моменту отрисовки уже перезаписан другим документом.
    /// Снимки копятся под уникальными ключами и не чистятся.
    static MASK_SNAPS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    static MASK_SNAP_N: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Снять снимок определения; ключ живёт до конца кадра и дольше.
fn snapshot_mask_def(id: &str) -> Option<String> {
    let markup = mask_def(id)?;
    let key = MASK_SNAP_N.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        format!("k{n}")
    });
    MASK_SNAPS.with(|m| {
        let mut map = m.borrow_mut();
        // Кадры идут бесконечно — тысяча снимков означает утечку, чистим.
        if map.len() > 1000 {
            map.clear();
        }
        map.insert(key.clone(), markup);
    });
    Some(key)
}

/// Разметка по ключу снимка (для отрисовки).
pub(crate) fn mask_snapshot(key: &str) -> Option<String> {
    MASK_SNAPS.with(|m| m.borrow().get(key).cloned())
}

/// Заменить ссылки `url(#id)` / `clipref:id` в строке маски снимками
/// определений: к отрисовке реестр может смениться другим документом.
fn resolve_mask_refs(raw: &str) -> String {
    if let Some(id) = raw.strip_prefix("clipref:") {
        return match snapshot_mask_def(id) {
            Some(key) => format!("clipsnap:{key}"),
            None => raw.to_string(),
        };
    }
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find("url(#") {
        out.push_str(&rest[..at]);
        let tail = &rest[at + 5..];
        let Some(end) = tail.find(')') else {
            out.push_str(&rest[at..]);
            return out;
        };
        let id = tail[..end].trim().trim_matches(|c| c == '"' || c == '\'');
        match snapshot_mask_def(id) {
            Some(key) => out.push_str(&format!("url(svgsnap:{key})")),
            None => out.push_str(&rest[at..at + 5 + end + 1]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Собрать определения масок ДО отрисовки: ссылка может стоять раньше
/// определения по тексту.
fn collect_mask_defs(nodes: &[Node]) {
    let key = nodes.as_ptr() as usize;
    if MASK_DEFS_FOR.with(|c| c.get()) == key {
        return;
    }
    MASK_DEFS_FOR.with(|c| c.set(key));
    fn walk(nodes: &[Node], out: &mut std::collections::HashMap<String, String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            let tag = e.tag.to_ascii_lowercase();
            if (tag == "mask" || tag == "clippath")
                && let Some(id) = e.attr("id")
            {
                let mut markup = String::new();
                for c in &e.children {
                    if let Node::Element(el) = c {
                        crate::svg::write_element(el, &mut markup);
                    }
                }
                out.insert(id.to_string(), markup);
            }
            // `<filter id>` — целиком, с атрибутами области (x/y/width/height,
            // filterUnits): ключ с префиксом, чтобы не спутать с маской.
            if tag == "filter" && let Some(id) = e.attr("id") {
                let mut markup = String::new();
                crate::svg::write_element(e, &mut markup);
                out.insert(format!("filter:{id}"), markup);
            }
            walk(&e.children, out);
        }
    }
    MASK_DEFS.with(|m| {
        let mut map = m.borrow_mut();
        map.clear();
        walk(nodes, &mut map);
    });
}

/// Один блок верхнего уровня — единица виртуализации.
///
/// Список GPUI спрашивает только видимые блоки, и невидимая часть документа
/// не стоит ничего: ни раскладки, ни отрисовки. Это то же ухищрение, которым
/// держится дерево файлов и чат.
pub fn render_block(nodes: &[Node], index: usize, opts: &RenderOpts) -> Option<AnyElement> {
    let node = nodes.get(index)?;
    crate::interact::frame_sanitize();
    IFRAME_DEPTH.with(|d| d.set(0));
    collect_mask_defs(nodes);
    let root = opts.root_style();
    // Слой ICB закрывается на блок ленты: дальше своего блока внепоточный
    // элемент всё равно не уедет, а без слоя он остался бы на месте.
    crate::interact::icb_open();
    let out = blocks(std::slice::from_ref(node), &root, opts);
    let layer = crate::interact::icb_close();
    let first = out.into_iter().next()?;
    if layer.is_empty() {
        return Some(first);
    }
    // Лента отдаёт РОВНО ОДИН элемент на блок, поэтому слой уходит внутрь
    // обёртки. Содержащим блоком становится она, а не окно: в ленте окна
    // всё равно нет — блок живёт в прокрутке. Без обёртки вынесенный
    // элемент просто пропадал бы с экрана.
    Some(
        div()
            .relative()
            .child(first)
            .children(layer)
            .into_any_element(),
    )
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

thread_local! {
    /// Ширина содержащего блока в точках, когда её видно из стиля родителя.
    /// Нужна замещаемому элементу БЕЗ собственного размера, но С соотношением:
    /// §10.3.2 (последний пункт) берёт его ширину из уравнения для блочных
    /// коробок, то есть из содержащего блока, а резерв 300×150 применяется
    /// только когда ширину взять неоткуда.
    static CB_WIDTH: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Вернуть прежнюю ширину содержащего блока по выходе из `blocks()`.
struct CbWidthGuard(Option<f32>);
impl Drop for CbWidthGuard {
    fn drop(&mut self) {
        CB_WIDTH.set(self.0);
    }
}
fn scopeguard_cb(prev: Option<f32>) -> CbWidthGuard {
    CbWidthGuard(prev)
}

// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): разворачивать `text-emphasis` в поштучные
// руби (по знаку-аннотации над каждой буквой базы, кроме пробелов и
// пунктуации — css-text-decor-3 §5.3). Срез руби и акцентов, 167 пар:
// 125 -> 125, приобретено 6 (`text-emphasis-line-height-001a/002a/002b`,
// `-position-over-left-002`, `-position-under-left-002`, `-punctuation-3`),
// потеряно 6 — `-line-height-004a..d` 0.07 -> 0.7 и `-punctuation-1/2`
// 0.00 -> 6.31/3.12: поштучный атом меняет разбивку строки и подъём базовой
// линии, а эталоны семьи считают её по-своему. Возвращать вместе с
// настоящей надстрочной аннотацией (сдвиг базовой линии без атома).
fn blocks(nodes: &[Node], inherited: &Computed, opts: &RenderOpts) -> Vec<AnyElement> {
    let cb_prev = CB_WIDTH.get();
    if let Some(Len::Px(w)) = inherited.width
        && w > 0.0
    {
        CB_WIDTH.set(Some(w));
    }
    let _cb_guard = scopeguard_cb(cb_prev);
    // `content-visibility: hidden`: содержимое пропускается целиком
    // (css-contain-2 §4) — коробка остаётся, детей нет.
    let stripped: Vec<Node>;
    let nodes = if nodes.iter().any(
        |n| matches!(n, Node::Element(e) if e.style.skip_content == Some(true) && !e.children.is_empty()),
    ) {
        stripped = nodes
            .iter()
            .map(|n| match n {
                Node::Element(e) if e.style.skip_content == Some(true) => {
                    let mut copy = e.clone();
                    copy.children.clear();
                    Node::Element(copy)
                }
                other => other.clone(),
            })
            .collect();
        &stripped
    } else {
        nodes
    };
    // `order` в CSS работает ТОЛЬКО внутри гибкого контейнера и сетки; в
    // обычном потоке он не значит ничего. Раньше сортировались дети любого
    // родителя — блоки меняли порядок там, где браузер их не трогает.
    let under_tf = inherited.transform_ancestor || inherited.transform.is_some();
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
    // Блок внутри строчного разрывает его на анонимные коробки (CSS 2.1
    // §9.2.1.1) — разбиение идёт ДО схлопывания полей: вынесенный блок
    // обязан схлопнуть свои поля с новыми соседями. В гибком контейнере и
    // сетке разрыва нет вовсе: там дети блокифицируются, и куски уехали бы
    // по чужим дорожкам.
    let split = if ordered_context {
        nodes.to_vec()
    } else {
        // Анонимная таблица вокруг ПРОГОНА табличных братьев (§17.2.1 шаг 3)
        // — до разбиения блока в строчном и до схлопывания полей, как это
        // делает и сборщик дерева в браузере.
        split_block_in_inline(&wrap_anon_tables(nodes))
    };
    // §10.3.3: у блока в потоке с `width: auto` боковое `auto`-поле
    // используется НУЛЁМ, а коробка занимает всю ширину. У нас блок — гибкая
    // колонка, и любое auto-поле на поперечной оси отменяет растяжение до
    // дорожки: абзац сжимался по содержимому и уезжал к краю.
    let split = if ordered_context {
        split
    } else {
        split
            .into_iter()
            .map(|n| match n {
                Node::Element(mut e)
                    if in_flow(&e.style)
                        && matches!(e.style.width, None | Some(Len::Auto))
                        && (e.style.margin.left == Some(Len::Auto)
                            || e.style.margin.right == Some(Len::Auto)) =>
                {
                    if e.style.margin.left == Some(Len::Auto) {
                        e.style.margin.left = Some(Len::Px(0.0));
                    }
                    if e.style.margin.right == Some(Len::Auto) {
                        e.style.margin.right = Some(Len::Px(0.0));
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    let nodes: &[Node] = &split;
    let collapsed = if ordered_context {
        reorder(nodes.to_vec())
    } else {
        // Схлопывание идёт ДО наследования стилей, поэтому кегль уровня
        // передаётся отдельно: `margin: 1em` без своего `font-size` меряется
        // от родительского.
        let base = match inherited.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        };
        let prev = COLLAPSE_FONT_PX.with(|c| c.replace(base));
        // Ширина содержащего блока для ПРОЦЕНТНЫХ полей (§8.3: проценты полей
        // считаются от ширины содержащего блока, схлопывание — по уже
        // разрешённым значениям). Известна только заданная в точках.
        let cb_w = match inherited.width {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        let prev_w = COLLAPSE_CB_WIDTH_PX.with(|c| c.replace(cb_w));
        let out = collapse_margins(
            nodes,
            matches!(
                inherited.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ),
        );
        COLLAPSE_FONT_PX.with(|c| c.set(prev));
        COLLAPSE_CB_WIDTH_PX.with(|c| c.set(prev_w));
        out
    };
    // Плавающий блок и выравнивание по базовой линии на элементе гибкого
    // контейнера или сетки НЕ действуют — так велит CSS. Без этого правила
    // `float: right` на элементе ряда выкидывал его из раскладки родителя.
    let collapsed: Vec<Node> = if ordered_context {
        collapsed
            .into_iter()
            // `visibility: collapse` на элементе гибкого контейнера убирает
            // его из строки, НО оставляет РАСПОРКУ (strut, css-flexbox §4.4):
            // поперечный размер и базовая линия ряда меряются как при нём
            // (flexbox-collapsed-item-baseline-001). Распорка — тот же
            // элемент с нулевой ГЛАВНОЙ осью и невидимой краской.
            .map(|n| match n {
                Node::Element(mut e) if e.style.collapsed == Some(true) => {
                    match inherited.flex_dir {
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse) => {
                            e.style.height = Some(Len::Px(0.0));
                            e.style.max_height = Some(Len::Px(0.0));
                        }
                        _ => {
                            e.style.width = Some(Len::Px(0.0));
                            e.style.max_width = Some(Len::Px(0.0));
                        }
                    }
                    e.style.hidden = Some(true);
                    Node::Element(e)
                }
                other => other,
            })
            // ПРОБЕЛЬНЫЙ текст между детьми ряда/сетки не рождает анонимный
            // элемент (css-flexbox §4): переводы строк разметки давали
            // лишние 2-3px между коробками
            // (flexbox-baseline-align-self-baseline-horiz-001: тест дышит
            // щелями, эталон написан слитно).
            .filter(|n| !matches!(n, Node::Text(_)) || !is_blank(n))
            .map(|n| match n {
                Node::Element(mut e) => {
                    e.style.float = None;
                    e.style.clear = None;
                    e.style.vertical_align = None;
                    // Элемент КОЛОНКИ: определён ли главный размер
                    // контейнера (css-flexbox-1 §9.8 п.1). От этого зависит,
                    // определён ли блок у ЕГО детей — доля высоты внутри
                    // элемента колонки без высоты решается как `auto`
                    // (Blink `flex_layout_algorithm.cc`:
                    // `is_initial_block_size_indefinite`).
                    if matches!(
                        inherited.display,
                        Some(Display::Flex) | Some(Display::InlineFlex)
                    ) && matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    ) {
                        let definite = matches!(inherited.height, Some(Len::Px(_)))
                            || (matches!(inherited.height, Some(Len::Pct(_)))
                                && inherited.cb_height_def)
                            || inherited.stretched
                            || inherited.root_box;
                        e.style.flex_main_def = Some(definite);
                    }
                    // ★ Эти три правила жили в ветке ОБЫЧНОГО потока (`else`
                    // ниже) с проверками на Flex/Grid-родителя — и были
                    // недостижимы по построению (скаут flexbox: пробы
                    // `flex2-colbasis-*` показали, что компенсация основы не
                    // действует). Их место — здесь, среди детей ряда/сетки.
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    // `<canvas>` в сетке: атрибуты `width/height` — природный
                    // размер и соотношение сторон, а не CSS-размер (HTML
                    // §4.12.5, §15.3.10). Ось, растянутая выравниванием
                    // (`stretch`, css-align-3 §6.1) или переносимая из
                    // заданной автором другой оси (css-sizing-4 «transferred
                    // size»), становится `auto`, соотношение — на коробку
                    // (`replaced-element-011`, `grid-item-inline-contribution-*`,
                    // `replaced-alignment-with-aspect-ratio-001`).
                    if e.tag == "canvas"
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && (e.style.attr_sized.0 || e.style.attr_sized.1)
                    {
                        let stretch = |own: Option<Align>, items: Option<Align>| {
                            own == Some(Align::Stretch)
                                || (own.is_none() && items == Some(Align::Stretch))
                        };
                        let free_x = e.style.attr_sized.0
                            && (stretch(e.style.justify_self, inherited.justify_items)
                                || !e.style.attr_sized.1);
                        let free_y = e.style.attr_sized.1
                            && (stretch(e.style.align_self, inherited.align_items)
                                || !e.style.attr_sized.0);
                        if free_x || free_y {
                            if let (Some(Len::Px(w)), Some(Len::Px(h))) =
                                (e.style.attr_width, e.style.attr_height)
                                && h > 0.0
                                && e.style.aspect_ratio.is_none()
                            {
                                e.style.aspect_ratio = Some(w / h);
                            }
                            if free_x {
                                e.style.width = None;
                            }
                            if free_y {
                                e.style.height = None;
                            }
                        }
                    }
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    // `flex-basis` задаёт размер СОДЕРЖИМОГО (css-flexbox-1 §7.2.3:
                    // «flex-basis determines the size of the content box, unless
                    // otherwise specified such as by box-sizing»), а в раскладку
                    // уходит внешний размер — как `width`/`height` в `apply`, основа
                    // получает отбивку и рамку по ГЛАВНОЙ оси родителя
                    // (`flexbox-mbp-horiz-*`, `flexbox-justify-content-horiz-002`).
                    if matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex))
                        && inherited.vertical.is_none()
                        && e.style.border_box != Some(true)
                        && let Some(Len::Px(b)) = e.style.flex_basis
                    {
                        let px_of = |l: Option<Len>| match l {
                            Some(Len::Px(v)) => v,
                            _ => 0.0,
                        };
                        let bd = e.style.borders();
                        let row = matches!(
                            inherited.flex_dir,
                            None
                                | Some(crate::computed::FlexDir::Row)
                                | Some(crate::computed::FlexDir::RowReverse)
                        );
                        let extra = if row {
                            px_of(e.style.padding.left)
                                + px_of(e.style.padding.right)
                                + px_of(bd.left)
                                + px_of(bd.right)
                        } else {
                            px_of(e.style.padding.top)
                                + px_of(e.style.padding.bottom)
                                + px_of(bd.top)
                                + px_of(bd.bottom)
                        };
                        e.style.flex_basis = Some(Len::Px(b + extra));
                    }
                    // Элемент сетки с `aspect-ratio` при `normal` (css-grid-2
                    // §6.6.1): «sized consistent with the size calculation
                    // rules for block-level elements» — строчная ось заполняет
                    // область (как stretch), а БЛОЧНАЯ идёт из соотношения, не
                    // растягиваясь на ряд: там `start`
                    // (`grid-aspect-ratio-001/007/010/038`). ★ ЗАМЕРЕНО: `start`
                    // и по строчной оси — `grid-aspect-ratio-018/038` в красное.
                    if ratio_ok
                        && matches!(inherited.display, Some(Display::Grid) | Some(Display::InlineGrid))
                        && !positioned_out
                        && inherited.vertical.is_none()
                    {
                        let auto_w = matches!(e.style.width, None | Some(Len::Auto));
                        let auto_h = matches!(e.style.height, None | Some(Len::Auto));
                        // Обе оси auto: строчная заполняет область, блочная — из
                        // соотношения. Блочная определена: строчная — из
                        // соотношения (CSS2 §10.3.2 для замещаемого с
                        // соотношением; css-sizing-4 §5.1).
                        if auto_w
                            && !auto_h
                            && e.style.justify_self.is_none()
                            && inherited.justify_items != Some(Align::Stretch)
                        {
                            e.style.justify_self = Some(Align::Start);
                        }
                        if auto_h
                            && e.style.align_self.is_none()
                            && inherited.align_items != Some(Align::Stretch)
                        {
                            e.style.align_self = Some(Align::Start);
                        }
                    }
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
    let flex_ctx = matches!(inherited.display, Some(Display::Flex) | Some(Display::InlineFlex));
    let collapsed = by_layer(wrap_floats(collapsed, inherited.width, inherited.clear), flex_ctx);
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
                    // `vertical-align` на элементе гибкого контейнера не
                    // действует (css-flexbox-1 §4): он выравнивается своими
                    // свойствами, а не как кусок строки.
                    e.style.vertical_shift = None;
                    e.style.vertical_shift_px = None;
                    // Элемент ряда под обособлением строчной оси: главный
                    // размер берётся из `contain-intrinsic-size`, а не от
                    // содержимого. В колонке главная ось блочная — её уже
                    // держит подмена высоты.
                    let row = !matches!(
                        inherited.flex_dir,
                        Some(FlexDir::Col) | Some(FlexDir::ColReverse)
                    );
                    if row
                        && e.style.contains_width()
                        && matches!(e.style.width, None | Some(Len::Auto))
                    {
                        e.style.width = Some(Len::Px(e.style.contain_intrinsic.0.unwrap_or(0.0)));
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
                Node::Element(mut e) => {
                    if e.style.flex_shrink.is_none() {
                        e.style.flex_shrink = Some(0.0);
                    }
                    // rtl: переполняющий блок с ЗАДАННОЙ шириной прижат к
                    // правому краю и вылезает влево (csswg-drafts#5572);
                    // только горизонтальное письмо — в вертикали cross-ось
                    // иная (abs-pos-border-offset-001/002).
                    if inherited.rtl == Some(true)
                        && inherited.vertical_rl.is_none()
                        && e.style.width.is_some()
                        && e.style.align_self.is_none()
                        && !e.inline
                        && !matches!(
                            e.style.position,
                            Some(crate::computed::Position::Absolute)
                                | Some(crate::computed::Position::Fixed)
                        )
                    {
                        e.style.align_self = Some(Align::End);
                    }
                    // Коробка с `aspect-ratio` при auto-ширине и определённой
                    // высоте — fit-content, а не растяжка (css-sizing-4 §5.1:
                    // «automatic sizes are calculated the same as for a replaced
                    // element with a natural aspect ratio»; Blink length_utils
                    // `may_apply_aspect_ratio` → FitContent). Блок у нас —
                    // колонка flex, и `stretch` тянул ширину на всю строку
                    // (`block-aspect-ratio-002/006/…`).
                    // Нулевое и бесконечное отношение — как `auto`
                    // (css-sizing-4 §5.1; `zero-or-infinity-002`).
                    let positioned_out = matches!(
                        e.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                    let ratio_ok = e.style.aspect_ratio.is_some_and(|r| r.is_finite() && r > 0.0);
                    if ratio_ok
                        && !ordered_context
                        && matches!(e.style.width, None | Some(Len::Auto))
                        && matches!(e.style.height, Some(Len::Px(_)))
                        && e.style.align_self.is_none()
                        && inherited.vertical.is_none()
                        && !e.inline
                        && !positioned_out
                    {
                        e.style.align_self = Some(Align::Start);
                    }
                    Node::Element(e)
                }
                other => other,
            })
            .collect()
    };
    // §9.9 шаг 8: позиционированная коробка рисуется ПОВЕРХ содержимого
    // потока, а порядок краски у нас — порядок детей. В обычном потоке это
    // делает верхний слой, но в сетке и гибком контейнере он выключен
    // (`!ordered_context` у гейтов ниже), и абсолютный ребёнок оказывался под
    // соседями. Здесь его достаточно переставить в конец: место он берёт не
    // из потока (в раскладку сетки такой ребёнок не входит), поэтому
    // перестановка меняет только краску.
    let collapsed: Vec<Node> = if ordered_context {
        let (flow, over): (Vec<Node>, Vec<Node>) = collapsed.into_iter().partition(|n| match n {
            Node::Element(e) => {
                e.style.position != Some(crate::computed::Position::Absolute)
                    || e.style.z_index.is_some_and(|z| z < 0)
            }
            Node::Text(_) => true,
        });
        flow.into_iter().chain(over).collect()
    } else {
        collapsed
    };
    let mut out = vec![];
    // Порядок краски подслоя (§9.9 шаг 3): соседние распорки отрицательного
    // `z-index` стоят в порядке разметки, а рисоваться обязаны по z. Высота у
    // них нулевая и y общий, поэтому перестановка СОСЕДЕЙ раскладку не меняет
    // — в отличие от перестановки в общем списке детей, замеренной в минус
    // (см. `movable`). Прогон рвётся сам, как только между распорками встаёт
    // что-то ещё.
    let mut below_run_start = 0usize;
    let mut below_run_end = usize::MAX;
    let mut below_zs: Vec<i32> = vec![];
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
    for (idx, n) in nodes.iter().enumerate() {
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
            // Только когда заданы ОБЕ оси: у коробки с одним краем свободная
            // ось остаётся статической, а статическая позиция строчного — в
            // строке, не в блочном потоке. Такую коробку ведёт щуп в
            // `atom_element` (`x_set != y_set`).
            Node::Element(e)
                if matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) && !at_static_position(&e.style)
                    && {
                        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
                        (edge(e.style.inset.left) || edge(e.style.inset.right))
                            && (edge(e.style.inset.top) || edge(e.style.inset.bottom))
                    }
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
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable) => !ordered_context,
                // `display: inline grid-lanes` — такая же строчная коробка:
                // разбор держит её как `GridLanes` с пометкой `lanes_inline`,
                // и по css-display-3 внешний вид у неё `inline`. Эталоны
                // семьи `grid-lanes-intrinsic-sizing-*` написаны на
                // `display: inline-grid`, и без этой строки девять сеток
                // вставали столбиком вместо ряда.
                Some(Display::GridLanes) if e.style.lanes_inline => !ordered_context,
                // Прежний откат этой строки СНЯТ (03.09). Он мерился, когда
                // строчный атом строил лунки голым `blocks()` и терял их
                // целиком — оттого вся восьмёрка `flow-tolerance-*` и уходила
                // в красное (0.00 -> 5.66 и родня). Теперь `atom_element`
                // отдаёт лунки блочному пути (`element()`), и обе правки
                // вместе дают по всему CSS3 2420 -> 2442: приобретено 27,
                // потеряно 5 (`row-line-names-007/008/010/012`,
                // `row-subgrid-abs-pos-002` — рядные лунки, они ждут обтяжку
                // по РЯДАМ, корень R4 из `target/scout-subgrid-orthogonal-
                // 2026-09.md`).
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строчный путь для абсолюта
                // с объявленным `display: inline` на статической позиции
                // (css-position-3 §staticpos-rect). Срез 12086 пар вместе с
                // патчем барьера `contain`: 9343 -> 9346 (+9/-6), причём вся
                // шестёрка потерь — этого рукава:
                // `inline-level-absolute-in-block-level-context-002`
                // (0.26->0.52), `-007` (0.00->0.54), `-010` (0.00->1.04),
                // `position-absolute-dynamic-static-position-inline`
                // (0.00->2.10), `abs-pos-border-offset-003` (0.46->1.75),
                // `css-flexbox-height-animation-stretch` (0.10->1.90), против
                // всего двух приобретений (`-009`, `-012`). Строчная ветка
                // теряет полосу обтекания и рамочные смещения — рукав нужен
                // не здесь, а в `atom_element`.
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
            let _deferred_guard = DeferGuard::enter(defers(&e.style, under_tf));
            // Ряд обтекания: текст рядом с плавающим блоком и остаток под ним.
            if e.tag == "kamin-float" {
                out.push(float_flow(e, inherited, opts));
                continue;
            }
            if let Some(el) = scrollable(e, inherited, opts) {
                out.push(layered(el, &e.style, layer_ok, under_tf));
                continue;
            }
            if let Some(el) = resizable(e, inherited, opts) {
                out.push(el);
                continue;
            }
            if let Some(el) = transitioned(e, inherited, opts) {
                // Наложение считается и для узла с переходом: раньше ветка
                // уходила мимо, и `z-index` у него пропадал.
                out.push(layered(el, &e.style, layer_ok, under_tf));
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
                && !replaced_tag(e)
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
                // Фон-КАРТИНКА канваса красит всю область просмотра тем же
                // слоем (CSS 2.2 §14.2: painting area корневого фона —
                // канвас): на коробке корня она начиналась с его сдвинутого
                // схлопкой верха, и над краской проступала полоса
                // (background-size-document-root-vrl-*).
                let mut layer = layer.into_any_element();
                // Донор — САМ корень: область ОТСЧЁТА плитки это его коробка
                // (§14.2 «sized and positioned relative to the root element's
                // box»), а красит она весь холст. Донор-тело сюда не входит:
                // его слой лежит в детях корня, и отсчёт от padding-box корня
                // получается сам (см. записи о двух откатах ниже).
                if e.tag == "html"
                    && e.style.bg_image.is_some()
                    && let Some(tiles) = {
                        // Единицы шрифта тоже длина: `html { margin-top: 1em }`
                        // роняло отсчёт в ноль, и плитка начиналась с края
                        // холста (`margin-collapse-020`).
                        let em = match e.style.font_size {
                            Some(Len::Px(v)) => v,
                            _ => opts.base_size(),
                        };
                        let fam = e.style.font_family.clone().unwrap_or_default();
                        let side = |l: Option<Len>| match l {
                            Some(Len::Px(v)) => v,
                            Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                                crate::metrics::spacing_px(Some(l), &fam, em)
                            }
                            _ => 0.0,
                        };
                        let b = e.style.borders();
                        let area = crate::background::RootArea {
                            left: side(e.style.margin.left) + side(b.left),
                            top: side(e.style.margin.top) + side(b.top),
                            right: side(e.style.margin.right) + side(b.right),
                            bottom: side(e.style.margin.bottom) + side(b.bottom),
                            width: match e.style.width {
                                Some(Len::Px(w)) => Some(
                                    w + side(e.style.padding.left) + side(e.style.padding.right),
                                ),
                                _ => None,
                            },
                            height: match e.style.height {
                                Some(Len::Px(h)) => Some(
                                    h + side(e.style.padding.top) + side(e.style.padding.bottom),
                                ),
                                _ => None,
                            },
                            from_right: e.style.vertical_rl == Some(true),
                        };
                        crate::background::canvas_layer(&e.style, area)
                    }
                {
                    layer = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(layer)
                        .child(tiles)
                        .into_any_element();
                } else if e.style.bg_image.is_some()
                    && let Some(tiles) = crate::background::layer(&e.style)
                {
                    // Область ПОЗИЦИОНИРОВАНИЯ краски — PADDING-BOX корня:
                    // ширина + горизонтальные отступы; полоса прижата по
                    // письму с учётом поля и рамки с той стороны.
                    //
                    // ЗАМЕРЕНО ВТОРОЙ РАЗ (перенос фона тела на корень по §14.2
                    // ВМЕСТЕ с `RootArea` + `canvas_layer`): приобретено 1,
                    // потеряно 2 — `background-position-001` 0.36 -> 2.39 и
                    // `background-root-024` 0.17 -> 5.74. Перенос сам по себе
                    // даёт 0 и −2. Значит дело не в кегле тела: расходится
                    // геометрия коробки корня, и её надо чинить первой.
                    //
                    // ЗАМЕРЕНО: считать область от коробки корня целиком
                    // (`RootArea` + `canvas_layer`, плитка красит весь холст)
                    // — CSS2 +2 в `background-root-001/002`, но -3 в
                    // `margin-collapse-020/021` и `block-formatting-contexts-003`:
                    // слой на весь холст перекрывает то, что рисуется выше по
                    // потоку. Возвращаться вместе с переносом фона тела на
                    // корень, когда кегль тела будет разрешаться до переноса.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    let b = e.style.borders();
                    let mut band = div().absolute().top_0().bottom_0();
                    band = match e.style.width {
                        Some(Len::Px(w)) => {
                            let pad_w =
                                w + side(e.style.padding.left) + side(e.style.padding.right);
                            let band = band.w(px(pad_w));
                            if e.style.vertical_rl == Some(true) {
                                band.right(px(side(e.style.margin.right) + side(b.right)))
                            } else {
                                band.left(px(side(e.style.margin.left) + side(b.left)))
                            }
                        }
                        _ => band.left_0().right_0(),
                    };
                    layer = div()
                        .absolute()
                        .top_0()
                        .left_0()
                        .right_0()
                        .bottom_0()
                        .child(layer)
                        .child(band.child(tiles))
                        .into_any_element();
                }
                out.push(layer);
                let mut copy = e.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                copy.style.bg_image = None;
                canvas_stripped = copy;
                &canvas_stripped
            } else {
                e
            };
            // Абсолют с КЛЮЧЕВЫМ СЛОВОМ содержимого по оси и краями с обеих
            // сторон этой оси (css-position-3 §3.7-3.8): растяжение краями —
            // только для автоматического размера; заданный ключевым словом
            // размер — по содержимому, а остаток делят auto-поля. Держатель =
            // inset-modified containing block (абсолют с краями элемента,
            // гибкий контейнер вдоль оси), внутри — та же коробка статической,
            // без краёв и полей (`div-{min,max,fit}-content-block-size`,
            // `div-*-auto-margin-*`).
            let kw_len = |l: Option<Len>| {
                matches!(
                    l,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                )
            };
            let positioned_out = matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            );
            let holder_axis = if positioned_out
                && e.style.vertical.is_none()
                && kw_len(e.style.height)
                && edge_set(e.style.inset.top)
                && edge_set(e.style.inset.bottom)
            {
                Some(true)
            } else if positioned_out
                && e.style.vertical.is_none()
                && kw_len(e.style.width)
                && edge_set(e.style.inset.left)
                && edge_set(e.style.inset.right)
            {
                Some(false)
            } else {
                None
            };
            let built = if let Some(block_axis) = holder_axis {
                let auto = |l: Option<Len>| l == Some(Len::Auto);
                let mut holder = Computed::default();
                holder.position = e.style.position;
                holder.inset = e.style.inset;
                holder.z_index = e.style.z_index;
                holder.display = Some(Display::Flex);
                holder.flex_dir = Some(if block_axis {
                    crate::computed::FlexDir::Col
                } else {
                    crate::computed::FlexDir::Row
                });
                // Поля вдоль оси остаются у ВНУТРЕННЕЙ коробки: auto-поля
                // элемента гибкого контейнера забирают остаток, а при нехватке
                // места обнуляются (css-flexbox-1 §8.1) — ровно как auto-поля
                // абсолюта (§3.8; `fit-content-block-size-abspos` с
                // переполнением). Поперечные не-auto поля — у держателя.
                // Поперечные поля — у держателя целиком, включая auto: с
                // заданным размером и краями с обеих сторон они центрируют
                // сам абсолют (css-position-3 §3.8; `inline-size: 100px;
                // margin: auto; inset: 0`).
                if block_axis {
                    holder.margin.left = e.style.margin.left;
                    holder.margin.right = e.style.margin.right;
                } else {
                    holder.margin.top = e.style.margin.top;
                    holder.margin.bottom = e.style.margin.bottom;
                }
                // Поперечный размер держателя — border-box внутренней коробки:
                // её рамка и отбивка прибавляются (у держателя своих нет).
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                let bd = e.style.borders();
                if block_axis {
                    let extra = px_of(e.style.padding.left)
                        + px_of(e.style.padding.right)
                        + px_of(bd.left)
                        + px_of(bd.right);
                    holder.width = match e.style.width {
                        Some(Len::Px(w)) => Some(Len::Px(w + extra)),
                        other => other.filter(|l| !kw_len(Some(*l))),
                    };
                } else {
                    let extra = px_of(e.style.padding.top)
                        + px_of(e.style.padding.bottom)
                        + px_of(bd.top)
                        + px_of(bd.bottom);
                    holder.height = match e.style.height {
                        Some(Len::Px(h)) => Some(Len::Px(h + extra)),
                        other => other.filter(|l| !kw_len(Some(*l))),
                    };
                }
                let mut inner = e.clone();
                inner.style.position = None;
                inner.style.inset = Default::default();
                if block_axis {
                    inner.style.margin.left = None;
                    inner.style.margin.right = None;
                } else {
                    inner.style.margin.top = None;
                    inner.style.margin.bottom = None;
                }
                inner.style.z_index = None;
                crate::apply::apply(div(), &holder)
                    .child(element(&inner, inherited, opts))
                    .into_any_element()
            } else {
                grouped(
                    transformed(animated(e, inherited, opts), &e.style),
                    &e.style,
                )
            };
            let built = vertical_hug(built, e, inherited);
            let built = sticky_wrap(built, &e.style, &frame, layer_ok);
            // Таблица сжимается по содержимому (§17.5.2.2), и выражено это у
            // нас гибким рядом. В контейнере с БЛОЧНОЙ раскладкой гибкого
            // ряда нет, `align_self` мёртв, и таблица растягивалась на всю
            // ширину родителя — видно на `<span style="display:block">` с
            // табличными детьми.
            let table_child = e.tag == "table"
                || matches!(
                    e.style.display,
                    Some(Display::Table) | Some(Display::InlineTable)
                );
            let block_parent = matches!(
                inherited.display,
                Some(Display::Block) | Some(Display::ListItem) | Some(Display::TableCell)
            );
            let built = if table_child && block_parent && e.style.width.is_none() {
                div().flex().flex_row().child(built).into_any_element()
            } else {
                built
            };
            // Абсолютный блок без заданных краёв стоит на СТАТИЧЕСКОЙ позиции —
            // там, где он оказался бы в потоке, а не в углу содержащего блока.
            // Пустышка нулевой высоты держит это место в потоке, элемент висит
            // от её угла. Без неё такой блок уезжал к началу родителя и
            // накрывал собой всё, что стояло выше.
            // Только в обычном потоке: в сетке и гибком контейнере пустышка
            // стала бы ЯЧЕЙКОЙ и сдвинула соседей, а по CSS абсолютный
            // ребёнок из раскладки родителя выключен.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ — наследование,
            // шрифт, письмо и маски остаются верными, — а готовый уходит
            // последним ребёнком документа, где края решит уже вьюпорт.
            //
            // Пока только при заданных ОБЕИХ осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Внепоточный элемент, которому не нашлось позиционированного
            // предка: его содержащий блок — область просмотра (§10.1 п.4), а
            // не родитель. Элемент строится НА СВОЁМ МЕСТЕ (наследование,
            // шрифт, письмо и маски остаются верными), а готовый уходит
            // последним ребёнком документа, где края решает уже вьюпорт.
            //
            // Предок считается ВКЛЮЧАЯ непосредственного родителя:
            // `inherited.cb_ancestor` отвечает за предков строго выше него.
            // ЗАМЕРЕНО без этого слагаемого: CSS2 4636 -> 4626, все двенадцать
            // потерь — абсолют внутри `position: relative`-РОДИТЕЛЯ.
            //
            // Пока только при заданных обеих осях: при пустой оси элемент
            // стоит на статической позиции, а её знает лишь раскладка.
            // Отрицательный `z-index` рисуется ПОД потоком, слой же идёт
            // последним — такие остаются на месте.
            // Достаточно ОДНОЙ заданной оси: по ней край считает раскладка от
            // области просмотра, по пустой элемент стоит на СТАТИЧЕСКОЙ
            // позиции (§10.3.7, §10.6.4), и её сообщает щуп, оставшийся на
            // месте элемента. Ось задана, если задана хотя бы одна сторона.
            //
            // Внутри отложенного поддерева щуп готовится ПОЗЖЕ слоя, и дырка
            // была бы пуста — такие остаются на месте.
            let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
            let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
            // `fixed` считается ОТ ОКНА всегда (§10.1 п.3): позиционированный
            // предок ему не содержащий блок, и заданной оси от него не
            // требуется — незаданная сторона держит статическое место. Пока он
            // шёл общим путём, коробка висела от края родителя.
            let fixed = e.style.position == Some(crate::computed::Position::Fixed) && !under_tf;
            // `fixed` под трансформом — абсолют относительно этого предка.
            let abs_like = e.style.position == Some(crate::computed::Position::Absolute)
                || (e.style.position == Some(crate::computed::Position::Fixed) && under_tf);
            let orphan_abs = abs_like
                && !(inherited.cb_ancestor || crate::inline::establishes_cb(inherited))
                && (x_set || y_set);
            // Позиционированный предок ЕСТЬ, но это не родитель: коробку
            // забирает слой ближайшего содержащего блока (§10.1).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09, шесть заходов): пускать в слой
            // РОДИТЕЛЯ коробку, у которой родитель сам образует содержащий
            // блок (снять вето `!establishes_cb`). Замысел верный —
            // `Spot::fixed_axes` писался под смешанный случай «одна ось от
            // края, другая статическая», и без выноса такой коробке
            // статическую позицию не считает никто (`probe/svpc.html`: y = 30
            // вместо 90). Целевой срез 650 пар (305 зелёных): 258 при ЛЮБОМ
            // гейте — по позиции родителя, по флагу «слой открыт», без
            // табличных видов, только для одной оси. Приобретено 9, и это
            // ровно те пары, которые ждал прежний откат: `abspos-009`,
            // `position-absolute-007`, `right-offset-003`, `abs-pos-non-
            // replaced-vlr-087/089`, `-vrl-086/088/158/164`. Потеряно 46 —
            // `table-anonymous-objects-011..091`: родитель там обычный
            // `position: relative` div (`display: None`, слой открыт), и
            // коробка с ОДНОЙ заданной осью в его слое встаёт не туда, где
            // стояла в потоке. Значит неверно не условие входа, а сама
            // статическая позиция, которую слой считает горизонтальной
            // одноосной коробке. Возвращать вместе с проверкой щупа на
            // `table-anonymous-objects-011` (три абсолюта, у одного задан
            // лишь `top`).
            let far_abs = abs_like
                && inherited.cb_ancestor
                && !crate::inline::establishes_cb(inherited)
                && (x_set || y_set);
            let to_icb = !ordered_context
                && layer_ok
                && (fixed || orphan_abs)
                && e.style.z_index.unwrap_or(0) >= 0
                && !stays_positioned(&nodes[idx + 1..]);
            let to_cb = !to_icb
                && !ordered_context
                && layer_ok
                && far_abs
                && e.style.z_index.unwrap_or(0) >= 0
                && !stays_positioned(&nodes[idx + 1..]);
            let built = if to_icb || to_cb {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    fixed_axes: (x_set, y_set),
                    free_margin: (
                        if x_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.left, &e.style).unwrap_or(0.0)
                        },
                        if y_set {
                            0.0
                        } else {
                            margin_px(e.style.margin.top, &e.style).unwrap_or(0.0)
                        },
                    ),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    ..Default::default()
                });
                let sent = if to_icb {
                    crate::interact::icb_push(spot.clone(), built)
                } else {
                    crate::interact::cb_push(spot.clone(), built)
                };
                match sent {
                    None => {
                        // Пустая ось требует щупа: статическую позицию взять
                        // больше неоткуда. При заданных обеих осях на месте
                        // не остаётся ничего.
                        if !(x_set && y_set) {
                            out.push(crate::interact::spot_probe(spot, true));
                        }
                        continue;
                    }
                    Some(kept) => kept,
                }
            } else {
                built
            };
            // Абсолютная коробка с ОТРИЦАТЕЛЬНЫМ `z-index` не идёт ни в слой
            // ICB, ни в верхний слой: её место ПОД потоком (§9.9 шаг 3). Но
            // пустая ось у неё считается от СТАТИЧЕСКОЙ позиции, а гибкая
            // раскладка такой коробке её не даёт и ставит в начало содержимого
            // родителя. Нулевая распорка держит место в потоке, и коробка
            // висит от её угла — там, где написана.
            let below_free_axis = e.style.position == Some(crate::computed::Position::Absolute)
                && e.style.z_index.is_some_and(|z| z < 0)
                && !(x_set && y_set);
            // ЗАМЕРЕНО И ОТКАЧЕНО: уводить в верхний слой ВСЯКУЮ абсолютную
            // коробку с одной свободной осью (§9.9 шаг 8) — по симметрии с
            // `below_free_axis`. Полный свод CSS2: приобретено 3, ПОТЕРЯНО
            // 165 (вся семья `vertical-align-0NN` уходит в «красное видно»,
            // `floats-wrap-bfc-outside-001` 0.08 -> 7.28). Распорка держит
            // место свободной оси только там, где элемент и так вне строки;
            // в абзаце она рвёт строку. Возвращаться только с настоящей
            // статической позицией внутри строки.
            if !ordered_context && (at_static_position(&e.style) || below_free_axis) {
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
                    own_vertical: e.style.vertical == Some(true),
                    ..Default::default()
                });
                let probe = crate::interact::spot_probe(spot.clone(), true);
                // Поля сдвигают абсолютный элемент ОТ статической позиции
                // (CSS 2.1 §10.3.7: auto-края = static + margin). Раскладка
                // под нами поля у absolute без краёв не считает — сдвиг
                // даёт absolute-обёртка (clip-path-rectangle-ref и родня:
                // эталонный зелёный стоял без своих margin: 50px).
                let ml = margin_px(e.style.margin.left, &e.style).unwrap_or(0.0);
                let mt = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
                let built = if ml != 0.0 || mt != 0.0 {
                    div()
                        .absolute()
                        .left(px(ml))
                        .top(px(mt))
                        .child(built)
                        .into_any_element()
                } else {
                    built
                };
                let taken = if below {
                    Some(built)
                } else {
                    crate::interact::late_push(spot, built)
                };
                match taken {
                    None => out.push(probe),
                    Some(kept) => {
                        // ЗАМЕРЕНО И ОТКАЧЕНО: заворачивать `kept` в
                        // `Underlay`, чтобы коробка с отрицательным `z-index`
                        // легла ПОД поток (§9.9 шаг 3) — срез из 259 пар семей
                        // *shape*: 0 и 0, НИ ОДНО число не сдвинулось.
                        // Подложка порядок не меняет: нижним слоям сцена даёт
                        // общий номер, а сортировка устойчива. Тройку
                        // `spec-examples/shape-outside-004…006` сломал коммит
                        // `05b7a4f` (28.08, гейт `x_set && y_set` в `movable`),
                        // и возвращать её надо порядком краски, а не слоем.
                        let contiguous = below && out.len() == below_run_end;
                        out.push(
                            div()
                                .relative()
                                .w_full()
                                .h_0()
                                .flex_shrink_0()
                                .child(kept)
                                .into_any_element(),
                        );
                        if below {
                            if !contiguous {
                                below_run_start = out.len() - 1;
                                below_zs.clear();
                            }
                            below_zs.push(e.style.z_index.unwrap_or(0));
                            let mut at = below_zs.len() - 1;
                            while at > 0 && below_zs[at - 1] > below_zs[at] {
                                below_zs.swap(at - 1, at);
                                out.swap(below_run_start + at - 1, below_run_start + at);
                                at -= 1;
                            }
                            below_run_end = out.len();
                        }
                    }
                }
                continue;
            }
            let _ = hoist_margins;
            // Замещаемому дорожка по содержимому не нужна: его размер по
            // ключевому слову — природный, считается в `image_with`.
            let layered_built = layered(built, &e.style, layer_ok, under_tf);
            let mut done = if replaced_tag(e) {
                layered_built
            } else {
                content_sized(layered_built, &e.style)
            };
            // Корень vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
            // flow): свой анкор-ряд вокруг ОДНОГО узла — соседей не трогает.
            // Корню с фоном-картинкой не ставится (гасил canvas-слой).
            if matches!(e.tag.as_str(), "html" | "body") && e.style.vertical_rl == Some(true) {
                if e.style.bg_image.is_none() {
                    done = div()
                        .w_full()
                        .flex()
                        .justify_end()
                        .child(done)
                        .into_any_element();
                } else if let Some(Len::Px(w)) = e.style.width {
                    // Корню с фоном-картинкой флекс-обёртка гасила слой
                    // краски — прижим вправо считается сдвигом по известной
                    // ширине (background-size-document-root-vrl-*).
                    let shift = (opts.viewport.0 - w).max(0.0);
                    if shift > 0.0 {
                        done = div().ml(px(shift)).child(done).into_any_element();
                    }
                }
            }
            // Релятивный элемент с отрицательным `z-index`: место в потоке —
            // своё, краска — под содержимым до него (CSS 2.1 §9.9, шаг 3).
            // Расширение на элементы сетки без `position` ЗАМЕРЕНО В МИНУС
            // (display-inline-grid 0.08 -> 8.20, inline-z-axis-002/004) —
            // подложка в строчной сетке рвёт свою же краску.
            // `<body>` исключён: его родитель — корневой элемент, а тот
            // всегда образует КОРНЕВОЙ контекст наложения. Шаги 1-2
            // приложения E — собственные фон и рамка корня, шаг 3 —
            // отрицательный `z-index` ПОВЕРХ них, а не под всем окном; братьев
            // у `<body>` нет, уходить не подо что
            // (`root-element-creates-stacking-context`).
            // Родитель — СВОЙ контекст наложения (transform, opacity < 1,
            // позиционированный с z-index, isolation, filter; CSS 2.1 прил. E,
            // css-transforms-1 §transform-rendering): отрицательный z-index
            // ребёнка ложится под его содержимое, но ПОВЕРХ его фона — не под
            // весь документ (`individual-transform/stacking-context-00*`,
            // `transform-stacking-001`).
            if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
                && e.tag != "body"
                && !stacking_context(inherited)
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
fn orthogonal_children(children: Vec<Node>, container: &Computed, icb_w: f32) -> Vec<Node> {
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
        // Доступное место ортогонального потока (css-writing-modes-3
        // §7.3.1): фиксированный размер контейнера, а без него — НАЧАЛЬНЫЙ
        // содержащий блок. Процентная ширина htb-ребёнка в вертикальном
        // контейнере без размера считалась от сжатого по содержимому
        // родителя (two-levels-of-orthogonal-flows-percentage: 50% от
        // трёх букв вместо половины окна).
        if let Some(Len::Pct(k)) = ch.style.width {
            let base = match container.width {
                Some(Len::Px(w)) => w,
                _ => icb_w,
            };
            ch.style.width = Some(Len::Px(k * base));
        }
        if ch.style.width.is_none() && ch.style.max_width.is_none() {
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let margins = side(ch.style.margin.left) + side(ch.style.margin.right);
            match container.width {
                // Заданный размер контейнера — доступное место потока
                // целиком: авто-размер блочного ортогонального ребёнка
                // РАСТЯГИВАЕТСЯ на него (stretch-fit, css-sizing-3 §5), а не
                // жмётся к содержимому (two-levels-of-orthogonal-flows-fixed:
                // жёлтый ребёнок обязан накрыть красный контейнер 10em).
                Some(Len::Px(w)) => {
                    // Ширина здесь — то, что коробке отдаст раскладка, а
                    // рендер к ЗАДАННОЙ ширине добавит отступы и рамку (как
                    // общий разбор): их доля вычитается заранее, иначе
                    // ребёнок вылезал из контейнера на их толщину.
                    let b = ch.style.borders();
                    let extra = side(ch.style.padding.left)
                        + side(ch.style.padding.right)
                        + side(b.left)
                        + side(b.right);
                    ch.style.width = Some(Len::Px((w - margins - extra).max(0.0)));
                }
                _ => {
                    // css-writing-modes-4 §7.3.1: без фиксированного размера
                    // контейнера предел ортогонального потока — начальный
                    // содержащий блок за вычетом полей ребёнка (Blink
                    // `space_utils.cc` fallback = ICB). Доля от сжатого по
                    // содержимому родителя давала бесконечную строку
                    // (`sizing-orthog-htb-in-vrl-*`).
                    ch.style.max_width = Some(Len::Px((icb_w - margins).max(0.0)));
                }
            }
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
        // Элемент СЕТКИ с невытягивающим выравниванием: по строчной оси
        // (у него вертикальной) он размером в содержимое, а не в область
        // (css-grid-1 §6.6 вместе с css-align-3 §6.1 — `stretch` растягивает,
        // остальное нет). Пока вертикальный абзац брал весь предел
        // ортогонального потока, эталоны `orthogonal-positioned-grid-items-*`
        // (`place-items: start`) вылезали за сетку на всю высоту окна.
        if matches!(
            container.display,
            Some(Display::Grid) | Some(Display::InlineGrid)
        ) && matches!(
            ch.style.align_self.or(container.align_items),
            Some(Align::Start) | Some(Align::Center) | Some(Align::End) | Some(Align::Baseline)
        ) {
            ch.style.hug_inline = true;
        }
        // Корень с vertical-rl прижат к ПРАВОМУ краю окна (§8.2 principal
        // flow). Прижим самим стилем корня (align-self) — контейнеры-колонки
        // его уважают; для корня с ФОНОМ-КАРТИНКОЙ якорь ранее гасил
        // canvas-слой — тем страницам якорь не ставится (замерено).
        if matches!(ch.tag.as_str(), "html" | "body")
            && ch.style.vertical_rl == Some(true)
            && ch.style.align_self.is_none()
            && std::env::var("ANCH_BG").map_or(ch.style.bg_image.is_none(), |_| true)
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
    // ЗАМЕРЕНО, ЭФФЕКТА НЕТ (03.09): прижимать АБСОЛЮТНУЮ коробку к началу
    // ряда (`items_start`), чтобы по строчной оси вертикального письма она
    // сжималась до содержимого, а не растягивалась на содержащий блок
    // (§10.3.7; на голой пробе выходило 80x320 вместо 80x80). Срез
    // css-writing-modes: 479 -> 479, ноль и ноль — до абсолютов эта обёртка
    // не доезжает вовсе.
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

/// Образует ли коробка контекст наложения (CSS 2.1 прил. E; css-transforms-1
/// §transform-rendering; css-color-3 §3.2 opacity; css-compositing isolation).
fn stacking_context(c: &Computed) -> bool {
    c.transform.is_some()
        || c.translate.is_some()
        || c.opacity.is_some_and(|o| o < 1.0)
        || c.isolate == Some(true)
        || c.filter.is_some()
        || (c.z_index.is_some()
            && matches!(
                c.position,
                Some(crate::computed::Position::Relative)
                    | Some(crate::computed::Position::Absolute)
                    | Some(crate::computed::Position::Fixed)
                    | Some(crate::computed::Position::Sticky)
            ))
}

/// Будет ли элемент с таким стилем отложен.
fn defers(c: &Computed, under_tf: bool) -> bool {
    // `fixed` под трансформированным предком — абсолют в его блоке, а не
    // слой окна (css-transforms-1 §transform-rendering).
    (c.position == Some(crate::computed::Position::Fixed) && !under_tf)
        || c.position == Some(crate::computed::Position::Sticky)
        || c.z_index.is_some_and(|z| z > 0)
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
fn layered(el: AnyElement, c: &Computed, allowed: bool, under_tf: bool) -> AnyElement {
    let fixed_to_window = c.position == Some(crate::computed::Position::Fixed) && !under_tf;
    if !allowed {
        // Внутри отложенного поддерева `position: fixed` отсчитывается от
        // ближайшего отложенного предка, а не от окна: своей системы
        // координат ему взять неоткуда.
        if fixed_to_window {
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
    if fixed_to_window {
        let priority = c.z_index.unwrap_or(0).max(0) as usize;
        return gpui::deferred(div().absolute().top_0().left_0().size_full().child(el))
            .with_priority(priority)
            .into_any_element();
    }
    match c.z_index {
        // Отложенный слой рисуется вне масок дерева — маску обрезающего
        // предка ему передаёт пара обёрток (`interact::MaskKeep/MaskUse`).
        Some(z) if z > 0 => {
            let cell: crate::interact::MaskCell = Default::default();
            let inner = crate::interact::MaskUse { cell: cell.clone(), child: el };
            let deferred = gpui::deferred(inner)
                .with_priority(z as usize)
                .into_any_element();
            crate::interact::MaskKeep { cell, child: deferred }.into_any_element()
        }
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
/// Уходит ли элемент из потока: плавающие и внепоточные строчного не рвут
/// (Blink `layout_inline.cc`: разрыв вызывают только блоки В ПОТОКЕ).
fn out_of_flow(c: &Computed) -> bool {
    c.float.is_some_and(|f| f != 0)
        || matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
}

/// Настоящий ли это строчный элемент.
///
/// `display: inline` хранится как строчная коробка с пометкой — по одному
/// лишь тегу судить нельзя: `<div style="display:inline">` строчный, а
/// `<span style="display:block">` блочный.
fn real_inline(e: &Element) -> bool {
    // Атомарные строчные — кнопка, поле, список выбора и замещаемые — стоят
    // в строке целиком, и содержимое их не разрывает: рвутся только
    // НЕзамещаемые строчные коробки (CSS 2.1 §9.2.1.1).
    const ATOMIC: &[&str] = &[
        "button", "select", "textarea", "input", "img", "svg", "canvas", "video", "audio",
        "object", "embed", "iframe", "meter", "progress",
    ];
    if ATOMIC.contains(&e.tag.as_str()) {
        return false;
    }
    if e.style.inline_display == Some(true) {
        return true;
    }
    match e.style.display {
        Some(_) => false,
        None => e.inline || crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Блочный ли это узел с точки зрения разрыва строчного.
fn breaks_inline(n: &Node) -> bool {
    let Node::Element(e) = n else { return false };
    if out_of_flow(&e.style) || real_inline(e) {
        return false;
    }
    // Рвут строку только НАСТОЯЩИЕ блочные виды. Внутренние части таблицы
    // (ряд, ячейка, группа) сами по себе разрыва не вызывают: вокруг них
    // сборщик строит анонимную таблицу, и её судьба решается отдельно.
    match e.style.display {
        // Строчные лунки строку НЕ рвут — внешний вид у них `inline`.
        Some(Display::GridLanes) => !e.style.lanes_inline,
        Some(Display::Block)
        | Some(Display::Flex)
        | Some(Display::Grid)
        | Some(Display::Table)
        | Some(Display::ListItem) => true,
        Some(_) => false,
        None => !e.inline && !crate::dom::INLINE_TAGS.contains(&e.tag.as_str()),
    }
}

/// Есть ли в поддереве строчного блочный потомок в потоке.
///
/// `display: contents` своей коробки не даёт — блок из-под него виден
/// строчному хозяину как свой (css-display-3 §box-generation).
fn contains_block(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Element(e) if real_inline(e) || e.style.display == Some(Display::Contents) => {
            !out_of_flow(&e.style) && contains_block(&e.children)
        }
        other => breaks_inline(other),
    })
}

/// Разорвать строчные, внутри которых лежит блок (CSS 2.1 §9.2.1.1).
///
/// Строчный элемент с блочным потомком превращается в тройку «анонимный
/// блок | блок | анонимный блок»: строчное содержимое до и после блока
/// остаётся в своих анонимных коробках, сам блок встаёт между ними. Подряд
/// идущие блоки в отдельные анонимные коробки не заворачиваются.
fn split_block_in_inline(nodes: &[Node]) -> Vec<Node> {
    let need = nodes.iter().any(|n| match n {
        Node::Element(e) => real_inline(e) && !out_of_flow(&e.style) && contains_block(&e.children),
        Node::Text(_) => false,
    });
    if !need {
        return nodes.to_vec();
    }
    let mut out: Vec<Node> = vec![];
    for node in nodes {
        let Node::Element(e) = node else {
            out.push(node.clone());
            continue;
        };
        if !real_inline(e) || out_of_flow(&e.style) || !contains_block(&e.children) {
            out.push(node.clone());
            continue;
        }
        // Куски строчного содержимого копят стиль хозяина: анонимная коробка
        // своего оформления не имеет, а спан внутри неё — имеет.
        let mut piece: Vec<Node> = vec![];
        let flush = |piece: &mut Vec<Node>, out: &mut Vec<Node>| {
            // Кусок из одних схлопываемых пробелов коробки не создаёт —
            // иначе он рисовал бы фон и рамку строчного на пустом месте.
            let blank = piece.iter().all(|n| match n {
                Node::Text(t) => t.chars().all(|c| matches!(c, ' ' | '\t' | '\r' | '\n')),
                Node::Element(_) => false,
            });
            if piece.is_empty() || blank {
                piece.clear();
                return;
            }
            let mut host = e.clone();
            host.children = std::mem::take(piece);
            out.push(Node::Element(anon_element(
                "anon-block",
                vec![Node::Element(host)],
            )));
        };
        // Блок может лежать глубже, внутри вложенных строчных: сперва
        // раскрываем их, и тогда на этом уровне он виден анонимной коробкой.
        let kids = split_block_in_inline(&e.children);
        for child in &kids {
            if breaks_inline(child) {
                flush(&mut piece, &mut out);
                // Относительный сдвиг строчного хозяина переносится на
                // вынесенный блок (§9.2.1.1: разрыв не отменяет смещения).
                let mut block = match child {
                    Node::Element(c) => c.clone(),
                    Node::Text(_) => unreachable!("блоком бывает только элемент"),
                };
                // Сам блок ПОЗИЦИОНИРОВАН: его собственные края нельзя ни
                // заменить, ни сложить с чужими (у хозяина они бывают в долях,
                // у блока — в точках). Сдвиг хозяина накладывается ОБЁРТКОЙ:
                // каждый слой решает свою долю от того же содержащего блока
                // (`position-relative-001/002`), а `fixed` едет вместе со
                // своей статической позицией (`-003`).
                let host_shift = e.style.position == Some(crate::computed::Position::Relative)
                    && (e.style.inset.left.is_some() || e.style.inset.top.is_some());
                let wrap_shift = host_shift
                    && matches!(
                        block.style.position,
                        Some(crate::computed::Position::Relative)
                            | Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    );
                if host_shift && !wrap_shift {
                    block.style.position = Some(crate::computed::Position::Relative);
                    if block.style.inset.left.is_none() {
                        block.style.inset.left = e.style.inset.left;
                    }
                    if block.style.inset.top.is_none() {
                        block.style.inset.top = e.style.inset.top;
                    }
                }
                // `inherit` на размере вынесенного блока брал бы значение уже
                // не у хозяина, а у его родителя: разрыв делает блок БРАТОМ
                // хозяина. Значение забирается здесь, пока связь ещё видна.
                if block.style.width_inherit {
                    block.style.width = e.style.width;
                    block.style.width_inherit = false;
                }
                if block.style.height_inherit {
                    block.style.height = e.style.height;
                    block.style.height_inherit = false;
                }
                // Прозрачность и слой хозяина действуют на ВЕСЬ разорванный
                // элемент, включая вынесенный блок: раньше блок был куском
                // строки и получал их заодно с ней.
                if e.style.opacity.is_some() && block.style.opacity.is_none() {
                    block.style.opacity = e.style.opacity;
                }
                if e.style.z_index.is_some() && block.style.z_index.is_none() {
                    block.style.z_index = e.style.z_index;
                }
                if wrap_shift {
                    let mut shifter = anon_element("anon-relshift", vec![Node::Element(block)]);
                    shifter.style.position = Some(crate::computed::Position::Relative);
                    shifter.style.inset.left = e.style.inset.left;
                    shifter.style.inset.top = e.style.inset.top;
                    out.push(Node::Element(shifter));
                    continue;
                }
                out.push(Node::Element(block));
                continue;
            }
            piece.push(child.clone());
        }
        flush(&mut piece, &mut out);
    }
    // ПРОБОВАЛИ И ОТКАТИЛИ: сливать прогон между разрывами в ОДНУ анонимную
    // коробку (§9.2.1.1 обнимает всю строчную коробку, а не только куски
    // разорванного строчного; братья по бокам идут голыми). Проход-склейка
    // поверх `out` при условии «в прогоне есть анонимная коробка и больше
    // одного непробельного узла» на 108 парах семей `block-in-inline-*`,
    // `inline-box-001`, `abspos-029`, `text-indent-014` не сдвинул НИ ОДНОЙ:
    // `blocks()` и без склейки собирает такой прогон одним абзацем. Разница
    // эталонов лежит не в числе анонимных коробок.
    out
}

fn wrap_floats(nodes: Vec<Node>, cb_width: Option<Len>, parent_clear: Option<i8>) -> Vec<Node> {
    // `clear: inherit` — сторона родителя (`clear-005`: `clear: left` на
    // контейнере и `inherit` на ребёнке). Разрешается здесь: своего
    // наследования у ненаследуемого свойства нет, а родительский стиль есть
    // только у вызывающего.
    let nodes: Vec<Node> = nodes
        .into_iter()
        .map(|n| match n {
            Node::Element(mut e) if e.style.clear_inherit => {
                e.style.clear = parent_clear;
                Node::Element(e)
            }
            other => other,
        })
        .collect();
    let floated = nodes.iter().any(|n| match n {
        Node::Element(e) => e.style.float.is_some_and(|f| f != 0),
        Node::Text(_) => false,
    });
    if !floated {
        return nodes;
    }
    let mut nodes = nodes;
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
        // Бандовый хост: пробег флоатов ОБЕИХ сторон, не обрывающийся на
        // `clear`, и хвост, раскладываемый по полосам занятости вместо
        // флекс-ряда. Гейт узкий (см. `band_host`); не сошёлся — идём
        // сегодняшней веткой ниже, ни строки в ней не меняя.
        // Правило 6 (§9.5.1): верх флоата — верх строки, в которой он
        // объявлен. Прогон АТОМОВ известного размера перед флоатом уходит в
        // хост вместе с ним, иначе флоат встаёт ПОД прогоном (`floats-001`).
        // Прогон ТЕКСТА полосам не отдаём: наборщик строк про них не знает.
        let lead_at = out
            .iter()
            .rposition(|n| !is_blank(n) && band_piece(n) != Some(BandPiece::Atom))
            .map_or(0, |p| p + 1);
        let has_lead = out[lead_at..]
            .iter()
            .any(|n| band_piece(n) == Some(BandPiece::Atom));
        let hosted = has_lead
            .then(|| band_host(&nodes, i, cb_width, &out[lead_at..]))
            .flatten()
            .map(|(h, n)| (h, n, true))
            .or_else(|| band_host(&nodes, i, cb_width, &[]).map(|(h, n)| (h, n, false)));
        if let Some((host, next, took_lead)) = hosted {
            if took_lead {
                out.truncate(lead_at);
            }
            out.push(Node::Element(host));
            i = next;
            continue;
        }
        // Подряд идущие плавающие блоки стоят в ОДНОМ ряду, а не каждый в
        // своём: `float: left` у четырёх соседей выстраивает их бок о бок.
        // Прежде каждый начинал свой ряд, и они вставали столбиком.
        let mut floaters: Vec<Element> = vec![];
        // Сторона КАЖДОГО собранного флоата: пробег берёт обе, и левые с
        // правыми стоят в одном ряду. Прежде пробег обрывался на смене
        // стороны, `rest` выходил пустым, и одинокий флоат становился обычным
        // блоком — он съедал строку потока, а всё за ним падало на его высоту
        // (`floats-wrap-top-below-bfc-*` и родня).
        let mut sides: Vec<i8> = vec![];
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
            // `clear` у соседа обрывает ряд: он обязан начать свой. Так
            // написаны эталоны WPT — колонка из `float: right` + `clear: both`.
            if j > i && clears_side(next.style.clear, side) {
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
            sides.push(next_side);
            j += 1;
        }
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("FL_DBG").is_ok());
            *ON
        } {
            eprintln!(
                "FL floaters={} i={} j={} total={}",
                floaters.len(),
                i,
                j,
                nodes.len()
            );
        }
        // Соседи до ближайшего `clear` — они и обтекают. Внепоточный
        // (absolute/fixed) сосед НЕ обтекает: в колонке ряда он получил бы
        // её своим содержащим блоком, и `right: 96px` считался от узкой
        // колонки, а не от контейнера (эталоны css-shapes с рядом
        // absolute-коробок выходили пустыми).
        let mut rest: Vec<Node> = vec![];
        let mut out_of_flow: Vec<Node> = vec![];
        while j < nodes.len() {
            if let Node::Element(next) = &nodes[j]
                && (clears_side(next.style.clear, side) || next.style.float.is_some_and(|f| f != 0))
            {
                break;
            }
            if let Node::Element(next) = &nodes[j]
                && matches!(
                    next.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                )
                && !at_static_position(&next.style)
            {
                out_of_flow.push(nodes[j].clone());
                j += 1;
                continue;
            }
            rest.push(nodes[j].clone());
            j += 1;
        }
        // ПРОБОВАЛИ И ОТКАТИЛИ: забирать строчный прогон, стоящий ПЕРЕД
        // пробегом флоатов, обратно из `out` в хвост — §9.5.1 п.6 держит
        // верх флоата на верху текущей строчной коробки, а §9.5 сужает саму
        // эту строку. Гейт «только при иначе пустом хвосте». Проба по 319
        // парам семей `floats*`, `float-*`, `clear-float-*`: приобретено 1
        // (`floats-001` 3.84 -> 0.00), потеряно 4 — `float-nowrap-3` 0.14 ->
        // 0.50, `-7` 0.00 -> 0.40, `-9` 0.26 -> 0.67, `floats-114` 0.06 ->
        // 1.14, и `float-nowrap-hyphen-rewind-1` 0.42 -> 2.46. Прогон надо
        // не переносить целиком, а сужать по полосам — этого канала нет.
        //
        // §9.5.2: у очищающей коробки верхнее поле ЗАМЕНЯЕТСЯ зазором, а не
        // складывается с ним: её верх = max(своё место, низ флоатов). Ряд
        // обтекания сам даёт `max(флоаты, колонка)`, поэтому остаток поля
        // переносится распоркой в КОНЕЦ колонки, а у самой коробки гасится —
        // иначе она опускалась на своё поле ниже низа флоата.
        //
        // Оговорка: если брат ПЕРЕД флоатом схлопывается насквозь, его поле
        // ещё не выложено, и верх ряда у нас и так ниже настоящего — тогда
        // перенос только удваивает сдвиг (`clearance-006`).
        let laid_out = out
            .iter()
            .rev()
            .find(|n| !is_blank(n))
            .is_none_or(|n| match n {
                Node::Element(prev) => through_strut(prev).is_none(),
                Node::Text(_) => true,
            });
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): убрать подмену целиком. §9.5.2 хочет
        // `max(низ флоатов, своё место + поле)`, а распорка даёт
        // `низ флоатов + поле` и гасит поле — оно выпадает из схлопывания с
        // полем родителя и следующего брата. Но без распорки хуже: срез из 17
        // пар жилы зазора дал 0 зелёных и до, и после, а три пары просели —
        // `margin-collapse-122` 1.55 → 2.94, `-125` 1.54 → 3.82,
        // `-142` 2.47 → «красное видно». Значит распорка держит положение, и
        // чинить надо не её удаление, а канал «поле участвует в схлопывании,
        // не двигая коробку».
        if laid_out
            && let Some(Node::Element(next)) = nodes.get(j)
            && clears_side(next.style.clear, side)
            && let Some(top) = margin_px(next.style.margin.top, &next.style).filter(|v| *v > 0.0)
        {
            rest.push(Node::Element(Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "div".into(),
                style: Computed {
                    display: Some(Display::Block),
                    height: Some(Len::Px(top)),
                    ..Computed::default()
                },
                hover: None,
                first_letter: None,
                first_line: None,
                children: vec![],
                attrs: vec![],
                inline: false,
            }));
            if let Some(Node::Element(next)) = nodes.get_mut(j) {
                next.style.margin.top = Some(Len::Px(0.0));
            }
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
                // ПРОБОВАЛИ И ОТКАТИЛИ: заодно делать строчный по природе тег
                // блочным при `float: right` (обещание комментария ниже, кода
                // не было). Замерено: приобретено 4, потеряно 5 —
                // `float-nowrap-*` и `border-color-006`. Возвращать вместе с
                // §9.7 целиком.
                //
                // Обтекать нечем — но сторону блок обязан держать: `float: right`
                // без соседей всё равно стоит У ПРАВОГО края. Ряда тут нет, и
                // сторону задаёт выравнивание себя в колонке родителя. Оно
                // действует только на ЭЛЕМЕНТ раскладки, поэтому строчный по
                // природе тег (картинка) здесь же делается блочным: иначе он
                // уходит в абзац, и выравнивание достаётся абзацу, а не ему.
                // Выравнивание себя действует только на ЭЛЕМЕНТ раскладки:
                // строчный по природе тег иначе уходит в абзац, и сторона
                // достаётся абзацу, а не картинке. Гейт узкий — только
                // замещаемый тег и только когда перед ним в блоке ничего нет:
                // широкий уже мерился в минус (запись выше).
                // Доля размера у замещаемого считается от содержащего блока,
                // и блокификация его подменяет: `<iframe height="50%">` теряет
                // отсчёт (`float-replaced-height-005`). Такие остаются как есть.
                let pct_size = matches!(lone.style.width, Some(Len::Pct(_)))
                    || matches!(lone.style.height, Some(Len::Pct(_)));
                if replaced_inline(&lone.tag) && !pct_size && out.iter().all(is_blank) {
                    lone.inline = false;
                    lone.style.display = Some(Display::Block);
                }
                // Перед флоатом стоят одни АТОМЫ (замещаемые и строчные
                // блоки): они и флоат обязаны остаться в ОДНОЙ строке, а
                // сторону флоат держит сам. Выражается гибким рядом с
                // раздачей по краям — блокификация тут не годится, она
                // унесла бы флоат на свою строку
                // (`borders/border-color-001-ref`: вторая картинка обязана
                // стоять у правого края той же строки).
                // Атомом здесь считается и замещаемый тег БЕЗ заданных
                // размеров: у картинки они приходят из файла, а `band_piece`
                // требует точек.
                let atom_like = |n: &Node| match n {
                    Node::Text(_) => false,
                    Node::Element(e) => {
                        band_piece(n) == Some(BandPiece::Atom)
                            || (replaced_inline(&e.tag) && e.style.float.is_none())
                    }
                };
                let lead_at = out
                    .iter()
                    .rposition(|n| !is_blank(n) && !atom_like(n))
                    .map_or(0, |p| p + 1);
                let lead_atoms = out[lead_at..].iter().any(atom_like);
                // ЛЕВЫЙ флоат, перед которым в этом же блоке уже вышел
                // строчный прогон (§9.5 п.1 и п.6): его верх — верх ТЕКУЩЕЙ
                // строки, а сама строка вокруг него сужается, то есть на
                // экране он стоит ЛЕВЕЕ прогона, хотя в разметке идёт после.
                // Мы же дописывали его блоком следом, и полосы менялись
                // местами (`box-generation-001`: жёлтая «Float» уезжала на 70
                // точек вправо от оранжевой «Inline box»).
                let inline_run_like = |n: &Node| match n {
                    Node::Text(_) => true,
                    Node::Element(e) => e.style.float.is_none() && inline_level_box(e),
                };
                let run_at = out
                    .iter()
                    .rposition(|n| !is_blank(n) && !inline_run_like(n))
                    .map_or(0, |p| p + 1);
                if side < 0 && !lead_atoms && run_at < out.len() {
                    let row: Vec<Node> = out.split_off(run_at);
                    let mut children = vec![Node::Element(lone)];
                    children.extend(row);
                    out.push(Node::Element(Element {
                        list_item: None,
                        node_id: 0,
                        anim: None,
                        tag: "float-row".into(),
                        style: Computed {
                            display: Some(Display::Flex),
                            ..Computed::default()
                        },
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children,
                        attrs: vec![],
                        inline: false,
                    }));
                    out.extend(rest);
                    out.extend(out_of_flow);
                    i = j;
                    continue;
                }
                if lead_atoms && side > 0 {
                    let mut row: Vec<Node> = out.split_off(lead_at);
                    lone.style.margin.left = Some(Len::Auto);
                    row.push(Node::Element(lone));
                    out.push(Node::Element(Element {
                        list_item: None,
                        node_id: 0,
                        anim: None,
                        tag: "float-row".into(),
                        style: Computed {
                            display: Some(Display::Flex),
                            ..Computed::default()
                        },
                        hover: None,
                        first_letter: None,
                        first_line: None,
                        children: row,
                        attrs: vec![],
                        inline: false,
                    }));
                    out.extend(rest);
                    out.extend(out_of_flow);
                    i = j;
                    continue;
                }
                lone.style.align_self = Some(if side < 0 { Align::Start } else { Align::End });
                out.push(Node::Element(lone));
            }
            out.extend(rest);
            out.extend(out_of_flow);
            i = j;
            continue;
        }
        // ПРОБОВАЛИ И ОТКАТИЛИ: пробег РАВНОШИРОКИХ флоатов без заданной
        // высоты раскладывать колонкой флекс-рядов по `floor(cb / mw)` штук в
        // ряд (§9.5.1 п.3 и п.5) — высоты для этого знать не нужно. Замерено
        // по всему CSS2: 0 и 0. Пары `c414-flt-fit-002/003/004` держит не
        // раскладка рядов, а что-то ещё.
        //
        // Обтекание ФОРМОЙ (`shape-outside`): ряд-колонка его не выразит —
        // строки должны сужаться каждая по-своему. Плавающие блоки с
        // ИЗВЕСТНЫМИ размерами уходят синтетическим узлом shape-flow:
        // сборка положит их absolute и передаст вырезы абзацу.
        let px_of = |l: &Option<Len>| match l {
            None => Some(0.0),
            Some(Len::Px(v)) => Some(*v),
            _ => None,
        };
        let sized = |e: &Element| -> Option<(f32, f32)> {
            let b = e.style.borders();
            Some((
                px_of(&e.style.width)?
                    + px_of(&e.style.padding.left)?
                    + px_of(&e.style.padding.right)?
                    + px_of(&b.left)?
                    + px_of(&b.right)?
                    + px_of(&e.style.margin.left)?
                    + px_of(&e.style.margin.right)?,
                px_of(&e.style.height)?
                    + px_of(&e.style.padding.top)?
                    + px_of(&e.style.padding.bottom)?
                    + px_of(&b.top)?
                    + px_of(&b.bottom)?
                    + px_of(&e.style.margin.top)?
                    + px_of(&e.style.margin.bottom)?,
            ))
        };
        let img_float = |f: &Element| {
            f.style
                .shape_outside
                .as_deref()
                .is_some_and(|r| r.contains("url("))
                && (f.tag == "img"
                    || f.children
                        .iter()
                        .any(|n| matches!(n, Node::Element(c) if c.tag == "img")))
        };
        // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда пробег из флоатов ОБЕИХ сторон
        // (снять этот конъюнкт и вернуть сторону детям перед пушем). Срез из
        // 259 пар семей *shape*: 0 и 0 — тройка `spec-examples/shape-outside-
        // 001…003` как была «красное видно», так и осталась, её держит не
        // односторонность пробега.
        if sides.iter().all(|s| *s == side)
            && floaters.iter().any(|f| f.style.shape_outside.is_some())
            && floaters.iter().all(|f| sized(f).is_some() || img_float(f))
        {
            let mut host = Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "shape-flow".into(),
                style: Computed {
                    // Ширина содержащего блока — для долей формы и поля.
                    width: cb_width,
                    ..Computed::default()
                },
                hover: None,
                first_letter: None,
                first_line: None,
                children: Vec::new(),
                // Подготовка выше сняла float с самих блоков — сторона и
                // число уезжают атрибутами.
                attrs: vec![
                    (
                        "side".into(),
                        if side < 0 {
                            "left".into()
                        } else {
                            "right".into()
                        },
                    ),
                    ("count".into(), floaters.len().to_string()),
                ],
                inline: false,
            };
            host.children = floaters.into_iter().map(Node::Element).collect();
            host.children.extend(rest);
            out.push(Node::Element(host));
            out.extend(out_of_flow);
            i = j;
            continue;
        }
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): §9.5 говорит, что коробка блочного
        // уровня флоат НЕ обходит — обходят только её строки. Пробовал
        // выразить это наложением: когда в хвосте нет ни одного текстового
        // узла, блоки идут своим чередом, а флоат кладётся поверх них
        // абсолютом в относительной обёртке. Срез из 299 пар флоатов:
        // 198 → 189 без гейта и 198 → 193 с гейтом «хвост не образует
        // своего контекста». Приобретение одно (`floats-rule3-outside-
        // right-001` 1.84 → 0.00), потери — `floats-rule7-outside-left-001`
        // 0.00 → 1.79, `floats-wrap-bfc-001/003-*-table` 0.00 → 2-7,
        // `floats-wrap-bfc-with-margin-008/009` 0.00 → 1.05. Наложение
        // рушит вертикальное место: у нас флоат в ряду задаёт высоту, а
        // абсолют её больше не держит.
        // Хвост со СВОИМИ боковыми полями остаётся на прежней основе: остаток
        // ряда достаётся ему без учёта этих полей, и коробка выходит у́же
        // нужного (`floats-wrap-bfc-with-margin-004/005/008/009`).
        let хвост_с_полями = rest.iter().any(|n| match n {
            Node::Element(e) => [e.style.margin.left, e.style.margin.right]
                .iter()
                .any(|m| !matches!(m, None | Some(Len::Px(0.0)))),
            Node::Text(_) => false,
        });
        let mut column = Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: "div".into(),
            style: Computed {
                flex_grow: Some(1.0),
                // ЗАМЕРЕНО: CSS2 5336 → 5338 (+10/−8), CSS3 2418 → 2417
                // (+1/−2), итого +1. Приобретения — обтекание текстом
                // (`floats-rule3-outside-right-001` 1.84 → 0.00,
                // `floats-wrap-bfc-002/003-*-overflow` 5-8 → 0.00, четвёрка
                // `float-nowrap-*`). Потери — БФК со СВОИМИ полями рядом с
                // флоатом (`floats-wrap-bfc-with-margin-004/005/008/009`,
                // `floats-132`, `floats-rule7-outside-left-001`): им остаток
                // ряда достаётся без учёта их полей. Чинится каналом «поле
                // БФК входит в остаток», которого в ряду нет.
                // Колонка обтекания берёт ОСТАТОК ряда, а не своё содержимое:
                // при основе «по содержимому» её max-content складывался с
                // шириной флоата, ряд переносился, и `float: right` уезжал
                // ПОД текст к левому краю вместо правого края той же строки
                // (проба: `float:right` 60 точек и три слова в двухстах).
                flex_basis: (!хвост_с_полями).then_some(Len::Px(0.0)),
                flex_shrink: Some(1.0),
                min_width: (!хвост_с_полями).then_some(Len::Px(0.0)),
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
        let paired: Vec<(i8, Element)> = sides.iter().copied().zip(floaters).collect();
        row_children.extend(
            paired
                .iter()
                .filter(|(s, _)| *s < 0)
                .map(|(_, f)| Node::Element(f.clone())),
        );
        row_children.push(Node::Element(column));
        // Прижатые вправо идут справа налево в порядке разметки.
        row_children.extend(
            paired
                .iter()
                .rev()
                .filter(|(s, _)| *s > 0)
                .map(|(_, f)| Node::Element(f.clone())),
        );
        out.push(Node::Element(Element {
            list_item: None,
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
        out.extend(out_of_flow);
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
/// Роль соседа плавающих блоков в бандовом хосте.
#[derive(Clone, Copy, Debug, PartialEq)]
enum BandPiece {
    /// Инлайн-блок с известным margin-box — строчный поток атомов
    /// (`FlowRow`): такие коробки стоят В СТРОКУ и делят её.
    Atom,
    /// Коробка со СВОИМ контекстом форматирования: её border-box не
    /// перекрывает флоаты вовсе (§9.5, последний абзац) — она ищет окно и
    /// съезжает вниз.
    Bfc,
    /// ПРОБОВАЛИ И ОТКАТИЛИ: вариант `BfcAuto` — коробка со своим контекстом
    /// и БЕЗ заданной ширины, которой ширину назначает само размещение
    /// (§10.3.3 плюс §9.5). Замерено по семьям *float*, *clear*, *shape*:
    /// 0 и 0 — обе ожидавшиеся пары (`floats-wrap-bfc-with-margin-006/007`)
    /// держит не ширина.
    ///
    /// Пустая распорка без ширины: обычный блок в потоке. Флоаты она
    /// перекрывает (обтекают только СТРОКИ), а показать ей нечего — от неё
    /// нужна одна высота. Держит эталоны `floats-wrap-top-below-002*-ref`.
    Strut,
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (03.09): вариант `Flow` — ОБЫЧНЫЙ блочный бокс в
    // потоке. Перечисление §9.5 закрытое (таблица, блочный замещаемый,
    // коробка своего контекста), значит обычный блок флоат обязан
    // ПЕРЕКРЫВАТЬ, а не отодвигаться от него; сегодня такой хвост уводит
    // пробег на флекс-ряд, который наложение выразить не умеет.
    //
    // Написано и работало: кусок с известной высотой (пустой коробке
    // `height: auto` даёт ноль по §10.6.3 — отдельная `px_margin_box_float`),
    // без требования ширины (она равна ширине содержащего блока), встающий
    // на потолок потока в инлайн-начало БЕЗ `place_among`; порядок краски
    // Приложения E соблюдён — куски потока уходят в хост ПЕРЕД флоатами,
    // коробки своего контекста после них.
    //
    // Срез флоатов (1685 пар, 1539 зелёных): 1536, потом 1538 после снятия
    // полей и со СЛИТОГО стиля (без этого коробка сдвигалась на поле
    // дважды, `floats-015` 0.00 -> 0.73). Итог: приобретено 2
    // (`floats-rule3-outside-right-001` 1.84 -> 0.11,
    // `block-formatting-contexts-016` «красное видно» -> 0.00), потеряно 3
    // (`floats-rule3-outside-right-002` 0.33 -> 2.06,
    // `floats-rule7-outside-left-001` 0.00 -> 1.79,
    // `adjoining-float-nested-forced-clearance-003` 0.00 -> «красное видно»).
    //
    // Зеркальность приобретения и потери у `rule3-outside-right-001/002`
    // говорит, что кусок потока встаёт по правильной оси, но правила 3 и 7
    // §9.5.1 считают ПОЛОЖЕНИЕ ФЛОАТА относительно предыдущих коробок, а
    // полосы этого не знают. Возвращать вместе с правилами 3 и 7 в
    // `bands.rs` и с клиренсом как величиной в потоке (шаги B-D и F6 из
    // `target/scout-floats-cluster-2026-09.md`). Патч целиком —
    // `target/a2-bands.patch`.
}

// Точки поля: `auto` считается нулём.
//
// Отдельно от `px_of2`, потому что `margin: auto` у коробки С ЗАДАННОЙ
// шириной поле не растит, а лишь выбирает, к какому краю прижаться
// (§10.3.3); занятость от него не меняется. Без этого гейт бандового хоста
/// не сработал бы вовсе: `floats-wrap-top-below-bfc-001l` — `margin-right:
/// auto`, `-001r` — `margin-left: auto`.
fn px_margin(l: &Option<Len>) -> Option<f32> {
    match l {
        Some(Len::Auto) => Some(0.0),
        other => px_of2(other),
    }
}

/// margin-box коробки в точках, когда ВСЕ стороны заданы точками.
///
/// Полосы занятости меряют только числа (`bands.rs`), и брать их можно лишь
/// у коробки, чей размер известен из стиля целиком.
fn px_margin_box(c: &Computed) -> Option<(f32, f32)> {
    let b = c.borders();
    Some((
        px_of2(&c.width)?
            + px_of2(&c.padding.left)?
            + px_of2(&c.padding.right)?
            + px_of2(&b.left)?
            + px_of2(&b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
        px_of2(&c.height)?
            + px_of2(&c.padding.top)?
            + px_of2(&c.padding.bottom)?
            + px_of2(&b.top)?
            + px_of2(&b.bottom)?
            + px_margin(&c.margin.top)?
            + px_margin(&c.margin.bottom)?,
    ))
}

/// Чем сосед флоатов может быть в бандовом хосте; `None` — не может ничем,
/// и весь хост отменяется.
///
/// Порядок проверок важен: `own_context` истинен и для `inline-block`
/// (`:3087`), а строчную коробку блочной веткой ставить нельзя — она встанет
/// на свою строку вместо общей.
fn band_piece(n: &Node) -> Option<BandPiece> {
    let Node::Element(c) = n else {
        // Непустой текст рядом с флоатом бандовый хост не набирает: это
        // работа наборщика строк, а он про полосы ещё не знает.
        return None;
    };
    // Внепоточный сосед получил бы хост своим содержащим блоком; `clear`
    // требует зазора, которого шаг B1 не считает.
    if matches!(
        c.style.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) || c.style.float.is_some_and(|f| f != 0)
        || c.style.clear.is_some()
    {
        return None;
    }
    let (mw, mh) = px_margin_box(&c.style)?;
    // ОБЕ стороны обязаны стоять в стиле точками. Одного `px_margin_box` мало:
    // `width: auto` там `None` и складывается как НОЛЬ, а по §10.3.5 это
    // shrink-to-fit, которого полосы не считают. Без этой проверки коробка с
    // одними полями прошла бы гейт с нулевой шириной.
    let sized = matches!(c.style.height, Some(Len::Px(_)))
        && matches!(c.style.width, Some(Len::Px(_)))
        && mw > 0.0
        && mh > 0.0;
    if matches!(
        c.style.display,
        Some(Display::InlineBlock) | Some(Display::InlineFlex)
    ) {
        return sized.then_some(BandPiece::Atom);
    }
    if own_context(c) {
        return sized.then_some(BandPiece::Bfc);
    }
    // Распорка: своей ширины нет, внутри пусто, краски нет — видно её нечем,
    // и место она занимает только по высоте.
    let invisible = c.style.background.is_none()
        && c.children.iter().all(is_blank)
        && matches!(c.style.width, None | Some(Len::Auto))
        && matches!(c.style.height, Some(Len::Px(_)))
        && mw == 0.0;
    (invisible && mh > 0.0).then_some(BandPiece::Strut)
}

/// Синтетический узел бандового хоста для пробега флоатов, начинающегося на
/// `i`; вернуть хост и номер узла ЗА хвостом.
///
/// Отличий от сегодняшнего пробега (`wrap_floats:1985-2038`) три:
///
/// 1. пробег НЕ обрывается ни на смене стороны, ни на `clear` — обе стороны
///    и очистка уезжают в один хост;
/// 2. `style.float` и `style.clear` с детей НЕ снимаются: их читает
///    `shape_flow`, который гасит их сам перед сборкой (`:5091`);
/// 3. флоатам не задаются ни `flex_shrink`, ни `fit-content` — ряда, ради
///    которого это делалось, здесь нет, а размер и так обязан быть в точках.
///
/// Гейт (все условия разом, иначе `None`):
///
/// * ширина содержащего блока известна ТОЧКАМИ и больше нуля — без неё
///   полосам негде поставить дальнюю стенку (`shape_flow` ставит
///   недостижимую `NO_WALL`, и сужения не будет вовсе);
/// * у каждого флоата пробега margin-box в точках;
/// * хвост непустой и состоит ТОЛЬКО из пустого текста и кусков `BandPiece`.
///
/// Последнее условие и держит радиус поражения: любой абзац, любой блок без
/// размеров, любой текст рядом с флоатом уводит на сегодняшний флекс-ряд.
fn band_host(
    nodes: &[Node],
    i: usize,
    cb_width: Option<Len>,
    lead: &[Node],
) -> Option<(Element, usize)> {
    let cb_w = match cb_width {
        Some(Len::Px(v)) if v > 0.0 => v,
        _ => return None,
    };
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
        if !next.style.float.is_some_and(|f| f != 0) {
            break;
        }
        // Размер флоата обязан быть в точках: полосы ничего не мерят, а
        // `width: auto` у флоата — это shrink-to-fit (§10.3.5), который
        // `px_margin_box` сложил бы как НОЛЬ и посадил флоат нулевой ширины.
        if !matches!(next.style.width, Some(Len::Px(_)))
            || !matches!(next.style.height, Some(Len::Px(_)))
        {
            return None;
        }
        px_margin_box(&next.style)?;
        floaters.push(next.clone());
        j += 1;
    }
    if floaters.is_empty() {
        return None;
    }
    let mut rest: Vec<Node> = vec![];
    while j < nodes.len() {
        if let Node::Element(next) = &nodes[j]
            && (next.style.float.is_some_and(|f| f != 0) || next.style.clear.is_some())
        {
            break;
        }
        rest.push(nodes[j].clone());
        j += 1;
    }
    if !lead.is_empty() {
        // Прогон отдаём полосам ЦЕЛИКОМ и только когда он вместе с флоатами
        // помещается в одну строку: разъехавшийся прогон — это перенос, а
        // его считает наборщик строк, не полосы.
        if !lead
            .iter()
            .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom))
        {
            return None;
        }
        let mut row = 0.0f32;
        for n in lead {
            if let Node::Element(e) = n {
                row += px_margin_box(&e.style)?.0;
            }
        }
        for f in &floaters {
            row += px_margin_box(&f.style)?.0;
        }
        if row > cb_w + 0.01 {
            return None;
        }
        let mut all = lead.to_vec();
        all.extend(rest);
        rest = all;
    }
    // Пустой хвост хост НЕ отменяет, если флоатов НЕСКОЛЬКО: лесенку
    // (§9.5.1 п.5) и правило 3 флекс-ряд не выражает вовсе. Одинокий флоат с
    // пустым хвостом полосам не нужен — его кладёт ветка ниже, и хост ей
    // только мешал (замерено: с пустым хвостом при любом числе флоатов
    // приобретено 10, потеряно 9).
    // ЗАМЕРЕНО: отсекать здесь ещё и пробеги с `clear` — 5065 -> 5064.
    if (floaters.len() < 2 && !rest.iter().any(|n| !is_blank(n)))
        || !rest.iter().all(|n| is_blank(n) || band_piece(n).is_some())
    {
        return None;
    }
    let side = floaters[0].style.float.unwrap_or(-1);
    let mut host = Element {
        list_item: None,
        node_id: 0,
        anim: None,
        tag: "shape-flow".into(),
        style: Computed {
            // Ширина содержащего блока — дальняя стенка полос.
            width: cb_width,
            ..Computed::default()
        },
        hover: None,
        first_letter: None,
        first_line: None,
        children: Vec::new(),
        attrs: vec![
            // Сторона первого флоата — только запасной ответ для живого пути
            // `shape-outside`: у бандового хоста сторона лежит на КАЖДОМ
            // ребёнке, и `shape_flow` читает её оттуда.
            (
                "side".into(),
                if side < 0 {
                    "left".into()
                } else {
                    "right".into()
                },
            ),
            ("count".into(), floaters.len().to_string()),
            // Метка бандового хоста: включает блочную ветку `shape_flow`.
            ("bands".into(), "1".into()),
        ],
        inline: false,
    };
    host.children = floaters.into_iter().map(Node::Element).collect();
    host.children.extend(rest);
    Some((host, j))
}

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
    count: Option<usize>,
    col_w: Option<f32>,
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
        return column_flow(only, &inside, opts, count, col_w);
    }
    // `<br>` — жёсткий разрыв: в собранном тексте он помечается U+2028,
    // замер режет по нему принудительно. В сырых узлах <br> текста не несёт,
    // поэтому маркер в счёт сырых байт не входит.
    fn gather_cols(nodes: &[Node], out: &mut String) {
        for n in nodes {
            match n {
                Node::Text(t) => out.push_str(t),
                Node::Element(e) if e.tag == "br" => out.push('\u{2028}'),
                Node::Element(e) => gather_cols(&e.children, out),
            }
        }
    }
    let mut raw_plain = String::new();
    gather_cols(&e.children, &mut raw_plain);
    // Разрезы приходят в байтах НОРМАЛИЗОВАННОЙ строки (по ней меряет
    // line_wrapper), а split_nodes режет сырые узлы по сырым байтам —
    // без обратного маппинга разрез уезжает на длину схлопнутых пробелов.
    // Пробелы вокруг жёсткого разрыва схлопываются в него (css-text §4.1.2).
    let (normalized, norm_to_raw): (String, Vec<(usize, usize)>) = {
        let mut out = String::new();
        let mut map = Vec::new();
        let mut nodes_off = 0usize;
        let mut prev_space = false;
        for ch in raw_plain.chars() {
            let is_space = matches!(ch, ' ' | '\t' | '\n' | '\r');
            if ch == '\u{2028}' {
                if prev_space && out.ends_with(' ') {
                    out.pop();
                    map.pop();
                }
                map.push((out.len(), nodes_off));
                out.push('\n');
                prev_space = true;
                continue;
            }
            if is_space {
                if !prev_space {
                    map.push((out.len(), nodes_off));
                    out.push(' ');
                }
            } else {
                map.push((out.len(), nodes_off));
                out.push(ch);
            }
            prev_space = is_space;
            nodes_off += ch.len_utf8();
        }
        (out, map)
    };
    let plain = normalized.trim().to_string();
    if plain.trim_matches('\n').is_empty() {
        return None;
    }
    let lead = normalized.len() - normalized.trim_start().len();
    let raw_len = raw_plain
        .chars()
        .filter(|c| *c != '\u{2028}')
        .map(|c| c.len_utf8())
        .sum::<usize>();
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
    // Линейка колонок: видима при заданном стиле; цвет — currentColor.
    let rule_px = |w: &Option<Len>, size: f32| match w {
        Some(Len::Px(v)) => *v,
        Some(Len::Em(k)) => k * size,
        _ => 3.0,
    };
    let rule_owned: Option<(f32, crate::value::Color)> =
        if e.style.column_rule_visible == Some(true) {
            Some((
                rule_px(&e.style.column_rule_width, size),
                e.style
                    .column_rule_color
                    .or(inherited.color)
                    .unwrap_or(crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    }),
            ))
        } else {
            None
        };
    let nodes = e.children.clone();
    let inherited_owned = inherited.clone();
    let opts_owned = opts.clone();
    let depth = defer_depth();
    let build: std::rc::Rc<dyn Fn(&[usize], usize, gpui::Pixels) -> AnyElement> =
        std::rc::Rc::new(move |cuts: &[usize], used: usize, width: gpui::Pixels| {
            let _depth = DepthScope::enter(depth);
            // Куски текста по местам разрезов: каждый — своя колонка.
            let mut parts: Vec<Vec<Node>> = vec![];
            let mut rest = nodes.clone();
            let mut base = 0usize;
            for cut in cuts {
                let full = cut + lead;
                let raw_cut = match norm_to_raw.binary_search_by_key(&full, |p| p.0) {
                    Ok(i) => norm_to_raw[i].1,
                    Err(i) => norm_to_raw.get(i).map(|p| p.1).unwrap_or(raw_len),
                };
                let (head, tail) = split_nodes(&rest, raw_cut.saturating_sub(base));
                parts.push(head);
                base = raw_cut;
                rest = tail;
            }
            parts.push(rest);
            // Разрез по жёсткому разрыву оставляет сам <br> в начале хвоста
            // (нулевая длина ставит его после разреза) — новая колонка
            // начиналась бы с пустой строки.
            for part in parts.iter_mut().skip(1) {
                while let Some(first) = part.first() {
                    match first {
                        Node::Element(e) if e.tag == "br" => {
                            part.remove(0);
                        }
                        Node::Text(t) if t.trim().is_empty() => {
                            part.remove(0);
                        }
                        _ => break,
                    }
                }
            }
            // Ширина колонки и линейки — от used count: контента может быть
            // меньше, чем колонок (rule-001: две колонки, две строки).
            let n_cols = used.max(cuts.len() + 1);
            let inner = (f32::from(width) - gap * (n_cols - 1) as f32) / n_cols as f32;
            // Линейка между колонками (`column-rule`, css-multicol §4):
            // абсолютный держатель по центру промежутка на всю высоту ряда —
            // линейка шире промежутка накрывает соседние колонки (rule-001),
            // а при недозаполненных колонках всё равно тянется на всю их
            // высоту (rule-004). Рисуется ДО колонок: под контентом.
            let rule = rule_owned.filter(|(w, _)| *w > 0.0);
            let mut row = div().flex().flex_row().w(width).gap_x(px(gap)).relative();
            if let Some((rw, color)) = rule {
                for i in 0..n_cols.saturating_sub(1) {
                    let center = inner * (i as f32 + 1.0) + gap * i as f32 + gap / 2.0;
                    row = row.child(
                        div()
                            .absolute()
                            .left(px(center - rw / 2.0))
                            .top_0()
                            .bottom_0()
                            .w(px(rw))
                            .bg(color.to_hsla()),
                    );
                }
            }
            for part in parts.into_iter() {
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
            col_w,
            gap,
            measure_font(inherited, opts),
            size,
            line,
            // `column-fill: auto` с заданной высотой: колонки заполняются
            // подряд до неё (css-multicol-1 §3.3).
            match (e.style.column_fill_auto, e.style.height) {
                (Some(true), Some(Len::Px(h))) if h > 0.0 => Some(h),
                _ => None,
            },
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
    // Разрез обтекания понимает РОВНО пару «плавающий блок + колонка
    // текста». Ряд из НЕСКОЛЬКИХ плавающих (четыре float:left подряд) обязан
    // остаться обычным флекс-рядом: прежде сюда попадали первые два, а
    // остальные ВЫБРАСЫВАЛИСЬ (flex-flow-001-ref: из «1 2 3 4» рисовались
    // «1 2» — 23 красных flexbox-ref'а с float).
    if row.children.iter().filter(|n| !is_blank(n)).count() != 2 {
        return plain_row(&row.children);
    }
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
            list_item: None,
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
/// Многоколоночный контейнер: `column-*` применяются только к блочным
/// контейнерам (css-multicol-1 §2), сетка и гибкий контейнер ими не
/// становятся (`grid-multicol-001`,
/// `column-property-should-not-apply-on-grid-container-001`).
fn multicol_container(c: &Computed) -> bool {
    (c.column_count.is_some() || c.column_width.is_some())
        && !matches!(
            c.display,
            Some(Display::Grid)
                | Some(Display::InlineGrid)
                | Some(Display::GridLanes)
                | Some(Display::Flex)
                | Some(Display::InlineFlex)
        )
}

fn is_blank(n: &Node) -> bool {
    matches!(n, Node::Text(t) if blank_text(t))
}

/// Порядок наложения внутри одного родителя.
///
/// Отрицательный `z-index` кладёт элемент ПОД поток: отложенной отрисовкой это
/// не выражается — она всегда рисует поверх. Зато порядок детей мы задаём
/// сами: такие элементы уходят в начало списка и рисуются раньше.
/// `flex_ctx` — дети гибкого контейнера: у элемента ряда `z-index` кроме
/// auto создаёт контекст наложения и без `position` (css-flexbox-1 §4.3:
/// «z-index values other than auto create a stacking context even if
/// position is static»; `flex-item-z-ordering-001/002`).
fn by_layer(mut nodes: Vec<Node>, flex_ctx: bool) -> Vec<Node> {
    // Элемент на статической позиции переставлять НЕЛЬЗЯ: место в потоке и
    // есть его координата. `z-index` меняет только порядок отрисовки, а
    // перестановка меняла и раскладку — абсолютный блок с `z-index: -1`
    // уезжал к началу родителя и накрывал собой абзац над собой.
    // Двигать можно только ВНЕПОТОЧНЫЙ элемент: у релятивного слот в потоке и
    // есть его координата, перестановка меняла раскладку всего родителя
    // (красная полоса `overlapped-red` уезжала в начало страницы). Релятивный
    // с отрицательным `z-index` остаётся на месте и рисуется подложкой.
    // Переставлять можно только коробку, чьё положение задано краями ПО ОБЕИМ
    // осям: статической позиции у неё нет вовсе, и место в списке детей на
    // раскладку не влияет. С пустой осью место по ней держит нулевая распорка,
    // стоящая там, где элемент написан, — перестановка увозила бы её к началу
    // родителя.
    let movable = |e: &Element| {
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        // Элемент сетки с ЯВНЫМИ дорожками по обеим осям место в списке детей
        // тоже не держит: его позицию задаёт размещение, а не порядок
        // (css-grid-2 §8). Значит, его можно переставить ради порядка краски
        // (§9.9 шаг 3), как и внепоточную коробку с заданными краями.
        let placed = e.style.grid_col.is_some() && e.style.grid_row.is_some();
        e.style.z_index.is_some_and(|z| z < 0)
            && (placed
                || flex_ctx
                || (matches!(
                    e.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                ) && x_set
                    && y_set))
    };
    // §9.9 шаг 8, обратная сторона того же правила: позиционированная коробка
    // без отрицательного `z-index` рисуется ПОВЕРХ содержимого потока. Порядок
    // краски у нас — порядок детей, поэтому написанный ПЕРВЫМ абсолют
    // закрашивался следующим за ним братом (проба: белый квадрат
    // `right-offset-003` пропадал под синим блоком, хотя стоял верно).
    // Условие то же, что у подслоя: края заданы по ОБЕИМ осям, значит места в
    // потоке коробка не держит и перестановка меняет только краску.
    let over = |e: &Element| {
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        (matches!(
            e.style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) && x_set
            && y_set
            && !e.style.z_index.is_some_and(|z| z < 0))
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): элемент ряда с z-index > 0 —
        // поверх соседей той же перестановкой: css-flexbox 734 -> 733
        // (`flexbox-items-as-stacking-contexts-001` 0.44 -> 0.60), плюсов
        // ноль. Подслой для z < 0 (`movable` выше) оставлен: +1.
    };
    // Переставлять можно только ЧЕРЕЗ ПОТОК: порядок между позиционированными
    // соседями — это их порядок в разметке (§9.9 шаг 8 сохраняет его), и
    // прыжок через такого соседа менял бы наложение (`abspos-013`: красный
    // `fixed` с краями обязан лежать ПОД зелёным `fixed` без краёв).
    // Позиционированным считается и ПОДДЕРЕВО: краску поверх даёт не сам
    // сосед, а его потомок (`position-relative-table-tbody-top`: зелёный
    // `tbody` лежит внутри непозиционированной таблицы, и прыжок абсолюта
    // через неё открывал красный индикатор).
    fn positioned_inside(n: &Node, depth: usize) -> bool {
        match n {
            Node::Element(e) => {
                e.style.position.is_some()
                    || (depth < 8 && e.children.iter().any(|c| positioned_inside(c, depth + 1)))
            }
            Node::Text(_) => false,
        }
    }
    let mut after_positioned = vec![false; nodes.len()];
    let mut seen = false;
    for (i, n) in nodes.iter().enumerate().rev() {
        after_positioned[i] = seen;
        if positioned_inside(n, 0) {
            seen = true;
        }
    }
    // Внутри таблицы порядок детей — это её СТРОЕНИЕ (ряды, группы, ячейки), и
    // перестановка ломает саму решётку, а не краску.
    let table_here = nodes.iter().any(|n| match n {
        Node::Element(e) => {
            e.style.row_group_kind.is_some()
                || e.style.col_role.is_some()
                || e.style.is_caption == Some(true)
                || matches!(
                    e.style.display,
                    Some(Display::TableRow)
                        | Some(Display::TableCell)
                        | Some(Display::TableRowGroup)
                        | Some(Display::Table)
                )
                || matches!(
                    e.tag.as_str(),
                    "tr" | "td" | "th" | "tbody" | "thead" | "tfoot"
                )
        }
        Node::Text(_) => false,
    });
    let over_at = |i: usize, n: &Node| match n {
        Node::Element(e) => over(e) && !after_positioned[i] && !table_here,
        Node::Text(_) => false,
    };
    let touched = nodes.iter().enumerate().any(|(i, n)| {
        over_at(i, n)
            || match n {
                Node::Element(e) => movable(e),
                Node::Text(_) => false,
            }
    });
    if !touched {
        return nodes;
    }
    let keys: Vec<i32> = nodes
        .iter()
        .enumerate()
        .map(|(i, n)| match n {
            Node::Element(e) if movable(e) => e.style.z_index.unwrap_or(0).min(0),
            _ if over_at(i, n) => 1,
            _ => 0,
        })
        .collect();
    let mut order: Vec<usize> = (0..nodes.len()).collect();
    order.sort_by_key(|i| keys[*i]);
    let mut taken: Vec<Option<Node>> = nodes.into_iter().map(Some).collect();
    order.into_iter().filter_map(|i| taken[i].take()).collect()
}

/// Схлопывание вертикальных отступов соседних блоков.
///
/// В CSS нижний отступ одного блока и верхний отступ следующего не
/// складываются, а сливаются в больший из двух. Движок раскладки под нами
/// складывает их, и документ становится длиннее браузерного — расхождение
/// накапливается сверху вниз и было поймано сравнением с Chrome.
/// `abs_parent` — родитель абсолютно позиционирован: по §10.6.7 его
/// автовысота ВКЛЮЧАЕТ плавающих детей, и правило «блок из одних флоатов
/// высотой ноль» (§10.6.3) к его детям не применяется.
fn collapse_margins(nodes: &[Node], abs_parent: bool) -> Vec<Node> {
    let mut out: Vec<Node> = nodes.to_vec();
    // §10.6.3: в высоту `auto` входят только дети В ПОТОКЕ — «floating boxes
    // are ignored». Блок, у которого в потоке нет ничего, кроме плавающих
    // детей, высотой НОЛЬ и схлопывается насквозь. Наша раскладка ставит
    // плавающий блок обычным ребёнком, и родитель набирал его высоту.
    //
    // Гейт — сам `through_strut`: он уже требует нулевых рамок, отступов и
    // `min-height`, высоты `auto`, отсутствия строчной коробки и своего
    // контекста форматирования. Коробка со СВОИМ контекстом плавающего ребёнка
    // содержит и высоту от него получает по праву — её ветка не трогает.
    //
    // Замерено: CSS2 5078 -> 5082 (+5, потеряна одна —
    // `block-formatting-context-height-002`: там нулевая коробка лежит внутри
    // АБСОЛЮТНОГО контейнера, и по §10.6.7 высоту флоата обязан взять он, а
    // наша раскладка её оттуда уже не получает). oldfront 2352 -> 2349:
    // `flexbox_item-float`, `flexbox_item-top-float`, `flex-box-wrap` —
    // там контейнер приходит сюда БЕЗ своего `display`, то есть гибким его
    // никто не сделал, и прежняя зелень держалась на этой же ошибке.
    for node in out.iter_mut().filter(|_| !abs_parent) {
        let Node::Element(e) = node else { continue };
        let has_float = e
            .children
            .iter()
            .any(|n| matches!(n, Node::Element(c) if c.style.float.is_some_and(|f| f != 0)));
        if has_float && through_strut(e).is_some() {
            e.style.height = Some(Len::Px(0.0));
        }
    }
    // Отступ первого ребёнка «протекает» наружу, если родителя от него не
    // отделяют ни рамка, ни внутренний отступ: в CSS это один и тот же отступ,
    // а не два. Без этого блок уезжает вниз на величину детского отступа.
    for node in out.iter_mut() {
        let Node::Element(e) = node else { continue };
        if inline_level_box(e) {
            continue;
        }
        // Отступ не протекает наружу и через край блока с собственным
        // контекстом: прокрутка, обрезка, гибкая раскладка, сетка,
        // позиционирование. Раньше учитывались только рамка и внутренний
        // отступ, и содержимое прокручиваемой панели вставало на 6-8 точек
        // выше браузерного.
        let own_context = own_context(e);
        // Отсечка по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и
        // `border: 0` схлопыванию не мешают (CSS 2.1 §8.3.1).
        let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
        // `margin-trim` (css-box-4 §margin-trim-block): поле первого/последнего
        // ребёнка у ВНУТРЕННЕГО края контейнера обнуляется вместе со всем,
        // что с ним схлопнулось. Идёт ДО гейта схлопывания: обрезка работает
        // и когда край закрыт рамкой или внутренним отступом — там поле
        // наружу не уходит, но обрезать его всё равно надо.
        // Собственное поле контейнера не трогается («but not its own»).
        if e.style.margin_trim & 1 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if leading_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, true, *deep);
                }
            }
        }
        if e.style.margin_trim & 2 != 0 {
            let mut path: Vec<usize> = vec![];
            let mut eat: Vec<(Vec<usize>, bool)> = vec![];
            if trailing_chain(&e.children, &mut path, &mut eat).is_some() {
                for (p, deep) in &eat {
                    zero_at(&mut e.children, p, false, *deep);
                }
            }
        }
        // Поля КОРНЯ ни с чем не схлопываются (§8.3.1).
        if e.tag == "html" || !top_edge_open(e) {
            continue;
        }
        // Верхнее поле родителя и ВСЯ ведущая цепочка полей потомков — одно
        // поле (§8.3.1). Прежде поднималось поле ровно ОДНОГО ребёнка, и на
        // следующем уровне то же поле внука поднималось повторно.
        let mut path: Vec<usize> = vec![];
        let mut eat: Vec<(Vec<usize>, bool)> = vec![];
        let chain = leading_chain(&e.children, &mut path, &mut eat);
        if let (Some(s), Some(own)) = (chain, margin_or_bail(e.style.margin.top, &e.style))
            && !eat.is_empty()
        {
            e.style.margin.top = Some(Len::Px(solve(adjoin(strut_of(own), s))));
            for (p, deep) in &eat {
                zero_at(&mut e.children, p, true, *deep);
            }
        }
        // То же СНИЗУ: отступ последнего ребёнка протекает наружу, если
        // родителя от него не отделяют ни рамка, ни внутренний отступ, ни
        // заданная высота. Иначе следующий за родителем блок отодвигался на
        // сумму двух отступов вместо большего из них
        // (`text-align-end-015`: вторая коробка стояла на 20 точек ниже).
        // ЗАМЕРЕНО И ОТКАЧЕНО: считать снизу ВСЮ хвостовую цепочку, зеркально
        // верхней (`trailing_chain` + `zero_at(.., false, ..)`). CSS2 4684 ->
        // 4594: прибавка 4 (`margin-collapse-101/105`, два
        // `inline-formatting-context`) против 94 потерь. Наверху цепочку
        // ограничивает первый ребёнок с содержимым, а внизу ограничитель —
        // ВЫСОТА родителя, которой на этом шаге ещё нет: подъём уходит вглубь
        // и снимает поля там, где родитель на деле уже кончился. Возвращать
        // вместе с настоящей проверкой итоговой высоты (тот же блокер, что у
        // `min-height` ниже).
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: снять отсюда `min-height`, потому что по
        // спеке подъём закрывает не написанное свойство, а РАСХОЖДЕНИЕ
        // итоговой высоты с высотой по содержимому (CSS 2.1 §8.3.1, так же
        // считает Blink). Замерено: oldfront 2332 -> 2328. Нашей раскладке
        // «дотянулась ли высота» на этом шаге ещё не известно, и снятие
        // условия открывало подъём там, где высота на деле выросла.
        // Возвращать вместе с настоящей проверкой итоговой высоты.
        // `min-height` закрывает подъём, только если он ДЕЙСТВИТЕЛЬНО тянет
        // коробку выше её содержимого (§8.3.1 говорит о РАСХОЖДЕНИИ итоговой
        // высоты с высотой по содержимому, а не о написанном свойстве).
        // Нижняя оценка содержимого — сумма разрешимых в точки высот блочных
        // детей в потоке; неизвестная высота хотя бы у одного оставляет
        // прежний запрет. Замерено: CSS2 5074 -> 5077, oldfront 2353 -> 2352
        // (`css-flexbox-height-animation-stretch` 0.47 -> 1.00).
        let raises = match margin_px(e.style.min_height, &e.style) {
            None => !zero(e.style.min_height),
            Some(mh) if mh <= 0.0 => false,
            Some(mh) => {
                let mut sum = 0.0f32;
                let mut known = true;
                for c in &e.children {
                    match c {
                        Node::Text(t) if blank_text(t) => {}
                        Node::Text(_) => known = false,
                        Node::Element(ch) if !in_flow(&ch.style) => {}
                        Node::Element(ch) if ch.inline => known = false,
                        Node::Element(ch) => {
                            let b = ch.style.borders();
                            let side = |l: Option<Len>| match l {
                                None => Some(0.0),
                                Some(Len::Px(v)) => Some(v),
                                _ => None,
                            };
                            let own = match (
                                margin_px(ch.style.height, &ch.style),
                                side(ch.style.padding.top),
                                side(ch.style.padding.bottom),
                                side(b.top),
                                side(b.bottom),
                            ) {
                                (Some(h), Some(pt), Some(pb), Some(bt), Some(bb)) => {
                                    Some(h + pt + pb + bt + bb)
                                }
                                _ => None,
                            };
                            match own {
                                Some(v) => sum += v,
                                None => known = false,
                            }
                        }
                    }
                    if !known {
                        break;
                    }
                }
                !known || mh > sum + 0.01
            }
        };
        // Край закрыт СВОИМИ свойствами: поле ребёнка остаётся внутри и
        // трогать его нечем.
        if !zero(e.style.padding.bottom)
            || !zero(e.style.borders().bottom)
            || !matches!(e.style.height, None | Some(Len::Auto))
            || own_context
        {
            continue;
        }
        // Снизу плавающий ИЛИ АБСОЛЮТНЫЙ ребёнок ЗАКРЫВАЕТ подъём: float
        // заякорен в потоке после последнего блока (box-shadow-overlapping-002),
        // а статическая позиция абсолютного считается от места в потоке —
        // утёкший отступ поднимал их обоих (z-index-015: квадрат вставал
        // на отступ параграфа выше эталона).
        let child_bottom =
            first_in_flow(e.children.iter().enumerate().rev().take_while(|(_, c)| {
                !matches!(c, Node::Element(ch)
                if ch.style.float.is_some()
                    || matches!(
                        ch.style.position,
                        Some(crate::computed::Position::Absolute)
                            | Some(crate::computed::Position::Fixed)
                    ))
            }))
            .and_then(|(i, ch)| margin_px(ch.style.margin.bottom, &ch.style).map(|v| (i, v)));
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): запрет поглощения, когда в хвосте
        // есть коробка с клиренсом (CSS 2.1 §8.3.1, «does not collapse with a top
        // margin that has clearance»): срез CSS2 6204 пары, 5691 -> 5691 (+0/-0),
        // целевые `margin-collapse-clear-012/-013` как были «красное видно»,
        // так и остались. Значит потеря не здесь: до этого места дело либо не
        // доходит (гейты выше), либо `child_bottom` уже `None` — искать
        // надо во втором проходе (`cleared_run`).
        if let Some((i, v)) = child_bottom {
            // Минимальная высота выше содержимого: поле последнего ребёнка
            // ПРИМЫКАЕТ к его нижнему краю (§8.3.1), но наружу не идёт и
            // родителя не растит — низ родителя решает `min-height` (§10.6.3).
            // Третьего исхода не было вовсе: поле оставалось внутри и место
            // занимало.
            if raises {
                if let Node::Element(ch) = &mut e.children[i] {
                    ch.style.margin.bottom = Some(Len::Px(0.0));
                }
                continue;
            }
            let own = margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0);
            e.style.margin.bottom = Some(Len::Px(collapsed(own, v)));
            if let Node::Element(ch) = &mut e.children[i] {
                ch.style.margin.bottom = Some(Len::Px(0.0));
            }
        }
    }
    // Струна примыкающих полей соседей (§8.3.1). `emitted` — сколько точек уже
    // ЗАПИСАНО в стили этого зазора: раскладка складывает поля сама, и
    // верхнему полю следующего блока достаётся только разница.
    let mut strut: Option<Strut> = None;
    let mut emitted = 0.0f32;
    // Последняя коробка прогона с клиренсом: её остаток поля остаётся ВНУТРИ
    // родителя и наружу не уходит.
    let mut cleared_run: Option<usize> = None;
    for (idx, node) in out.iter_mut().enumerate() {
        let Node::Element(e) = node else {
            // Переводы строк между блоками разрывом потока не считаются: в
            // форматированной разметке они стоят везде, и из-за них
            // схлопывание не срабатывало ни разу.
            if matches!(node, Node::Text(t) if blank_text(t)) {
                continue;
            }
            strut = None;
            continue;
        };
        // Строчный элемент С СОДЕРЖИМЫМ порождает строчную коробку, и поля
        // блоков через неё уже не примыкают.
        if inline_level_box(e) {
            if !e.children.is_empty() {
                strut = None;
            }
            continue;
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: обрывать струну на атомарном строчном
        // (`inline-block` и родня в потоке рождают строчную коробку). CSS2
        // -38, вся потеря — семья `bidi-box-model-*`: там такой сосед стоит
        // между блоками сплошь, и разрыв струны разводит их полями врозь.
        // Возвращаться вместе с настоящей строчной коробкой в раскладке.
        if !in_flow(&e.style) {
            continue;
        }
        let top = margin_px(e.style.margin.top, &e.style).unwrap_or(0.0);
        let bottom = margin_px(e.style.margin.bottom, &e.style).unwrap_or(0.0);
        let through = through_strut(e);
        let mut merged = match strut {
            Some(s) => {
                let m = adjoin(s, strut_of(top));
                // Верхний край насквозь-схлопнутой коробки встаёт там, где
                // разрешается струна до неё вместе с её верхним полем. Та же
                // строка даёт это и обычной коробке: нижнее поле соседа
                // раскладка уже поставила, верхнему достаётся разница.
                e.style.margin.top = Some(Len::Px(solve(m) - emitted));
                emitted = solve(m);
                m
            }
            None => {
                // Примыкать не к чему: верхнее поле остаётся как написано.
                emitted = top;
                strut_of(top)
            }
        };
        // Коробка с клиренсом, которая иначе схлопнулась бы насквозь
        // (§8.3.1): её поля СЛИВАЮТСЯ между собой, но получившееся поле не
        // схлопывается с нижним полем родителя — «these margins collapse with
        // the adjoining margins of following siblings but the resulting margin
        // does not collapse with the bottom margin of the parent block».
        // Верхнее поле уже выложено рядом обтекания, поэтому наружу идёт
        // только остаток.
        if through.is_none()
            && e.style.clear.is_some()
            && through_strut_no_clear(e).is_some()
        {
            emitted = top;
            e.style.margin.bottom = Some(Len::Px(0.0));
            strut = Some(adjoin(strut_of(top), strut_of(bottom)));
            cleared_run = Some(idx);
            continue;
        }
        if let Some(own) = through {
            merged = adjoin(merged, own);
            // Своё нижнее поле коробка не ставит: оно ушло в струну, и
            // раскладка сложила бы его второй раз.
            e.style.margin.bottom = Some(Len::Px(0.0));
            // ЗАМЕРЕНО И ОТКАЧЕНО: снимать поля и у ДЕТЕЙ насквозь-коробки
            // (`zero_margins_deep`) — CSS2 4675 -> 4672, потери
            // `floats-clear/margin-collapse-033/034/035`, прибавки нет. Поля
            // детей уже учтены струной, но раскладка ставит саму коробку не
            // по струне, а по своим полям — снятие уводит её вверх.
            strut = Some(merged);
            continue;
        }
        strut = Some(strut_of(bottom));
        emitted = bottom;
        cleared_run = None;
    }
    // Прогон кончился на коробке с клиренсом: остаток слитого поля пишется ей
    // самой — родителя он растит, но наружу не выходит.
    if let (Some(i), Some(s)) = (cleared_run, strut) {
        let rest = solve(s) - emitted;
        if rest > 0.0
            && let Some(Node::Element(e)) = out.get_mut(i)
        {
            e.style.margin.bottom = Some(Len::Px(rest));
        }
    }
    out
}

/// Слитый отступ CSS 2.1 §8.3.1: больший из положительных плюс меньший
/// (самый отрицательный) из отрицательных.
fn collapsed(a: f32, b: f32) -> f32 {
    a.max(0.0).max(b.max(0.0)) + a.min(0.0).min(b.min(0.0))
}

/// Первый (по направлению итератора) IN-FLOW блочный ребёнок: плавающие,
/// абсолютные и пустые строчные пропускаются, непробельный текст и строчный
/// элемент с содержимым (строчная коробка!) обрывают поиск.
fn first_in_flow<'a>(
    it: impl Iterator<Item = (usize, &'a Node)>,
) -> Option<(usize, &'a crate::dom::Element)> {
    for (i, c) in it {
        match c {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return None,
            Node::Element(ch) if ch.inline => {
                // Замещаемый атом (img и родня) — строчная КОРОБКА, а не
                // пустой спан: он рождает line box и рвёт примыкание.
                // Пропуск ронял отступ параграфа перед голым <img> в
                // эталонах (130 пар «эталон рисует img»).
                if ch.children.is_empty()
                    && !matches!(
                        ch.tag.as_str(),
                        "img"
                            | "svg"
                            | "canvas"
                            | "video"
                            | "embed"
                            | "object"
                            | "iframe"
                            | "input"
                            | "br"
                    )
                {
                    continue;
                }
                return None;
            }
            Node::Element(ch) => {
                // Строчный КОНТЕЙНЕР (`inline-block` и родня) рождает
                // строчную коробку и РВЁТ примыкание (CSS 2.1 §8.3.1), в
                // отличие от плавающего и абсолютного, которых в потоке нет
                // вовсе. Пока он просто пропускался, отступ предыдущего
                // блока «протекал» наружу мимо него, и строка вставала на
                // отступ выше (`box-sizing-010`: квадраты разъезжались на
                // кегль).
                // Разбор держит `display: inline` как `InlineBlock` с
                // пометкой `inline_display`, поэтому одного взгляда на
                // display мало: обычный строчный элемент своей коробки не
                // имеет и примыкание НЕ рвёт (пустой `<span>` между блоками
                // прозрачен).
                // Своей коробки у `display: inline` нет, и примыкание он
                // рвёт не собой, а СТРОЧНОЙ КОРОБКОЙ, которую рождает его
                // содержимое: вокруг неё встаёт анонимная блочная коробка
                // (§9.2.1.1). Пустой такой элемент прозрачен, а с текстом
                // внутри обязан оборвать поиск — иначе отступ предыдущего
                // блока протекает под анонимную коробку, и строка встаёт на
                // него выше (`inline-formatting-context-002`).
                if ch.style.inline_display == Some(true) {
                    if replaced_inline(&ch.tag) || holds_line_box(&ch.children) {
                        return None;
                    }
                    continue;
                }
                if ch.style.inline_display != Some(true)
                    && matches!(
                        ch.style.display,
                        Some(Display::InlineBlock)
                            | Some(Display::InlineFlex)
                            | Some(Display::InlineGrid)
                            | Some(Display::InlineTable)
                    )
                {
                    return None;
                }
                if !in_flow(&ch.style) {
                    continue;
                }
                return Some((i, ch));
            }
        }
    }
    None
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

thread_local! {
    /// Кегль РОДИТЕЛЯ на разбираемом уровне: единицы шрифта в отступах
    /// меряются от кегля элемента, а он к моменту схлопывания ещё не
    /// унаследован — наследование живёт ниже по пути (`inline::inherit`).
    /// Значение ставит `blocks()` вокруг вызова `collapse_margins` и
    /// возвращает на место после него.
    static COLLAPSE_FONT_PX: std::cell::Cell<f32> = const { std::cell::Cell::new(16.0) };
    /// Ширина содержащего блока уровня схлопывания (для процентных полей).
    static COLLAPSE_CB_WIDTH_PX: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Заводит ли коробка СВОЙ блочный контекст форматирования: через её край
/// поля не схлопываются ни с детьми, ни насквозь (CSS 2.1 §8.3.1).
fn own_context(e: &Element) -> bool {
    !matches!(
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
        || e.style.contain_paint == Some(true)
        || e.style.contain_layout == Some(true)
        || e.style.contain_size == Some(true)
        || e.style.flow_root == Some(true)
        || matches!(e.style.display, Some(Display::TableCell))
        || e.style.column_count.is_some()
        || e.style.column_width.is_some()
}

/// Ноль по ЗНАЧЕНИЮ, а не по «свойство написано»: `padding: 0` и `border: 0`
/// схлопыванию не мешают (CSS 2.1 §8.3.1).
fn zero_len(l: Option<Len>) -> bool {
    matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)))
}

/// Открыт ли ВЕРХНИЙ край коробки для примыкания к полю первого ребёнка: нет
/// ни рамки, ни поля сверху, и коробка не заводит своего контекста (§8.3.1).
fn top_edge_open(e: &Element) -> bool {
    !own_context(e) && zero_len(e.style.padding.top) && zero_len(e.style.borders().top)
}

/// Струна примыкающих полей (CSS 2.1 §8.3.1): больший положительный и самый
/// отрицательный. Свёртка ассоциативна, поэтому все три случая спеки — сосед,
/// родитель с ребёнком и схлопывание насквозь — считаются одним кодом.
type Strut = (f32, f32);

fn strut_of(v: f32) -> Strut {
    (v.max(0.0), v.min(0.0))
}

fn adjoin(a: Strut, b: Strut) -> Strut {
    (a.0.max(b.0), a.1.min(b.1))
}

/// Итог струны: максимум положительных минус максимум модулей отрицательных.
fn solve(s: Strut) -> f32 {
    s.0 + s.1
}

/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// не содержит строчной коробки, и поля всех её детей в потоке тоже

/// Замещаемый строчный атом: своих детей не имеет, но КОРОБКУ рождает —
/// значит, рождает и строчную коробку. Пустой `<span>` — не рождает.
fn replaced_inline(tag: &str) -> bool {
    matches!(
        tag,
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input" | "br"
    )
}

/// Строчного УРОВНЯ, но В ПОТОКЕ: `inline-block` и родня. Рождает строчную
/// коробку, в отличие от плавающего и абсолютного, которых в потоке нет.
/// Разбор держит `display: inline` как `InlineBlock` с пометкой
/// `inline_display`, поэтому одного взгляда на `display` мало.
fn atomic_inline(c: &Computed) -> bool {
    c.float.is_none()
        && !matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
        && c.inline_display != Some(true)
        && matches!(
            c.display,
            Some(Display::InlineBlock)
                | Some(Display::InlineFlex)
                | Some(Display::InlineGrid)
                | Some(Display::InlineTable)
        )
}

/// Содержит ли коробка строчную коробку (§8.3.1, «does not contain a line
/// box»; нулевые строчные коробки §9.4.2 не в счёт).
///
/// Правила те же, что у `first_in_flow`: пробельный текст прозрачен,
/// непробельный рождает строку; ПУСТОЙ строчный элемент прозрачен, а
/// замещаемый атом (`img` и родня) — нет; вне потока строки не рождает никто;
/// `display: contents` своей коробки не даёт — смотреть надо в его детей.
fn holds_line_box(children: &[Node]) -> bool {
    children.iter().any(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(ch) => {
            if ch.style.display == Some(Display::None) {
                return false;
            }
            if ch.style.display == Some(Display::Contents) {
                return holds_line_box(&ch.children);
            }
            // Плавающий и абсолютный строчной коробки не рождают.
            if ch.style.float.is_some_and(|f| f != 0)
                || matches!(
                    ch.style.position,
                    Some(crate::computed::Position::Absolute)
                        | Some(crate::computed::Position::Fixed)
                )
            {
                return false;
            }
            // Атомарный строчный в потоке — своя строчная коробка. Проверять
            // ДО `in_flow`: он их не различает и валит в одну корзину с
            // плавающим.
            if atomic_inline(&ch.style) {
                return true;
            }
            // `display: inline` делает строчным ЛЮБОЙ тег: своей коробки у
            // него нет, а строчную рождает его содержимое. Без этой ветки
            // `<div style="display:inline">` считался блочным ребёнком и
            // строки «не рождал», хотя текст внутри него её рождает.
            if ch.inline || ch.style.inline_display == Some(true) {
                return replaced_inline(&ch.tag) || holds_line_box(&ch.children);
            }
            // Блочный ребёнок строки не рождает: его содержимое разбирает
            // рекурсия `through_strut`.
            false
        }
    })
}

/// Поле в точках или `None`, если единица нам не по зубам (доля, `vh`,
/// `calc`). Ноль подставлять НЕЛЬЗЯ: ветка насквозь значение ЗАПИСЫВАЕТ
/// обратно, и написанное пропадёт навсегда (`margin-bottom-103`: `50%`
/// превращалось в `0`).
fn margin_or_bail(l: Option<Len>, style: &Computed) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(_) => margin_px(l, style),
    }
}

/// Схлопывается ли коробка НАСКВОЗЬ, и какая струна из неё выходит.
///
/// §8.3.1: своими полями коробка схлопывается, когда у неё нулевой
/// `min-height`, нет рамок и полей по вертикали, высота ноль или `auto`, она
/// НЕ СОДЕРЖИТ СТРОЧНОЙ КОРОБКИ, и поля всех её детей в потоке тоже
/// схлопываются. Возвращаются слитые поля — свои плюс поля всех
/// насквозь-потомков: это и есть транзитивность примыкания.
fn through_strut(e: &Element) -> Option<Strut> {
    through_strut_inner(e, false)
}

/// То же, но без вето по `clear`: нужно, чтобы отличить «не схлопывается
/// вовсе» от «схлопнулась бы, если бы не клиренс».
fn through_strut_no_clear(e: &Element) -> Option<Strut> {
    through_strut_inner(e, true)
}

fn through_strut_inner(e: &Element, ignore_clear: bool) -> Option<Strut> {
    if e.inline || !in_flow(&e.style) || own_context(e) {
        return None;
    }
    // Поля КОРНЯ ни с чем не схлопываются (§8.3.1).
    if e.tag == "html" {
        return None;
    }
    // Коробка с `clear` насквозь не схлопывается: клиренс разделяет её поля
    // (§8.3.1, «if the element's margins are collapsed … clearance»). Пустой
    // `<div class="clear-left">` между флоатом и соседом иначе пропускал бы
    // поле соседа наружу (`floats-clear/margin-collapse-033…035`).
    //
    // ПРОБОВАЛИ И ОТКАТИЛИ: делать вето инертным, когда во всём документе нет
    // ни одного флоата (§9.5.2 вводит клиренс только при флоате выше). Проба
    // по паре `margin-collapse-135` и `margin-collapse-clear-016`, на которые
    // правка и рассчитывалась: обе остались «красное видно», флипов ноль.
    // Держит их не вето, а что-то ниже по цепи. Признак документа пришлось бы
    // нести отдельным thread-local, а рамка рисует вложенный документ тем же
    // `render()` и признак бы затёрла.
    if e.style.clear.is_some() && !ignore_clear {
        return None;
    }
    let zero = |l: Option<Len>| matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let b = e.style.borders();
    // §10.5: доля высоты при НЕОПРЕДЕЛЁННОМ содержащем блоке «computes to
    // auto», а `auto` схлопыванию насквозь не мешает (§8.3.1). Прежде любая
    // ненулевая доля закрывала ветку, и три пустых блока с полями 100 давали
    // 200 вместо 100 (`margin-collapse-through-percentage-height-block`).
    // Признак блока несёт сам стиль — `cb_height_def` ставит `inline::inherit`.
    let height_is_auto = matches!(
        e.style.height,
        None | Some(Len::Auto) | Some(Len::Px(0.0)) | Some(Len::Pct(0.0))
    ) || matches!(e.style.height, Some(Len::Pct(_)) if !e.style.cb_height_def);
    let min_height_is_zero = zero(e.style.min_height)
        || matches!(e.style.min_height, Some(Len::Pct(_)) if !e.style.cb_height_def);
    if !zero(e.style.padding.top)
        || !zero(e.style.padding.bottom)
        || !zero(b.top)
        || !zero(b.bottom)
        || !min_height_is_zero
        || !height_is_auto
    {
        return None;
    }
    // Главная проверка содержимого: без неё `<div>` из четырёх `<img>`
    // считался пустым (ЗАМЕРЕНО: -30 на эталонах `bidi-box-model-*`).
    if holds_line_box(&e.children) {
        return None;
    }
    let mut s = adjoin(
        strut_of(margin_or_bail(e.style.margin.top, &e.style)?),
        strut_of(margin_or_bail(e.style.margin.bottom, &e.style)?),
    );
    // «all of its in-flow children's margins collapse» — рекурсия по блочным
    // детям в потоке. Строчных здесь уже нет (проверка выше), вне потока —
    // запрета не создают.
    for c in &e.children {
        let Node::Element(ch) = c else { continue };
        // `clear` у ребёнка в потоке закрывает схлопывание насквозь (§8.3.1
        // исключение 2) — проверять ДО отсечки строчных: псевдоэлемент
        // помечается строчным независимо от своего `display`, и клирфикс
        // `div::after { clear: both; display: block }` был отсюда не виден.
        if in_flow(&ch.style) && ch.style.clear.is_some() {
            return None;
        }
        if ch.inline
            || ch.style.display == Some(Display::None)
            || ch.style.display == Some(Display::Contents)
            || !in_flow(&ch.style)
        {
            continue;
        }
        s = adjoin(s, through_strut(ch)?);
    }
    Some(s)
}

/// Ведущая цепочка примыкания к ВЕРХНЕМУ краю коробки (§8.3.1).
///
/// Верхнее поле коробки примыкает к верхнему полю её первого ребёнка в потоке.
/// Если тот схлопывается насквозь, примыкание тянется ВБОК, к следующему
/// брату; если не схлопывается — ВГЛУБЬ, к его собственному первому ребёнку.
///
/// Меряет ИММУТАБЕЛЬНО и копит пути до съеденных полей: обнулять на ходу
/// нельзя, потому что доля или `calc` на середине цепи заставят вернуть
/// `None`, а записанные нули уже не откатить.
fn leading_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    for (i, c) in children.iter().enumerate() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            // Непробельный текст — строчная коробка, примыкание кончилось.
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        if ch.inline {
            // Пустой `<span>` прозрачен, замещаемый атом рождает строку.
            if ch.children.is_empty() && !replaced_inline(&ch.tag) {
                continue;
            }
            return Some(s);
        }
        // Атомарный строчный в потоке есть и рождает строчную коробку.
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        // Плавающий и абсолютный в потоке не участвуют и примыкания не рвут.
        if !in_flow(&ch.style) {
            continue;
        }
        // Клиренс разделяет поля (§8.3.1): дальше по цепи примыкание не
        // идёт, и поле такого ребёнка наружу не уходит.
        if ch.style.clear.is_some() {
            return Some(s);
        }
        // Поле САМОГО ребёнка примыкает к полю родителя всегда — даже когда
        // ребёнок заводит свой контекст: запрет §8.3.1 лежит на РОДИТЕЛЕ.
        s = adjoin(s, strut_of(margin_or_bail(ch.style.margin.top, &ch.style)?));
        path.push(i);
        if let Some(t) = through_strut(ch) {
            // Насквозь: поля всего поддерева уже в струне, идём к брату.
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            continue;
        }
        eat.push((path.clone(), false));
        if top_edge_open(ch) {
            s = adjoin(s, leading_chain(&ch.children, path, eat)?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Хвостовая цепочка примыкающих НИЖНИХ полей — зеркало `leading_chain`.
///
/// В схлопывании она НЕ участвует: попытка поднимать её наружу замерена и
/// откачена (см. комментарий в `collapse_margins`, CSS2 4684 -> 4594) —
/// вглубь подъём уходил мимо ещё неизвестной высоты родителя. Для
/// `margin-trim: block-end` этой опасности нет: наружу ничего не поднимается,
/// поля только гасятся, и цепочка ограничена последним ребёнком в потоке.
fn trailing_chain(
    children: &[Node],
    path: &mut Vec<usize>,
    eat: &mut Vec<(Vec<usize>, bool)>,
) -> Option<Strut> {
    let mut s = strut_of(0.0);
    for (i, c) in children.iter().enumerate().rev() {
        let ch = match c {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return Some(s),
            Node::Element(ch) => ch,
        };
        if ch.inline {
            if ch.children.is_empty() && !replaced_inline(&ch.tag) {
                continue;
            }
            return Some(s);
        }
        if atomic_inline(&ch.style) {
            return Some(s);
        }
        if !in_flow(&ch.style) {
            continue;
        }
        s = adjoin(
            s,
            strut_of(margin_or_bail(ch.style.margin.bottom, &ch.style)?),
        );
        path.push(i);
        if let Some(t) = through_strut(ch) {
            s = adjoin(s, t);
            eat.push((path.clone(), true));
            path.pop();
            continue;
        }
        eat.push((path.clone(), false));
        // Заданная высота или нижняя рамка/отступ отрезают цепочку: поле
        // внука к краю контейнера уже не примыкает.
        if ch.style.height.is_none() && top_edge_open(ch) {
            s = adjoin(s, trailing_chain(&ch.children, path, eat)?);
        }
        path.pop();
        return Some(s);
    }
    Some(s)
}

/// Обнулить поле по пути: наружу оно ушло одним полем родителя, и раскладка
/// сложила бы его второй раз. `deep` — коробка схлопнулась насквозь: чистится
/// она сама с обеих сторон.
fn zero_at(children: &mut [Node], path: &[usize], top: bool, deep: bool) {
    let Some((&i, rest)) = path.split_first() else {
        return;
    };
    let Some(Node::Element(ch)) = children.get_mut(i) else {
        return;
    };
    if !rest.is_empty() {
        zero_at(&mut ch.children, rest, top, deep);
        return;
    }
    if deep {
        ch.style.margin.top = Some(Len::Px(0.0));
        ch.style.margin.bottom = Some(Len::Px(0.0));
    } else if top {
        ch.style.margin.top = Some(Len::Px(0.0));
    } else {
        ch.style.margin.bottom = Some(Len::Px(0.0));
    }
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
    // Кегль элемента: свой, если задан, иначе унаследованный от уровня
    // (см. `COLLAPSE_FONT_PX`). Прежде вместо унаследованного брались
    // постоянные 16 точек, и `table{font-size:50px} div{margin:1em 0}`
    // схлопывался по 16 вместо 50 — вся семья Hixie `margin-collapse-1xx`
    // расходилась с эталоном ровно на эту разницу.
    let parent = COLLAPSE_FONT_PX.with(std::cell::Cell::get);
    let base = match style.font_size {
        Some(Len::Px(v)) => v,
        Some(Len::Em(k)) => k * parent,
        Some(Len::Pct(k)) => k * parent,
        _ => parent,
    };
    match l? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * base),
        // Единицы шрифта, считающиеся по МЕТРИКЕ гарнитуры: сюда они доезжают
        // неразрешёнными, потому что `resolve_em` живёт в наследовании
        // (`inline::inherit`), а схлопывание идёт раньше. Прежде `-6ex`
        // отдавало `None`, поле пропадало целиком (`positioning/top-091`).
        l @ (Len::Ch(_) | Len::Ex(_)) => Some(crate::metrics::spacing_px(
            Some(l),
            &style.font_family.clone().unwrap_or_default(),
            base,
        )),
        // Процент — от ширины содержащего блока, когда она известна в точках
        // (`margin-top-103`, `margin-bottom-113`); иначе поле пропускается.
        Len::Pct(k) => COLLAPSE_CB_WIDTH_PX.with(std::cell::Cell::get).map(|w| k * w),
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

/// Знак стоит прямо в вертикальном письме с `text-orientation: mixed`
/// (UTR#50, vo=U, упрощённо): иероглифика, кана, CJK-знаки препинания и
/// полноширинные формы. Остальное — лежит боком.
fn upright_in_mixed(c: char) -> bool {
    matches!(c as u32,
        0x3000..=0x303F   // CJK-знаки и пунктуация (「」、。 …)
        | 0x3040..=0x30FF // хирагана и катакана
        | 0x31F0..=0x31FF // фонетические расширения каны
        | 0x3200..=0x33FF // обведённые и совместимые CJK
        | 0x3400..=0x4DBF // иероглифика, расширение A
        | 0x4E00..=0x9FFF // иероглифика единая
        | 0xAC00..=0xD7AF // хангыль
        | 0xF900..=0xFAFF // совместимая иероглифика
        | 0xFE30..=0xFE4F // вертикальные формы совместимости
        | 0xFF01..=0xFF60 // полноширинные формы
        | 0x20000..=0x2FFFD // иероглифика, плоскость 2
    )
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
            // Толщина вертикальной строки — LINE-HEIGHT, как у горизонтальной
            // (стопка глифов стоит в полосе высоты строки, повернутой набок):
            // читается ДО подмены шага стопки кеглем, иначе полоса всегда
            // равнялась кеглю (`vertical-alignment-vrl-022`).
            let lane = match stack.line_height {
                Some(Len::Px(v)) => v,
                Some(Len::Em(k)) | Some(Len::Pct(k)) => k * em,
                // До этой точки `ch` мог не разрешиться: стоячий ноль
                // продвигается на кегль (§7.4) — считаем сами.
                Some(Len::Ch(k)) => k * em,
                _ => em,
            };
            stack.line_height = Some(Len::Px(em));
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
        // `vertical-lr`: колонки идут слева направо — строки подаются снизу
        // вверх, чтобы после поворота ПО ЧАСОВОЙ первая оказалась левой (у
        // vertical-rl порядок родной: первая строка правой колонкой).
        //
        // `sideways-lr` вертится ПРОТИВ часовой (css-writing-modes-4,
        // Abstract-Physical Mapping: line-left = низ, over = лево), и после
        // такого поворота первая горизонтальная строка САМА оказывается левой
        // колонкой. Подавать строки снизу вверх тут — второй разворот,
        // ровно он и давал 180° (`block-flow-direction-slr-043` 32.20).
        let ccw_line = inherited.sideways == Some(true) && inherited.vertical_rl != Some(true);
        if inherited.vertical_rl != Some(true) && !ccw_line {
            horizontal.lines_reversed = Some(true);
        }
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
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09, пятый заход): предел ортогонального
        // потока без `ortho_limit` = ICB минус свои рамки/отбивки/поля и
        // shrink-to-fit (`max_w` + `fit_within` только при `ortho_limit`
        // = None, css-writing-modes-4 §7.3.1) — css-writing-modes 572 -> 554
        // (+10/−28): десять `sizing-orthog-*-in-htb` позеленели, но
        // `available-size-001…018`, `float-*-orthog-*`, `line-box-height-*`
        // ушли в красное — у них тоже нет `ortho_limit`, а заявка высоты во
        // весь ICB им противопоказана. Развилка та же, что в четырёх откатах
        // ниже: предел обязан приходить от родителя, а не от окна.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (четыре захода подряд): физическая высота
        // повёрнутого блока по СОДЕРЖИМОМУ. Жёсткая ширина делает
        // `natural.width` тождественно равной пределу, поэтому без
        // `ortho_limit` не срабатывает ни один гейт заявки (высота нулевая), а
        // с ним всегда срабатывает `fit_within` (высота во весь предел).
        // Срез вертикального письма (571 пара, 423 зелёных):
        //   1) `max_w` + безусловная заявка высоты, замер MaxContent — 407;
        //   2) один `max_w`, замер MaxContent — 419;
        //   3) `max_w` + замер ПОД пределом (`VerticalText::flowing`,
        //      `AvailableSpace::Definite`) + безусловная заявка — 403
        //      (приобретено 4, потеряно 24);
        //   4) то же без безусловной заявки — 421 (лучший из четырёх).
        // Во всех четырёх рушится семья `available-size-*`: заявленная высота
        // ортогонального потока ломает их ожидание, а без заявки коробка
        // остаётся нулевой. Значит развилка не в способе замера, а в том, что
        // предел ортогонального потока обязан приходить от РОДИТЕЛЯ (§7.3), а
        // не подменяться шириной обёртки. Возвращать вместе с ним.
        // …и всё же ОДИН случай жёсткую ширину не терпит: АБСОЛЮТНАЯ коробка
        // со свободной строчной осью. Её размер по этой оси — по содержимому
        // (§10.3.7), а жёсткая ширина делает `natural.width` тождественно
        // равной пределу, и высота выходит во весь предел ортогонального
        // потока — коробка растягивалась на весь содержащий блок и вылезала
        // за него. Гейт узкий: потоковых коробок, на которых мерились четыре
        // отката выше, он не касается.
        let edge = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
        // Коробка на СТАТИЧЕСКОЙ позиции тоже абсолютна: `position` с неё
        // снято ради отсчёта, и без пометки `abs_static` гейт её не узнавал.
        let free_inline = (matches!(
            inherited.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) || inherited.abs_static
            || inherited.hug_inline)
            && !matches!(inherited.height, Some(Len::Px(_)) | Some(Len::Pct(_)))
            && !(edge(inherited.inset.top) && edge(inherited.inset.bottom));
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09), пятый заход по §7.3.1 «Auto-sizing
        // Block Containers in Orthogonal Flows»: без определённого предка
        // брать пределом НАЧАЛЬНЫЙ содержащий блок (`max_w` + `fit_within`
        // от окна). Срез вертикального письма 1086 пар: 596 -> 602,
        // приобретено 29 (вся семья `sizing-orthog-{vlr,vrl}-in-htb-*`,
        // `clip-rect-v*`, `caption-side-v*`), потеряно 23 —
        // `available-size-003…018` (0.05-0.11 -> «красное видно»),
        // `line-box-height-v{lr,rl}-*` (0.15 -> 0.67), четыре
        // `float-*-orthog-*-in-htb-*` (0.02-0.32 -> 4.7-6.7),
        // `direction-upright-001`, `border-slice-001`, `text-combine-*`.
        // С вычетом собственных полей и рамки из предела — ещё хуже (592).
        // Значит предел ICB как таковой верен, но коробке нужен НЕ он, а
        // ближайший определённый scrollport (§7.3.1 п.2), которого у нас нет;
        // возвращать вместе с ним.
        let inner = if free_inline {
            div().max_w(px(limit)).child(inner).into_any_element()
        } else {
            div().w(px(limit)).child(inner).into_any_element()
        };
        // Высота заявляется только под ортогональным зажимом (max-height от
        // §7.3) и только при ПОЛНОМ зажиме — иначе коробка без высоты
        // схлопывалась в ноль (даже фон пропадал), а заявка без зажима
        // делала её бесконечной (замерено: wm 118 → 104).
        let vt = crate::interact::VerticalText::new(inner)
            .counter_clockwise(ccw_line)
            .keyed(crate::interact::vt_seq_key(
                text_id(&plain) ^ opts.doc_salt ^ (nodes.len() as u64).wrapping_mul(0x9E3779B9),
            ));
        // Настоящий предел от родителя (ортогональная ячейка): строка,
        // которая уже влезает, заявляет высоту честно — без неё гибкая
        // ячейка мерила коробку нулём и justify уводил глиф из виду.
        let vt = if let Some(l) = inherited.ortho_limit {
            vt.fit_within(px(l))
        } else {
            vt
        };
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
        // Статическая позиция считается для ГИПОТЕТИЧЕСКОГО статического
        // элемента (§10.3.7: «if position had been static») — блокификация
        // `display: inline` под absolute на неё не влияет, метка
        // inline_display возвращает такой элемент в строчный путь.
        if plain_flow
            && at_static_position(&e.style)
            && (inline_level(e) || e.style.inline_display == Some(true))
        {
            let mut merged = inline::inherit(inherited, &e.style);
            merged.position = None;
            merged.abs_static = true;
            // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
            // путь блоков давал пустую коробку (clip-path-ellipse-2-ref).
            // Картинка — тем же порядком: у неё детей нет вовсе, и путь
            // блоков давал пустую коробку, то есть абсолютная картинка без
            // краёв не рисовалась ВООБЩЕ (`clip-rect-v*`, проба
            // `probe/absimg2.html`).
            let inner = if e.tag == "svg" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
            } else if e.tag == "img" {
                let mut copy = e.clone();
                copy.style.image_orient_none = merged.image_orient_none;
                copy.style.position = None;
                // `clip: rect(...)` и маска у картинки живут в буфере группы:
                // путь наложения идёт мимо `grouped`, и без обёртки картинка
                // рисовалась бы целиком (`clip-rect-v*`).
                grouped(image(&copy), &e.style)
            } else {
                // Тот же буфер группы, что и у картинки: маска, обрезка и
                // фильтр иначе не доходят до коробки на статической позиции.
                grouped(
                    styled_div_with(e, &merged)
                        .children(blocks(&e.children, &merged, opts))
                        .into_any_element(),
                    &e.style,
                )
            };
            return Some(inline::Piece::Overlay(inner));
        }
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: контр-поворот физических четвёрок краёв
        // (`margin`/`padding`/`border-width` и четвёрки цвета, стиля,
        // видимости) у КАЖДОГО куска повёрнутого абзаца — поворот уносит их
        // с собой, а стороны письмом не переставляются (§3.2). Направление
        // проверено обоими: по часовой (верх←право, право←низ, низ←лево,
        // лево←верх) заметно лучше обратного — девять пар `grid-self-
        // baseline-*` шли 8.38/5.83/16.62/21.74/5.13/3.98/1.52/4.22/6.88, по
        // часовой стало 7.98/5.83/13.78/12.65/5.02/4.39/1.52/6.11/6.88,
        // против часовой 8.31/5.83/16.61/12.99/5.26/6.09/1.52/7.81/6.88.
        // Зелёной не стала НИ ОДНА: их держит отсутствие физической высоты
        // (см. откат про `max_w` выше), а `wm-propagation-body-049` ушла
        // 0.00 → 2.24. Итог на срезе 571 пары: 423 → 422.
        // Возвращать вместе с высотой повёрнутого блока по содержимому.
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
                // Сжатие ШРИФТОВОЙ фичей раньше масштаба (css-writing-modes-3
                // §9.1.3): 2 знака — half-width, 3 — third, 4 — quarter.
                // Шрифт без фичи набор игнорирует — тогда работает прежний
                // масштаб (text-combine-upright-compression-004: qwid).
                let feature = match text.chars().count() {
                    2 => Some("hwid"),
                    3 => Some("twid"),
                    4 => Some("qwid"),
                    _ => None,
                };
                if let Some(f) = feature {
                    merged.font_features.push((f.to_string(), 1));
                }
                let inner = paragraph(&e.children, &merged, opts);
                return Some(inline::Piece::Atom(
                    crate::interact::CombinedUpright::new(inner, em).into_any_element(),
                ));
            }
        }
        // Боковые поля атома с собственным прижимом несёт ОБЁРТКА: внутри
        // неё они сдвигают коробку, но в продвижение строки не входят —
        // следующий кусок наезжал на предыдущий ровно на его поле
        // (эталоны `fixed-table-layout-021..023`: `img{vertical-align:top}`
        // плюс `margin-left`).
        let wrapped = match e.style.vertical_align {
            Some(crate::computed::Align::Start)
            | Some(crate::computed::Align::End)
            | Some(crate::computed::Align::Center) => true,
            _ => false,
        };
        let original_margin = e.style.margin;
        let bare;
        let e = if wrapped {
            let mut copy = e.clone();
            copy.style.margin = Default::default();
            bare = copy;
            &bare
        } else {
            e
        };
        atom_element(e, inherited, opts).map(|el| {
            // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
            // ряд строит базовую линию, а кускам с top/middle/bottom нужен
            // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
            // в строке с квадратом прижата к верху, у нас висела на базовой).
            // ПРОБОВАЛИ И ОТКАТИЛИ: выражать `text-top`/`text-bottom` у
            // атомарного куска прижимом к краю строки. Замерено по семьям
            // linebox/*, css1/*, *vertical*: 0 и 0 — этим парам нужен сдвиг
            // относительно ТЕКСТОВОЙ области родителя, а не край строки.
            use crate::computed::Align;
            let self_align = match e.style.vertical_align {
                Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
                Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
                Some(Align::Center) => Some(gpui::AlignItems::Center),
                _ => None,
            };
            let el = match self_align {
                Some(a) => {
                    let mut w = crate::apply::margins(div().flex_shrink_0(), &original_margin);
                    w.style().align_self = Some(a);
                    // Доля куска считается от его КОНТЕЙНЕРА, а обёртка встаёт
                    // между ним и рядом: без своей ширины она сжимается по
                    // содержимому, и `width: 100%` внутри разрешался в ноль —
                    // картинка пропадала целиком (`background-repeat-002-ref`:
                    // `img{vertical-align:top}` + `width="100%"`). Долю
                    // повторяем на обёртке, чтобы отсчёт остался прежним.
                    if let Some(Len::Pct(k)) = e.style.width {
                        w = w.w(gpui::relative(k));
                    }
                    if let Some(Len::Pct(k)) = e.style.height {
                        w = w.h(gpui::relative(k));
                    }
                    w.child(el).into_any_element()
                }
                None => el,
            };
            // Отрицательный `z-index` строчного замещаемого: краска уходит
            // ПОД содержимое до него (CSS 2.1 §9.9 шаг 3) — как у блочного
            // (background-size-document-root-vrl-*: красный маркер обязан
            // лечь под зелёный фон iframe).
            let el = if e.style.z_index.is_some_and(|z| z < 0)
                && e.style.position == Some(crate::computed::Position::Relative)
            {
                crate::interact::Underlay::new(el).into_any_element()
            } else {
                el
            };
            // Абсолютная коробка с заданными краями места в строке не
            // занимает — `atom_element` вернул пустышку нулевого размера.
            // Атомом её отдавать нельзя: атом уводит абзац с текстового пути
            // в ряд, и содержащим блоком абсолюта становится коробка ВСЕГО
            // абзаца, а §10.1 п.4 требует прямоугольник фрагментов строчного
            // предка. `Overlay` абзац с текстового пути не уводит.
            if matches!(
                e.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) && !at_static_position(&e.style)
                && !matches!(
                    e.tag.as_str(),
                    "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
                )
            {
                return inline::Piece::Overlay(el);
            }
            inline::Piece::Atom(el)
        })
    };
    let mut pieces = inline::collect(nodes, inherited, &mut atom);
    if pieces.is_empty() {
        return div().into_any_element();
    }
    // Ряд пробелов через границу строчной коробки — один пробел (§16.6.1).
    // Проход идёт ПЕРВЫМ: всё дальше считает байтовые смещения по готовому
    // тексту кусков.
    inline::collapse_across_pieces(&mut pieces);
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
    // Ряда из слов под отступ первой строки больше нет: `lines::rules` отдаёт
    // правила переноса ВСЕГДА, и своя раскладка строк умеет и отступ, и
    // отрицательный отступ.
    if inline::single_block(&pieces, opts.base_size())
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
            //
            // ПРОБОВАЛИ И ОТКАТИЛИ: считать долю от кегля САМОГО блока
            // (струт §10.8), раз крупный кусок теперь растит строку каналом
            // `lh_spans`. Замерено: приобретено 4, потеряно 4 — три пары
            // `*-applies-to-008` уходят с 0.02 на 0.67. Возвращать вместе с
            // разбором `vertical-align: top/bottom` на тексте.
            let line = match inherited.line_height {
                Some(Len::Px(v)) => gpui::px(v),
                Some(Len::Pct(k)) => gpui::px(k * biggest),
                Some(Len::Em(k)) => gpui::px(k * biggest),
                // Своей `line-height` у блока нет — её задают КУСКИ: у куска
                // со своей высотой строки она и берётся, у остальных доля от
                // кегля (§10.8.1). Канал `lh_spans` умеет строку только
                // растить, и объявленная `font: 100px/1` терялась.
                _ => gpui::px(inline::max_line_height(
                    &pieces,
                    own_size(inherited, opts),
                    opts.base_size(),
                    normal_fraction(inherited, opts),
                )),
            };
            if {
                static ON: std::sync::LazyLock<bool> =
                    std::sync::LazyLock::new(|| std::env::var("PLAIN_DBG").is_ok());
                *ON
            } && inherited.preserve_newlines == Some(true)
            {
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
            .reversed_lines(inherited.lines_reversed == Some(true))
            .plaintext(
                inherited
                    .bidi_plaintext
                    .unwrap_or(false)
                    .then(|| {
                        inherited
                            .text_align
                            .unwrap_or(crate::computed::TextAlign::Start)
                    })
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
            .shift_spans(inline::shift_spans(&pieces, biggest, f32::from(line)))
            .lh_spans(inline::line_height_spans(
                &pieces,
                inherited,
                biggest,
                crate::metrics::normal_line(&inherited.font_family.clone().unwrap_or_default()),
            ))
            .rel_spans(inline::rel_spans(&pieces))
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
            .flow_shapes(
                inherited
                    .flow_shapes
                    .clone()
                    .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new()))),
            )
            .line_clamp(inherited.clamp_lines().map(|n| n as usize))
            .text_ellipsis(
                inherited.ellipsis == Some(true)
                    && inherited
                        .overflow_x
                        .is_some_and(|o| o != crate::computed::Overflow::Visible),
            )
            .overflow_marker(
                inherited.overflow_marker.clone(),
                Some(measure_font(inherited, opts)),
            )
            .text_fit(inherited.text_fit)
            .fit_parts(
                // Масштабируемы только интервалы в ДОЛЯХ кегля; `px` и `em`
                // (от вычисленного кегля) подбор не трогает.
                [inherited.letter_spacing, inherited.word_spacing]
                    .iter()
                    .all(|l| matches!(l, None | Some(Len::Pct(_)))),
                matches!(
                    inherited.line_height,
                    Some(Len::Px(_)) | Some(Len::Em(_)) | Some(Len::Ex(_)) | Some(Len::Ch(_))
                ),
            )
            .hyphen_char(inherited.hyphen_char.clone())
            .tab_stop(gpui::px(match inherited.tab_size_len {
                // Длина задаёт шаг НАПРЯМУЮ, ширина знака к ней не примешана.
                Some(Len::Px(v)) if v > 0.0 => v,
                _ => {
                    // Число — кратное ПОЛНОЙ ширины пробела: с letter-spacing
                    // и word-spacing (css-text-3 §tab-size,
                    // tab-size-spacing-001 — вскрылось честным calc(8ch+...)).
                    let spacing = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    inherited.tab_size.unwrap_or(8).max(1) as f32
                        * (crate::metrics::ch_ex_px(&family, biggest).0
                            + spacing(inherited.letter_spacing)
                            + spacing(inherited.word_spacing))
                }
            }))
            .overlays(inline::overlays(pieces))
            .selectable(id, opts.selection_color());
            return para.into_any_element();
        }
    }
    let mut render_text = |t: String, style: &Computed| -> AnyElement {
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("RT_DBG").is_ok());
            *ON
        } {
            eprintln!(
                "RT t={:?} rot={:?} lh={:?} fs={:?}",
                t.chars().take(3).collect::<String>(),
                style.rotated_line,
                style.line_height,
                style.font_size
            );
        }
        // Стоячие знаки в вертикальном письме (`text-orientation: mixed`,
        // CJK): набор идёт вертикальными формами шрифта — возможность `vert`
        // подставляет глиф, а продвижение берётся из его вертикальных метрик
        // (css-writing-modes-3 §7.3, реализация в DirectWrite-слое). Пока
        // только для кусков, стоячих ЦЕЛИКОМ: смешанный кусок потребовал бы
        // резки на прогоны по ориентации.
        let vert_style;
        let style = if style.rotated_line == Some(true)
            && style.sideways != Some(true)
            && t.chars().any(upright_in_mixed)
            && t.chars().all(|c| c.is_whitespace() || upright_in_mixed(c))
        {
            let mut s = style.clone();
            s.font_features.push(("vert".into(), 1));
            vert_style = s;
            &vert_style
        } else {
            style
        };
        // На кусок текста идут ТОЛЬКО текстовые свойства: фон, отступы и
        // рамка принадлежат абзацу целиком, а не каждому его слову.
        let d = apply(div(), &style.text_only())
            .max_w_full()
            .child(SharedString::from(t.clone()));
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("RT_DBG").is_ok());
            *ON
        } {
            let tag = t.chars().take(3).collect::<String>();
            return d
                .relative()
                .child(
                    gpui::canvas(
                        |_, _, _| {},
                        move |b: gpui::Bounds<gpui::Pixels>, _, _, _| {
                            eprintln!("RTB {:?} bounds={:?}", tag, b);
                        },
                    )
                    .absolute()
                    .size_full(),
                )
                .into_any_element();
        }
        d.into_any_element()
    };
    // Начальное значение `text-align` — `start`, а он при письме справа налево
    // означает ПРАВЫЙ край. Без этого ряд из слов оставался слева, и строка
    // расходилась с абзацем-соседом.
    let align = inherited
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(inherited.rtl == Some(true));
    inline::as_wrapped_row(
        pieces,
        inherited.vertical_align,
        Some(align),
        match inherited.text_indent {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        },
        inherited.nowrap == Some(true),
        &mut render_text,
    )
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
        // Строчный контейнер лунок — атом в строке, как inline-grid
        // (grid-lanes-align-content-001: четыре сетки стоят В РЯД).
        Some(Display::GridLanes) => e.style.lanes_inline,
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
        // Трансформы поля формы шли МИМО обёртки: инпуты стояли ровно, а
        // эталон сдвигал (transform-input-001..019).
        return Some(transformed(el, &e.style));
    }
    // Абсолютный элемент без заданных краёв стоит на СТАТИЧЕСКОЙ позиции — там,
    // где он оказался бы в потоке. Внутри строки это место знает только сама
    // строка, поэтому в неё встаёт пустышка нулевого размера, а элемент висит
    // от её угла. Без этого раскладка ставила его в угол ближайшего
    // позиционированного предка, и текст уезжал в начало абзаца.
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (01.09): пускать сюда коробку с ОДНИМ заданным
    // краем — §10.6.4 решает оси независимо, и `left: 0` не должен отменять
    // статическое разрешение `top: auto` (`abspos-009`: зелёный на 37.6
    // точки выше эталона). Занятую ось при этом сообщал `fixed_axes`.
    // Срез из 1691 пары абсолютов: зелёных 1620 → 1596. Вся потеря —
    // замещаемые (`absolute-replaced-height-010/013/017/020/024/027/031/034`,
    // `-width-022..025` из 0.00-0.18 в 1.9-7.9): у них пустышка нулевая, и
    // размер по свободной оси считается уже не от содержащего блока.
    // Возвращать вместе с ненулевой распоркой по занятой оси.
    if at_static_position(&e.style) {
        let mut merged = inline::inherit(inherited, &e.style);
        // Позиционирование с внутренней коробки СНИМАЕТСЯ. Содержащим блоком
        // ей стала бы нулевая пустышка, а ширина у неё «по содержимому» —
        // в нулевом блоке это ноль, и элемент пропадал вовсе. Без
        // позиционирования она меряется своим содержимым и висит от угла
        // пустышки, то есть ровно от статической позиции.
        merged.position = None;
        merged.abs_static = true;
        // Замещаемый элемент строит своя ветка: голая коробка со стилем
        // теряла содержимое (сломанная картинка с alt-подписью пропадала,
        // `abs-pos-vlr-border-001`). Позиция снимается копией — та же
        // причина, что и у merged ниже.
        let replaced: Option<AnyElement> = match e.tag.as_str() {
            "img" | "svg" => {
                let mut copy = e.clone();
                copy.style.position = None;
                // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4), а
                // копия несёт только собственный стиль элемента: без этой
                // строки `image-orientation: none`, заданный на `body`, до
                // картинки не доезжает и EXIF-разворот применяется всё равно.
                copy.style.image_orient_none = merged.image_orient_none;
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
            Some(Display::GridLanes) => e.style.lanes_inline,
            // Блокифицированный `display: inline` (§9.7) для СТАТИЧЕСКОЙ
            // позиции остаётся строчным: гипотеза §10.3.7 считается без
            // блокификации (htb-rtl-*).
            Some(_) => e.style.inline_display == Some(true),
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
        // ПРОБОВАЛИ И ОТКАТИЛИ: считать «растянутым» и элемент с ДОЛЕЙ
        // размера, чтобы доля не бралась от нулевой пустышки. Замерено по
        // семьям *replaced*, positioning/*, normal-flow/*, *float*: 0 и 0.
        let stretched = (edge_set(e.style.inset.left) && edge_set(e.style.inset.right))
            || (edge_set(e.style.inset.top) && edge_set(e.style.inset.bottom))
            || matches!(e.style.width, Some(Len::Pct(_)));
        // Замещаемый элемент строит своя ветка: дети `<svg>` — не блоки,
        // путь блоков давал пустую коробку (clip-path-ellipse-2-ref: рисунок
        // absolute с left/top не рисовался вовсе).
        // Тот же список, что и у обычного пути: у замещаемых детей-блоков
        // нет, и блочная ветка давала пустую коробку — картинка с краями
        // не рисовалась вовсе.
        if matches!(
            e.tag.as_str(),
            "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
        ) {
            let mut copy = e.clone();
            copy.style.image_orient_none = merged.image_orient_none;
            copy.style.position = None;
            // Поля несёт ДЕРЖАТЕЛЬ: снимали только позицию, и `margin`
            // прикладывался дважды — раз держателем, раз внутренней коробкой
            // (`absolute-replaced-width-050`: край 48 превращался в 72).
            copy.style.margin = Default::default();
            // Долю размера держатель тоже несёт сам: внутри она считалась ОТ
            // НЕГО и выходила долей от доли — `width: 50%` давало четверть
            // содержащего блока (`absolute-replaced-width-006`). Внутренней
            // коробке остаётся заполнить держателя.
            if matches!(copy.style.width, Some(Len::Pct(_))) {
                copy.style.width = Some(Len::Pct(1.0));
            }
            if matches!(copy.style.height, Some(Len::Pct(_))) {
                copy.style.height = Some(Len::Pct(1.0));
            }
            let built = if e.tag == "svg" {
                crate::svg::element(&copy).unwrap_or_else(|| image(&copy))
            } else {
                image(&copy)
            };
            let holder = styled_div_with(e, &merged).child(built);
            // Замещаемый атом БЕЗ позиционированного предка считается от
            // начального содержащего блока (§10.1 п.4), а не от строки, где он
            // написан: с обеими заданными осями место в строке ему не нужно
            // вовсе. Тот же приём, что у блочного пути (`to_icb`).
            let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
            let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
            // РОВНО ОДНА заданная ось: по ней коробку ставит край, по
            // свободной — статическая позиция в строке (§10.3.7 п.1/§10.6.4
            // п.1, оси независимы). Прежде такая коробка сидела в нулевом
            // держателе, стоявшем в статической точке по ОБЕИМ осям, и край
            // ПРИБАВЛЯЛСЯ к статической координате вместо того, чтобы её
            // заменить (`probe/svpf.html`: `left: 80` после «34» давало 240,
            // а не 80). Щуп в строке отдаёт дырку, `LatePlace` обнуляет сдвиг
            // по заданной оси и ставит свободную в дырку — ровно как у коробки
            // без краёв, только с `fixed_axes`.
            if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    hole: None,
                    next_line: None,
                    fixed_axes: (x_set, y_set),
                    line_thickness: line_height_px(inherited, opts),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    ..Default::default()
                });
                let probe = crate::interact::spot_probe(spot.clone(), false);
                return match crate::interact::late_push(spot, holder.into_any_element()) {
                    None => Some(probe),
                    Some(kept) => {
                        let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                        hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                        Some(hole.child(kept).into_any_element())
                    }
                };
            }
            if x_set
                && y_set
                && e.style.z_index.unwrap_or(0) >= 0
                && !(inherited.cb_ancestor || crate::inline::establishes_cb(inherited))
            {
                let spot: crate::interact::SpotCell = Default::default();
                spot.set(crate::interact::Spot {
                    fixed_axes: (true, true),
                    rtl: inherited.rtl == Some(true),
                    vertical: inherited.vertical == Some(true),
                    vertical_rl: inherited.vertical_rl == Some(true),
                    own_vertical: e.style.vertical == Some(true),
                    ..Default::default()
                });
                // Слоя нет — элемент возвращается назад, и рисуем его на
                // месте прежним путём.
                match crate::interact::icb_push(spot, holder.into_any_element()) {
                    None => {
                        return Some(div().w_0().h_0().flex_shrink_0().into_any_element());
                    }
                    Some(kept) => {
                        return Some(
                            div()
                                .w_0()
                                .h_0()
                                .flex_shrink_0()
                                .child(kept)
                                .into_any_element(),
                        );
                    }
                }
            }
            return Some(if stretched {
                holder.into_any_element()
            } else {
                div()
                    .w_0()
                    .h_0()
                    .flex_shrink_0()
                    .child(holder)
                    .into_any_element()
            });
        }
        let inner = styled_div_with(e, &merged).children(blocks(&e.children, &merged, opts));
        // Незамещаемая коробка с РОВНО ОДНОЙ заданной осью — тем же щупом, что
        // и замещаемая выше: без этого `<span>` с одним краем падал в нулевой
        // держатель, и край считался от него (`abs-pos-non-replaced-vlr-017`:
        // `right: 2em` уводил коробку на свои же 160 влево).
        let x_set = edge_set(e.style.inset.left) || edge_set(e.style.inset.right);
        let y_set = edge_set(e.style.inset.top) || edge_set(e.style.inset.bottom);
        if x_set != y_set && e.style.z_index.unwrap_or(0) >= 0 {
            let spot: crate::interact::SpotCell = Default::default();
            spot.set(crate::interact::Spot {
                hole: None,
                next_line: None,
                fixed_axes: (x_set, y_set),
                line_thickness: line_height_px(inherited, opts),
                rtl: inherited.rtl == Some(true),
                vertical: inherited.vertical == Some(true),
                vertical_rl: inherited.vertical_rl == Some(true),
                own_vertical: e.style.vertical == Some(true),
                ..Default::default()
            });
            let probe = crate::interact::spot_probe(spot.clone(), false);
            return match crate::interact::late_push(spot, inner.into_any_element()) {
                None => Some(probe),
                Some(kept) => {
                    let mut hole = div().relative().w_0().h_0().flex_shrink_0();
                    hole.style().align_self = Some(gpui::AlignItems::FlexStart);
                    Some(hole.child(kept).into_any_element())
                }
            };
        }
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
    // Многоколоночная коробка В СТРОКЕ (`inline-block` с `column-count`) —
    // такой же случай, как строчная таблица ниже: своя раскладка живёт в
    // блочном пути, а сюда приходил голый `blocks()`, и колонок не
    // получалось вовсе — шесть детей вставали одним столбцом. Так красными
    // были ЭТАЛОНЫ семьи `column-grid-lanes-container-baseline-*`: сам тест
    // на лунках рисуется верно, а эталон написан на многоколоннике в
    // `display: inline-block`.
    //
    // Ширину сжатой коробке даём по css-multicol-1 §3.4: когда заданы и
    // число колонок, и их ширина, использованная ширина коробки —
    // `count * width + (count - 1) * gap`. Без этого колонки делили бы
    // ширину содержимого строки. Умолчание `column-gap: normal` — кегль
    // (css-align §8.3), как и в блочном пути.
    //
    // ЗАМЕРЕНО: правка верна по пробе, но своды не двигает — CSS2
    // 5343 -> 5343 и CSS3 2420 -> 2420, ноль приобретено, ноль потеряно:
    // эталоны этой семьи держит ещё и выравнивание по базовой линии.
    // Лунки сетки — та же история: раскладку лунок строит только блочный
    // путь, а строчный отдавал голый `blocks()`, и `display: inline
    // grid-lanes` терял лунки целиком.
    if e.style.display == Some(Display::GridLanes) {
        return Some(element(e, inherited, opts));
    }
    if multicol_container(&e.style) {
        let merged = inline::inherit(inherited, &e.style);
        let gap = match e.style.column_gap {
            Some(Len::Px(v)) => v,
            _ => match merged.font_size {
                Some(Len::Px(size)) => size,
                _ => opts.base_size(),
            },
        };
        let shrink_to_fit = match (e.style.column_count, e.style.column_width) {
            (Some(c), Some(Len::Px(w))) if w > 0.0 => Some(c as f32 * w + (c as f32 - 1.0) * gap),
            _ => None,
        };
        if e.style.width.is_some() || shrink_to_fit.is_some() {
            let mut copy = e.clone();
            if copy.style.width.is_none() {
                copy.style.width = shrink_to_fit.map(Len::Px);
            }
            return Some(element(&copy, inherited, opts));
        }
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
        "img" => Some(image_with(
            &with_inherited_font(&pct_height_to_px(e, inherited), inherited),
            Some(atom_base_font(inherited, opts)),
        )),
        // Замещаемые с адресом в СВОЁМ атрибуте: у блочного пути такие рукава
        // есть, у строчного не было, и `<object data>` в строке терял
        // собственный размер (§10.3.2, §10.6.2) — коробки не заводил и уходил
        // прогоном запасного текста. Отсечка по атрибуту обязательна: объект
        // без `data` и видео без `poster` замещаемыми не являются.
        "embed" if e.attr("src").is_some() => Some(image_with(
            &with_inherited_font(e, inherited),
            Some(atom_base_font(inherited, opts)),
        )),
        "object" if e.attr("data").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            Some(image_with(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
            ))
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            Some(image_with(
                &with_inherited_font(&copy, inherited),
                Some(atom_base_font(inherited, opts)),
            ))
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): кадру без пригодного `src` давать
        // замещаемую коробку 300×150 (CSS 2.2 §10.3.2) или по атрибутам.
        // Срез из 116 пар с `<iframe>`: 83 -> 73, приобретено 2
        // (`flexbox-basic-iframe-horiz-001`, `stretch-anonymous-block-001`),
        // потеряно 12 — `inline-block-replaced-height-004/005/007`,
        // `inline-replaced-height-004/005/007` уходят в «красное видно»,
        // `contain-size-replaced-003a..d` 0.26 -> 1.42. Пустая коробка кадра
        // ломает высоту строки у соседей: замещаемому нужен ещё и правильный
        // вклад в строку, а не только размер.
        "iframe" => {
            if let Some(el) = iframe(e, opts) {
                return Some(el);
            }
            None
        }
        "svg" => crate::svg::element(e).or_else(|| {
            Some(image_with(
                &with_inherited_font(e, inherited),
                Some(atom_base_font(inherited, opts)),
            ))
        }),
        // Свой бокс (фон, рамка, отступы) означает, что кусок не может быть
        // прогоном текста: прогон не умеет рисовать вокруг себя рамку.
        _ if has_own_box(
            &e.style,
            match inherited.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            },
        )
            // Инлайн с СОБСТВЕННЫМ письмом, отличным от родителя, — по
            // css-writing-modes-4 §3.1 «its display computes instead to
            // inline-block»: коробка со своими размерами
            // (`different-block-flow-dir-001/002`).
            || (e.style.vertical.is_some() && e.style.vertical != inherited.vertical) =>
        {
            let merged = inline::inherit(inherited, &e.style);
            let mut box_ = styled_div_with(e, &merged);
            // Строчная коробка БЕЗ содержимого всё равно высотой в строку:
            // рамка и фон рисуются по кеглю, а не по тексту. Без этого
            // `<span style="border-left:30px solid green">  </span>` выходил
            // нулевой высоты и не рисовался вовсе
            // (`line-edge-white-space-collapse-001`).
            // Обособленная блочная ось высоту уже задала (пусть нулевую) —
            // подставлять кегль строки поверх неё нельзя.
            // …и только у НАСТОЯЩЕГО строчного: у `inline-block` высота идёт от
            // содержимого, и подставленный кегль строки накрывал детей-блоков
            // (`border-left-width-applies-to-012`: квадрат 96 выходил 19).
            let genuine_inline = e.style.display.is_none() || e.style.inline_display == Some(true);
            if genuine_inline
                && merged.height.is_none()
                && !has_text(&e.children)
                && !merged.contains_height()
            {
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
            // ★ ЗАМЕРЕНО, ЭФФЕКТА НЕТ (01.09): открывать здесь слой содержащего
            // блока, как это делает блочный путь (`cb_open`/`cb_close` ниже по
            // файлу), чтобы абсолютный ребёнок строчного `position: relative`
            // не уезжал к внешнему содержащему блоку. Срез из 229 пар:
            // 205 → 205 при гейте «только настоящая строчная коробка», и
            // 205 → 197 без него (`position-relative-table-{tbody,thead,tfoot,
            // tr}-*-absolute-child` уходили 0.00 → «красное видно»).
            // До `position-absolute-in-inline-*` правка НЕ доезжает: там у
            // строчного нет своей коробки, он идёт прогоном текста, и вешать
            // слой не на что — чинить надо в сборке прогонов.
            // `inline-block` с ВЕРТИКАЛЬНЫМ письмом — контейнер блоков со своей
            // осью блочного потока (css-writing-modes-4 §3.1): дети-блоки идут
            // колонками справа налево (`vertical-rl`) или слева направо
            // (`block-flow-direction-vrl-011`). Путь атома минует общий гейт в
            // `element()`, поэтому ось ставится здесь.
            let mut merged = merged;
            if e.style.display == Some(Display::InlineBlock)
                && merged.vertical == Some(true)
                && e.children.iter().any(|n| {
                    matches!(n, Node::Element(k)
                        if !k.inline || matches!(k.style.display, Some(Display::Block)))
                })
            {
                // Предел строчной оси детей — собственная высота коробки (как
                // `element()` сеет `ortho_limit` от высоты предка): без него
                // колонки тянулись за низ (`block-flow-direction-vrl-012`).
                let em_base = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => Some(v),
                    Some(Len::Em(k)) => Some(k * em_base),
                    _ => None,
                };
                if let Some(h) = px_of(e.style.height) {
                    let b = e.style.borders();
                    let edges = if e.style.border_box == Some(true) {
                        px_of(b.top).unwrap_or(0.0)
                            + px_of(b.bottom).unwrap_or(0.0)
                            + px_of(e.style.padding.top).unwrap_or(0.0)
                            + px_of(e.style.padding.bottom).unwrap_or(0.0)
                    } else {
                        0.0
                    };
                    merged.ortho_limit = Some((h - edges).max(0.0));
                }
                box_ = box_.flex();
                box_ = if merged.vertical_rl == Some(true) {
                    box_.flex_row_reverse()
                } else {
                    box_.flex_row()
                };
            }
            // Маска, обрезка формой и фильтр действуют и на СТРОЧНУЮ коробку
            // (css-masking §1: `clip-path` применяется ко всем элементам):
            // блочный путь заворачивает её в буфер группы, а атомный шёл
            // мимо, и `clip-path` на `inline-block` не резал ничего
            // (`clip-path-contentBox-1d/1e`). Обёртка сама возвращает
            // элемент как есть, когда группировать нечего.
            Some(grouped(
                box_.children(blocks(&e.children, &merged, opts))
                    .into_any_element(),
                &e.style,
            ))
        }
        // `<ruby>` — база с НАДСТРОЧНОЙ аннотацией (css-ruby-1 §2): `<rt>`
        // рисуется над своей базой кеглем в половину, `<rp>` — только для
        // движков без поддержки руби и не показывается. Собирается атомом:
        // колонка «аннотация над базой», выключенная по центру. Строка
        // растёт сама — атом выше её обычной высоты.
        "ruby" => {
            let mut merged = inline::inherit(inherited, &e.style);
            // Внутри руби знак акцента больше не разворачивается: сам знак
            // — уже надпись, и рекурсия ушла бы в бесконечность.
            merged.text_emphasis = None;
            let mut base: Vec<Node> = Vec::new();
            let mut over: Vec<Node> = Vec::new();
            for c in &e.children {
                match c {
                    Node::Element(k) if k.tag == "rt" => over.push(c.clone()),
                    Node::Element(k) if k.tag == "rp" => {}
                    other => base.push(other.clone()),
                }
            }
            if over.is_empty() {
                return None;
            }
            let mut ann = merged.clone();
            // Кегль аннотации — половина базового (умолчание браузеров).
            let font = match merged.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            ann.font_size = Some(Len::Px(font * 0.5));
            ann.line_height = Some(Len::Px(font * 0.5));
            let ann_el = div()
                .flex()
                .flex_row()
                .children(blocks(&over, &ann, opts))
                .into_any_element();
            let base_el = div()
                .flex()
                .flex_row()
                .children(blocks(&base, &merged, opts))
                .into_any_element();
            let col = div().flex().flex_col().items_center().flex_shrink_0();
            // `ruby-position: under` и знак акцента снизу ставят надпись ПОД
            // базой (css-ruby-1 §4.1, css-text-decor-3 §5.2).
            let col = if merged.emphasis_under {
                col.child(base_el).child(ann_el)
            } else {
                col.child(ann_el).child(base_el)
            };
            Some(col.into_any_element())
        }
        // `<canvas>` — замещаемый элемент с собственными размерами 300x150
        // по умолчанию (HTML §4.12.5); рисовать в нём нечего, но место он
        // занимает и фон несёт. Своего рукава у него не было, и голый
        // `<canvas>` с одним лишь фоном пропадал из строки целиком: коробки
        // фон не заводит (`has_own_box`), а строчный путь возвращал `None`.
        // Размеры уже проставлены разбором (`dom.rs`), здесь нужна коробка.
        // ЗАМЕРЕНО И ОТКАЧЕНО (04.09): не переносить атрибуты холста в
        // `style.width/height` (по HTML §15.3.10 холста в списке
        // «dimension attributes» нет — атрибуты дают ПРИРОДНЫЙ размер), а
        // ставить коробке природный размер и соотношение сторон здесь. Срез
        // из 169 пар с `<canvas>`: 54 -> 52, приобретено 2
        // (`replaced-alignment-with-aspect-ratio-002`,
        // `percent-height-replaced-in-percent-cell-003`), потеряно 4 —
        // `flexbox-flex-basis-content-001a/002a` 0.00 -> 1.9,
        // `contain-size-replaced-003a`, `replaced-content-spanner-auto-width`
        // в «красное видно». На заданный размер холста опираются
        // `flex-basis: content`, `contain: size` и спаннер многоколоночника;
        // возвращать вместе с ними.
        "canvas" => {
            let merged = inline::inherit(inherited, &e.style);
            Some(styled_div_with(e, &merged).flex_shrink_0().into_any_element())
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
/// Замещаемый элемент (css-display-3 §2.4): размер даёт содержимое, а не
/// раскладка детей.
fn replaced_tag(e: &Element) -> bool {
    matches!(
        e.tag.as_str(),
        "img" | "svg" | "canvas" | "video" | "embed" | "object" | "iframe" | "input"
    )
}

fn content_sized_wraps(c: &Computed) -> bool {
    let keyword = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    // Обособленная ось содержимого не видит: размер по нему заменяется
    // `contain-intrinsic-size` (css-contain-2 §size-containment), и мерить
    // дорожкой сетки больше нечего.
    let contained =
        (keyword(c.width) && c.contains_width()) || (keyword(c.height) && c.contains_height());
    if contained {
        return false;
    }
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
    let (col, row) = (
        (!c.contains_width()).then(|| track(c.width)).flatten(),
        (!c.contains_height()).then(|| track(c.height)).flatten(),
    );
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
    // Авто-поля прижимают КОРОБКУ в свободном месте (CSS 2.1 §10.3.3):
    // `margin-left: auto` при точечном max-content — прижим вправо
    // (align-baseline-ref: правый столбец текста уезжал влево).
    let auto = |l: Option<Len>| matches!(l, Some(Len::Auto));
    wrap.style().justify_items = Some(match (auto(c.margin.left), auto(c.margin.right)) {
        (true, false) => gpui::AlignItems::FlexEnd,
        (true, true) => gpui::AlignItems::Center,
        _ => gpui::AlignItems::FlexStart,
    });
    // Выравнивание элемента поперёк РОДИТЕЛЯ переезжает на обёртку: во
    // флексе родителя стоит она, и без переноса `justify-items: center` в
    // лунках глох на min-content-элементах
    // (column-fill-reverse-justify-items-001). В вертикальном письме
    // align-self несёт ось самого движка — перенос ломал ортогональные
    // потоки (three-levels-of-orthogonal-flows).
    if let Some(a) = c.align_self.filter(|_| c.vertical != Some(true)) {
        wrap.style().align_self = Some(match a {
            Align::Center => gpui::AlignItems::Center,
            Align::Start => gpui::AlignItems::FlexStart,
            Align::End => gpui::AlignItems::FlexEnd,
            Align::Baseline => gpui::AlignItems::Baseline,
            Align::Stretch => gpui::AlignItems::Stretch,
        });
    }
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
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: снять вето с доли по ИНЛАЙНОВОЙ оси (оставив
    // его только блочной) — по спеке прямоугольник статической позиции
    // нулевой лишь по одной оси (css-position-3, Blink `absolute_utils.cc`).
    // Срез позиционирования (1919 пар, 1781 зелёная): 1776. Приобретений
    // ноль, потери — `absolute-replaced-width-006/013/020` (0.00 → 1.92-3.84)
    // и `margin-bottom-103/104` (0.19 → «красное видно»). Значит доля ширины у
    // нас решается ОТ РАСПОРКИ, а не от содержащего блока: сперва нужен
    // настоящий прямоугольник статической позиции, потом снятие вето.
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
        && !edge_set(c.inset.top)
        && !edge_set(c.inset.right)
        && !edge_set(c.inset.bottom)
        && !edge_set(c.inset.left)
}

/// Обрывает ли `clear` обтекание со стороны `side` (-1 слева, 1 справа).
///
/// `clear: left` правый флоат не трогает и наоборот (CSS 2.1 §9.5.2);
/// прежде `clear` был двузначным, и любая сторона обрывала любой ряд.
fn clears_side(clear: Option<i8>, side: i8) -> bool {
    matches!(clear, Some(c) if c == 0 || c == side)
}

/// Есть ли ДАЛЬШЕ по разметке позиционированный элемент, который останется на
/// месте.
///
/// Слой начального содержащего блока дописывается последним ребёнком
/// документа, поэтому вынесенный рисуется поверх всего, что осталось в потоке.
/// По CSS 2.1 §9.9 шаг 8 позиционированные с `z-index: auto` рисуются В
/// ПОРЯДКЕ РАЗМЕТКИ: сосед, стоящий ПОСЛЕ, обязан лечь СВЕРХУ. Пока он
/// остаётся на месте, вынос переворачивает пару местами.
///
/// Сосед, который сам уйдёт в слой, порядок НЕ ломает: слой копится в порядке
/// сборки. `fixed` не считается: он и так рисуется отложенно, поверх всего.
fn stays_positioned(rest: &[Node]) -> bool {
    fn walk(nodes: &[Node], under_cb: bool) -> bool {
        nodes.iter().any(|n| {
            let Node::Element(e) = n else { return false };
            let pos = e.style.position;
            let positioned = matches!(
                pos,
                Some(crate::computed::Position::Relative)
                    | Some(crate::computed::Position::Sticky)
                    | Some(crate::computed::Position::Absolute)
            );
            // Тот же предикат, что и у выноса: такой сосед уедет в слой, и
            // взаимный порядок сохранится.
            let hoisted = pos == Some(crate::computed::Position::Absolute)
                && !under_cb
                && e.style.z_index.unwrap_or(0) >= 0
                && (edge_set(e.style.inset.left)
                    || edge_set(e.style.inset.right)
                    || edge_set(e.style.inset.top)
                    || edge_set(e.style.inset.bottom));
            if positioned && !hoisted {
                return true;
            }
            walk(
                &e.children,
                under_cb || crate::inline::establishes_cb(&e.style),
            )
        })
    }
    walk(rest, false)
}

/// Задан ли край позиционированного элемента.
///
/// `left: auto` — это ОТСУТСТВИЕ края (CSS 2.1 §9.3.2: начальное значение
/// `auto`), а разбор даёт на него `Some(Len::Auto)`. Проверка `is_some()`
/// читала явный `auto` как заданный край, и элемент терял статическую
/// позицию: `abspos-*-applies-to-*` вставали в угол содержащего блока
/// вместо своего места в потоке.
fn edge_set(l: Option<Len>) -> bool {
    !matches!(l, None | Some(Len::Auto))
}

/// Есть ли у инлайнового куска собственная коробка.
///
/// Прогон текста не умеет рисовать вокруг себя ничего: ни рамку, ни тень, ни
/// отступ. Раньше проверялись только верх и лево, поэтому `padding-right`,
/// боковая рамка, тень, прозрачность и три угла из четырёх у `<span>` молча
/// пропадали. У настоящего `display: inline` размеры не проверяются: CSS их
/// такому элементу и не даёт.
fn has_own_box(c: &Computed, font_px: f32) -> bool {
    // Нулевая величина коробки не создаёт: `padding: 0` и `border: 0` пишут
    // в стиль ноль, и по одному лишь «задано» кусок вынимался из строки —
    // а вынутый кусок рвёт соединение букв и общий перенос по словам.
    let set = |l: &Option<Len>| !matches!(l, None | Some(Len::Px(0.0)) | Some(Len::Pct(0.0)));
    let any =
        |s: &crate::computed::Sides| set(&s.top) || set(&s.right) || set(&s.bottom) || set(&s.left);
    // Вертикальные поля признаком коробки НЕ служат: строку они не двигают
    // (замерено на `flexbox_inline`, где `margin-top: -20em` обязан пройти
    // впустую), и по ним коробка заводилась бы только затем, чтобы уехать за
    // экран.
    // Атомарная строчная коробка — коробка по определению: у неё свои ширина,
    // высота и вертикальные поля, а прогон текста не умеет ни одного из трёх.
    //
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: раньше коробкой считался только атом С
    // ЗАДАННЫМ размером, а безразмерный оставался прогоном — под флагом
    // `ATOM_BOX`, потому что прошлый замер давал flexbox 363→359 (атом-коробка
    // не отдавала строке базовую линию содержимого). Сейчас не
    // воспроизводится: CSS2 5039 → 5043, oldfront 2345 без изменений. Внутри
    // такой коробки живут отступ первой строки, сжатие по содержимому и свой
    // перенос — прогон их не знает.
    // Настоящий `display: inline` сюда НЕ входит: разбор держит его как
    // `InlineBlock` с пометкой `inline_display`, и без этой отсечки каждый
    // `<span>` становился атомарной коробкой — вместе с ней уезжали
    // сохранённые пробелы и перенос (`white-space-pre-005`).
    let atomic = (matches!(
        c.display,
        Some(Display::InlineBlock)
            | Some(Display::InlineFlex)
            | Some(Display::InlineGrid)
            | Some(Display::InlineTable)
    ) && c.inline_display != Some(true))
        || (c.display == Some(Display::GridLanes) && c.lanes_inline);
    // ПЕРЕМЕРЕНО 28.08 и ВКЛЮЧЕНО: прежний замер (flexbox 363→359 из-за
    // непрокинутой базовой линии атома) больше не воспроизводится — CSS2
    // 5039 → 5043, oldfront 2345 без изменений. Флаг `ATOM_BOX`, под которым
    // проба жила, снят.
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
    // ЗАМЕРЕНО И ОТКАЧЕНО: отдавать прогону ТОЛЬКО ровную рамку, а рамку с
    // разными гранями уводить в коробку. По семьям `borders/*` и `css1/*brdr*`
    // это +19/−1, но по всему CSS2 — 4917 → 4906: семьи `bidi-*` теряют 33
    // пары, потому что кусок, разорванный переносом, коробке не даётся.
    // Порог «все ЗАДАННЫЕ грани одной толщины» тоже замерен: +13/−14
    // (`border-top-width-0NN` уходят в прогон и там ложатся мимо). Настоящая
    // развилка — геометрия полосы: 1.16 кегля вместо подъёма и спуска шрифта,
    // и рамка внутрь вместо наружу. Возвращаться вместе с ней.
    let inline_level = c.display.is_none();
    if inline_level
        && (crate::inline::uniform_border(c, font_px).is_some()
            || crate::inline::sided_border(c, font_px).is_some())
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

/// Обтекание плавающих блоков ФОРМОЙ (`shape-outside`, css-shapes-1 §2).
///
/// Плавающие дети встают absolute у своей стороны, остальным строится
/// обычный поток, но абзацам передаются ВЫРЕЗЫ — формы в координатах от
/// верха потока; каждая строка абзаца сужается по своей высоте. Формула
/// формы считается от выбранной опорной коробки (по умолчанию margin-box),
/// затем переводится в координаты margin-box (позиция флоата).
fn shape_flow(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let px_of = |l: &Option<Len>| match l {
        None => 0.0,
        Some(Len::Px(v)) => *v,
        _ => 0.0,
    };
    let mut left: Vec<crate::flow::FloatShape> = Vec::new();
    let mut right: Vec<crate::flow::FloatShape> = Vec::new();
    // Ширина содержащего блока: от неё считаются доли формы и поля
    // (`shape-margin: 5%`), она же — дальний край для правила 7 §9.5.1.
    // Известна только точками: непроходную единицу `px_of` глушит в ноль.
    let cb_w = px_of(&e.style.width);
    // Стенка РАЗМЕЩЕНИЯ. Без известной ширины переноса флоатов здесь нет
    // совсем, а вертикальное письмо не переносит никогда — полосам в обоих
    // случаях ставится заведомо недостижимая стенка. 8192 = 2^13: обратный
    // перевод правого края в отступ от своей стороны (`wall - fx - mw`)
    // остаётся точным до 2^-11 точки, на два порядка точнее допуска полос.
    const NO_WALL: f32 = 8192.0;
    let wall = if cb_w > 0.0 && inherited.vertical_rl != Some(true) {
        cb_w
    } else {
        NO_WALL
    };
    // Полосы занятости (CSS 2.1 §9.5.1) держат ПРЯМОУГОЛЬНУЮ занятость
    // margin-box и отвечают только за размещение. Точная форма выреза
    // (`shape-outside`) в полосы не попадает вовсе и идёт отдельными
    // списками `left`/`right`: форма меняет область ОБТЕКАНИЯ, но не
    // позицию самого флоата (css-shapes-1 §1).
    let mut bands = crate::bands::FloatBands::new(wall);
    let mut floats: Vec<AnyElement> = Vec::new();
    let mut rest: Vec<Node> = Vec::new();
    let host_side: i32 = if e.attr("side") == Some("right") {
        1
    } else {
        -1
    };
    let count: usize = e.attr("count").and_then(|c| c.parse().ok()).unwrap_or(0);
    for (idx, n) in e.children.iter().enumerate() {
        let Node::Element(f) = n else {
            rest.push(n.clone());
            continue;
        };
        if idx >= count {
            rest.push(n.clone());
            continue;
        }
        // Сторона — с САМОГО флоата: бандовый хост собирает пробег обеих
        // сторон и `float` с детей не снимает. Атрибут `side` остаётся
        // запасным ответом для живого пути `shape-outside`.
        let side = f
            .style
            .float
            .filter(|v| *v != 0)
            .map(i32::from)
            .unwrap_or(host_side);
        let b = f.style.borders();
        let (ml, mr) = (px_of(&f.style.margin.left), px_of(&f.style.margin.right));
        let (mt, mb) = (px_of(&f.style.margin.top), px_of(&f.style.margin.bottom));
        let (bl, br_) = (px_of(&b.left), px_of(&b.right));
        let (bt, bb) = (px_of(&b.top), px_of(&b.bottom));
        let (pl, pr) = (px_of(&f.style.padding.left), px_of(&f.style.padding.right));
        let (pt, pb) = (px_of(&f.style.padding.top), px_of(&f.style.padding.bottom));
        let (mut cw, mut chh) = (px_of(&f.style.width), px_of(&f.style.height));
        // `box-sizing: border-box` — заданная длина ВКЛЮЧАЕТ отступы и рамку
        // (css-ui-3 §5.1), а дальше здесь считается контентная. Без вычитания
        // опорная коробка выходила шире содержащего блока, и вырез уводил
        // строки в минус (`shape-outside-content-box-003`, `-padding-box-003`).
        if f.style.border_box == Some(true) {
            if cw > 0.0 {
                cw = (cw - pl - pr - bl - br_).max(0.0);
            }
            if chh > 0.0 {
                chh = (chh - pt - pb - bt - bb).max(0.0);
            }
        }
        // Флоат без своих размеров с картинкой-формой: размер — интринзик
        // картинки (частый паттерн shape-image-тестов).
        if cw <= 0.0
            && chh <= 0.0
            && let Some(raw0) = f.style.shape_outside.as_deref()
            && raw0.contains("url(")
            && let Some(u) = crate::computed::parse_url(raw0)
            && let Some(img) = crate::background::load(&u)
        {
            let sz = img.size(0);
            cw = sz.width.0 as f32;
            chh = sz.height.0 as f32;
        }
        let (mw, mh) = (
            ml + bl + pl + cw + pr + br_ + mr,
            mt + bt + pt + chh + pb + bb + mb,
        );
        let raw = f.style.shape_outside.clone().unwrap_or_default();
        // Опорная коробка формы: margin-box по умолчанию (css-shapes §3).
        let (bx, by, bw, bh) = if raw.contains("border-box") {
            (ml, mt, mw - ml - mr, mh - mt - mb)
        } else if raw.contains("padding-box") {
            (
                ml + bl,
                mt + bt,
                mw - ml - mr - bl - br_,
                mh - mt - mb - bt - bb,
            )
        } else if raw.contains("content-box") {
            (ml + bl + pl, mt + bt + pt, cw, chh)
        } else {
            (0.0, 0.0, mw, mh)
        };
        // Размещение по правилам 1-9 §9.5.1. `clear` сюда не доезжает:
        // группу и хвост `wrap_floats` рвёт на первом же `clear` своей
        // стороны.
        // `clear` берётся с самого флоата: у бандового хоста пробег на нём не
        // рвётся, и очистку исполняют полосы. На живом пути `shape-outside`
        // группа рвётся раньше, и `clear` там всегда `None`.
        let (fx, fy) = bands.add_float(side as i8, mw, mh, f.style.clear);
        // Форма выреза и держатель адресуются ОТ СВОЕЙ стороны, а полосы
        // считают обе границы от инлайн-начала: перевод здесь и только здесь.
        let off = if side < 0 { fx } else { wall - fx - mw };
        let sm = match f.style.shape_margin {
            Some(Len::Px(v)) => v,
            Some(Len::Pct(p)) => p * cb_w,
            _ => 0.0,
        };
        let shape = if let Some(at) = raw.find("circle(").or_else(|| raw.find("ellipse(")) {
            let inner = &raw[at..];
            let inner = match inner.find(')') {
                Some(end) => &inner[..=end],
                None => inner,
            };
            match crate::background::shape_params(inner, bw, bh, 1.0) {
                Some((cx, cy, rx, ry)) => {
                    // Координаты — от опорной коробки; переводим к margin-box.
                    let (cx, cy) = (cx + bx, cy + by);
                    let cx = if side < 0 { cx } else { mw - cx };
                    crate::flow::FloatShape::Ellipse {
                        top: 0.0,
                        cx: cx + off,
                        cy,
                        rx: rx + sm,
                        ry: ry + sm,
                    }
                }
                None => crate::flow::FloatShape::Band {
                    top: 0.0,
                    h: mh,
                    w: off + mw + sm,
                },
            }
        // ЗАМЕЧАНИЕ: особый путь картинки и градиента снят — общий
        // растровый путь строит ту же маску в content-box и с подключённым
        // `shape-margin` (см. `background::shape_profile`) раздувает её по
        // обеим осям, а особый раздувал только по горизонтали.
        } else {
            // Общий путь произвольной формы (css-shapes-1 §3): растровая
            // маска margin-box -> интервалы строк -> дилатация Минковского
            // диском shape-margin -> экстенты. Закрывает polygon (включая
            // evenodd), inset/rect/xywh С радиусами `round`, слово-коробку
            // с border-radius, path()/shape(), картинку и градиент с
            // вертикальным полем (план target/scout-dilation.md).
            let radius_of = |c: &Option<crate::value::Len>| match c {
                Some(crate::value::Len::Px(v)) => (*v, *v),
                Some(crate::value::Len::Pct(k)) => (k * bw, k * bh),
                _ => (0.0, 0.0),
            };
            let sb = crate::background::ShapeBox {
                mw,
                mh,
                rx: bx,
                ry: by,
                rw: bw,
                rh: bh,
                cx: ml + bl + pl,
                cy: mt + bt + pt,
                cw,
                ch: chh,
                radius: [
                    radius_of(&f.style.radius.tl),
                    radius_of(&f.style.radius.tr),
                    radius_of(&f.style.radius.br),
                    radius_of(&f.style.radius.bl),
                ],
                threshold: f.style.shape_threshold.unwrap_or(0.0),
            };
            match crate::background::shape_profile(&raw, &sb, sm.max(0.0), side) {
                Some(ext) => crate::flow::FloatShape::Profile {
                    top: 0.0,
                    ext: std::sync::Arc::new(
                        ext.into_iter()
                            .map(|v| if v > 0.0 { off + v } else { 0.0 })
                            .collect(),
                    ),
                },
                None => {
                    // Непонятная запись: прямоугольник опорной коробки со
                    // стороны текста.
                    let w_cut = if side < 0 { bx + bw } else { mw - bx };
                    crate::flow::FloatShape::Band {
                        top: by,
                        h: bh,
                        w: off + w_cut + sm,
                    }
                }
            }
        };
        let mut shape = shape;
        if fy > 0.0 {
            shape.shift_top(fy);
        }
        if side < 0 {
            left.push(shape);
        } else {
            right.push(shape);
        }
        // Сам флоат — absolute у своей стороны.
        let mut copy = f.clone();
        // Сторона и очистка уже прочитаны выше; гасить их надо ДО слияния:
        // у бандового хоста `float` доживает до сюда, а слитый стиль с
        // `float` заводит лишний контекст обрезки.
        copy.style.float = None;
        copy.style.clear = None;
        let mut merged = inline::inherit(inherited, &copy.style);
        // Поля кладёт держатель (позиция absolute от края) — на самой
        // коробке они сдвигали бы её обратно (float: right с margin-left
        // вылезал за правый край контейнера). Снимать их надо И СО СЛИТОГО
        // стиля: коробку строит он, и через него поле возвращалось —
        // четвёрка флоатов с `margin: 10px` уезжала на поле целиком
        // (`floats-014`).
        copy.style.margin = crate::computed::Sides::default();
        merged.margin = crate::computed::Sides::default();
        let built = if copy.tag == "img" {
            image(&copy)
        } else {
            styled_div_with(&copy, &merged)
                .children(blocks(&copy.children, &merged, opts))
                .into_any_element()
        };
        let holder = if inherited.vertical_rl == Some(true) {
            // Вертикальное письмо: блок-старт — ПРАВЫЙ край, колонки
            // флоатов идут влево; инлайн-старт — верх, а у float:right
            // (line-right) — НИЗ (css-writing-modes §7,
            // shape-outside-circle-049 и родня).
            let col = div().absolute().right(px(off + mr));
            if side < 0 {
                col.top(px(mt))
            } else {
                col.bottom(px(mt))
            }
        } else if side < 0 {
            div().absolute().left(px(off + ml)).top(px(mt + fy))
        } else {
            div().absolute().right(px(off + mr)).top(px(mt + fy))
        };
        floats.push(holder.child(built).into_any_element());
    }
    let shapes = std::sync::Arc::new((left, right));
    // В вертикальном письме ширина контейнера — блок-прогресс контента
    // (число колонок): полная ширина растягивала бы его на страницу.
    let mut host = if inherited.vertical_rl == Some(true) {
        div().relative()
    } else if bands.bottom(None) > 0.0 {
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: гейтить охват флоатов признаком «коробка
        // образует свой контекст форматирования», как это делают Blink
        // (`block_layout_algorithm.cc`: `IsNewFormattingContext()`) и Servo
        // (`BlockFormattingContext::layout`). Предикат был полный: флоат,
        // внепоточность, `inline-*`, таблица, ячейка, гибкий, сетка,
        // `flow-root`, `contain`, обрезка по `overflow`, корень и тело.
        // Срез флоатов (333 пары, 233 зелёных): 230. Приобретений ноль,
        // потери — `clear-003` 0.00 → 3.84, `floats-005` 0.00 → 0.72,
        // `floats-wrap-top-below-bfc-001l` 0.01 → 0.61.
        // Причина: у нас флоаты в этом хосте АБСОЛЮТНЫЕ, и `min_h` держит не
        // только §10.6.7, но и высоту, которую по спеке дают ОЧИСТИВШИЕ их
        // братья в потоке. Возвращать вместе с клиренсом как величиной в
        // потоке (шаг F6 из `target/scout-float-bands-design.md`).
        // §10.6.7: хост обязан охватить флоаты высотой — здесь они
        // абсолютные и сами её не растят.
        div().relative().w_full().min_h(px(bands.bottom(None)))
    } else {
        div().relative().w_full()
    };
    for f in floats {
        host = host.child(f);
    }
    // Блочные коробки среди полос (§9.5, последний абзац): «The border box of
    // a table, a block-level replaced element, or an element in the normal
    // flow that establishes a new block formatting context must not overlap
    // the margin box of any floats in the same block formatting context».
    // Такая коробка ищет ОКНО на всю свою высоту и съезжает вниз, пока не
    // найдёт (`bands.place_among`), а не встаёт «ниже всех флоатов»
    // (`floats-wrap-bfc-004`: BFC встаёт на y=6, оставаясь сбоку от флоата с
    // низом 20).
    //
    // Ветка включается только на бандовом хосте и только когда в хвосте есть
    // хоть один НЕ-атом: сплошные инлайн-блоки обязаны идти строчным потоком
    // `FlowRow` — они делят строку, а здесь каждый кусок берёт свою.
    //
    // Вертикальное письмо сюда не пускается: дальняя стенка полос там
    // недостижимая (`NO_WALL`, `:4845-4850`), и окна не сузятся.
    let band_host = e.attr("bands") == Some("1");
    if band_host
        && inherited.vertical_rl != Some(true)
        && rest
            .iter()
            .any(|n| matches!(band_piece(n), Some(BandPiece::Bfc) | Some(BandPiece::Strut)))
    {
        // Потолок потока: низ предыдущего куска (правило 5 §9.5.1). Бежит по
        // кускам и служит стартом поиска окна для следующего.
        let mut y = 0.0f32;
        for n in &rest {
            let Node::Element(c) = n else {
                continue;
            };
            let (Some(kind), Some((mw, mh))) = (band_piece(n), px_margin_box(&c.style)) else {
                continue;
            };
            if matches!(kind, BandPiece::Strut) {
                // Распорка своего контекста не заводит: её border-box флоаты
                // перекрывают (обтекают только строки), а показать ей нечего
                // — от неё нужна одна высота.
                y += mh;
                continue;
            }
            let (l, top, _avail) = bands.place_among(mw, mh, y);
            y = top + mh;
            let (ml, mt) = (
                px_margin(&c.style.margin.left).unwrap_or(0.0),
                px_margin(&c.style.margin.top).unwrap_or(0.0),
            );
            // Коробка прижимается к инлайн-НАЧАЛУ полосы. `margin: auto`
            // прижимом не считается СОЗНАТЕЛЬНО: эталоны `-001r` выравнивают
            // свои коробки `text-align: right`, о котором `FlowRow` не знает
            // вовсе, и учесть одно без другого — значит развести пару
            // (см. scout-bfcdraft §0.3(г)).
            let mut inner = c.clone();
            // Поля кладёт держатель — на самой коробке они сдвинули бы её
            // ещё раз (та же причина, что у флоатов, `:5092-5095`).
            inner.style.margin = crate::computed::Sides::default();
            let merged = inline::inherit(inherited, &c.style);
            let built = styled_div_with(&inner, &merged)
                .children(blocks(&inner.children, &merged, opts))
                .into_any_element();
            host = host.child(
                div()
                    .absolute()
                    .left(px(l + ml))
                    .top(px(top + mt))
                    .child(built),
            );
        }
        // §10.6.7 плюс собственная высота потока: держатели абсолютные и сами
        // хост не растят. `min_h` переопределяет поставленный на `:5129` —
        // это и нужно, там учтены только флоаты.
        return host.min_h(px(bands.bottom(None).max(y))).into_any_element();
    }
    // Картина из инлайн-блоков с известными размерами — построчный поток
    // атомов (FlowRow): flex-переносом вырезы по строкам не выразить, а
    // абзац таких детей не набирает.
    let mut atoms: Vec<crate::flow::FlowChild> = Vec::new();
    let mut atoms_ok = true;
    for n in &rest {
        match n {
            Node::Text(t) => {
                if !t.trim().is_empty() {
                    atoms_ok = false;
                    break;
                }
            }
            Node::Element(c) => {
                let inline_box = matches!(
                    c.style.display,
                    Some(Display::InlineBlock) | Some(Display::InlineFlex)
                );
                // Размер атома — MARGIN-box: эталон
                // `floats-wrap-top-below-003l-ref` держится на
                // `margin-top: 25px; margin-right: 250px` у второй коробки, а
                // без полей она встаёт вплотную и уезжает на 25 точек вверх.
                let dims = px_margin_box(&c.style);
                let (ml, mt) = (
                    px_margin(&c.style.margin.left).unwrap_or(0.0),
                    px_margin(&c.style.margin.top).unwrap_or(0.0),
                );
                // Пустая коробка без размеров — разделитель разметки
                // (незакрытый div в хвосте) — просто пропускается.
                let empty = !inline_box
                    && c.children
                        .iter()
                        .all(|n| matches!(n, Node::Text(t) if t.trim().is_empty()))
                    && dims == Some((0.0, 0.0))
                    && c.style.background.is_none();
                if empty {
                    continue;
                }
                match (inline_box, dims) {
                    (true, Some((w, h))) if w > 0.0 && h > 0.0 => {
                        let merged = inline::inherit(inherited, &c.style);
                        let mut inner = c.clone();
                        // Поля кладёт СЛОТ, а не сама коробка: `FlowRow`
                        // раскладывает ребёнка `layout_as_root` с
                        // ОПРЕДЕЛЁННЫМ размером (`flow.rs:256-263`), и поле,
                        // оставленное на элементе, вынесло бы его за слот —
                        // margin-box уехал бы дважды.
                        inner.style.margin = crate::computed::Sides::default();
                        let built = styled_div_with(&inner, &merged)
                            .children(blocks(&inner.children, &merged, opts))
                            .into_any_element();
                        // Полей нет — слот совпадает с коробкой, лишнего узла
                        // в дереве не появляется (71 зелёная пара css-shapes
                        // идёт прежним деревом).
                        let el = if px_margin_box(&inner.style) == Some((w, h)) {
                            built
                        } else {
                            div()
                                .relative()
                                .w(px(w))
                                .h(px(h))
                                .child(div().absolute().left(px(ml)).top(px(mt)).child(built))
                                .into_any_element()
                        };
                        atoms.push(crate::flow::FlowChild { el, w, h });
                    }
                    _ => {
                        atoms_ok = false;
                        break;
                    }
                }
            }
        }
    }
    if atoms_ok && !atoms.is_empty() {
        let rtl = inherited.rtl == Some(true);
        // Вертикальное письмо (vertical-rl): раскладка идёт в
        // транспонированном мире — формы переводятся туда же (инлайн-ось =
        // физическая вертикаль, блок-старт = правый край).
        if inherited.vertical_rl == Some(true) {
            let transpose = |v: &Vec<crate::flow::FloatShape>| -> Vec<crate::flow::FloatShape> {
                v.iter()
                    .map(|f| match f.clone() {
                        crate::flow::FloatShape::Band { top, h, w } => {
                            // Полоса блок-прогресса: top/h — вдоль X справа.
                            crate::flow::FloatShape::Band { top, h, w }
                        }
                        crate::flow::FloatShape::Circle { top, cx, cy, r } => {
                            crate::flow::FloatShape::Ellipse {
                                top,
                                cx: cy,
                                cy: cx,
                                rx: r,
                                ry: r,
                            }
                        }
                        crate::flow::FloatShape::Ellipse {
                            top,
                            cx,
                            cy,
                            rx,
                            ry,
                        } => crate::flow::FloatShape::Ellipse {
                            top,
                            cx: cy,
                            cy: cx,
                            rx: ry,
                            ry: rx,
                        },
                        crate::flow::FloatShape::Poly { top, pts } => {
                            crate::flow::FloatShape::Poly {
                                top,
                                pts: std::sync::Arc::new(
                                    pts.iter().map(|&(x, y)| (y, x)).collect(),
                                ),
                            }
                        }
                        // Профиль не транспонируется профилем — полосой.
                        crate::flow::FloatShape::Profile { top, ext } => {
                            crate::flow::FloatShape::Band {
                                top,
                                h: ext.len() as f32,
                                w: ext.iter().fold(0.0f32, |m, &v| m.max(v)),
                            }
                        }
                    })
                    .collect()
            };
            let t_shapes = std::sync::Arc::new((
                transpose(&shapes.0)
                    .into_iter()
                    .chain(transpose(&shapes.1))
                    .collect(),
                Vec::new(),
            ));
            return host
                .child(crate::flow::FlowRow::new(atoms, t_shapes, false).vertical_rl())
                .into_any_element();
        }
        return host
            .child(crate::flow::FlowRow::new(atoms, shapes, rtl))
            .into_any_element();
    }
    let mut flowed = inherited.clone();
    flowed.flow_shapes = Some(shapes);
    host.children(blocks(&rest, &flowed, opts))
        .into_any_element()
}

/// Точки стороны коробки: только явный `px` (None непроходной).
fn px_of2(l: &Option<Len>) -> Option<f32> {
    match l {
        None => Some(0.0),
        Some(Len::Px(v)) => Some(*v),
        _ => None,
    }
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
    // Маска-изображение (css-masking §7.1): источник уходит строкой, его
    // альфа гасит готовый буфер группы при композите; резолв — при
    // отрисовке, когда известен размер коробки. Базовая форма `clip-path`
    // (circle/ellipse) идёт тем же путём — растровой альфа-маской, как и
    // эллиптический `border-radius: H / V` (углы rx≠ry растеризатор круглить
    // не умеет; круглые пары дополняются из обычного радиуса).
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    // Большой НЕОДНОРОДНЫЙ круглый радиус — тоже маской: растеризатор жмёт
    // каждый угол к половине меньшей стороны, а спека — одним множителем от
    // суммы СМЕЖНЫХ радиусов (§5.5): `border-radius: 100px 100px 0 0` на
    // 200x100 — законный полукруг, растеризатор рисовал стадион
    // (clip-path-semicircle-ref). Однородные радиусы совпадают с растеризатором.
    // Фигурные углы (`corner-shape`, css-borders-4) — той же маской: запись
    // несёт радиусы (точки либо доли, резолв при растре) и параметр K по углам.
    let rrect = c
        .radius_masked()
        .then(|| format!("shape:{}", crate::background::rrect_spec(c, None)));
    let mask = c
        .mask_image
        .clone()
        .or_else(|| c.clip_shape.clone())
        .or(rrect)
        .map(|m| resolve_mask_refs(&m));
    // `clip: rect()` действует только на абсолютный элемент (CSS 2.1).
    // ПАРК: буфер группы создаёт stacking context, которого у `clip` нет —
    // z-переплетение детей с внешними соседями рвётся
    // (clip-no-stacking-context, 1 пара).
    // Единицы шрифта в `clip: rect(...)` меряются ЗДЕСЬ: на разборе кегль ещё
    // неизвестен, а к отрисовке он уже слит. Прежде ненулевые `em`/`ex`
    // читались как `auto`, и обрезки не было вовсе (`visufx/clip-079` и родня).
    let clip_rect = c
        .clip_len
        .map(|sides| {
            let base = match c.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            };
            let family = c.font_family.clone().unwrap_or_default();
            sides.map(|l| match l {
                Some(Len::Px(v)) => Some(v),
                Some(other) => Some(crate::metrics::spacing_px(Some(other), &family, base)),
                None => None,
            })
        })
        .or(c.clip_rect)
        .filter(|_| {
        matches!(
            c.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
    });
    // Голое слово коробки — срез краями этой коробки от border-box:
    // margin-box шире на поля, padding-box уже на рамку, content-box — на
    // рамку и отбивку (clip-path-marginBox-*, -paddingBox-*, -contentBox-*).
    let bare_inset = if c.clip_bare_box && c.clip_inset.is_none() {
        let b = c.borders();
        let s = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let [t, r, bo, l] = match c.clip_ref {
            Some(1) => [
                -s(c.margin.top),
                -s(c.margin.right),
                -s(c.margin.bottom),
                -s(c.margin.left),
            ],
            Some(2) => [s(b.top), s(b.right), s(b.bottom), s(b.left)],
            Some(3) => [
                s(b.top) + s(c.padding.top),
                s(b.right) + s(c.padding.right),
                s(b.bottom) + s(c.padding.bottom),
                s(b.left) + s(c.padding.left),
            ],
            _ => [0.0; 4],
        };
        Some([Len::Px(t), Len::Px(r), Len::Px(bo), Len::Px(l)])
    } else {
        None
    };
    let clip_inset = c.clip_inset.or(bare_inset);
    if blur <= 0.0
        && blend == 0
        && polygon.is_empty()
        && c.isolate != Some(true)
        && mask.is_none()
        && clip_rect.is_none()
        && clip_inset.is_none()
        && c.clip_edges.is_none()
        && c.clip_xywh.is_none()
    {
        return el;
    }
    let mut wrapper = crate::interact::Grouped::new(el);
    wrapper.blur = blur;
    wrapper.blend = u32::from(blend);
    wrapper.mask = mask;
    wrapper.mask_size = c.mask_size;
    wrapper.mask_fit = c.mask_fit.unwrap_or(0);
    wrapper.mask_no_repeat = c.mask_no_repeat.unwrap_or((false, false));
    wrapper.mask_luminance = c.mask_luminance == Some(true);
    wrapper.mask_pos = c.mask_pos;
    wrapper.mask_pos_far = c.mask_pos_far;
    // Коробки маски (css-masking §7.10-7.11): сдвиги краёв от border-box
    // внутрь — рамка (padding-box) либо рамка+отступ (content-box).
    let box_off = |kind: Option<u8>| -> [f32; 4] {
        let b = c.borders();
        match kind {
            Some(2) => [side(b.top), side(b.right), side(b.bottom), side(b.left)],
            Some(3) => [
                side(b.top) + side(c.padding.top),
                side(b.right) + side(c.padding.right),
                side(b.bottom) + side(c.padding.bottom),
                side(b.left) + side(c.padding.left),
            ],
            _ => [0.0; 4],
        }
    };
    wrapper.mask_origin_off = box_off(c.mask_origin);
    wrapper.clip_rect = clip_rect;
    wrapper.clip_inset = clip_inset;
    wrapper.clip_edges = c.clip_edges;
    wrapper.clip_xywh = c.clip_xywh;
    // `clip`/`mask-clip` живут в системе координат элемента ДО трансформа, а
    // трансформ рисуется ВНУТРИ буфера группы — коробка клипа обязана ехать
    // вместе (clip-transform-order: сдвинутый рисунок резался по старому
    // месту). Честно поддержан только сдвиг; поворот с клипом — парк.
    if let Some(t) = &c.transform {
        wrapper.clip_shift = (
            t.translate.0,
            t.translate.1,
            t.translate_pct.0,
            t.translate_pct.1,
        );
    }
    wrapper.mask_composite = c.mask_composite.clone().unwrap_or_default();
    wrapper.mask_clip_off = c.mask_clip.filter(|k| *k != 255).map(|k| box_off(Some(k)));
    // Точки уходят КАК ЕСТЬ (Len): проценты и пиксели резолвятся при
    // отрисовке от опорной коробки формы (css-masking §1.3.1.1): margin-box
    // расширяет bounds на поля, content-box сужает на рамку+паддинг
    // (clip-path-polygon-008: полигон в margin-box; masking 82→84).
    wrapper.polygon = polygon.to_vec();
    wrapper.polygon_evenodd = c.clip_polygon_evenodd;
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let b = c.borders();
    wrapper.poly_expand = match c.clip_ref {
        Some(1) => [
            side(c.margin.top),
            side(c.margin.right),
            side(c.margin.bottom),
            side(c.margin.left),
        ],
        Some(2) => [-side(b.top), -side(b.right), -side(b.bottom), -side(b.left)],
        Some(3) => [
            -(side(b.top) + side(c.padding.top)),
            -(side(b.right) + side(c.padding.right)),
            -(side(b.bottom) + side(c.padding.bottom)),
            -(side(b.left) + side(c.padding.left)),
        ],
        _ => [0.0; 4],
    };
    wrapper.into_any_element()
}

fn transformed(el: AnyElement, c: &Computed) -> AnyElement {
    let Some(t) = c.transform else {
        return el;
    };
    // Обратная сторона: элемент прячется, когда после поворота на него
    // смотрят с изнанки — css-transforms-2 §backface-visibility, шаг 2:
    // «if the computed value of backface-visibility is hidden and the
    // used transform matrix has m33 < 0, the element is not rendered».
    // `rotateY(180deg)` для нас — тот же `scaleX(-1)`, и по плоской матрице
    // изнанку не отличить: её держит отдельно посчитанный m33.
    if c.backface_hidden == Some(true) && t.m33 < 0.0 {
        return div().into_any_element();
    }
    let mut wrapper = crate::interact::Transformed::new(el);
    wrapper.rotate = t.rotate_rad;
    wrapper.skew = t.skew_rad;
    wrapper.scale = t.scale;
    wrapper.translate = t.translate;
    wrapper.translate_pct = t.translate_pct;
    wrapper.lin = t.lin;
    wrapper.tr = t.tr;
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
/// Вынуть из поддерева ленты прокрутки абсолютных потомков, которым лента не
/// содержащий блок.
///
/// §11.1.1: предок обрезает ТОЛЬКО того потомка, для которого он содержащий
/// блок. У абсолютного элемента без позиционированного предка содержащий блок
/// — область просмотра (§10.1 п.4), и `overflow: scroll|auto` его не касается.
///
/// Лента строит поддерево в замыкании, которое зовёт
/// `ScrollArea::request_layout` — уже после `icb_close()`, и `icb_push` вернул
/// бы элемент назад «рисовать на месте». Поэтому кандидатов вынимаем ЗДЕСЬ.
///
/// Условия — те же, что у `to_icb`, плюс два ужесточения: только
/// НЕПОСРЕДСТВЕННЫЕ дети ленты и только при ОБЕИХ заданных осях (по свободной
/// оси место сообщает щуп, а он остался бы в замыкании).
fn hoist_from_scroll(e: &mut Element, inherited: &Computed, opts: &RenderOpts) {
    use crate::computed::Position;
    if !crate::interact::icb_active() || inside_deferred() {
        return;
    }
    let merged = crate::inline::inherit(inherited, &e.style);
    // Лента внутри позиционированного предка не выносит ничего: у её потомков
    // содержащий блок есть.
    if merged.cb_ancestor || crate::inline::establishes_cb(&merged) {
        return;
    }
    if matches!(
        merged.display,
        Some(Display::Flex)
            | Some(Display::InlineFlex)
            | Some(Display::Grid)
            | Some(Display::InlineGrid)
            | Some(Display::GridLanes)
    ) {
        return;
    }
    let take: Vec<bool> = (0..e.children.len())
        .map(|i| {
            let Node::Element(c) = &e.children[i] else {
                return false;
            };
            let x_set = edge_set(c.style.inset.left) || edge_set(c.style.inset.right);
            let y_set = edge_set(c.style.inset.top) || edge_set(c.style.inset.bottom);
            c.style.position == Some(Position::Absolute)
                && c.style.z_index.unwrap_or(0) >= 0
                && x_set
                && y_set
                && !stays_positioned(&e.children[i + 1..])
        })
        .collect();
    if !take.iter().any(|t| *t) {
        return;
    }
    let mut keep = Vec::with_capacity(e.children.len());
    for (i, child) in std::mem::take(&mut e.children).into_iter().enumerate() {
        if !take[i] {
            keep.push(child);
            continue;
        }
        // `blocks` на одном узле повторяет ВЕСЬ путь `to_icb`: строит элемент
        // и отдаёт открытому слою ICB. При обеих заданных осях щуп не нужен,
        // поэтому список возвращается пустым.
        let left = blocks(std::slice::from_ref(&child), &merged, opts);
        if left.is_empty() {
            continue;
        }
        // Условия предиката разошлись с `to_icb`: узел остаётся на месте, а не
        // теряется.
        drop(left);
        keep.push(child);
    }
    e.children = keep;
}

fn scrollable(e: &Element, inherited: &Computed, opts: &RenderOpts) -> Option<AnyElement> {
    use crate::computed::Overflow;
    let horizontal = e.style.overflow_x == Some(Overflow::Scroll);
    let vertical = e.style.overflow_y == Some(Overflow::Scroll);
    if !horizontal && !vertical {
        return None;
    }
    let mut node = e.clone();
    // Слой ICB закрывается раньше, чем `ScrollArea` позовёт `build`, — поэтому
    // выносим кандидатов сейчас, из ещё не отданного в замыкание клона.
    hoist_from_scroll(&mut node, inherited, opts);
    let node = node;
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
            inner.style.scroller = true;
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
    // Фильтры интерполируются покомпонентно; `none` = нейтральный
    // (filter-effects-1 §Interpolation, css-filters-animation-*).
    if prev.1.filter.is_some() || next.1.filter.is_some() {
        let a = prev
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        let b = next
            .1
            .filter
            .unwrap_or_else(crate::computed::Filter::neutral);
        out.filter = Some(crate::computed::Filter {
            grayscale: lerp(a.grayscale, b.grayscale),
            brightness: lerp(a.brightness, b.brightness),
            saturate: lerp(a.saturate, b.saturate),
            invert: lerp(a.invert, b.invert),
            sepia: lerp(a.sepia, b.sepia),
            opacity: lerp(a.opacity, b.opacity),
            hue_rotate: lerp(a.hue_rotate, b.hue_rotate),
            contrast: lerp(a.contrast, b.contrast),
            blur: lerp(a.blur, b.blur),
        });
    }
    if prev.1.backdrop_blur.is_some() || next.1.backdrop_blur.is_some() {
        out.backdrop_blur = Some(lerp(
            prev.1.backdrop_blur.unwrap_or(0.0),
            next.1.backdrop_blur.unwrap_or(0.0),
        ));
    }
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
    // Остановленная анимация — не анимация: кадр `(-delay)/duration`
    // запекается прямо в стиль элемента, и дальше работает весь обычный
    // конвейер (фильтр по цветам, групповой blur). Живая обёртка здесь
    // делала reftest недетерминированным по построению.
    if spec.paused {
        let t = if spec.seconds > 0.0 {
            ((-spec.delay) / spec.seconds).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let c = frame_at(&frames, t);
        let mut inner = e.clone();
        let st = &mut inner.style;
        if c.opacity.is_some() {
            st.opacity = c.opacity;
        }
        if c.background.is_some() {
            st.background = c.background;
        }
        if c.color.is_some() {
            st.color = c.color;
        }
        if c.width.is_some() {
            st.width = c.width;
        }
        if c.height.is_some() {
            st.height = c.height;
        }
        if c.translate.is_some() {
            st.translate = c.translate;
        }
        if c.filter.is_some() {
            st.filter = c.filter;
        }
        if c.backdrop_blur.is_some() {
            st.backdrop_blur = c.backdrop_blur;
        }
        return element(&inner, inherited, opts);
    }
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
        // `height: 5em` доживает сюда неразрешённым — доля считается от
        // кегля самого блока (outline-inline-vlr-006: предел колонки 5em).
        let em_base = match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => opts.base_size(),
        };
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): переводить сюда и единицы шрифта
        // (`max-height: 8ch`), чтобы предел ортогонального потока не терялся.
        // Срез вертикального письма 1086 пар: приобретено 0, потеряно 7 —
        // `available-size-003…018` (0.05-0.11 -> «красное видно»). Тесты
        // прямо пишут: «**max**-height does not give the element a definite
        // block size» (§7.3.1 берёт предел у ОПРЕДЕЛЁННОГО размера, а
        // `max-height` определённым не делает). Значит и нынешний перевод
        // `max-height` в точках — тоже неверный источник предела.
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Em(k)) => Some(k * em_base),
            _ => None,
        };
        let h = px_of(e.style.height);
        let min_h = px_of(e.style.min_height);
        let max_h = px_of(e.style.max_height);
        if h.is_some() || min_h.is_some() || max_h.is_some() {
            // Клэмп как у CSS-высоты: max режет, min ПЕРЕБИВАЕТ max; без
            // своей высоты базой служит НАЧАЛЬНЫЙ содержащий блок, и он же —
            // общий потолок («larger than ICB» не расширяет место).
            let mut avail = h.unwrap_or(opts.viewport.1);
            if let Some(m) = max_h {
                avail = avail.min(m);
            }
            if let Some(m) = min_h {
                avail = avail.max(m);
            }
            avail = avail.min(opts.viewport.1);
            // У блока без вертикали предел ставится детям всегда; у самого
            // вертикального — только если родитель не дал своего
            // (table-cell-002: max-height ячейки).
            if e.style.vertical != Some(true) || merged.ortho_limit.is_none() {
                merged.ortho_limit = Some(avail);
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
        return transformed(el, &merged);
    }
    // Рамка строится ОДИН раз до match: прежний `is_some() => unwrap()`
    // читал файл с диска и разбирал вложенный документ дважды за кадр.
    // Неразобранная рамка по-прежнему падает в общий рукав.
    let mut built_iframe = if e.tag == "iframe" {
        iframe(e, opts)
    } else {
        None
    };
    match e.tag.as_str() {
        // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4): слитый стиль
        // его уже несёт, а копия для замещаемой коробки — нет. Без переноса
        // блочная картинка под `body { image-orientation: none }` всё равно
        // разворачивалась по метке EXIF.
        "img" => {
            let mut copy = pct_height_to_px(e, inherited);
            copy.style.image_orient_none = merged.image_orient_none;
            image(&copy)
        }
        // Замещаемые с картинкой-источником рисуются как <img>: embed через
        // src, object через data, video через poster (css-images §5:
        // object-fit/-position действуют на всех замещаемых).
        "embed" if e.attr("src").is_some() => image(e),
        "object" if e.attr("data").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        "iframe" if built_iframe.is_some() => built_iframe.take().unwrap(),
        // ЗАМЕРЕНО И ОТКАЧЕНО: давать рамке БЕЗ адреса резервную ширину 300
        // точек при `display: block` (§10.3.4 -> §10.3.2). Замерено по срезу
        // из 416 пар семей *image*/*replaced*: приобретено 0, потеряно 1 —
        // `float-replaced-height-004` 0.00 -> «красное видно». Это второй
        // заход на резервный размер бесадресной рамки; первый (коробка
        // 300×150 целиком) стоил 13 пар, запись выше.
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => crate::svg::element(e).unwrap_or_else(|| {
            styled_div_with(e, &merged)
                .child(SharedString::from("[рисунок]"))
                .into_any_element()
        }),
        "hr" => styled_div_with(e, &merged).w_full().into_any_element(),
        // Синтетический узел обтекания формой (см. wrap_floats).
        "shape-flow" => shape_flow(e, &merged, opts),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        _ if merged.display == Some(Display::GridLanes) => {
            // Минимума высоты в видимую область здесь тоже нет: §10.6.3
            // одинаков для любой раскладки корня (см. главный путь ниже).
            lanes(e, &merged, opts)
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
            // ЗАМЕРЕНО И ЗАКРЕПЛЕНО: минимума высоты в видимую область на
            // коробке корня БОЛЬШЕ НЕТ. §10.6.3 — высота корня `auto`, ростом
            // с окно обязан быть начальный содержащий блок, а не коробка:
            // рамка и фон корня уходили полосами до низа окна
            // (`background-root-011`, `normal-flow/root-box-001`). Абсолютные
            // потомки всплывают в слой ICB (`icb_open`/`icb_close`), и
            // содержащим блоком им служит корневой `div` стенда.
            //
            // Замер по семьям backgrounds/*, *root*, positioning/*,
            // containing-block*, normal-flow/*, *margin*: приобретено 11,
            // потеряна одна (`background-root-024`: слой донора-тела лежит в
            // детях корня и отсчитывался от его padding-box).
            // Многоколоночный поток. Своей многоколоночной раскладки нет, но
            // сетка даёт то же расположение: число рядов считаем по числу
            // детей, а заполнение идёт по колонкам — тогда порядок совпадает
            // с браузерным (сверху вниз, затем в следующую колонку).
            // Число колонок бывает задано и КОСВЕННО — их шириной: сколько
            // целых колонок этой ширины влезает в коробку, столько их и будет
            // (css-multicol-1 §7.3). Ширина коробки нужна заданная: без неё
            // считать не от чего, и остаётся прежняя дорожечная раскладка.
            // Умолчание `column-gap: normal` — один кегль (css-align §8.3).
            let used_gap = match e.style.column_gap {
                Some(Len::Px(v)) => v,
                _ => match e.style.font_size {
                    Some(Len::Px(size)) => size,
                    _ => opts.base_size(),
                },
            };
            // `column-*` — только у блочных контейнеров (css-multicol-1 §2):
            // сетка ими не режется (`grid-multicol-001`).
            let multicol = multicol_container(&e.style);
            let column_width = e.style.column_width.filter(|_| multicol);
            let column_count = e.style.column_count.filter(|_| multicol);
            let count_from_width = match (column_width, e.style.width) {
                (Some(Len::Px(w)), Some(Len::Px(box_w))) if w > 0.0 => {
                    Some((((box_w + used_gap) / (w + used_gap)).floor().max(1.0)) as u16)
                }
                _ => None,
            };
            // Used column-count (css-multicol §3.4, как ResolveUsedColumnCount
            // в blink): заданы оба — МЕНЬШЕЕ из числа и «сколько влезает»;
            // только ширина — сколько влезает.
            let used_count = match (column_count, count_from_width) {
                (Some(c), Some(fw)) => Some(c.min(fw)),
                (Some(c), None) => Some(c),
                (None, fw) => fw,
            };
            let width_driven = column_count.is_none()
                && count_from_width.is_none()
                && matches!(column_width, Some(Len::Px(w)) if w > 0.0);
            if let Some(cols) = used_count.filter(|n| *n > 1).or(width_driven.then_some(0)) {
                // Сплошной текст режется на колонки по строкам, а не по детям:
                // один длинный абзац иначе оставался в первой колонке целиком.
                // `columns: auto <w>` без ширины коробки решается в замере —
                // туда уходит и число, и ширина колонки (§3.4).
                let col_w_px = match column_width {
                    Some(Len::Px(w)) if w > 0.0 => Some(w),
                    _ => None,
                };
                let want = (cols > 0).then_some(cols as usize);
                // Спаннер среди инлайнового потока: режем детей на сегменты,
                // каждый сегмент — свой поток колонок, спаннер — блок между
                // ними (css-multicol §6).
                let is_span = |n: &Node| {
                    matches!(n, Node::Element(c)
                        if c.style.column_span == Some(true) && !c.inline)
                };
                if e.children.iter().any(&is_span) {
                    for chunk in e.children.split_inclusive(&is_span) {
                        let (body, span) = match chunk.split_last() {
                            Some((last, head)) if is_span(last) => (head, Some(last)),
                            _ => (chunk, None),
                        };
                        if body.iter().any(|n| !is_blank(n)) {
                            let mut seg = e.clone();
                            seg.children = body.to_vec();
                            seg.style.column_span = None;
                            if let Some(el) = column_flow(&seg, &merged, opts, want, col_w_px) {
                                d = d.child(el);
                            } else {
                                // Блочный сегмент: рекурсия в общий рендер —
                                // он сам выберет укладку колонок; коробка
                                // (фон/рамки/поля) остаётся на хосте.
                                let mut sub = seg.clone();
                                sub.style.background = None;
                                sub.style.margin = Default::default();
                                sub.style.padding = Default::default();
                                sub.style.border_width = Default::default();
                                sub.style.width = None;
                                sub.style.height = None;
                                d = d.child(div().children(blocks(
                                    &[Node::Element(sub)],
                                    &merged,
                                    opts,
                                )));
                            }
                        }
                        if let Some(Node::Element(sp)) = span {
                            let inner = inline::inherit(&merged, &sp.style);
                            d = d.child(styled_div_with(sp, &inner).children(blocks(
                                &sp.children,
                                &inner,
                                opts,
                            )));
                        }
                    }
                    return d.into_any_element();
                }
                if let Some(el) = column_flow(e, &merged, opts, want, col_w_px) {
                    // Коробка элемента остаётся своей: отступы и фон
                    // принадлежат ей, поток живёт внутри.
                    return d.child(el).into_any_element();
                }
                if cols == 0 {
                    // Число колонок при `columns: auto <w>` решается только в
                    // замере текстового потока; блочный фоллбек — дорожками.
                    if let Some(w) = col_w_px {
                        d = d.grid().grid_cols_min(px(w));
                    }
                } else {
                    // Блочные дети с ИЗВЕСТНЫМИ высотами — честная укладка по
                    // колонкам с балансом и монолитами (css-break, фаза 1;
                    // план target/scout-multicol.md / scout-fragmentation.md).
                    // Коробка ребёнка и его вертикальные поля отдельно:
                    // поля схлопываются между соседями и на границах колонок.
                    // Флоаты в поддереве ломают известность высоты.
                    // Прямые абсолюты многоколоночника — не в стопку: их
                    // содержащий блок — весь контейнер, рисуются его детьми
                    // рядом со стопкой (`out-of-flow-in-multicolumn-094…097`
                    // при нулевой записи в стопке уходили в колонку).
                    let direct_oof: Vec<Element> = e
                        .children
                        .iter()
                        .filter_map(|n| match n {
                            Node::Element(c) if out_of_flow(&c.style) => Some(c.clone()),
                            _ => None,
                        })
                        .collect();
                    let stackable: Option<Vec<(Element, Shape)>> = e
                        .children
                        .iter()
                        .filter(|n| !is_blank(n))
                        .filter(|n| !matches!(n, Node::Element(c) if out_of_flow(&c.style)))
                        .map(|n| match n {
                            // `position: relative` укладке не мешает — сдвиг
                            // накладывается на месте (корень A1).
                            Node::Element(c)
                                if !c.inline
                                    && (c.style.position.is_none()
                                        || c.style.position
                                            == Some(crate::computed::Position::Relative))
                                    && c.style.float.unwrap_or(0) == 0 =>
                            {
                                shape_full(c, 4).map(|h| ((*c).clone(), h))
                            }
                            _ => None,
                        })
                        .collect();
                    if let Some(kids) = stackable.filter(|k| !k.is_empty()) {
                        let fixed = if e.style.column_fill_auto == Some(true) {
                            match e.style.height {
                                Some(Len::Px(h)) => Some(h),
                                _ => None,
                            }
                        } else {
                            None
                        };
                        let rule = if e.style.column_rule_visible == Some(true) {
                            Some((
                                match e.style.column_rule_width {
                                    Some(Len::Px(v)) => v,
                                    Some(Len::Em(k)) => {
                                        k * match e.style.font_size {
                                            Some(Len::Px(fs)) => fs,
                                            _ => opts.base_size(),
                                        }
                                    }
                                    _ => 3.0,
                                },
                                e.style
                                    .column_rule_color
                                    .or(merged.color)
                                    .unwrap_or(crate::value::Color {
                                        r: 0.0,
                                        g: 0.0,
                                        b: 0.0,
                                        a: 1.0,
                                    })
                                    .to_hsla(),
                            ))
                        } else {
                            None
                        };
                        let children: Vec<crate::flow::StackChild> = kids
                            .into_iter()
                            .map(|(c, (h, mt, mb, cuts, forced, solid))| {
                                let mut copy = c;
                                // Поля кладёт укладка колонок, не коробка.
                                copy.style.margin.top = None;
                                copy.style.margin.bottom = None;
                                let inner = inline::inherit(&merged, &copy.style);
                                // Копии на случай разреза между колонками:
                                // элемент GPUI рисуется один раз, а фрагмент
                                // нужен свой в каждой колонке. Больше, чем
                                // колонок, ребёнок занять не может.
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): строить копию
                                // через `element(&copy, &merged, opts)`, чтобы сетка
                                // и гибкий контейнер внутри стопки рисовались
                                // (`grid-container-fragmentation-*`): срез
                                // фрагментации 445 -> 405 (+12/−52) — рамки, тени,
                                // `break-between-avoid-*`, `fieldset` ушли в
                                // красное: общий путь элемента кладёт слои и
                                // выносит абсолюты иначе, чем ждёт стопка.
                                // Возвращать узкой веткой только для сетки.
                                /// css-position-3 §abspos-breaking: «User
                                /// agents must not paginate the content of
                                /// fixed-positioned boxes». Копия фрагмента —
                                /// ПОЛНЫЙ клон поддерева, и `position: fixed`
                                /// внутри неё уезжает в слой ICB из КАЖДОЙ
                                /// копии (`render.rs:1790` -> `:1823` ->
                                /// `icb_push`). Слой лежит вне коробки
                                /// многоколоночника, маска колонки
                                /// (`flow.rs:998`) его не режет — на экране
                                /// вышло бы столько зелёных коробок, сколько
                                /// колонок. Оставляем фиксированного потомка
                                /// только в ПЕРВОЙ копии: там же, где стоит
                                /// его щуп статической позиции.
                                /// Blink делает это тем же разделением —
                                /// `out_of_flow_layout_part.cc:1607`: «This
                                /// does not include repeated fixed-positioned
                                /// elements».
                                fn drop_fixed(n: &Node) -> Option<Node> {
                                    match n {
                                        Node::Element(k)
                                            if k.style.position
                                                == Some(crate::computed::Position::Fixed) =>
                                        {
                                            None
                                        }
                                        Node::Element(k) => {
                                            let mut c = k.clone();
                                            c.children =
                                                k.children.iter().filter_map(drop_fixed).collect();
                                            Some(Node::Element(c))
                                        }
                                        other => Some(other.clone()),
                                    }
                                }
                                let build = |first: bool| {
                                    let kids: Vec<Node> = if first {
                                        copy.children.clone()
                                    } else {
                                        copy.children.iter().filter_map(drop_fixed).collect()
                                    };
                                    // css-break-3 §5.5: «Fragmentation … occurs
                                    // before relative positioning, transforms,
                                    // and any other graphical effects. Such
                                    // effects are applied per fragment». Разрезы
                                    // трансформ не двигают (`shape_full` его и не
                                    // читает), но САМ трансформ обязан быть на
                                    // каждом фрагменте. Общий путь вешает его
                                    // через `transformed()` (render.rs:1719,
                                    // :5938, :8185); узкая ветка копии шла мимо
                                    // всех трёх, и `transform` у ребёнка
                                    // многоколоночника пропадал целиком
                                    // (`transform-000…005`: `translateX(60px)`
                                    // контейнера гасил `left:-60px` потомков, а
                                    // без него содержимое уезжало из колонки).
                                    // Начало отсчёта пока общее на всю коробку,
                                    // а не своё на фрагмент, — для `translate`
                                    // это точно, для `rotate`/`scale` нет.
                                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): строить эту
                                    // копию через общий `element()` вместо узкой
                                    // ветки `styled_div_with`. Срез 3029 пар:
                                    // 1900 -> 1902 (+14/-12), и семь потерь —
                                    // грубые (99.00, страница разъезжается):
                                    // `multi-line-column-flex-fragmentation-035`,
                                    // `multi-line-row-flex-fragmentation-039/040/
                                    // 059`, `multicol-nested-013/021`,
                                    // `multicol-fill-balance-nested-000`. Тот же
                                    // путь, на котором прежде мерился откат -52.
                                    transformed(
                                        styled_div_with(&copy, &inner)
                                            .children(blocks(&kids, &inner, opts))
                                            .into_any_element(),
                                        &inner,
                                    )
                                };
                                // Монолиты (css-break-3 §4.1) — их разрыв
                                // запрещён, и в следующую колонку они уходят
                                // целиком: `break-inside: avoid`,
                                // прокручиваемая или обрезающая коробка,
                                // замещаемый элемент, таблица и ячейка,
                                // атомарная строчная коробка. Сюда же —
                                // сплошной СТРОЧНЫЙ набор: резать его можно
                                // только между строками, а строк укладка
                                // колонок не видит, и разрез приходился бы
                                // посреди строки.
                                // Монолитен ПРОКРУЧИВАЕМЫЙ контейнер (css-break-4
                                // §4.1 «scroll containers»); `hidden`/`clip` —
                                // обрезка, не прокрутка, и режется как блок
                                // (корень A4).
                                let scrolls = |o: Option<crate::computed::Overflow>| {
                                    matches!(o, Some(crate::computed::Overflow::Scroll))
                                };
                                let block_kid = |n: &Node| {
                                    matches!(n, Node::Element(k)
                                        if !k.inline || k.style.display == Some(Display::Block))
                                };
                                // ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): `contain: size` как
                                // монолит (Blink `IsMonolithic`) — срез фрагментации
                                // 469 -> 467 (+1/−3): `single-line-column-flex-
                                // fragmentation-051/063` режутся у Blink иначе (рост
                                // элемента от фрагментации, корень R5 скаута).
                                let monolith = copy.style.break_inside_avoid
                                    || scrolls(copy.style.overflow_x)
                                    || scrolls(copy.style.overflow_y)
                                    || matches!(
                                        copy.tag.as_str(),
                                        "img"
                                            | "svg"
                                            | "canvas"
                                            | "video"
                                            | "embed"
                                            | "object"
                                            | "iframe"
                                    )
                                    // Таблица и ячейка — не монолиты
                                    // (css-break-4 §4.1).
                                    || matches!(
                                        copy.style.display,
                                        Some(Display::InlineBlock)
                                            | Some(Display::InlineFlex)
                                            | Some(Display::InlineGrid)
                                    )
                                    // Сплошной СТРОЧНЫЙ набор тоже монолит:
                                    // резать его можно лишь между строками, а
                                    // строк укладка колонок не видит, и разрез
                                    // приходился бы посреди строки.
                                    // ПУСТАЯ коробка с высотой режется по своей
                                    // высоте (css-break-4 §4.2; корень A3).
                                    || (copy.children.iter().any(|n| !is_blank(n))
                                        && !copy.children.iter().any(block_kid));
                                crate::flow::StackChild {
                                    el: build(true),
                                    frags: (1..cols.max(1)).map(|_| build(false)).collect(),
                                    monolith,
                                    cuts,
                                    force_before: copy.style.break_before_force,
                                    force_after: copy.style.break_after_force,
                                    forced,
                                    solid,
                                    h,
                                    mt,
                                    mb,
                                }
                            })
                            .collect();
                        let mut d = d.child(crate::flow::ColumnStack::new(
                            children,
                            cols as usize,
                            used_gap,
                            fixed,
                            rule,
                        ));
                        for oof in &direct_oof {
                            d = d.child(element(oof, &merged, opts));
                        }
                        return d.into_any_element();
                    }
                    let count = e.children.iter().filter(|n| !is_blank(n)).count().max(1);
                    let rows = count.div_ceil(cols as usize).max(1) as u16;
                    let gap = used_gap;
                    d = d
                        .grid()
                        .grid_template_cols(
                            (0..cols).map(|_| gpui::GridTrack::Fraction(1.0)).collect(),
                        )
                        .grid_template_rows((0..rows).map(|_| gpui::GridTrack::Auto).collect())
                        .gap_x(px(gap));
                    d.style().grid_auto_flow = Some(gpui::GridAutoFlow::Column);
                }
            } else if let Some(Len::Px(w)) = column_width {
                // Ширина колонки без их числа — это «сколько влезет»: ровно
                // то, что умеет короткая форма дорожек в GPUI.
                d = d.grid().grid_cols_min(px(w));
            // Ось блочного потока не зависит от `display` (css-writing-modes-4
            // §3.1): inline-block, ячейка, list-item с вертикальным письмом
            // раскладывают детей той же горизонтальной осью, что и голый блок
            // (`block-flow-direction-*`, `line-box-direction-*`).
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: явный `Some(Block)` — он же стоит у
            // блокифицированных (абсолют в сетке), и
            // `grid-positioned-children-writing-modes-001` 0.39 -> 1.32 даже с
            // гейтом «родитель не сетка».
            // `list-item` сюда НЕ пускать (★ ЗАМЕРЕНО: `li` с одним текстом
            // становился рядом — `line-box-direction-vrl-019/vlr-020`
            // 6.58 -> 12.55).
            } else if merged.vertical == Some(true)
                && matches!(
                    e.style.display,
                    None | Some(Display::InlineBlock) | Some(Display::TableCell)
                )
            {
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
                // Прижим коробки к низу — ЗАПЛАТКА того времени, когда абзац
                // вертелся по часовой и его содержимое росло от верха.
                // С поворотом против часовой (`VerticalText::ccw`) строка сама
                // начинается у нижнего края, и второй прижим снова уводит
                // рисунок. Снимать ВМЕСТЕ с патчем поворота и мерить
                // `wm-propagation-body-047`, `abs-pos-border-offset-002` —
                // ровно те две пары, ради которых заплатка ставилась.
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
                // rtl-прижим переполняющих блоков — ТОЧЕЧНЫЙ align_self End
                // детям с заданной шириной (в blocks): items_end на контейнере
                // снимал stretch у всех, и абзац в rtl ужимался до текста —
                // text-align внутри пустел (text-align-end-001: bw=124.8
                // вместо 300). ЗАМЕРЕНО: +18 css-text при −2..3 wm и −4 mix
                // (spot-механика abs-pos-border-offset полагалась на
                // items_end — новый след) — нетто +10.
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
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: отдавать субсетке обычной сетки
            // РАЗРЕШЁННЫЙ кусок родительских дорожек — тем же кодом, что и
            // проход лунок (`if item.style.subgrid` в `lanes`), с вычетом
            // своих краёв и правкой зазора. Кусок брался из ЯВНЫХ линий
            // ребёнка (в обычной сетке размещение делает вендор, и другого
            // источника `at`/`span` до раскладки нет). Срез css-grid (1133
            // пары, 646 зелёных): 645, приобретено НОЛЬ, и
            // `row-auto-placed-subgrid-nested-subgrid-inherited-tracks-001`
            // ушла 0.00 → HUNG (вложенная субсетка зацикливает раскладку).
            // Возвращать только вместе с размещением субсетки НА НАШЕЙ
            // стороне: пока `at`/`span` берутся из линий, вложенный случай
            // получает кусок от куска и сходится не всегда.
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
                    opts.viewport.0,
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
            ) || merged.float.is_some()
                || merged.flow_root == Some(true)
                // Независимый контекст форматирования и без overflow/float/
                // flow-root: гибкий контейнер, сетка, таблица — своя
                // раскладка по определению, и строки внутри в бюджет
                // `line-clamp` не входят (css-overflow-4: «skip lines in
                // independent formatting contexts»; `webkit-line-clamp-012/013`).
                || matches!(
                    merged.display,
                    Some(Display::Flex)
                        | Some(Display::InlineFlex)
                        | Some(Display::Grid)
                        | Some(Display::InlineGrid)
                        | Some(Display::GridLanes)
                        | Some(Display::Table)
                        | Some(Display::InlineTable)
                        | Some(Display::TableCell)
                )
                // `<fieldset>` — тоже отдельная раскладка
                // (`webkit-line-clamp-027`).
                || e.tag == "fieldset";
            let _bfc_guard = (!is_clamp && makes_bfc && crate::interact::clamp_context().is_some())
                .then(crate::interact::ClampGuard::enter_bfc);
            // Проба элемента сетки/гибкого контейнера: пишет свои разложенные
            // границы в буфер родителя. Ставится ДО clamp-пробы, чтобы её
            // ранний `return` не съел запись.
            if let Some(key) = crate::interact::gap_context() {
                kids.push(crate::interact::gap_item_probe(
                    crate::interact::gap_items_for(key),
                ));
            }
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
            // Абсолютный потомок ищет ближайшего позиционированного предка
            // (§10.1), а раскладка под нами знает только непосредственного
            // родителя. Пока строятся дети, открыт слой: коробка, чей родитель
            // содержащим блоком не является, переезжает сюда.
            let cb_layer = crate::inline::establishes_cb(&merged);
            if cb_layer {
                crate::interact::cb_open();
            }
            // Линейки промежутков (css-gaps-1). Слой заводится ТОЛЬКО когда
            // задан стиль хотя бы одной линейки: начальное `none` (§1871)
            // означает, что рисовать нечего, и ни одна старая пара сюда не
            // попадает. Ширина — по тем же ключевым словам, что у
            // многоколонника; цвет по умолчанию — `currentcolor`.
            let gap_rules = matches!(
                merged.display,
                Some(Display::Flex)
                    | Some(Display::InlineFlex)
                    | Some(Display::Grid)
                    | Some(Display::InlineGrid)
                    | Some(Display::GridLanes)
            )
            .then(|| {
                let size = match merged.font_size {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                let w = |l: &Option<Len>| match l {
                    Some(Len::Px(v)) => *v,
                    Some(Len::Em(k)) => k * size,
                    _ => 3.0,
                };
                let fallback = merged.color.unwrap_or(crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                });
                let col = (e.style.column_rule_visible == Some(true))
                    .then(|| {
                        (
                            w(&e.style.column_rule_width),
                            e.style.column_rule_color.unwrap_or(fallback),
                        )
                    })
                    .filter(|(w, _)| *w > 0.0);
                let row = (e.style.row_rule_visible == Some(true))
                    .then(|| {
                        (
                            w(&e.style.row_rule_width),
                            e.style.row_rule_color.unwrap_or(fallback),
                        )
                    })
                    .filter(|(w, _)| *w > 0.0);
                (col, row)
            })
            .filter(|(col, row)| col.is_some() || row.is_some());
            let gap_buf = gap_rules
                .is_some()
                .then(|| crate::interact::gap_items_for(e.node_id ^ opts.doc_salt));
            let _gap_guard = gap_buf
                .as_ref()
                .map(|_| crate::interact::GapGuard::enter(e.node_id ^ opts.doc_salt));
            kids.extend(blocks(&children, &merged, opts));
            if cb_layer {
                kids.extend(crate::interact::cb_close());
            }
            if let (Some(buf), Some((col, row))) = (gap_buf, gap_rules) {
                kids.push(
                    crate::interact::GapRulePainter::new(buf, col, row).into_any_element(),
                );
            }
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
            // Абсолют с `anchor()`-вставками: раскладка поставила его к краю
            // содержащего блока (нулевая вставка), сдвиг до края якоря
            // считает заместитель на подготовке кадра. Без якорных вставок
            // коробка возвращается как есть.
            crate::anchor::place(d.children(kids).into_any_element(), &merged, inherited)
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

/// ПРОБОВАЛИ И ОТКАТИЛИ: считать `<iframe>` БЕЗ адреса замещаемой коробкой
/// 300×150 (§10.3.2, §10.6.2). Замерено по семьям *replaced*, positioning/*,
/// normal-flow/*, *float*: приобретено 0, ПОТЕРЯНО 13 — все тринадцать ушли
/// в «красное видно». Пустая коробка умолчания встаёт поверх зелёной и
/// открывает красную подложку; этим семьям нужен не размер пустого кадра, а
/// доля от содержащего блока.
///
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
/// Кусок с УНАСЛЕДОВАННЫМ шрифтом для разрешения шрифтовых единиц.
///
/// `image_with` разрешает `em`/`ex`/`ch` по стилю САМОГО куска, а гарнитуры у
/// `<img>` своей нет — метрика бралась запасная (пол-кегля вместо настоящего
/// роста строчной), и `height: 0.25ex` при `font: 250px/1 Ahem` давало 31
/// точку вместо 50 (`units-003`: оранжевый квадрат не совпадал с навесными
/// прямоугольниками).
fn with_inherited_font(e: &Element, inherited: &Computed) -> Element {
    let mut copy = e.clone();
    if copy.style.font_family.is_none() {
        copy.style.font_family = inherited.font_family.clone();
    }
    if copy.style.monospace.is_none() {
        copy.style.monospace = inherited.monospace;
    }
    // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4, «Inherited: yes»),
    // а копия несёт только собственный стиль элемента. Замещаемая коробка
    // строится ИМЕННО ИЗ НЕЁ: и растр (`background::key(src, &e.style)`), и
    // собственный фон (`styled_div` -> `background::layer(&e.style)`) читают
    // стиль копии, поэтому без переноса `image-orientation: none`, заданный
    // на предке, до картинки не доезжает и EXIF-разворот применяется всё
    // равно (`image-orientation-none`, `-none-content-images`).
    // Ранний возврат снят намеренно: он экономил только клон, а перенос
    // обязан идти и у элемента со своим шрифтом.
    if copy.style.image_orient_none.is_none() {
        copy.style.image_orient_none = inherited.image_orient_none;
    }
    copy
}

fn atom_base_font(inherited: &Computed, opts: &RenderOpts) -> f32 {
    match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    }
}

fn image(e: &Element) -> AnyElement {
    image_with(e, None)
}

/// Доля ВЫСОТЫ замещаемого, сведённая к точкам по содержащему блоку.
///
/// Держатель картинки стоит внутри анонимного ряда строки
/// (`inline::as_wrapped_row`): у ряда высота `auto`, элемент прижат по базовой
/// линии и не растягивается, поэтому доля высоты бралась ОТ РЯДА и разрешалась
/// в ноль — картинка не рисовалась ни одной точкой
/// (`background-image-cover-002-ref`). Содержащий блок здесь известен точно —
/// это `inherited`, и доля сводится к точкам ещё на сборке.
///
/// Только обычный блочный контейнер: у гибкого, сеточного и лунок высота
/// приходит от раскладки, и подстановка ломает `row-auto-repeat-auto-023`.
fn pct_height_to_px(e: &Element, inherited: &Computed) -> Element {
    let e = &pct_limits_to_px(e, inherited);
    let (Some(Len::Pct(k)), Some(Len::Px(h))) = (e.style.height, inherited.height) else {
        return e.clone();
    };
    // ЗАМЕРЕНО И ОТКАЧЕНО: пускать сюда и `inline-block` — 0 и 0 по обоим
    // сводам, `sizing-percentages-replaced-orthogonal-001` не сдвинулась.
    if !matches!(inherited.display, None | Some(Display::Block)) {
        return e.clone();
    }
    // `height` в разборе — высота СОДЕРЖИМОГО (§10.6.2, content-box): отступы
    // и рамку прибавляет уже раскладка. Вычитать их отсюда нельзя — картинка
    // выходила на 6 точек короче (0.91 вместо 0.00 на `cover-002`).
    if h <= 0.0 {
        return e.clone();
    }
    let mut copy = e.clone();
    copy.style.height = Some(Len::Px(k * h));
    copy
}

/// Пределы замещаемого в ДОЛЯХ — в точки от содержащего блока.
///
/// Размер рисунка считается на сборке (`image_with`), и доля там уже не с чем
/// сравнивать: `max-width: 100%` у картинки отбрасывался целиком, и она
/// вылезала за родителя (css-sizing-3 §5: пределы решаются от содержащего
/// блока, как и сам размер). Ширина берётся от ширины содержащего блока,
/// высота — от его высоты и только у обычного блочного контейнера: у гибкого
/// и сеточного высота приходит от раскладки (та же оговорка, что у
/// `pct_height_to_px`).
fn pct_limits_to_px(e: &Element, inherited: &Computed) -> Element {
    let of = |l: Option<Len>, base: Option<Len>| match (l, base) {
        (Some(Len::Pct(k)), Some(Len::Px(b))) if b > 0.0 => Some(Len::Px(k * b)),
        _ => l,
    };
    let block_cb = matches!(inherited.display, None | Some(Display::Block));
    let h_base = block_cb.then_some(inherited.height).flatten();
    let mut copy = e.clone();
    copy.style.max_width = of(e.style.max_width, inherited.width);
    copy.style.min_width = of(e.style.min_width, inherited.width);
    copy.style.max_height = of(e.style.max_height, h_base);
    copy.style.min_height = of(e.style.min_height, h_base);
    copy
}

/// То же, но с базовым кеглем для разрешения долей: атом строится от СЫРОГО
/// стиля, и `padding-right: 1em` без разрешения терялся вовсе
/// (wm-propagation-body-040: сосед вставал на 16 точек левее эталона).
fn image_with(e: &Element, base_font: Option<f32>) -> AnyElement {
    // Ключевое слово содержимого в оси замещаемого — его природный (или
    // перенесённый через соотношение) размер, то есть `auto` (css-sizing-3
    // §5.1: «When the box has a preferred aspect ratio, size constraints in
    // the opposite dimension will transfer through»; Blink
    // `ComputeReplacedSizeInternal`): `width: min-content; height: 100px` —
    // ширина из соотношения (`intrinsic-size-017…025`).
    let kw = |l: Option<Len>| {
        matches!(
            l,
            Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
        )
    };
    let normalized;
    let e = if kw(e.style.width) || kw(e.style.height) {
        let mut copy = e.clone();
        if kw(copy.style.width) {
            copy.style.width = None;
        }
        if kw(copy.style.height) {
            copy.style.height = None;
        }
        normalized = copy;
        &normalized
    } else {
        e
    };
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
    // Замещаемый элемент в СТРОКЕ не сжимается: браузер даёт строке
    // переполниться или перенести коробку целиком (CSS 2.1 §10.3.2, замер
    // wm-propagation-body-040: картинка 340px ужималась на 6-7%). А вот
    // элемент ГИБКОГО РЯДА сжимается как всякий другой — `flex-shrink` ему
    // уже посчитан выше по файлу (единица в гибком окружении, ноль вне его),
    // и глушить его здесь значило запрещать картинке ужиматься даже при
    // явных `min-width: 0` и `flex-shrink: 1` (css-flexbox-1 §7.2).
    let mut d = d;
    match e.style.flex_shrink {
        Some(k) => d.style().flex_shrink = Some(k),
        None => d = d.flex_shrink_0(),
    }
    let d = d;
    // Обособление размера меряет замещаемый элемент КАК ПУСТОЙ (css-contain-2
    // §size containment): своих размеров у картинки нет вовсе — ни сторон, ни
    // соотношения, — их задаёт `contain-intrinsic-size`. Врезка ранняя: ниже
    // по ветке `intrinsic()` вернул бы настоящие 100×100, и всё посчиталось бы
    // по ним.
    if e.style.contains_width() || e.style.contains_height() {
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = e.style.borders();
        let pad_x =
            side(e.style.padding.left) + side(e.style.padding.right) + side(b.left) + side(b.right);
        let pad_y =
            side(e.style.padding.top) + side(e.style.padding.bottom) + side(b.top) + side(b.bottom);
        let used = |explicit: Option<Len>, ci: Option<f32>| match explicit {
            Some(Len::Px(v)) => v,
            _ => ci.unwrap_or(0.0),
        };
        let cw = used(e.style.width, e.style.contain_intrinsic.0);
        let ch = used(e.style.height, e.style.contain_intrinsic.1);
        let mut d = d;
        if e.style.contains_width() && matches!(e.style.width, None | Some(Len::Auto)) {
            d = d.w(px(cw + pad_x));
        }
        if e.style.contains_height() && matches!(e.style.height, None | Some(Len::Auto)) {
            d = d.h(px(ch + pad_y));
        }
        // Источник берётся тем же путём, что и вне обособления: строку со
        // схемой `file:` система уходит скачивать, и картинка молча не
        // рисуется. Растр декодируется своим декодером, вектор растрируется
        // в уже посчитанный размер.
        let local = src
            .strip_prefix("file:///")
            .or_else(|| src.strip_prefix("file://"))
            .or_else(|| (src.starts_with('/') && !src.starts_with("//")).then_some(src));
        let mut image = match crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style)) {
            Some(crate::background::Source::Vector { markup, .. }) if cw > 0.0 && ch > 0.0 => {
                match crate::svg::rasterize(&markup, cw, ch) {
                    Some(r) => gpui::img(r),
                    None => gpui::img(SharedString::from(src.to_string())),
                }
            }
            Some(crate::background::Source::Raster(ready)) => gpui::img(ready),
            _ => match local {
                Some(path) => gpui::img(std::path::PathBuf::from(path)),
                None => gpui::img(SharedString::from(src.to_string())),
            },
        };
        image = image.w(px(cw)).h(px(ch)).object_fit(gpui::ObjectFit::Fill);
        return d.child(image).into_any_element();
    }
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
        // Ключ источника несёт `image-orientation`: развёрнутый и сырой растр
        // — РАЗНЫЕ картинки с разным природным размером, и кэш обязан их
        // различать (css-images-3 §5.4).
        let key = crate::background::key(local.unwrap_or(src), &e.style);
        let own = crate::background::source(&key).and_then(|s| match s {
            crate::background::Source::Raster(image) => Some(image),
            crate::background::Source::Vector { .. }
            | crate::background::Source::Gradient { .. }
            | crate::background::Source::Shape { .. } => None,
        });
        // `object-position` (и точные режимы `object-fit`) — фоновой трубой:
        // concrete object size = размер плитки, позиционирование = origin,
        // клип по content box (css-images-3 §5.1/5.2). Путь включается только
        // при заданной позиции и точной коробке — прочее живёт старым путём.
        // Труба включается при ЛЮБОМ из object-fit/object-position: тест и
        // эталон (background-*) обязаны сойтись одной механикой; позиция по
        // умолчанию — центр (css-images-3 §5.2). Раньше последняя картинка
        // ряда (без object-position) шла другой трубой и расходилась.
        let wants_pipe = e.style.object_position.is_some()
            || e.style
                .object_fit
                .as_deref()
                .is_some_and(|f| matches!(f, "contain" | "cover" | "none" | "scale-down"));
        if let (true, Some(Len::Px(w)), Some(Len::Px(h))) =
            (wants_pipe, e.style.width, e.style.height)
        {
            let pos = e.style.object_position.unwrap_or(crate::computed::BgPos {
                x: Some(Len::Pct(0.5)),
                y: Some(Len::Pct(0.5)),
            });
            use crate::computed::BgSize;
            let mut bgc = crate::computed::Computed::default();
            bgc.bg_image = Some(local.unwrap_or(src).to_string());
            bgc.bg_repeat = Some(crate::computed::BgRepeat::NoRepeat);
            bgc.bg_pos = pos;
            // Стиль трубы собран с нуля, и отказ от EXIF-разворота в него надо
            // положить руками: иначе `image-orientation: none` вместе с
            // `object-fit`/`object-position` уходил бы мимо ключа источника.
            bgc.image_orient_none = e.style.image_orient_none;
            bgc.bg_size = match e.style.object_fit.as_deref() {
                Some("contain") => BgSize::Contain,
                Some("cover") => BgSize::Cover,
                Some("none") => BgSize::Auto,
                Some("scale-down") => {
                    // Меньшее из `none` и `contain`: влезает — своим
                    // размером, нет — вписать.
                    let fits = crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style))
                        .map(|s| s.intrinsic())
                        .is_some_and(|i| {
                            i.w.is_some_and(|iw| iw <= w) && i.h.is_some_and(|ih| ih <= h)
                        });
                    if fits { BgSize::Auto } else { BgSize::Contain }
                }
                _ => BgSize::Fixed(Some(Len::Pct(1.0)), Some(Len::Pct(1.0))),
            };
            if let Some(layer) = crate::background::layer(&bgc) {
                // Внутренняя коробка = content box: поля и рамка остаются
                // на хосте, слой не должен их накрывать.
                return d
                    .child(
                        div()
                            .w(px(w))
                            .h(px(h))
                            .relative()
                            .overflow_hidden()
                            .child(layer),
                    )
                    .into_any_element();
            }
        }
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
        // Векторный источник штатный загрузчик не рисует вовсе — растрируем
        // сами в конечный размер; фон канвы (`style="background:…"` корня) —
        // CSS-слой, не SVG-контент, растеризатор его тоже не рисует.
        let vector: Option<String> =
            crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style)).and_then(|s| match s {
                crate::background::Source::Vector { markup, .. } => Some(markup),
                _ => None,
            });
        let vectorize = |old: gpui::Img, w: f32, h: f32| -> gpui::Img {
            let Some(m) = &vector else { return old };
            let mut out = match crate::svg::rasterize(m, w, h) {
                Some(r) => gpui::img(r),
                None => old,
            };
            if let Some(c) = crate::background::svg_root_background(m) {
                out = out.bg(gpui::Rgba {
                    r: c.r,
                    g: c.g,
                    b: c.b,
                    a: c.a,
                });
            }
            out
        };
        // Контентная поправка: при `box-sizing: border-box` названный размер
        // или предел включает паддинг и рамку — рисунку остаётся остальное.
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let (sub_w, sub_h) = if e.style.border_box == Some(true) {
            let b = e.style.borders();
            (
                side(e.style.padding.left)
                    + side(e.style.padding.right)
                    + side(b.left)
                    + side(b.right),
                side(e.style.padding.top)
                    + side(e.style.padding.bottom)
                    + side(b.top)
                    + side(b.bottom),
            )
        } else {
            (0.0, 0.0)
        };
        let clamp = |l: Option<Len>, sub: f32| match l {
            Some(Len::Px(v)) => Some((v - sub).max(0.0)),
            _ => None,
        };
        let max_w = clamp(e.style.max_width, sub_w);
        let max_h = clamp(e.style.max_height, sub_h);
        // Сперва потолок, затем пол (§10.4).
        let limit = |v: f32, min: Option<f32>, max: Option<f32>| {
            let v = match max {
                Some(m) => v.min(m),
                None => v,
            };
            match min {
                Some(m) => v.max(m),
                None => v,
            }
        };
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: подсказка, ПЕРЕНЕСЁННАЯ через отношение
        // сторон, в автоматическом минимуме гибкого элемента (css-flexbox
        // §4.5). Патч вендора написан (`FlexItem::aspect_ratio` +
        // зажим `min_content` перенесённым размером в
        // `vendor/taffy/src/compute/flexbox.rs`). Срез flex/aspect/ratio
        // (493 пары, 416 зелёных): 416, ни одной пары в любую сторону, и
        // целевые `flexbox-min-{width,height}-auto-002` остались 0.53/0.96/
        // 1.56/2.10. Причина: отношение сторон ставится на ВНУТРЕННЮЮ
        // картинку (`image.style().aspect_ratio` ниже), а гибкий элемент —
        // это ВНЕШНЯЯ коробка замещённого, и у её узла отношения нет вовсе.
        // ПЕРЕПРОВЕРЕНО ПОСЛЕ правок §10.4 (пределы замещённого): патч
        // вендора и отношение на внешней коробке (теперь — только когда хоть
        // одна сторона не задана) возвращены вместе. Срез из 12 пар
        // `flexbox-min-*-auto-*`: сдвинулась одна и та же пара 0.96 → 0.94,
        // флипов ноль. Корень этих пар лежит не в автоматическом минимуме.
        // Проверено и это: отношение сторон ПОСТАВЛЕНО и на внешнюю коробку
        // (`image_with`, сразу после `flex_shrink_0`), патч вендора вернули —
        // срез из тех же 493 пар опять 416, целевые пары 0.53/1.56/2.10 без
        // движения, и только две из шести шевельнулись 0.96 → 0.91. Значит
        // автоматический минимум этим парам не корень; корень искать заново.
        // Использованный размер замещённого после §10.4: если пределы его
        // изменили, коробка обязана ужаться вместе с рисунком.
        let mut узкая: Option<(f32, f32)> = None;
        let natural_ratio = || {
            crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style))
                .map(|s| s.intrinsic())
                .and_then(|i| {
                    i.ratio.or(match (i.w, i.h) {
                        (Some(w), Some(h)) if h > 0.0 => Some(w / h),
                        _ => None,
                    })
                })
        };
        let ratio_of = || {
            // ЗАЯВЛЕННОЕ отношение сильнее природного (css-sizing-4 §4):
            // `aspect-ratio: 1/1` на картинке 2:1 обязан её переформатировать.
            // Запись `auto <ratio>` — обратный случай: природное сильнее, а
            // заявленное служит запасным (§5.1).
            if let Some(r) = e.style.aspect_ratio.filter(|r| *r > 0.0) {
                return Some(r);
            }
            // `auto <ratio>`: природное сильнее, заявленное — запасное.
            natural_ratio()
                .filter(|r| *r > 0.0)
                .or(e.style.aspect_ratio_auto.filter(|r| *r > 0.0))
        };
        if let (Some(Len::Px(w)), Some(Len::Px(h))) = (e.style.width, e.style.height) {
            image = vectorize(image, (w - sub_w).max(1.0), (h - sub_h).max(1.0))
                .w(px(w))
                .h(px(h));
        } else if let (Some(Len::Px(w)), None | Some(Len::Auto)) = (e.style.width, e.style.height) {
            // Заданная ширина + auto-высота: высота из соотношения (§10.6.2),
            // без соотношения — своя, резерв 150.
            // §10.7: пределы зажимают и НАЗВАННУЮ сторону тоже. Коробку
            // `apply` уже зажал, а картинка шла как написана и вылезала за неё.
            let cw = limit((w - sub_w).max(0.0), clamp(e.style.min_width, sub_w), max_w);
            let ch = match ratio_of() {
                Some(r) if r > 0.0 => cw / r,
                _ => crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style))
                    .and_then(|s| s.intrinsic().h)
                    .unwrap_or(150.0),
            };
            // §10.4: пределы держат ВЫВЕДЕННУЮ сторону тоже — заданная
            // остаётся как написана, а высота из соотношения обязана влезть
            // в свой потолок и пол. Замерено отдельно: 0 и 0 — правка по
            // спеке, счёт на ней не держится.
            let ch_free = ch;
            let ch = limit(ch, clamp(e.style.min_height, sub_h), max_h);
            // §10.4: когда потолок или пол ИЗМЕНИЛИ выведенную сторону,
            // заданная пересчитывается по соотношению — коробка остаётся
            // пропорциональной, а не растягивается. `width: 200px` при
            // `max-height: 50px` у картинки 100×50 — это 100×50, а не
            // 200×50.
            let cw = match ratio_of() {
                Some(r) if r > 0.0 && (ch - ch_free).abs() > 0.01 => {
                    let w = limit(ch * r, clamp(e.style.min_width, sub_w), max_w);
                    // Коробка ужимается ТОЛЬКО когда предел и правда изменил
                    // выведенную сторону: иначе высота у неё остаётся `auto`,
                    // и подстановка ломала замещённые без пределов вовсе
                    // (`replaced-intrinsic-004`).
                    узкая = Some((w, ch));
                    w
                }
                _ => cw,
            };
            image = vectorize(image, cw.max(1.0), ch.max(1.0))
                .w(px(cw))
                .h(px(ch))
                .object_fit(gpui::ObjectFit::Fill);
        } else if let (None | Some(Len::Auto), Some(Len::Px(h))) = (e.style.width, e.style.height) {
            // Зеркально: заданная высота + auto-ширина (§10.3.2).
            let ch = limit(
                (h - sub_h).max(0.0),
                clamp(e.style.min_height, sub_h),
                max_h,
            );
            let cw = match ratio_of() {
                Some(r) if r > 0.0 => ch * r,
                _ => crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style))
                    .and_then(|s| s.intrinsic().w)
                    .unwrap_or(300.0),
            };
            let cw_free = cw;
            let cw = limit(cw, clamp(e.style.min_width, sub_w), max_w);
            // Зеркально §10.4: изменённая ширина тянет за собой высоту.
            let ch = match ratio_of() {
                Some(r) if r > 0.0 && (cw - cw_free).abs() > 0.01 => {
                    let h = limit(cw / r, clamp(e.style.min_height, sub_h), max_h);
                    узкая = Some((cw, h));
                    h
                }
                _ => ch,
            };
            image = vectorize(image, cw.max(1.0), ch.max(1.0))
                .w(px(cw))
                .h(px(ch))
                .object_fit(gpui::ObjectFit::Fill);
        } else if let (Some(Len::Pct(_)), None | Some(Len::Auto)) = (e.style.width, e.style.height)
        {
            // Доля ширины при auto-высоте: ширину даёт содержащий блок, высоту
            // — собственное соотношение сторон (§10.3.2, §10.6.2). Раньше доля
            // не попадала ни в одну ветку, и замещаемый рисовался СВОИМ
            // пикселем (`absolute-replaced-width-006`: 15×15 вместо 96×96).
            // Долю по этой оси УЖЕ поставил хозяин (`styled_div(e)` несёт
            // стиль элемента целиком), поэтому картинке остаётся заполнить
            // его: `relative(kw)` внутри давал долю ОТ ДОЛИ — `width: 50%`
            // выходило четвертью содержащего блока.
            image = image.w(gpui::relative(1.0));
            if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
                image.style().aspect_ratio = Some(r);
            }
        } else if let (None | Some(Len::Auto), Some(Len::Pct(_))) = (e.style.width, e.style.height)
        {
            // Зеркально: доля высоты при auto-ширине.
            image = image.h(gpui::relative(1.0));
            if let Some(r) = ratio_of().filter(|r| *r > 0.0) {
                image.style().aspect_ratio = Some(r);
            }
        } else if !matches!(e.style.width, None | Some(Len::Auto))
            && !matches!(e.style.height, None | Some(Len::Auto))
        {
            // Обе стороны заданы, но не обе в точках: каждая ось — своим
            // значением. `width:100%; height:100%` растягивается на коробку
            // (sizing-percentages-replaced-orthogonal-001), а смешанная
            // запись `width:50%; height:15px` раньше падала в size_full и
            // ТЕРЯЛА пиксельную сторону (inline-replaced-width-011..015).
            image = match (e.style.width, e.style.height) {
                (Some(Len::Pct(_)), Some(Len::Px(h))) => image.w(gpui::relative(1.0)).h(px(h)),
                (Some(Len::Px(w)), Some(Len::Pct(_))) => image.w(px(w)).h(gpui::relative(1.0)),
                _ => image.size_full(),
            };
        } else if !matches!(e.style.width, Some(Len::Px(_)))
            && !matches!(e.style.height, Some(Len::Px(_)))
        {
            let both_auto = matches!(e.style.width, None | Some(Len::Auto))
                && matches!(e.style.height, None | Some(Len::Auto));
            if both_auto && let Some(ready) = crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style)) {
                // Авто-размер замещаемого считается ЗДЕСЬ, а не отдаётся
                // загрузчику картинок: тот работает асинхронно, и в первом
                // кадре коробка выходила нулевой высоты (box-sizing-007 —
                // страница вовсе без квадратов). Недостающая сторона
                // достраивается соотношением, резерв — 300×150
                // (CSS 2.1 §10.3.2/§10.6.2, css-images-3 §default-sizing).
                let side = ready.intrinsic();
                let (w0, h0) = match (side.w, side.h, side.ratio) {
                    (Some(w), Some(h), _) => (w, h),
                    (Some(w), None, Some(r)) => (w, w / r),
                    (None, Some(h), Some(r)) => (h * r, h),
                    (Some(w), None, None) => (w, 150.0),
                    (None, Some(h), None) => (300.0, h),
                    (None, None, Some(r)) => {
                        // §10.3.2, последний пункт: при обеих `auto` и одном
                        // лишь соотношении ширина берётся из содержащего
                        // блока, а не из резерва. Резерв остаётся, когда
                        // ширину взять неоткуда (`ratio-2.svg` в
                        // `visudet/replaced-elements-*`: мы рисовали 300×150,
                        // эталон — 200×100 по `div { width: 200px }`).
                        let w = CB_WIDTH
                            .get()
                            .filter(|v| *v > 0.0)
                            .unwrap_or_else(|| (150.0 * r).min(300.0));
                        (w, w / r)
                    }
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: считать ДОЛЮ собственного
                    // размера рисунка (`<svg width="100%">` и SVG вовсе без
                    // атрибутов — у него подразумевается `100%`) и разрешать
                    // её от содержащего блока. Написано было и в `Intrinsic`
                    // (поля `w_pct`/`h_pct` в `background::svg_size`), и
                    // здесь. Срез из 943 пар (replaced/normal-flow/svg/
                    // background-size/image): 880 → 874. `replaced-intrinsic-
                    // 002` пошла 6.10 → 1.41, но вся семья `replaced-
                    // elements-*` рухнула (0.01 → 24.17 и родня).
                    // Причина в РАЗНИЦЕ ДВУХ СЛУЧАЕВ: у `<img>` доля значит
                    // «своего размера нет» и работает умолчальный размер
                    // 300×150 (css-images-3 §5.2), а у `<object>` SVG — это
                    // вложенный ДОКУМЕНТ, и его `100%` считается от коробки
                    // объекта, то есть от содержащего блока. Возвращать
                    // вместе с этим различением.
                    (None, None, None) => (300.0, 150.0),
                };
                if w0 > 0.0 && h0 > 0.0 {
                    // §10.4: пределы держат соотношение — сперва потолки,
                    // затем полы.
                    let min_w = clamp(e.style.min_width, sub_w);
                    let min_h = clamp(e.style.min_height, sub_h);
                    let mut scale = 1.0f32;
                    if let Some(m) = max_w {
                        scale = scale.min(m / w0);
                    }
                    if let Some(m) = max_h {
                        scale = scale.min(m / h0);
                    }
                    if let Some(m) = min_w {
                        scale = scale.max(m / w0);
                    }
                    if let Some(m) = min_h {
                        scale = scale.max(m / h0);
                    }
                    // Без СОБСТВЕННОГО соотношения стороны независимы: потолок
                    // высоты режет только высоту, и ширина остаётся своей
                    // (§10.4, таблица «no intrinsic ratio»). Прежде общий
                    // множитель ужимал и её — картинка без соотношения под
                    // `max-height: 20px` выходила у́же в пятнадцать раз
                    // (`replaced-elements-max-height-20`, снимки `no-ratio` и
                    // `height-25-no-ratio`).
                    let без_соотношения = side.ratio.is_none()
                        && !(side.w.is_some() && side.h.is_some());
                    let (rw, rh) = if без_соотношения {
                        (
                            limit(w0, min_w, max_w),
                            limit(h0, min_h, max_h),
                        )
                    } else {
                        (w0 * scale, h0 * scale)
                    };
                    image = vectorize(image, rw, rh)
                        .w(px(rw))
                        .h(px(rh))
                        .object_fit(gpui::ObjectFit::Fill);
                }
            } else if (max_w.is_some() || max_h.is_some())
                && let Some(ready) = crate::background::source(&crate::background::key(local.unwrap_or(src), &e.style))
            {
                let side = ready.intrinsic();
                if let (Some(w0), Some(h0)) = (side.w, side.h)
                    && w0 > 0.0
                    && h0 > 0.0
                {
                    let mut scale = 1.0f32;
                    if let Some(m) = max_w {
                        scale = scale.min(m / w0);
                    }
                    if let Some(m) = max_h {
                        scale = scale.min(m / h0);
                    }
                    if scale < 1.0 {
                        image = vectorize(image, w0 * scale, h0 * scale)
                            .w(px(w0 * scale))
                            .h(px(h0 * scale))
                            .object_fit(gpui::ObjectFit::Fill);
                    }
                }
            }
        }
        // CSS-умолчание для замещаемого содержимого — заполнить коробку, но
        // держится оно на СОБСТВЕННОМ соотношении сторон картинки: заданная
        // одна сторона задаёт вторую. Соотношения мы до загрузки не знаем,
        // поэтому вторая сторона остаётся своей, и заполнение растягивало бы
        // рисунок в чужой прямоугольник (замерено: `flexbox-min-width-auto`
        // ушёл в минус шестью парами). Вписывание в этих условиях ближе.
        image = match e.style.object_fit.as_deref() {
            Some("cover") => image.object_fit(gpui::ObjectFit::Cover),
            Some("contain") => image.object_fit(gpui::ObjectFit::Contain),
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
            // Заявленное `aspect-ratio` — preferred aspect ratio КОРОБКИ
            // (css-sizing-4 §5.1): вторая сторона уже посчитана из него, и
            // рисунок заполняет коробку (`object-fit: fill` по умолчанию,
            // css-images-3 §5.2), а не вписывается по своему соотношению
            // (`replaced-element-0*`, `flex-aspect-ratio-0*`).
            _ if e.style.aspect_ratio.is_some() => image.object_fit(gpui::ObjectFit::Fill),
            _ => image.object_fit(gpui::ObjectFit::Contain),
        };
        let d = match узкая {
            Some((w, h)) => d.w(px(w + sub_w)).h(px(h + sub_h)),
            None => d,
        };
        return d.child(image).into_any_element();
    }
    // Картинки БЕЗ АДРЕСА вовсе (`<img>` без `src`) не существует: коробки
    // она не порождает и в замер по содержимому не входит (HTML §4.8.4.4 —
    // «if the element has no src attribute … the element represents
    // nothing»). Подпись-заглушка тут вредна: она даёт ширину, и
    // `width: max-content` вокруг такой картинки выходил шире содержимого
    // (`white-space-intrinsic-size-024/025`).
    if e.attr("src").is_none_or(|s| s.trim().is_empty()) && e.attr("alt").is_none() {
        return d.into_any_element();
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

/// Пункт списка не сжимается, как и любой блок потока.
///
/// `blocks()` ставит `flex-shrink: 0` каждому ребёнку потока (умолчание GPUI —
/// 1.0), а строки списка строятся мимо него, напрямую. В колонке нулевой
/// высоты обе строки сжимались до автоминимума: пункт высотой 100 выходил
/// двадцатью точками (`flex-box-wrap-ref`).
///
/// Список со СВОИМ гибким или сеточным видом — исключение: его пункты
/// настоящие элементы контейнера, и по css-flexbox-1 §7.2 умолчание
/// `flex-shrink` у них 1.
fn shrink0(d: gpui::Div, li: &Element, list: &Element) -> gpui::Div {
    let flex_parent = matches!(
        list.style.display,
        Some(Display::Flex) | Some(Display::InlineFlex) | Some(Display::Grid) | Some(Display::InlineGrid)
    );
    if li.style.flex_shrink.is_none() && !flex_parent {
        d.flex_shrink_0()
    } else {
        d
    }
}
fn list(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let ordered = e.tag == "ol";
    let mut rows = vec![];
    for child in &e.children {
        let Node::Element(li) = child else { continue };
        if li.tag != "li" {
            // ★ ЗАМЕРЕНО И ОТКАЧЕНО: рисовать не-`li` ребёнка списка обычным
            // потоком (эталон `flexbox_direction-row-reverse-ref` — `<ul>` из
            // `<span>` пуст). Срез списков+выключки 383 пары: приобретено 0,
            // потеряно 2 (`foo-counter-reversed-007a/b` 0.38 -> 0.53).
            continue;
        }
        // Номер пункта считает ОБЩИЙ счётчик `list-item` (css-lists-3
        // §list-item-counter): он один знает и `<ol start>`, и `<li value>`,
        // и вложенные списки. Своей нумерации у отрисовки больше нет.
        let idx = li.list_item.unwrap_or(0);
        // Вид маркера задаёт документ; без указания — умолчание тега.
        // Строковый маркер берётся дословно и без суффикса-точки.
        let text_marker = li
            .style
            .marker_text
            .clone()
            .or_else(|| e.style.marker_text.clone());
        let kind = li
            .style
            .list_style_type
            .clone()
            .or_else(|| e.style.list_style_type.clone());
        let marker = if let Some(t) = text_marker {
            t
        } else {
            // Умолчание тега: у нумерованного перечня десятичный счёт, у
            // списка возможностей — точка.
            let name = kind.unwrap_or_else(|| if ordered { "decimal" } else { "disc" }.to_string());
            crate::counter_style::marker_repr(idx, &name)
        };
        // `list-style: none` — на списках верстают навигацию и наборы чипов,
        // и точки там лишние.
        let no_marker = e.style.no_marker == Some(true) || li.style.no_marker == Some(true);
        let merged = inline::inherit(inherited, &li.style);
        // `inside`: маркер — ПЕРВЫЙ инлайновый кусок содержимого пункта
        // (css-lists-3 §4), поэтому он просто дописывается текстом в начало.
        // Своей колонки при этом нет, и текст пункта начинается там же, где
        // у обычного абзаца.
        let inside = merged.list_style_inside == Some(true);
        if inside {
            let mut kids: Vec<Node> = Vec::with_capacity(li.children.len() + 1);
            if !no_marker {
                kids.push(Node::Text(marker));
            }
            kids.extend(li.children.iter().cloned());
            rows.push(
                shrink0(styled_div_with(li, &merged), li, e)
                    .flex()
                    .flex_col()
                    .children(blocks(&kids, &merged, opts))
                    .into_any_element(),
            );
            continue;
        }
        rows.push(
            shrink0(styled_div_with(li, &merged), li, e)
                .flex()
                .flex_row()
                .gap_x(px(6.))
                .items_start()
                .children((!no_marker).then(|| {
                    // Знаки маркера набираются шрифтом и цветом ПУНКТА:
                    // отдельной коробке текстовые свойства не достаются сами,
                    // и маркер выходил чужой гарнитурой и кеглем. Выключка
                    // текста на него НЕ переносится: маркер стоит у своего
                    // края колонки, куда бы ни равнялся текст пункта
                    // (`list-style-position-018`).
                    crate::apply::apply_text(div(), &merged)
                        .text_left()
                        .flex_shrink_0()
                        .min_w(px(14.))
                        // Хвост срезается только у СОБСТВЕННЫХ отбивок движка
                        // (обычный пробел после номера пункта). Авторская
                        // строка `list-style-type: "..."` идёт дословно:
                        // `trim_end` в Rust считает пробелом и U+00A0, а
                        // неразрывный пробел в такой строке — ЗНАЧАЩИЙ.
                        // Замер нейтрален (срез 2268 пар, +0/-0: у
                        // `list-style-type-string-005a/b/-006` остаток не
                        // здесь), но срезать значащий знак всё равно нельзя.
                        .child(SharedString::from(
                            marker.trim_end_matches(' ').to_string(),
                        ))
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
/// Уровень коробки для схлопывания полей: тег — только УМОЛЧАНИЕ, вид из
/// каскада сильнее. `e.inline` ставится по имени тега (`dom.rs`), поэтому
/// `<span style="display:block">` доезжал сюда «строчным», и поля соседей
/// через него не примыкали — эталоны `flex-direction-column*` разводило на
/// лишние 16 точек (в них разметка именно такая).
fn inline_level_box(e: &Element) -> bool {
    match e.style.display {
        // Строчными считаются только НАСТОЯЩИЕ строчные виды. Первый заход
        // писал `Some(_) => true`, и в строчные попадали лунки сетки: CSS2
        // +6, а css-grid −46 (`column-align-items-*`, `*-dense-packing-*`
        // уходили 0.00 → 1.5-3.2). `display: inline` после каскада — это
        // `InlineBlock` с пометкой `inline_display` (см. `computed.rs`).
        Some(Display::InlineBlock)
        | Some(Display::InlineFlex)
        | Some(Display::InlineGrid)
        | Some(Display::InlineTable) => true,
        Some(_) => false,
        None => e.inline,
    }
}

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
    // Презентационный `cellspacing="N"`: хинт стоит НИЖЕ авторского
    // `border-spacing`, но выше умолчания браузера. Каскад в вычисленном
    // стиле уже слит, поэтому хинт применяется, только когда значение
    // равно умолчанию тега `<table>` (2px) — авторская двойка при живом
    // атрибуте встречается на порядки реже, чем сами атрибуты.
    let cell_spacing_attr = e
        .attr("cellspacing")
        .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok());
    let ua_default = matches!(
        e.style.border_spacing,
        Some((Some(Len::Px(2.0)), Some(Len::Px(2.0))))
    );
    let border_spacing = match (cell_spacing_attr, ua_default, e.style.border_spacing) {
        (Some(v), true, _) | (Some(v), _, None) => Some((Some(Len::Px(v)), Some(Len::Px(v)))),
        _ => e.style.border_spacing,
    };
    // Раздельные рамки — умолчание; при `collapse` зазора между ячейками нет.
    let spacing = match (e.style.border_collapse, border_spacing) {
        (Some(true), _) => (0.0, 0.0),
        (None, _) if e.attr("rules").is_some() => (0.0, 0.0),
        // Заданный `border-spacing` перекрывает умолчание браузера в 2px.
        // Шрифтовые единицы разрешаются по кеглю САМОЙ таблицы: `1em` роняло
        // зазор в ноль, и вся подсемья Хикси с `border-spacing: 1em`
        // расходилась с эталоном ровно на зазор.
        (_, Some((x, y))) => {
            let em = atom_base_font(inherited, opts);
            let px_of = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_) | Len::Ic(_))) => {
                    crate::metrics::fallback_len_px(l, "", em).unwrap_or(0.0)
                }
                _ => 0.0,
            };
            (px_of(x), px_of(y))
        }
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
        // Роль группы задаётся ТЕГОМ ИЛИ `display` (§17.5.3): `div` с
        // `table-header-group` встаёт первым так же, как `<thead>`.
        let kind_of = |g: &Element| -> Option<u8> {
            match g.tag.as_str() {
                "thead" => Some(0),
                "tbody" => Some(1),
                "tfoot" => Some(2),
                _ => g.style.row_group_kind,
            }
        };
        let first_with = |k: u8| -> Option<u64> {
            fixed.iter().find_map(|n| match n {
                Node::Element(g) if kind_of(g) == Some(k) => Some(g.node_id),
                _ => None,
            })
        };
        // Заголовочной и подвальной становится только ПЕРВАЯ группа
        // своего рода; последующие — обычные группы рядов.
        let head = first_with(0);
        let foot = first_with(2);
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
    let (from_cols, cols_collapsed, cols_pct) =
        col_element_widths(&e.children, table_font, &table_family);
    // §17.5.2.1: при фиксированной раскладке и ЗАДАННОЙ ширине стола
    // колонка, которой места уже не осталось, получает НОЛЬ — и ячейка в ней
    // не вправе распирать дорожку своим отступом, иначе она вылезает за край
    // стола (`fixed-table-layout-025/028..031`). Ширина стола АВТО в этот
    // гейт не попадает: на ней держится семья `margin-*-applies-to-*`, на
    // которой умерли три прошлых захода (см. записи ниже по ячейке).
    let zero_cols: Vec<bool> = {
        let px_of = |l: Option<Len>| match l {
            Some(Len::Px(v)) => Some(v),
            _ => None,
        };
        // Ширина стола ДОЛЕЙ решается от содержащего блока: `width: 80%` в
        // шестистах сорока — это 512 (`fixed-table-layout-023`).
        let своя_ширина = match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        };
        match (e.style.table_fixed == Some(true), своя_ширина) {
            (true, Some(tw)) => {
                let side = |l: Option<Len>| px_of(l).unwrap_or(0.0);
                let tbz = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                let base = tw - side(tbz.left) - side(tbz.right) - gap * (f32::from(cols) + 1.0);
                let mut declared = vec![None::<f32>; cols as usize];
                for (i, w) in from_cols.iter().enumerate() {
                    if let (Some(w), Some(slot)) = (w, declared.get_mut(i)) {
                        *slot = Some(*w);
                    }
                }
                if let Some(row) = row_elements.first() {
                    let mut i = 0usize;
                    for c in &row.children {
                        let Node::Element(cell) = c else { continue };
                        if !is_cell(cell) {
                            continue;
                        }
                        let span = cell
                            .attr("colspan")
                            .and_then(|v| v.parse::<usize>().ok())
                            .unwrap_or(1)
                            .max(1);
                        // Доля ячейки первого ряда — тоже ЗАЯВЛЕННАЯ
                        // ширина: `width: 50%` при столе в сто точках это
                        // пятьдесят, и вместе со своим отступом дорожка
                        // забирает всё место (`fixed-table-layout-025`).
                        let своя = match cell.style.width {
                            Some(Len::Px(v)) => Some(v),
                            Some(Len::Pct(k)) => Some(base.max(0.0) * k),
                            _ => None,
                        };
                        if span == 1
                            && let Some(w) = своя
                            && let Some(slot) = declared.get_mut(i)
                            && slot.is_none()
                        {
                            let b = cell.style.borders();
                            *slot = Some(
                                w + side(cell.style.padding.left)
                                    + side(cell.style.padding.right)
                                    + side(b.left)
                                    + side(b.right),
                            );
                        }
                        i += span;
                    }
                }
                let sum: f32 = declared.iter().flatten().sum();
                let свободно = base - sum;
                (0..cols as usize)
                    .map(|i| declared.get(i).copied().flatten().is_none() && свободно <= 0.5)
                    .collect()
            }
            _ => vec![false; cols as usize],
        }
    };
    let mut busy: Vec<u16> = vec![0; cols as usize];
    // §17.6.2.1: ширина линии сетки — ПОБЕДИВШАЯ среди примыкающих коробок,
    // и внутрь каждой уходит её половина. Пока половина бралась от своей
    // кромки, дорожки выходили у́же эталона ровно на разницу.
    //
    // Разбор ведётся только по ячейкам и рамке таблицы: у рядов, колонок и
    // групп победа решается на краске, и пускать их в раскладку нельзя —
    // семья `border-*-applies-to-*` держится ровно на этом.
    let collapse_cells_pre = e.style.border_collapse == Some(true)
        || (e.style.border_collapse.is_none() && e.attr("rules").is_some());
    let px_of_pre = |l: Option<Len>| crate::metrics::spacing_px(l, &table_family, table_font);
    let tb = e.style.borders();
    let bw_pre = [
        px_of_pre(tb.top),
        px_of_pre(tb.right),
        px_of_pre(tb.bottom),
        px_of_pre(tb.left),
    ];
    let mut win_edges: std::collections::HashMap<u64, [f32; 4]> = Default::default();
    // Наружные полуширины таблицы: победитель на ЕЁ линиях. Копится прямо в
    // условиях края — свод по всем клеткам затягивал сюда и внутренние линии,
    // и таблица без рамки получала паддинг от кромок середины
    // (`fixed-table-layout-027`: крайние дорожки схлопывались в ноль).
    let mut outer_win = [0.0f32; 4];
    if collapse_cells_pre {
        struct Cel {
            r: usize,
            c: usize,
            sr: usize,
            sc: usize,
            w: [f32; 4],
            id: u64,
        }
        let mut cels: Vec<Cel> = vec![];
        let mut occ: Vec<u16> = vec![0; cols as usize];
        let mut r = 0usize;
        for row in &row_elements {
            for slot in occ.iter_mut() {
                *slot = slot.saturating_sub(1);
            }
            let mut c = 0usize;
            for child in &row.children {
                let Node::Element(cell) = child else { continue };
                if !is_cell(cell) {
                    continue;
                }
                while c < occ.len() && occ[c] > 0 {
                    c += 1;
                }
                let sc = cell
                    .attr("colspan")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let sr = cell
                    .attr("rowspan")
                    .and_then(|v| v.parse::<usize>().ok())
                    .unwrap_or(1)
                    .max(1);
                let b = cell.style.borders();
                cels.push(Cel {
                    r,
                    c,
                    sr,
                    sc,
                    w: [px_of_pre(b.top), px_of_pre(b.right), px_of_pre(b.bottom), px_of_pre(b.left)],
                    id: cell.node_id,
                });
                for k in c..(c + sc).min(occ.len()) {
                    occ[k] = occ[k].max(sr as u16);
                }
                c += sc;
            }
            r += 1;
        }
        let rows_n = r;
        for a in cels.iter() {
            let mut w = a.w;
            for b in cels.iter() {
                if b.id == a.id {
                    continue;
                }
                let cols_over = a.c < b.c + b.sc && b.c < a.c + a.sc;
                let rows_over = a.r < b.r + b.sr && b.r < a.r + a.sr;
                if cols_over && b.r + b.sr == a.r {
                    w[0] = w[0].max(b.w[2]);
                }
                if cols_over && a.r + a.sr == b.r {
                    w[2] = w[2].max(b.w[0]);
                }
                if rows_over && b.c + b.sc == a.c {
                    w[3] = w[3].max(b.w[1]);
                }
                if rows_over && a.c + a.sc == b.c {
                    w[1] = w[1].max(b.w[3]);
                }
            }
            // Внешние линии спорят с рамкой самой таблицы.
            if a.r == 0 {
                w[0] = w[0].max(bw_pre[0]);
                outer_win[0] = outer_win[0].max(w[0]);
            }
            if a.c == 0 {
                w[3] = w[3].max(bw_pre[3]);
                outer_win[3] = outer_win[3].max(w[3]);
            }
            if a.c + a.sc >= cols as usize {
                w[1] = w[1].max(bw_pre[1]);
                outer_win[1] = outer_win[1].max(w[1]);
            }
            if a.r + a.sr >= rows_n {
                w[2] = w[2].max(bw_pre[2]);
                outer_win[2] = outer_win[2].max(w[2]);
            }
            win_edges.insert(a.id, w);
        }
    }
    let outer_win = [
        outer_win[0].max(bw_pre[0]),
        outer_win[1].max(bw_pre[1]),
        outer_win[2].max(bw_pre[2]),
        outer_win[3].max(bw_pre[3]),
    ];
    // Вертикальность САМОЙ таблицы: `inherited` внутри цикла рядов
    // перекрыт слоем группы строк (`<tbody>` с письмом травил гейты,
    // table-progression-htb-001 — письмо к рядам и группам НЕ применяется).
    let table_is_vertical = e.style.vertical == Some(true) || inherited.vertical == Some(true);
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
                let source = if table_vertical || orthogonal {
                    // У ортогональной ячейки width несёт ЛОГИЧЕСКИЙ
                    // inline-size — физически это ВЫСОТА, не колонка
                    // (table-cell-align-005/006); колонку задаёт block-size,
                    // осевший в height.
                    cell.style.height
                } else {
                    cell.style.width
                };
                // ЗАМЕРЕНО: CSS2 5117 -> 5128, oldfront 2352 -> 2340. Двенадцать
                // потерянных — семья `css-writing-modes/table-progression-*`:
                // её эталон горизонтальный, а тест вертикальный, и слагаемые
                // ложатся по разным осям. Пробовали отсекать вертикальное
                // письмо (2339) и считать добавку только горизонтальной (2338)
                // — обе хуже. Возвращаться вместе с вертикальной табличной
                // раскладкой.
                //
                // Дорожка = `width` ячейки ПЛЮС её горизонтальные отступы и
                // рамки (§17.5.2.1, коробка содержимого); в сросшейся модели
                // рамка входит половиной. Та же формула стоит в ветке первого
                // ряда ниже; без неё колонка выходила у́же ячейки на её рамку
                // (`margin-applies-to-001..007`).
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(p)) => p,
                    _ => 0.0,
                };
                let extra = if cell.style.border_box == Some(true) {
                    0.0
                } else {
                    let b = cell.style.borders();
                    let border = if table_vertical || orthogonal {
                        side(b.top) + side(b.bottom)
                    } else {
                        side(b.left) + side(b.right)
                    };
                    let pad = if table_vertical || orthogonal {
                        side(cell.style.padding.top) + side(cell.style.padding.bottom)
                    } else {
                        side(cell.style.padding.left) + side(cell.style.padding.right)
                    };
                    // В сросшейся модели дорожка считает ПОЛОВИНУ победившей
                    // линии — ту же, что легла в паддинг ячейки; своя кромка
                    // могла быть у́же соседней.
                    pad + if e.style.border_collapse == Some(true) {
                        let w = win_edges.get(&cell.node_id).copied().unwrap_or([
                            side(cell.style.borders().top),
                            side(cell.style.borders().right),
                            side(cell.style.borders().bottom),
                            side(cell.style.borders().left),
                        ]);
                        if table_vertical || orthogonal {
                            (w[0] + w[2]) / 2.0
                        } else {
                            (w[1] + w[3]) / 2.0
                        }
                    } else {
                        border
                    }
                };
                match source {
                    Some(Len::Px(v)) => {
                        let v = v + extra;
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
                        let v = crate::metrics::spacing_px(Some(l), &family, size) + extra;
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
    if {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| std::env::var("TCA_DBG").is_ok());
        *ON
    } {
        eprintln!("TCA cols={col_widths:?}");
    }
    // Слой ГРУПП КОЛОНОК и слой КОЛОНОК — две полосы, снизу вверх (§17.5.1:
    // «the next layer contains the column groups… on top of the column groups
    // are the areas representing the column boxes»). У каждой свой буфер
    // проб: площадь группы шире колоночной, и `background-position` у них
    // разный. Обе идут ПЕРЕД рядами: колонка рисуется ниже ряда
    // (css-tables-3 §layers).
    let grp_els = colgroup_elements(&e.children);
    let col_els = col_elements(&e.children);
    let mut grp_rects: Vec<Option<crate::interact::RowRects>> = vec![None; cols as usize];
    let mut col_rects: Vec<Option<crate::interact::RowRects>> = vec![None; cols as usize];
    let have_rows = !row_elements.is_empty();
    push_col_bands(
        &grp_els,
        opts.doc_salt,
        have_rows,
        &mut grp_rects,
        &mut cells,
    );
    push_col_bands(
        &col_els,
        opts.doc_salt,
        have_rows,
        &mut col_rects,
        &mut cells,
    );
    // Ширины рамки самой таблицы: крайние ячейки расползаются фоном на её
    // половину в сросшейся модели.
    // Кегль СВОЙ, а не жёсткие 16 точек: `border: 0.5em` у таблицы с крупным
    // шрифтом давал вчетверо тоньше линию (`border-conflict-element-001d/e`).
    let table_em = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => 16.0,
    };
    let table_family = inherited.font_family.clone().unwrap_or_default();
    let px_of = |l: Option<Len>| crate::metrics::spacing_px(l, &table_family, table_em);
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
    // Границы снимаются с самих РЯДОВ, а не с детей таблицы: группа может
    // стоять на любом теге через `display: table-row-group`, её ряды — через
    // `display: table-row`, и до фильтра `thead|tbody|tfoot` они не доходили
    // (`border-*-width-applies-to-001/002/003`). Первым и последним рядом
    // группы считаются края её НЕПРЕРЫВНОГО куска в собранном порядке —
    // после перестановки §17.5.3 он уже правильный.
    let mut group_of: std::collections::HashMap<u64, (&Element, bool, bool)> =
        std::collections::HashMap::new();
    {
        let mut i = 0usize;
        while i < rows.len() {
            let Some(g) = rows[i].1.3 else {
                i += 1;
                continue;
            };
            let mut j = i;
            while j < rows.len() && rows[j].1.3.map(|o| o.node_id) == Some(g.node_id) {
                j += 1;
            }
            for k in i..j {
                group_of.insert(rows[k].0.node_id, (g, k == i, k + 1 == j));
            }
            i = j;
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
        // Фон ГРУППЫ рядов красится так же, как фон ряда (§17.5.1, слой 3):
        // полоса идёт от левого края крайней левой колонки до правого края
        // крайней правой и обрезается прямоугольниками ячеек. Своей коробки у
        // группы в сетке нет, поэтому картинка и градиент пропадали вовсе —
        // рисовался только сплошной цвет, который течёт вниз наследованием.
        let grp_band: Option<crate::interact::RowRects> = carry.3.and_then(|g| {
            (g.style.bg_image.is_some()
                || g.style.gradient_raw.is_some()
                || !g.style.shadows.is_empty())
            .then(|| crate::interact::row_rects_for(g.node_id ^ opts.doc_salt))
        });
        if let (Some(rects), Some(g)) = (&grp_band, carry.3)
            && group_of
                .get(&row.node_id)
                .is_some_and(|(_, first, _)| *first)
        {
            let mut band_style = g.style.clone();
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
                group_layer = inline::inherit(inherited, &g.style);
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
        // `inherited` — УЖЕ слитый стиль самой таблицы, поэтому второй мерж
        // сырого `e.style` разрешал относительные единицы повторно:
        // `font-size: 2em` на теге давал ячейке 64 точки вместо 32, строки
        // не влезали в колонку и таблица разъезжалась на лишние полосы
        // (вся семья `table-anonymous-objects-059…098`).
        let row_style = inline::inherit(inherited, &own);
        let mut col_ix = 0usize;
        for child in &row.children {
            let Node::Element(cell) = child else { continue };
            if !is_cell(cell) {
                continue;
            }
            while col_ix < occupied.len() && occupied[col_ix] > 0 {
                col_ix += 1;
            }
            // §17.6.1.1: `empty-cells: hide` прячет фон и рамку ПУСТОЙ
            // ячейки — в раздельной модели рамок. Пустая это та, у которой нет
            // ни текста, ни элементов-детей.
            let прячем_пустую = inline::inherit(&row_style, &cell.style).empty_cells_hide
                == Some(true)
                && e.style.border_collapse != Some(true)
                && {
                    let mut текст = String::new();
                    gather_text(&cell.children, &mut текст);
                    текст.trim().is_empty()
                        && !cell.children.iter().any(|n| matches!(n, Node::Element(_)))
                };
            // Ячейка в НУЛЕВОЙ дорожке: свои горизонтальные отступ и рамку
            // она держать не может — дорожки под них нет (§17.5.2.1).
            let cell = &if прячем_пустую {
                let mut copy = cell.clone();
                copy.style.background = None;
                copy.style.gradient = None;
                copy.style.bg_image = None;
                copy.style.border_visible = [Some(false); 4];
                copy.style.border_width = Default::default();
                copy
            } else {
                cell.clone()
            };
            let cell = &if zero_cols.get(col_ix).copied().unwrap_or(false) {
                let mut copy = cell.clone();
                copy.style.padding.left = Some(Len::Px(0.0));
                copy.style.padding.right = Some(Len::Px(0.0));
                copy.style.border_width.left = Some(Len::Px(0.0));
                copy.style.border_width.right = Some(Len::Px(0.0));
                copy
            } else {
                cell.clone()
            };
            let mut cm = inline::inherit(&row_style, &cell.style);
            // Потолок вертикальной ячейки режет доступное место её
            // ортогонального потока — как у блока (см. ortho_limit в
            // element): стопка глифов переносится на следующую колонку по
            // нему (table-cell-002: td vertical-rl с max-height 100 —
            // зелёный квадрат из двух колонок).
            if cell.style.vertical == Some(true)
                && cm.ortho_limit.is_none()
                && let Some(Len::Px(h)) = cell.style.height.or(cell.style.max_height)
            {
                cm.ortho_limit = Some(h);
            }
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
            // ПРОБОВАЛИ И ОТКАТИЛИ: держать внутри ячейки ПОЛОВИНУ её кромки
            // прозрачной рамкой, а внутри таблицы — половину своей (§17.6.2:
            // «row-width = (0.5 * border-width0) + padding-left1 + …», «the
            // width of the table includes half the table border»), заодно сняв
            // поправку `shift` у проб. Проба по 39 парам семей
            // `table-backgrounds-b[cs]-*`, `collapsing-border-model-*`,
            // `border-conflict-style-10*`: флипов ноль, все шесть `bc-*`
            // подтянулись (13.73 -> 12.10, 5.00 -> 4.01, 1.00 -> 0.81), но
            // потеряны `fixed-table-layout-027` (0.00 -> «красное видно») и
            // `collapsing-border-model-008` (0.00 -> 1.36). Половина берётся от
            // ПОБЕДИВШЕЙ кромки соседей (§17.6.2.1), а не от своей: без
            // разрешения ширин по всей сетке модель не сходится.
            //
            // Сросшиеся рамки (border-collapse): рамки С ЯЧЕЕК СНИМАЮТСЯ
            // целиком — их рисует отдельный слой кромок на линиях сетки
            // (см. interact::EdgePainter): кромка соседей ОДНА, рисуется
            // поверх фонов, и «шире побеждает» решается наложением.
            let cell_edge = if collapse_cells {
                let b = cell.style.borders();
                let widths = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                let black = crate::value::Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 1.0,
                };
                let side_colour = |i: usize| {
                    cell.style.border_colors[i]
                        .or(cell.style.border_color)
                        .unwrap_or(black)
                };
                let colors = [
                    side_colour(0),
                    side_colour(1),
                    side_colour(2),
                    side_colour(3),
                ];
                let side_style = |i: usize| {
                    cell.style.border_side_styles[i].unwrap_or(if widths[i] > 0.0 { 9 } else { 0 })
                };
                let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                // Половина кромки лежит ВНУТРИ ячейки и место занимает
                // (§17.6.2). Кладётся паддингом поверх авторского: проба
                // кромок стоит по паддинг-боксу, и рамкой линия уехала бы
                // внутрь.
                let win = win_edges.get(&cell.node_id).copied().unwrap_or(widths);
                let half = |i: usize, own: Option<Len>| {
                    let base = match own {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    Some(Len::Px(base + win[i] / 2.0))
                };
                cell.style.padding = crate::computed::Sides {
                    top: half(0, cell.style.padding.top),
                    right: half(1, cell.style.padding.right),
                    bottom: half(2, cell.style.padding.bottom),
                    left: half(3, cell.style.padding.left),
                };

                cell.style.border_width = Default::default();
                cell.style.border_visible = [None; 4];
                (widths.iter().any(|w| *w > 0.0) || styles.contains(&1)).then_some((
                    widths,
                    colors,
                    styles,
                    cell.node_id as u32,
                ))
            } else {
                None
            };
            if clipped {
                cell.style.overflow_x = None;
                cell.style.overflow_y = None;
            }
            // Презентационный `cellpadding="N"` таблицы: хинт ниже авторского
            // padding, но выше умолчания браузера `td { padding: 1px }` —
            // применяется, только когда у ячейки ровно оно.
            if let Some(pad) = e
                .attr("cellpadding")
                .and_then(|v| v.trim().trim_end_matches("px").parse::<f32>().ok())
            {
                let ua = |l: Option<Len>| matches!(l, Some(Len::Px(1.0)));
                let pd = &cell.style.padding;
                if ua(pd.top) && ua(pd.right) && ua(pd.bottom) && ua(pd.left) {
                    let v = Some(Len::Px(pad));
                    cell.style.padding = crate::computed::Sides {
                        top: v,
                        right: v,
                        bottom: v,
                        left: v,
                    };
                }
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
            // Кегльная ширина уходит с коробки, как Px и доля: колонка
            // разрешает её сама (Em-ветка col_widths), а на коробке она
            // падала в запасной кегль 16px — ячейка `width: 2em` при шрифте
            // 50px сжималась до 32 точек, и прижим строк оставался без места
            // (table-cell-valign-003-ref). Ортогональную ячейку не трогаем:
            // её width — логический inline-size, он ниже перекладывается в
            // высоту.
            if !(cell.style.vertical == Some(true)
                && e.style.vertical != Some(true)
                && cell.style.width_from_inline)
                // У ВЕРТИКАЛЬНОЙ таблицы width остаётся коробке: дорожку
                // задаёт высота (table-cell-align-001/002).
                && !table_is_vertical
                && matches!(
                    cell.style.width,
                    Some(Len::Em(_)) | Some(Len::Ch(_)) | Some(Len::Ex(_))
                )
            {
                cell.style.width = None;
            }
            // У ВЕРТИКАЛЬНОЙ таблицы кегльная ширина ячейки остаётся на
            // коробке, но в точки её никто не переводил: `apply` добавляет
            // отступы только к `Len::Px`, и коробка выходила у́же дорожки на
            // свои отступы и рамки. Перекладываем в минимум, зеркально
            // правилу `height` -> `min_height` ниже.
            if table_is_vertical
                && !cell.style.width_from_inline
                && let Some(l @ (Len::Em(_) | Len::Ch(_) | Len::Ex(_))) = cell.style.width
            {
                let size = match cell.style.font_size.or(row.style.font_size) {
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
                    cell.style.width = None;
                    cell.style.min_width = Some(Len::Px(v));
                }
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
            // Ортогональная ячейка (своё письмо вертикально, таблица
            // горизонтальна) живёт в НЕповёрнутой сетке: логический
            // inline-size осел в width (resolve_logical оси не переставляет —
            // подгонка под поворотную модель), но коробку ячейки никто не
            // вращает — её строчная ось физически ВЕРТИКАЛЬНА, и размер
            // обязан лечь высотой (table-cell-align-005/006).
            if cell.style.vertical == Some(true)
                && e.style.vertical != Some(true)
                && cell.style.height.is_none()
                && cell.style.width_from_inline
                && cell.style.width.is_some()
            {
                cell.style.height = cell.style.width.take();
            }
            // `em` на высоте ячейки — тем же точечным резолвом, что и `ch`:
            // без него логическая высота `inline-size: 2em` не проходила
            // Px-ветку ниже и min-height ряда не ставился.
            if let Some(Len::Em(k)) = cell.style.height {
                let base = match cell.style.font_size.or(inherited.font_size) {
                    Some(Len::Px(v)) => v,
                    _ => opts.base_size(),
                };
                cell.style.height = Some(Len::Px(k * base));
            }
            // Предел ортогонального потока пересчитывается ПОСЛЕ переклада
            // inline-size в высоту: блок выше (у `let mut cm`) высоты ещё
            // не видел (table-cell-align-005/006).
            if cell.style.vertical == Some(true)
                && cm.ortho_limit.is_none()
                && let Some(Len::Px(h)) = cell.style.height
            {
                cm.ortho_limit = Some(h);
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
                let upright = row.style.upright.or(inherited.upright) == Some(true);
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
                        let family = inherited.font_family.clone().unwrap_or_default();
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
                        && el
                            .style
                            .overflow_y
                            .is_some_and(|o| o != crate::computed::Overflow::Visible)
                        && let Some(Len::Px(m)) = el.style.min_height
                    {
                        el.style.height = Some(Len::Px(m * k));
                    }
                }
            }
            // ПРОБОВАЛИ И ОТКАТИЛИ (§17.5.2.1, ячейка обязана влезть в свою
            // дорожку при фиксированной раскладке): `border_box` + нулевой
            // `min_width` — потеряно 7 (`margin-bottom-applies-to-001..007`),
            // приобретено 0; один нулевой `min_width` — 0 и 0. Красное в
            // `fixed-table-layout-025..031` держит не минимум ячейки.
            // ПЕРЕПРОВЕРЕНО УЖЕ: сужение гейта до `table-layout: fixed` плюс
            // ячейка без своей ширины — те же семь потерь
            // (`margin-bottom-applies-to-001..007` 0.03 -> 3.80), флипов ноль.
            // Они тоже с фиксированной раскладкой; ячейка там без ширины, и
            // отличить их от `025..031` этим признаком нельзя.
            // ТРЕТЬЯ ПОПЫТКА, ЗАМЕРЕНА И ОТКАЧЕНА (01.09): не трогать размер
            // вовсе, а обрезать содержимое ячейки её коробкой
            // (`overflow_hidden` при `table-layout: fixed` и ячейке без своей
            // ширины). Срез из 81 пары `fixed-table-layout-*` +
            // `margin-bottom-applies-to-*`: зелёных 69 → 12. Обрезка режет
            // законно вылезающее содержимое — вся семья `003a..003f`
            // ушла 0.00 → 2.00, `017..020` 0.00 → 1.55.
            //
            // Потолок высоты к ячейке не применяется вовсе (браузеры
            // игнорируют max-height на ячейках): содержимое выше — растит.
            if matches!(cell.style.max_height, Some(Len::Px(_))) {
                cell.style.max_height = None;
            }
            let cell = &cell;
            // Коробке ячейки нужен СЛИТЫЙ стиль: у сырого `cell.style`
            // шрифтовые единицы не разрешены, и `apply` считает их от жёстких
            // 16 точек — `padding: 1em` при кегле 20 давало 16
            // (`table-height-algorithm-008a/b/c`). Берутся только те слоты,
            // где это безопасно: ширину и её минимум решает дорожка, и их
            // подмена уже мерилась отдельно.
            let box_style = {
                let mut c = cell.style.clone();
                let fixup = |own: Option<Len>, merged: Option<Len>| match own {
                    Some(Len::Px(_)) | None => own,
                    _ => merged,
                };
                c.padding = crate::computed::Sides {
                    top: fixup(c.padding.top, cm.padding.top),
                    right: fixup(c.padding.right, cm.padding.right),
                    bottom: fixup(c.padding.bottom, cm.padding.bottom),
                    left: fixup(c.padding.left, cm.padding.left),
                };
                c.height = fixup(c.height, cm.height);
                // Толщина рамки — тем же правилом: `border: 1em solid` у
                // ячейки доезжало сюда неразрешённым `Em`, а раскладка кладёт
                // только `Px` (`apply.rs`: прочие длины молча отбрасываются),
                // и рамка пропадала целиком (`table-height-algorithm-008b/c`
                // против зелёной `-008a`, где то же самое написано отступом).
                c.border_width = crate::computed::Sides {
                    top: fixup(c.border_width.top, cm.border_width.top),
                    right: fixup(c.border_width.right, cm.border_width.right),
                    bottom: fixup(c.border_width.bottom, cm.border_width.bottom),
                    left: fixup(c.border_width.left, cm.border_width.left),
                };
                c
            };
            let mut d = styled_div_with(cell, &box_style);
            // Заливка строки И ГРУППЫ строк: своей коробки у них в общей сетке
            // не остаётся, поэтому фон рисуют ячейки. Раньше бралась только
            // строка, и `<tbody style="background">` пропадал молча
            // (`position-relative-table-tbody-left`).
            if let Some(bg) = carry.2 {
                // Ряд с КАРТИНКОЙ красит и цвет САМ (см. CellsClipped) —
                // ячейка его не дублирует, иначе цвет ложится поверх
                // картинки. Ряду только с тенью цвет оставляют ячейки.
                let picture = row.style.bg_image.is_some() || row.style.gradient_raw.is_some();
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
                // `start`/`end` — края СТРОКИ: вертикальная строка идёт
                // сверху вниз, `dir=rtl` разворачивает её снизу вверх.
                let rtl = cm.rtl == Some(true);
                d = match cm.text_align {
                    Some(TextAlign::Right) => d.justify_end(),
                    Some(TextAlign::Center) => d.justify_center(),
                    Some(TextAlign::End) if !rtl => d.justify_end(),
                    Some(TextAlign::Start) if rtl => d.justify_end(),
                    _ => d.justify_start(),
                };
                // Начальное значение `vertical-align` — `baseline` (§17.5.3),
                // и у одиночного ряда это ВЕРХ ячейки, а не середина. Пока
                // умолчанием стояла середина, содержимое опускалось на
                // полразницы высот (`direction-applies-to-005`: квадрат на
                // 30 точек ниже эталона).
                d = match cm.vertical_align {
                    Some(Align::End) => d.items_end(),
                    Some(Align::Center) => d.items_center(),
                    _ => d.items_start(),
                };
            } else {
                d = match cm.vertical_align {
                    Some(Align::End) => d.justify_end(),
                    Some(Align::Center) => d.justify_center(),
                    _ => d.justify_start(),
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
                let rtl_line = e.style.rtl == Some(true) && e.style.upright != Some(true);
                let base_up = e.style.sideways == Some(true) && e.style.vertical_rl != Some(true);
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
            // Полкромки таблицы лежит в её паддинге, полкромки ячейки — в
            // паддинге ячейки: полосы фонов встают по месту без поправки.
            let shift = (0.0, 0.0);
            if let Some((widths, colors, styles, doc_ix)) = cell_edge {
                d = d.child(crate::interact::edge_probe(
                    table_edges.clone(),
                    widths,
                    colors,
                    styles,
                    5,
                    doc_ix,
                    [0.0; 4],
                ));
            }
            // Рамка ячейки — обратно в границы: канвас пробы лежит внутри
            // неё, а фон полосы идёт по внешним краям (§17.5.1).
            let cell_border = {
                let b = cell.style.borders();
                let px_of = |l: Option<Len>| match l {
                    Some(Len::Px(v)) => v,
                    _ => 0.0,
                };
                [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)]
            };
            if let Some(rects) = &row_rects {
                d = d.child(crate::interact::cell_rect_probe(
                    rects.clone(),
                    span_rows == 1,
                    shift,
                    cell_border,
                ));
            }
            if let Some(rects) = &grp_band {
                d = d.child(crate::interact::cell_rect_probe(
                    rects.clone(),
                    span_rows == 1,
                    shift,
                    cell_border,
                ));
            }
            // Проба и для колонок ячейки: объединённая регистрируется в
            // каждой накрытой колонке — полоса колонки красит её целиком.
            let cell_cols = (col_ix - span_cols as usize)..col_ix;
            let mut probed: Vec<u64> = vec![];
            for i in cell_cols.clone() {
                if let (Some(rects), Some(el)) = (
                    col_rects.get(i).and_then(|r| r.clone()),
                    col_els.get(i).copied().flatten(),
                ) {
                    if !probed.contains(&el.node_id) {
                        probed.push(el.node_id);
                        d = d.child(crate::interact::cell_rect_probe(
                            rects,
                            span_cols == 1,
                            shift,
                            cell_border,
                        ));
                    }
                }
            }
            // Та же проба для слоя ГРУППЫ: её коробка идёт «from the left
            // edge of its leftmost column to the right edge of its rightmost
            // column» (§17.5.1) — площадь шире колоночной, поэтому буфер
            // свой.
            let mut probed_group: Vec<u64> = vec![];
            for i in cell_cols.clone() {
                if let (Some(rects), Some(el)) = (
                    grp_rects.get(i).and_then(|r| r.clone()),
                    grp_els.get(i).copied().flatten(),
                ) {
                    if !probed_group.contains(&el.node_id) {
                        probed_group.push(el.node_id);
                        d = d.child(crate::interact::cell_rect_probe(
                            rects,
                            span_cols == 1,
                            shift,
                            cell_border,
                        ));
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
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        row.style.border_colors[k]
                            .or(row.style.border_color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        row.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        4,
                        row.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки ГРУППЫ РЯДОВ: верх у первого ряда группы, низ у
            // последнего; `rules=groups` даёт тонкую сплошную по умолчанию.
            if collapse_cells && let Some((g, first, last)) = group_of.get(&row.node_id).copied() {
                let b = g.style.borders();
                let default_w = if rules_groups { 1.0 } else { 0.0 };
                let explicit_top =
                    g.style.border_width.top.is_some() || g.style.border_visible[0].is_some();
                let explicit_bottom =
                    g.style.border_width.bottom.is_some() || g.style.border_visible[2].is_some();
                let top_w = if explicit_top {
                    px_of(b.top)
                } else {
                    default_w
                };
                let bottom_w = if explicit_bottom {
                    px_of(b.bottom)
                } else {
                    default_w
                };
                // Боковые кромки группы несут крайние ячейки ряда.
                let start_col = col_ix - span_cols as usize;
                let last_col = col_ix >= cols as usize;
                let widths = [
                    if first { top_w } else { 0.0 },
                    if last_col { px_of(b.right) } else { 0.0 },
                    if last { bottom_w } else { 0.0 },
                    if start_col == 0 { px_of(b.left) } else { 0.0 },
                ];
                // Нулевая толщина у `hidden` не значит «кромки нет»: скрытая
                // кромка ГАСИТ соседей (§17.6.2.1), поэтому в разбор она
                // обязана попасть наравне с видимыми.
                let hidden_grp = g.style.border_side_styles.contains(&Some(1));
                if widths.iter().any(|w| *w > 0.0) || hidden_grp {
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        g.style.border_colors[k]
                            .or(g.style.border_color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        g.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 { 9 } else { 0 })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        3,
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
                for i in cell_cols.clone() {
                    let Some(el) = col_els.get(i).copied().flatten() else {
                        continue;
                    };
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
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        el.style.border_colors[k]
                            .or(el.style.border_color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
                    };
                    let styles = [side_style(0), side_style(1), side_style(2), side_style(3)];
                    d = d.child(crate::interact::edge_probe(
                        table_edges.clone(),
                        widths,
                        colors,
                        styles,
                        2,
                        el.node_id as u32,
                        [0.0; 4],
                    ));
                }
            }
            // Кромки ГРУППЫ КОЛОНОК — свой источник, слабее колонки и сильнее
            // таблицы (§17.6.2.1 п.4). Без него `<colgroup style="border">` с
            // колонками внутри терял рамку целиком: `col_elements` отдаёт
            // внутренние колонки, а сама группа в разбор не попадала.
            if collapse_cells {
                for i in cell_cols {
                    let Some(el) = grp_els.get(i).copied().flatten() else {
                        continue;
                    };
                    let b = el.style.borders();
                    let cw = [px_of(b.top), px_of(b.right), px_of(b.bottom), px_of(b.left)];
                    let hidden = el.style.border_side_styles.contains(&Some(1));
                    if !(cw.iter().any(|w| *w > 0.0) || hidden) {
                        continue;
                    }
                    let same = |j: i64| -> bool {
                        j >= 0
                            && grp_els
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
                    let black = crate::value::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 1.0,
                    };
                    let side_colour = |k: usize| {
                        el.style.border_colors[k]
                            .or(el.style.border_color)
                            .unwrap_or(black)
                    };
                    let colors = [
                        side_colour(0),
                        side_colour(1),
                        side_colour(2),
                        side_colour(3),
                    ];
                    let side_style = |k: usize| {
                        el.style.border_side_styles[k].unwrap_or(if widths[k] > 0.0 {
                            9
                        } else {
                            0
                        })
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
            && (cap.tag == "caption" || cap.style.is_caption == Some(true))
        {
            let cm = inline::inherit(inherited, &cap.style);
            // Сторона — с самого заголовка, при пустоте — от таблицы
            // (наследование caption-side).
            caption_bottom = cap.style.caption_bottom.or(e.style.caption_bottom) == Some(true);
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
                    // Колонка = width + горизонтальные паддинги и рамки
                    // ячейки (§17.5.2.1, content-box); в сросшейся модели
                    // рамка входит ПОЛОВИНОЙ.
                    let side = |l: Option<Len>| match l {
                        Some(Len::Px(p)) => p,
                        _ => 0.0,
                    };
                    let extra = if cell.style.border_box == Some(true) {
                        0.0
                    } else {
                        let b = cell.style.borders();
                        let border = side(b.left) + side(b.right);
                        side(cell.style.padding.left)
                            + side(cell.style.padding.right)
                            + if e.style.border_collapse == Some(true) {
                                // Половина ПОБЕДИВШЕЙ линии — та же, что
                                // легла в паддинг ячейки (§17.6.2.1).
                                let w = win_edges
                                    .get(&cell.node_id)
                                    .map(|w| (w[1] + w[3]) / 2.0)
                                    .unwrap_or(border / 2.0);
                                w
                            } else {
                                border
                            }
                    };
                    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: разрешать ширину ячейки в
                    // единицах ШРИФТА (`width: 1em` доезжает сюда
                    // неразрешённой — `resolve_em` живёт в наследовании, а
                    // ширины колонок считаются раньше). Срез из 10 пар
                    // `separated-border-model-*`: 8 зелёных до и после, ни
                    // один вердикт не сдвинулся. Одной этой половины мало:
                    // цель (`-004c/-004d`) требует ещё нижней грани ширины
                    // стола по сумме дорожек — возвращать вместе с ней.
                    match cell.style.width {
                        Some(Len::Px(v)) if span == 1 => out.push(Some(v + extra)),
                        // Доля считается от места, отдаваемого дорожкам:
                        // ширина таблицы за вычетом зазоров (§17.5.2.1,
                        // «a percentage value ... of the table width»).
                        // Известна, только когда ширина таблицы в точках.
                        Some(Len::Pct(p)) if span == 1 => match e.style.width {
                            Some(Len::Px(tw)) => {
                                let gaps = spacing.0 * (f32::from(cols) + 1.0);
                                out.push(Some((tw - gaps).max(0.0) * p + extra));
                            }
                            _ => out.push(None),
                        },
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
    if {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| std::env::var("HTML_ROWBG").is_ok());
        *ON
    } {
        eprintln!(
            "TABLE cols={} col_widths={:?} first_row={:?}",
            cols, col_widths, first_row_widths
        );
    }
    let tracks = track_list_collapsed(
        cols,
        e.style.table_fixed == Some(true),
        &first_row_widths,
        &col_widths,
        &cols_collapsed,
        &cols_pct,
        // Место под КОЛОНКИ, а не вся ширина стола: §17.5.2.1 считает долю
        // от ширины таблицы БЕЗ её рамок и без зазоров между ячейками
        // (`fixed-table-layout-022` расписывает это прямо в тексте: 533 − 58
        // рамок − 75 зазоров = 400).
        match match e.style.width {
            Some(Len::Px(v)) => Some(v),
            Some(Len::Pct(k)) => CB_WIDTH.get().filter(|v| *v > 0.0).map(|cb| cb * k),
            _ => None,
        } {
            Some(v) => {
                let side = |l: Option<Len>| match l {
                    Some(Len::Px(w)) => w,
                    _ => 0.0,
                };
                let bs = e.style.borders();
                let gap = if e.style.border_collapse == Some(true) {
                    0.0
                } else {
                    match e.style.border_spacing {
                        Some((Some(Len::Px(g)), _)) => g,
                        _ => 0.0,
                    }
                };
                Some(v - side(bs.left) - side(bs.right) - gap * (cols as f32 + 1.0))
            }
            _ => None,
        },
    );
    if {
        static ON: std::sync::LazyLock<bool> =
            std::sync::LazyLock::new(|| std::env::var("HTML_ROWBG").is_ok());
        *ON
    } {
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
                    match own
                        .into_iter()
                        .chain(row.children.iter().filter_map(cell_h))
                        .fold(None::<f32>, |a, v| Some(a.map_or(v, |x| x.max(v))))
                    {
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
        Some(Len::Px(v)) if e.style.border_box != Some(true) => Some(Len::Px((v - edges).max(0.0))),
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
    // ПРОБОВАЛИ И ОТКАТИЛИ: поднимать нижнюю грань ширины таблицы до суммы
    // дорожек с зазорами (§17.5.2: «the used width is the greater of the value
    // of width and MIN»). Проба по 19 парам семей `separated-border-model-*` и
    // `fixed-table-layout-02*`: флипов ноль, `separated-border-model-004d`
    // 1.90 -> 1.75. Минимум доезжает, но заданную ширину не перебивает —
    // упирается ниже, в раздачу дорожек внутри сетки.
    let min_w = min_fix(e.style.min_width, bw[1] + bw[3] + pad_px[1] + pad_px[3]);
    // У ТЕГА `<table>` ширина считается по BORDER-BOX (UA-правило
    // css-tables-3: `table { box-sizing: border-box }`), у `display: table` на
    // прочих тегах — контентная. Тесты пишут это прямо: «the width of an
    // HTML/XHTML table is the distance between the left and right table border
    // edges» против «the width of a CSS table … excluding table padding and
    // table borders».
    let table_border_box = e.tag == "table" && e.style.border_box.is_none();
    // §17.4: `position` и края — свойства ОБЁРТКИ таблицы, а не её сетки;
    // ширину сетки решает §17.5.2.2 (сжатие по содержимому). Пока коробка
    // одна, абсолютная таблица с ОБОИМИ краями инлайн-оси получала ширину от
    // краёв, и колонки расползались: сжатие у нас выражено только
    // `align_self`, а его у абсолютной коробки с двумя краями не спрашивают.
    let split_wrapper = matches!(
        inherited.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) && edge_set(inherited.inset.left)
        && edge_set(inherited.inset.right)
        && e.style.width.is_none();
    // Стиль СЕТКИ — без позиционирования и краёв: их заберёт обёртка.
    let grid_style = split_wrapper.then(|| {
        let mut c = inherited.clone();
        c.position = None;
        c.inset = Default::default();
        c.z_index = None;
        c
    });
    let inherited: &Computed = grid_style.as_ref().unwrap_or(inherited);
    let needs_clone =
        collapse || table_border_box || min_h != e.style.min_height || min_w != e.style.min_width;
    let host_style;
    let mut outer = if needs_clone {
        let mut c = inherited.clone();
        if collapse {
            c.border_width = Default::default();
            c.border_visible = [None; 4];
            // §17.6.2: внутрь таблицы уходит ПОЛОВИНА её кромки. Паддингом,
            // а не рамкой: проба кромок — абсолютный ребёнок по паддинг-боксу,
            // и рамка утащила бы линию сетки внутрь на свою величину.
            c.padding = crate::computed::Sides {
                top: Some(Len::Px(outer_win[0] / 2.0)),
                right: Some(Len::Px(outer_win[1] / 2.0)),
                bottom: Some(Len::Px(outer_win[2] / 2.0)),
                left: Some(Len::Px(outer_win[3] / 2.0)),
            };
        }
        c.min_height = min_h;
        c.min_width = min_w;
        if table_border_box {
            c.border_box = Some(true);
        }
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
    let mut outer = outer.child(
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
    if collapse {
        // Граница СЕТКИ выдаётся ВСЕГДА: слой кромок обязан знать, где
        // кончаются дорожки, даже когда своей рамки у таблицы нет. Кромок
        // эта проба не несёт — только координаты.
        //
        // ЗАМЕРЕНО И ОТКАЧЕНО: подменять ею пробу КРОМОК таблицы (нулевые
        // ширины в общем разборе) — CSS2 +21/-25: у таблицы без рамки её
        // коробка совпадает с внешними краями ячеек, и те переставали
        // центрироваться.
        outer = outer.child(crate::interact::grid_probe(table_edges.clone(), bw));
    }
    if collapse && (bw.iter().any(|w| *w > 0.0) || e.style.border_side_styles.contains(&Some(1))) {
        // Рамка самой таблицы — участник разбора конфликтов: её кромки
        // уходят в тот же слой (EdgePainter), линии — внутренние края
        // рамочного места, победившая кромка рисуется наружу.
        let black = crate::value::Color {
            r: 0.0,
            g: 0.0,
            b: 0.0,
            a: 1.0,
        };
        let side_colour = |i: usize| {
            e.style.border_colors[i]
                .or(e.style.border_color)
                .unwrap_or(black)
        };
        let colors = [
            side_colour(0),
            side_colour(1),
            side_colour(2),
            side_colour(3),
        ];
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
        // `caption-side: top/bottom` — стороны block-start/block-end стола
        // (css-writing-modes-4 §6, особое исключение для caption-side): в
        // вертикальном письме заголовок стоит СБОКУ — справа при `vertical-rl`,
        // слева при `vertical-lr` (`caption-side-vrl-002`).
        let vertical = e.style.vertical == Some(true);
        let mut wrap = if vertical {
            div().flex().flex_row()
        } else {
            div().flex().flex_col()
        };
        wrap.style().align_self = Some(gpui::AlignItems::FlexStart);
        // Заголовок ПЕРЕД коробкой по порядку детей = у начала оси: для
        // vrl начало блочной оси — правый край, а ряд идёт слева направо.
        let cap_first = caption_bottom == (vertical && e.style.vertical_rl == Some(true));
        // В ряду второй ребёнок сжимался в ноль — обоим своя ширина.
        let (outer, cap) = if vertical {
            (
                div().flex_shrink_0().child(outer).into_any_element(),
                div().flex_shrink_0().child(cap).into_any_element(),
            )
        } else {
            (outer.into_any_element(), cap)
        };
        if cap_first {
            wrap.child(cap).child(outer).into_any_element()
        } else {
            wrap.child(outer).child(cap).into_any_element()
        }
    } else {
        outer.into_any_element()
    };
    // Вторая половина §17.4: сама обёртка. Гибкий ряд возвращает сетке сжатие
    // по содержимому — тот же приём, что у корневого стола ниже.
    if split_wrapper {
        let mut wrap = Computed::default();
        wrap.position = e.style.position;
        wrap.inset = e.style.inset;
        wrap.z_index = e.style.z_index;
        return crate::apply::apply(div(), &wrap)
            .flex()
            .flex_row()
            .child(outer)
            .into_any_element();
    }
    // Стол с `width: auto` СЖИМАЕТСЯ по содержимому (§17.5.2): у нас это
    // делает гибкий ряд-обёртка. Приём `align_self: FlexStart` выше работает
    // только когда родитель — гибкая колонка нашей сборки; под `body` со
    // сброшенными полями путь другой, и стол растягивался во всю ширину
    // (`html-display-table`, `root-box-002`). Обёртка снимает зависимость от
    // родителя. Элемент гибкого контейнера, сетки и ячейки не заворачивается:
    // там стол — сам элемент раскладки, и обёртка забрала бы его свойства.
    let shrink_wrap = root_table
        || (e.style.width.is_none()
            // Заданная высота или её порог приходят от РАСКЛАДКИ родителя:
            // обёртка рвёт эту связь (★ ЗАМЕРЕНО: без отсечки
            // `min-height-table-2` 0.00 -> 19.24).
            && e.style.height.is_none()
            && e.style.min_height.is_none()
            && !inherited.stretched
            && e.style.flex_basis.is_none()
            && e.style.align_self.is_none()
            && e.style.grid_col.is_none()
            && e.style.grid_row.is_none());
    if shrink_wrap {
        let mut wrap = div().flex().flex_row();
        if root_table {
            wrap = wrap.w_full();
        }
        return wrap.child(outer).into_any_element();
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
    pcts: &[Option<f32>],
    table_px: Option<f32>,
) -> Vec<gpui::GridTrack> {
    let mut tracks = track_list(cols, fixed, first_row, col_widths, pcts, table_px);
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
    pcts: &[Option<f32>],
    table_px: Option<f32>,
) -> Vec<gpui::GridTrack> {
    // `table-layout: fixed` — ширины из первого ряда, безразмерные колонки
    // делят остаток поровну; содержимое не меряется.
    if fixed {
        // Доли колонок (§17.5.2.1): доля берётся от ширины таблицы, а базис
        // всех дорожек здесь нулевой — значит свободное место равно ей самой,
        // и долю точно выражает `Fraction`. Остаток делят безразмерные.
        let pct_sum: f32 = (0..cols as usize)
            .filter_map(|i| pcts.get(i).copied().flatten())
            .sum();
        let auto_n = (0..cols as usize)
            .filter(|i| {
                pcts.get(*i).copied().flatten().is_none()
                    && first_row.get(*i).copied().flatten().is_none()
            })
            .count();
        // Доли переводятся в точки только рядом с ПИКСЕЛЬНОЙ колонкой: без
        // неё свободное место равно ширине стола, и `Fraction` точен сам.
        let в_точках = table_px.is_some() && first_row.iter().any(|w| w.is_some());
        let share = if auto_n > 0 {
            ((1.0 - pct_sum).max(0.0)) / auto_n as f32
        } else {
            0.0
        };
        return (0..cols as usize)
            .map(|i| {
                match (
                    pcts.get(i).copied().flatten(),
                    first_row.get(i).copied().flatten(),
                ) {
                    // Доля колонки берётся от ширины ТАБЛИЦЫ, а `Fraction`
                    // делит только СВОБОДНОЕ место: пока все дорожки долевые,
                    // это одно и то же, но рядом с ПИКСЕЛЬНОЙ колонкой
                    // расходится — 13 % от трёхсот выходило 39 вместо 52
                    // (`fixed-table-layout-022/023` против зелёной `-021`,
                    // которая отличается ровно отсутствием `col{width}`).
                    (Some(p), _) if в_точках => {
                        gpui::GridTrack::Pixels(px(p * table_px.unwrap_or(0.0)))
                    }
                    (Some(p), _) => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(p),
                    ))),
                    (None, Some(w)) => gpui::GridTrack::Pixels(px(w)),
                    // Когда доли уже переведены в точки, безразмерной
                    // колонке достаётся ОСТАТОК: равные доли делят его
                    // поровну, а прежний `share` считал его от всей ширины
                    // стола и отдавал 69 вместо 124.
                    (None, None) if в_точках => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(1.0),
                    ))),
                    (None, None) if pct_sum > 0.0 => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(share),
                    ))),
                    // Пол дорожки — ноль, а не содержимое: фиксированная
                    // раскладка содержимое НЕ меряет (CSS 2.1 §17.5.2.1), и
                    // колонка вправе быть у́же него. Голая доля брала минимумом
                    // вклад `min-content`, из-за чего сумма колонок перерастала
                    // заданную ширину таблицы (`fixed-table-layout-003a01`).
                    (None, None) => gpui::GridTrack::MinMax(Box::new((
                        gpui::GridTrack::Pixels(px(0.0)),
                        gpui::GridTrack::Fraction(1.0),
                    ))),
                }
            })
            .collect();
    }
    // Все колонки по содержимому. Остаток строки НЕ отдаётся последней:
    // раньше она забирала его целиком, и таблица из двух коротких ячеек
    // расползалась по краям окна (видно на `shaping-join-001`). Излишек
    // раздаёт раскладка между дорожками `auto` — это ближе к табличной
    // раздаче «пропорционально разнице полной и минимальной ширины».
    (0..cols as usize)
        .map(
            |i| match col_widths.get(i).copied().unwrap_or((None, None)) {
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
            },
        )
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
/// Сдвиг, фон и САМА ГРУППА рядов: от неё нужны и наследуемый стиль, и
/// `node_id` с рамками — кромки группы строит ряд.
type RowCarry<'a> = (f32, f32, Option<crate::value::Color>, Option<&'a Element>);

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
            // `visibility: collapse` на ряде или группе рядов ВЫБРАСЫВАЕТ их
            // из сетки, как и на колонке: ряды не рисуются, а их высота из
            // таблицы уходит (css-tables-3 §visibility-collapse). Прежде
            // читалась только колонка, и схлопнутый ряд оставлял пустую
            // полосу.
            if e.style.collapsed == Some(true) {
                continue;
            }
            // Роль задаётся тегом ИЛИ стилем: разметка на `div` с
            // `display: table-row` встречается не реже настоящих таблиц.
            if e.tag == "tr" || e.style.display == Some(Display::TableRow) {
                out.push((e, shift));
            } else if e.tag == "thead"
                || e.tag == "tbody"
                || e.tag == "tfoot"
                || e.style.display == Some(Display::TableRowGroup)
            {
                // Группа с КАРТИНКОЙ красит и цвет САМА — полосой `grp_band`
                // (§14.2: цвет лежит ПОД картинкой). Ячейка его не дублирует:
                // она рисуется после полосы, и цвет прятал бы картинку
                // (`background-repeat-applies-to-001/002/003`). У ряда та же
                // отсечка стоит давно.
                //
                // Первый заход сюда дал ровно ноль (+3 / −3): тройка
                // `background-attachment-applies-to-*` уходила 0.48 -> 1.44,
                // потому что полоса не знала `attachment: fixed`. Теперь
                // знает, и отсечка стала чистым приобретением.
                let picture = e.style.bg_image.is_some() || e.style.gradient_raw.is_some();
                let deeper = (
                    shift.0,
                    shift.1,
                    if picture { None } else { shift.2 },
                    Some(e),
                );
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

/// Табличная роль бесхозного узла: `Some(true)` — структурная (ряд, группа
/// рядов, колонка, группа колонок), `Some(false)` — ячейка, `None` — обычная
/// коробка.
///
/// Колонка приходит с `Display::None` и живой меткой роли: коробки она не
/// даёт, но прогон рвать не должна и обязана попасть в ту же анонимную
/// таблицу.
///
/// Плавающее и абсолютное по §9.7 блокифицируются и табличной ролью быть
/// перестают. Блокификации у нас пока нет, поэтому такие узлы проход не
/// трогает — их судьбу решает прежний путь.
fn anon_role(n: &Node) -> Option<bool> {
    let Node::Element(e) = n else { return None };
    if e.style.float.is_some_and(|f| f != 0)
        || matches!(
            e.style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        )
    {
        return None;
    }
    if col_role(e).is_some() {
        return Some(true);
    }
    if is_cell(e) {
        return Some(false);
    }
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: считать табличной ролью и ПОДПИСЬ (§17.2.1),
    // чтобы бесхозный `display: table-caption` попадал в анонимную таблицу
    // (`e.style.is_caption == Some(true)` и тег `caption`). Срез из 18 пар с
    // подписью: 17 зелёных до и после, `caption-position-001` ушла
    // 2.79 → 2.88. Значит её держит не сборка анонимной таблицы.
    match e.style.display {
        Some(Display::TableRow) | Some(Display::TableRowGroup) => Some(true),
        _ => match e.tag.as_str() {
            "tr" | "thead" | "tbody" | "tfoot" => Some(true),
            _ => None,
        },
    }
}

/// §17.2.1, шаг 3: ПОСЛЕДОВАТЕЛЬНЫЕ братья с табличной ролью, стоящие в
/// не-табличном родителе, заворачиваются в ОДНУ анонимную таблицу.
///
/// Поэлементная обёртка у нас уже была, и каждая бесхозная роль получала
/// СВОЮ таблицу — ряды вставали друг под друга отдельными таблицами вместо
/// одной. Починку содержимого (ряд вокруг ячеек, ячейка вокруг прочего)
/// делает `fixup_table_children` уже внутри собранной таблицы.
fn wrap_anon_tables(nodes: &[Node]) -> Vec<Node> {
    if !nodes.iter().any(|n| anon_role(n).is_some()) {
        return nodes.to_vec();
    }
    fn flush(run: &mut Vec<Node>, out: &mut Vec<Node>) {
        if run.is_empty() {
            return;
        }
        let mut t = anon_element("table", std::mem::take(run));
        t.style.display = Some(Display::Table);
        // Свой номер узла: по нему таблица просит буферы проб. Нулевой у
        // всех анонимных узлов общий, и две таблицы делили бы один буфер —
        // первая забрала бы его, вторая осталась пустой.
        t.node_id = match t.children.first() {
            Some(Node::Element(e)) => e.node_id.rotate_left(1) ^ 0x7ab1_e000,
            _ => 0,
        };
        out.push(Node::Element(t));
    }
    let mut out: Vec<Node> = vec![];
    let mut run: Vec<Node> = vec![];
    let mut gap: Vec<Node> = vec![];
    for n in nodes {
        if anon_role(n).is_some() {
            run.append(&mut gap);
            run.push(n.clone());
        } else if is_blank(n) && !run.is_empty() {
            // Пробел между табличными братьями прогона не рвёт (§17.2.1,
            // «consecutive»). Место ему решит следующий узел: внутри прогона
            // он уйдёт в таблицу и там пропадёт, за прогоном — останется
            // снаружи.
            gap.push(n.clone());
        } else {
            flush(&mut run, &mut out);
            out.append(&mut gap);
            out.push(n.clone());
        }
    }
    flush(&mut run, &mut out);
    out.append(&mut gap);
    out
}

fn anon_element(tag: &str, children: Vec<Node>) -> Element {
    Element {
        list_item: None,
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
                // §17.2.1 шаг 2: ребёнок ряда, который не ячейка, уходит в
                // АНОНИМНУЮ ЯЧЕЙКУ этого же ряда — ряд внутри ряда тоже.
                // Прежде он выталкивался сестринским рядом, и таблица
                // получала лишнюю строку (`table-anonymous-objects-090`).
                Node::Element(el)
                    if el.tag == "tr"
                        || matches!(
                            el.style.display,
                            Some(Display::TableRow) | Some(Display::TableRowGroup)
                        ) =>
                {
                    run.push(child.clone());
                    let _ = el;
                }
                Node::Element(el) if is_cell(el) => {
                    if !run.is_empty() {
                        cells.push(Node::Element(anon_element("td", std::mem::take(run))));
                    }
                    cells.push(child.clone());
                }
                Node::Text(t) if !t.trim().is_empty() => run.push(child.clone()),
                // §17.2.1 шаг 1 п.4 гасит пробел только МЕЖДУ внутренними
                // табличными коробками. Внутри прогона строчных братьев он
                // часть анонимной ячейки: без него соседние слова слипались,
                // и строка выходила короче.
                Node::Text(_) if !run.is_empty() => run.push(child.clone()),
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
                // Роль задаётся тегом ИЛИ `display` (§17.2.1).
                if col_role(el).is_some() {
                    continue;
                }
                let group = el.style.display == Some(Display::TableRowGroup)
                    || matches!(el.tag.as_str(), "thead" | "tbody" | "tfoot");
                let row = el.tag == "tr"
                    || el.style.display == Some(Display::TableRow)
                    || el.tag == "caption";
                let is_cap = el.tag == "caption" || el.style.is_caption == Some(true);
                if group {
                    // Группа рядов чинится ИЗНУТРИ тоже: contents и бесхозное
                    // содержимое встречаются и там.
                    flush(&mut stray, &mut out);
                    let mut copy = el.clone();
                    copy.children = fixup_table_children(&el.children);
                    out.push(Node::Element(copy));
                } else if is_cap {
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
/// Колоночная роль элемента: тег ИЛИ `display` (§17.2.1). `Some(false)` —
/// колонка, `Some(true)` — группа колонок.
fn col_role(el: &Element) -> Option<bool> {
    match el.tag.as_str() {
        "col" => Some(false),
        "colgroup" => Some(true),
        _ => match el.style.col_role {
            Some(0) => Some(false),
            Some(1) => Some(true),
            _ => None,
        },
    }
}

/// Пролёт колонки: атрибут `span` — только HTML-ный, у элемента с колоночным
/// `display` его нет, и пролёт всегда единичный.
fn col_span(el: &Element) -> usize {
    el.attr("span")
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1)
        .max(1)
}

fn col_elements(children: &[Node]) -> Vec<Option<&Element>> {
    let mut out: Vec<Option<&Element>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => out.extend(std::iter::repeat_n(Some(el), col_span(el))),
            Some(true) => {
                let inner = col_elements(&el.children);
                if inner.is_empty() {
                    // Группа без колонок внутри сама стоит колонкой: её
                    // пролёт и стиль ложатся на каждую дорожку.
                    out.extend(std::iter::repeat_n(Some(el), col_span(el)));
                } else {
                    out.extend(inner);
                }
            }
            None => {}
        }
    }
    out
}

/// Группы колонок по индексам дорожек: слой группы лежит ПОД слоем колонки
/// (§17.5.1) и красится отдельной полосой.
///
/// Длина и индексы совпадают с `col_elements` дорожка в дорожку: обе идут по
/// одному дереву и кладут ровно столько же записей.
///
/// Группа без своих колонок внутри уже стоит колонкой в `col_elements` —
/// второй раз её сюда не берём: буфер проб выдаётся по `node_id`, и обе
/// полосы делили бы один набор прямоугольников. Первая забрала бы его себе,
/// вторая осталась бы пустой.
fn colgroup_elements(children: &[Node]) -> Vec<Option<&Element>> {
    let mut out: Vec<Option<&Element>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => out.extend(std::iter::repeat_n(None, col_span(el))),
            Some(true) => {
                let inner = col_elements(&el.children);
                if inner.is_empty() {
                    out.extend(std::iter::repeat_n(None, col_span(el)));
                } else {
                    out.extend(std::iter::repeat_n(Some(el), inner.len()));
                }
            }
            None => {}
        }
    }
    out
}

/// Полоса слоя на каждую красящую дорожку плюс буфер проб по её индексу.
///
/// Одно тело на слой групп и слой колонок: различаются они только набором
/// элементов и порядком вызова (§17.5.1 — группы ПОД колонками).
fn push_col_bands<'a>(
    els: &[Option<&'a Element>],
    salt: u64,
    have_rows: bool,
    rects_by_col: &mut [Option<crate::interact::RowRects>],
    cells: &mut Vec<AnyElement>,
) {
    let mut seen: Vec<u64> = vec![];
    for (i, el) in els.iter().enumerate() {
        let Some(el) = el else { continue };
        // Полоса красит СВОИ ЯЧЕЙКИ. Ячеек нет — красить нечего, а пустая
        // полоса ещё и просит перерисовку два кадра подряд впустую
        // (`table-column-rendering-001`: колонка сама по себе не рисуется).
        // Дорожка за краем сетки буфера не получает: её пробы не напишет
        // никто.
        if !have_rows || i >= rects_by_col.len() {
            continue;
        }
        let picture = el.style.bg_image.is_some() || el.style.gradient_raw.is_some();
        // Дорожка с одним ЦВЕТОМ тоже красится полосой: своей коробки у неё
        // нет, фон рисуют её ячейки.
        if !(picture || el.style.background.is_some() || !el.style.shadows.is_empty()) {
            continue;
        }
        let rects = crate::interact::row_rects_for(el.node_id ^ salt);
        rects_by_col[i] = Some(rects.clone());
        if !seen.contains(&el.node_id) {
            seen.push(el.node_id);
            let mut band_style = el.style.clone();
            if band_style.bg_image.is_none() {
                band_style.bg_image = band_style.gradient_raw.clone();
            }
            cells.push(crate::interact::CellsClipped::new(rects, band_style).into_any_element());
        }
    }
}

fn col_element_widths(
    children: &[Node],
    base_font: f32,
    family: &str,
) -> (Vec<Option<f32>>, Vec<bool>, Vec<Option<f32>>) {
    let mut widths = vec![];
    let mut collapsed = vec![];
    // Доля ширины колонки (§17.5.2.1): в точках её не выразить, дорожка
    // получает её отдельно.
    let mut pcts: Vec<Option<f32>> = vec![];
    for child in children {
        let Node::Element(el) = child else { continue };
        match col_role(el) {
            Some(false) => {
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
                        if el.style.upright == Some(true) && el.style.vertical == Some(true) =>
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
                let span = col_span(el);
                let p = match el.style.width {
                    Some(Len::Pct(k)) => Some(k),
                    _ => None,
                };
                widths.extend(std::iter::repeat_n(w, span));
                collapsed.extend(std::iter::repeat_n(c, span));
                pcts.extend(std::iter::repeat_n(p, span));
            }
            Some(true) => {
                let (w, c, p) = col_element_widths(&el.children, base_font, family);
                if w.is_empty() {
                    let ww = match el.style.width {
                        Some(Len::Px(v)) => Some(v),
                        _ => None,
                    };
                    let pp = match el.style.width {
                        Some(Len::Pct(k)) => Some(k),
                        _ => None,
                    };
                    let cc = el.style.collapsed == Some(true);
                    let span = col_span(el);
                    widths.extend(std::iter::repeat_n(ww, span));
                    collapsed.extend(std::iter::repeat_n(cc, span));
                    pcts.extend(std::iter::repeat_n(pp, span));
                } else {
                    widths.extend(w);
                    collapsed.extend(c);
                    pcts.extend(p);
                }
            }
            None => {}
        }
    }
    (widths, collapsed, pcts)
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
            Node::Element(e) => collapse_margins(&e.children, false),
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
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
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
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
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
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => collapse_margins(&b.children, false),
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
        // Примыкание ТРАНЗИТИВНО (§8.3.1): поле уходит на самую внешнюю
        // коробку цепи, а у всех внутренних снимается. Пока подъём шёл на
        // один уровень, то же поле поднималось повторно на каждом.
        let inner = match &nodes[0] {
            Node::Element(html) => collapse_margins(&html.children, false),
            _ => panic!("нет корня"),
        };
        let body = match &inner[0] {
            Node::Element(b) => b,
            _ => panic!("нет body"),
        };
        assert_eq!(
            body.style.margin.top,
            Some(Len::Px(24.0)),
            "отступ вынесен на внешнюю коробку"
        );
        let wrap_top =
            find_class(std::slice::from_ref(&inner[0]), "wrap").and_then(|e| e.style.margin.top);
        assert_eq!(wrap_top, Some(Len::Px(0.0)), "у обёртки отступ снят");
        let child_top =
            find_class(std::slice::from_ref(&inner[0]), "in").and_then(|e| e.style.margin.top);
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
    let row_tracks = merged.grid_rows.is_some()
        || merged.auto_repeat_rows.is_some()
        || merged.grid_auto_fill_row.is_some();
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
    // Повтор «сколько влезет» ВНУТРИ непустого списка разворачивается НА
    // МЕСТЕ: `max-content repeat(auto-fill, max-content) max-content` — это
    // крайние дорожки плюс столько повторов, сколько влезет между ними.
    // Прежде разбор такой список не записывал вовсе, и крайние дорожки
    // исчезали (условие возврата из отката в `computed.rs` — «разворот при
    // непустом списке» — теперь выполнено).
    if let Some(at) = tracks
        .iter()
        .position(|t| matches!(t, TrackSize::AutoRepeat { .. }))
        && tracks.len() > 1
    {
        let TrackSize::AutoRepeat { tracks: тело, .. } = tracks[at].clone() else {
            unreachable!()
        };
        // Сколько влезет: место минус крайние дорожки в точках и зазоры.
        let px_track = |t: &TrackSize| match t {
            TrackSize::Single(Track::Px(v)) => Some(*v),
            _ => None,
        };
        let шаг: f32 = тело.iter().filter_map(px_track).sum::<f32>()
            + cross_gap * (тело.len().saturating_sub(1)) as f32;
        let края: f32 = tracks
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != at)
            .filter_map(|(_, t)| px_track(t))
            .sum();
        let n = match (room, шаг > 0.0) {
            (Some(room), true) => {
                let свободно = (room - края - cross_gap * tracks.len() as f32).max(0.0);
                (((свободно + cross_gap) / (шаг + cross_gap)).floor() as usize).max(1)
            }
            // Тело по СОДЕРЖИМОМУ (`max-content`) или место неизвестно: один
            // повтор — столько, сколько точно законно (§7.2.3.2: при
            // неопределённом месте `auto-fill` даёт одну итерацию).
            _ => 1,
        };
        let mut раскрыт: Vec<TrackSize> = Vec::with_capacity(tracks.len() + n * тело.len());
        раскрыт.extend_from_slice(&tracks[..at]);
        for _ in 0..n {
            раскрыт.extend(тело.iter().cloned());
        }
        раскрыт.extend_from_slice(&tracks[at + 1..]);
        tracks = раскрыт;
    }
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО: точечные дорожки ДО и ПОСЛЕ повтора
    // (`100px repeat(auto-fit, 100px)`, `repeat(4,50px) repeat(auto-fit,50px)
    // repeat(4,50px)`) — разбор их терял, и повтор делил ВЕСЬ контейнер.
    // Разбор написан (`computed::auto_fill_around` + `fixed_tracks`), число
    // повторов считалось от места без сторон. Срез css-grid (1133 пары):
    // 646 -> 646, ни одной пары в любую сторону. Печать показала, почему:
    // на `column-auto-repeat-023` (`100px repeat(auto-fit,100px)` в трёхстах)
    // прежний путь давал три дорожки по сто, новый — сто плюс две по сто,
    // то есть тот же список. Эти пары держит не число дорожек, а размещение
    // элементов на НЕСКОЛЬКО лунок (см. снимки 021: у эталона элементы 1/3/5
    // шире одной дорожки). Возвращать вместе с настоящими спанами лунок.
    // Тело повтора из НЕСКОЛЬКИХ дорожек: `repeat(auto-fill, 50px 50px)`
    // повторяет ПАРУ, и место делится на длину всей пары вместе с зазором
    // между её дорожками (css-grid-2 §7.2.3.2). Разбор список уже держит
    // (`computed::auto_fill_tracks`), но раскладка читала только скалярный
    // `repeat.track` — тело из пары разворачивалось по ОДНОЙ дорожке, и
    // колонок выходило вдвое больше нужного.
    let multi: &[f32] = if row_dir {
        &[]
    } else {
        &merged.grid_auto_fill_tracks
    };
    if tracks.is_empty()
        && multi.len() > 1
        && let Some(room) = room
    {
        let body: f32 = multi.iter().sum::<f32>() + cross_gap * (multi.len() - 1) as f32;
        if body > 0.0 {
            let n = (((room + cross_gap) / (body + cross_gap)).floor() as usize).max(1);
            tracks = (0..n)
                .flat_map(|_| multi.iter().map(|w| TrackSize::Single(Track::Px(*w))))
                .collect();
        }
    }
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
                            // ПРОБОВАЛИ И ОТКАТИЛИ: не резолвить долю вовсе —
                            // против неопределённой дорожки она циклическая и по
                            // css-sizing-3 §5.2.1 ведёт себя как `auto`. Замерено:
                            // css-grid 804 без изменений, oldfront 2340 -> 2339
                            // (дважды). Целевые пары выигрываются и теряются
                            // поровну, потому что часть ЭТАЛОНОВ набрана тем же
                            // неверным правилом. Возвращать вместе с настоящим
                            // вкладом содержимого в дорожку.
                            // Доля элемента считается от места контейнера:
                            // `width: 25%` в трёхстах шестидесяти — дорожка 90
                            // (`column-auto-repeat-auto-002`). Точечный замер её
                            // не видел, и дорожки не разворачивались вовсе.
                            // Процент БЛОЧНОЙ оси (height в рядах) считается от
                            // ДОРОЖКИ, а auto-дорожка размера не имеет — процент
                            // не резолвится, элемент остаётся контентным
                            // (row-auto-repeat-auto-002: ряды по строке, не 25%
                            // контейнера).
                            let pct = match (row_dir, item.style.height, item.style.width) {
                                (false, _, Some(Len::Pct(k))) => Some(k * room),
                                _ => None,
                            };
                            let size = pct.unwrap_or(size);
                            // Дорожка по содержимому меряет ТЕКСТ: у безразмерного
                            // элемента точечной ширины нет, и повторы не
                            // разворачивались вовсе (column-auto-repeat-
                            // max-content-001: Ahem-строка 120px задаёт дорожку).
                            let size = if size > 0.0 || row_dir {
                                size
                            } else {
                                let st = crate::inline::inherit(merged, &item.style);
                                let fs = match st.font_size {
                                    Some(Len::Px(v)) => v,
                                    _ => 16.0,
                                };
                                let family = st.font_family.clone().unwrap_or_else(|| {
                                    if st.monospace == Some(true) {
                                        crate::metrics::mono_family().to_string()
                                    } else {
                                        String::new()
                                    }
                                });
                                let ch = crate::metrics::ch_ex_px(&family, fs).0;
                                let ws = words(&item.children);
                                // `min-content` меряется самым ДЛИННЫМ СЛОВОМ,
                                // `max-content` — всей строкой. Шаг повтора брал
                                // строку в обоих случаях, и число повторов у
                                // `repeat(auto-fill, min-content)` совпадало с
                                // `max-content` до сотых.
                                let total = if repeat.intrinsic_min {
                                    ws.iter().copied().max().unwrap_or(0)
                                } else {
                                    ws.iter().sum::<usize>() + ws.len().saturating_sub(1)
                                };
                                total as f32 * ch
                            };
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
            // Явные линии тянут повторы ДАЛЬШЕ места: `grid-row: 9 / span 2`
            // требует десяти рядов, лишние пустые схлопнет auto-fit
            // (row-auto-repeat-auto-017).
            let need = e
                .children
                .iter()
                .filter_map(|nd| match nd {
                    Node::Element(item) => {
                        let (fixed, span) = lane_span(item, usize::MAX, row_dir);
                        Some(fixed.unwrap_or(0).saturating_add(span))
                    }
                    _ => None,
                })
                .filter(|v| *v < 1000)
                .max()
                .unwrap_or(0);
            let n = n.max(need);
            tracks = match repeat.track {
                Some(px) => vec![TrackSize::Single(Track::Px(px)); n],
                // Дорожка ПО СОДЕРЖИМОМУ: повторы считаются от самого
                // большого элемента (тот же step), а размер каждой лунке даёт
                // её содержимое — Fr делил бы ВЕСЬ контейнер поровну
                // (column-auto-repeat-max-content-001: две дорожки по 120, не
                // по 150).
                None if repeat.intrinsic => {
                    // КОЛОНКИ по содержимому = точечный step (вклады ДО
                    // размещения, все auto-элементы в любую;
                    // column-max-content-001 0.63→0.00 и span-суммы работают).
                    // РЯДЫ остаются по СВОЕМУ содержимому: точечный step
                    // ЗАМЕРЕН в минус (row-mc-001 0.63→3.33 — ряды у ref
                    // разной высоты). `fit-content(N)` — потолок.
                    if !row_dir {
                        let w = match repeat.fit_px {
                            Some(cap) => step.min(cap),
                            None => step,
                        };
                        vec![TrackSize::Single(Track::Px(w)); n]
                    } else {
                        match repeat.fit_px {
                            Some(cap) => {
                                vec![TrackSize::Single(Track::Px(step.min(cap))); n]
                            }
                            // `min-content` меряется самым УЗКИМ местом, а
                            // не самым широким: раньше обе записи давали одно
                            // и то же (вердикты min/max совпадали до сотых).
                            None if repeat.intrinsic_min => {
                                vec![TrackSize::Single(Track::MinContent); n]
                            }
                            None => vec![TrackSize::Single(Track::MaxContent); n],
                        }
                    }
                }
                // Дорожка по содержимому делит место поровну: свой размер ей
                // назначать нельзя, иначе `auto-fit` не сможет отдать место
                // схлопнутых дорожек соседям.
                None => vec![TrackSize::Single(Track::Fr(1.0)); n],
            };
        }
    }
    // Дорожки-КОЛОНКИ по содержимому меряются ДО размещения, и вклад в КАЖДУЮ
    // вносят ВСЕ авто-размещаемые элементы — встать могут в любую (css-grid-3
    // track sizing; intrinsic-sizing-cols-002-auto: четыре auto-колонки
    // шириной с самый широкий элемент). Заданные линии — только своим.
    // Ряды НЕ трогаем: item_height врёт на ортогональном письме и субгриде
    // (row-subgrid-orthogonal 0→11, grid-gap-007 0.15→36 — ЗАМЕРЕНО).
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (перепроверка по Blink, у которого обе оси идут
    // одним алгоритмом — `core/layout/grid_lanes/`): мерить и РЯДЫ, взяв
    // высоту `item_height`, с узким гейтом «среди детей нет ни вертикального
    // письма, ни субсетки» (он снимает ровно те два случая, на которых
    // провалился прошлый заход). Срез css-grid (1133 пары): 645 → 644.
    // Приобретено 2 (`row-defined-height` 2.08 → 0.00, `row-auto-placement-
    // 001` 2.28 → 0.00), потеряно 3 (`row-min-max-content-container`,
    // `row-auto-repeat-{max,min}-content-005` — все 0.00 → ~2).
    // Причина потерь: `item_height` меряет ОДНУ строку (`line_height_px`), а
    // `max-content` ряда требует настоящей высоты содержимого с переносами.
    // Возвращать вместе с дешёвой моделью высоты (перенос по ширине дорожки),
    // а не с `item_height`.
    let intrinsic_track = |t: &TrackSize| {
        matches!(
            t,
            TrackSize::Single(Track::Auto | Track::MaxContent | Track::MinContent)
                | TrackSize::MinMax(..)
        )
    };
    // ★ ЗАМЕРЕНО И ОТКАЧЕНО (05.09): подразумеваемые колонки лунок,
    // рождённые только `grid-column-start` детей, мерить по содержимому с
    // умолчанием `grid-auto-columns: auto` (css-grid-2 §7.2.3). Срез 6445 пар
    // (сетка/позиционирование/флекс): целевая `column-explicit-placement-001`
    // стала ХУЖЕ, 6.77 -> 7.53, `-002` не сдвинулась (3.88 -> 3.86).
    // Ширина по содержимому здесь не та ось: тесты ждут дорожки от
    // ЯВНОГО размещения с учётом щелей, а не свободную усадку.
    if !row_dir && tracks.iter().any(intrinsic_track) {
        let n = tracks.len();
        // ПРОБОВАЛИ И ОТКАТИЛИ: считать вклад по СОДЕРЖИМОМУ плюс края
        // коробки, когда ширина `auto` (css-grid-2 §11.5.1) — сейчас у
        // элемента с отступом весь вклад дают одни поля, и ветка содержимого
        // не запускается вовсе. Проба по всей папке css-grid (1130 пар):
        // приобретено 0, потеряно 0, сдвинулось 12, из них восемь в худшую
        // сторону (`column-subgrid-grid-gap-001` 80.82 -> 83.12,
        // `grid-lanes-gap-002` 8.89 -> 10.02). Эталоны этих семей держит не
        // вклад дорожки, а что-то ниже.
        let cross_of = |item: &Element| -> f32 {
            let w = item_width(item);
            if w > 0.0 {
                return w;
            }
            let st = crate::inline::inherit(merged, &item.style);
            let fs = match st.font_size {
                Some(Len::Px(v)) => v,
                _ => 16.0,
            };
            let family = st.font_family.clone().unwrap_or_else(|| {
                if st.monospace == Some(true) {
                    crate::metrics::mono_family().to_string()
                } else {
                    String::new()
                }
            });
            let ch = crate::metrics::ch_ex_px(&family, fs).0;
            // `width: 2ch` — точечная мера в знаках (intrinsic-sizing-cols-*:
            // первый элемент задаёт ширину ВСЕМ auto-колонкам).
            if let Some(Len::Ch(k)) = item.style.width {
                return k * ch;
            }
            let ws = words(&item.children);
            let total = ws.iter().sum::<usize>() + ws.len().saturating_sub(1);
            total as f32 * ch
        };
        let fixed_px = |t: &TrackSize| match t {
            TrackSize::Single(Track::Px(w)) => Some(*w),
            _ => None,
        };
        let mut contrib = vec![0.0f32; n];
        let mut auto_shares: Vec<(usize, f32)> = vec![];
        // Раздача вклада спаннера: фикс-дорожки охвата вычитаются ПЕРВЫМИ,
        // остаток поровну между интрин-дорожками (css-grid-2 §11.5.1;
        // пример спеки css-grid-3 §track-sizing: 220px рядом со 100px-дорожкой
        // даёт «120 во вторую», а не по 110).
        let spread =
            |contrib: &mut Vec<f32>, tracks: &[TrackSize], at: usize, span: usize, size: f32| {
                let range = at..(at + span).min(tracks.len());
                let fixed_sum: f32 = range.clone().filter_map(|i| fixed_px(&tracks[i])).sum();
                // Fr-дорожка при неопределённом контейнере тоже интрин-получатель
                // (§11.8: доля без свободного места ведёт себя как max-content).
                let intr: Vec<usize> = range
                    .filter(|i| {
                        intrinsic_track(&tracks[*i])
                            || matches!(tracks[*i], TrackSize::Single(Track::Fr(_)))
                    })
                    .collect();
                if intr.is_empty() {
                    return;
                }
                let share = ((size - fixed_sum) / intr.len() as f32).max(0.0);
                for i in intr {
                    contrib[i] = contrib[i].max(share);
                }
            };
        for nd in &e.children {
            let Node::Element(item) = nd else { continue };
            if matches!(
                item.style.position,
                Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
            ) {
                continue;
            }
            let (fixed, span) = lane_span(item, n, row_dir);
            let span = span.clamp(1, n);
            let size = cross_of(item);
            match fixed {
                Some(at) => spread(&mut contrib, &tracks, at, span, size),
                // Авто-размещаемый вкладывается В КАЖДУЮ возможную стартовую
                // позицию (css-grid-3 §track-sizing).
                None => auto_shares.push((span, size)),
            }
        }
        for (span, size) in auto_shares {
            for at in 0..=n.saturating_sub(span) {
                spread(&mut contrib, &tracks, at, span, size);
            }
        }
        for (i, t) in tracks.iter_mut().enumerate() {
            match t {
                TrackSize::Single(Track::Auto | Track::MaxContent | Track::MinContent) => {
                    if contrib[i] > 0.0 {
                        *t = TrackSize::Single(Track::Px(contrib[i]));
                    }
                }
                // minmax: база из min-функции, потолок из max; потолок ниже
                // базы поднимается до неё (css-grid-2 §11.4). Интрин-вклад
                // зажимается в [min..max].
                TrackSize::MinMax(min, max) => {
                    let lo = match min {
                        Track::Px(v) => *v,
                        _ => 0.0,
                    };
                    let hi = match max {
                        Track::Px(v) => *v,
                        // Интрин-потолок не ограничивает.
                        _ => f32::INFINITY,
                    };
                    let w = match min {
                        // Интрин-минимум зажимается потолком: у
                        // `minmax(auto, 0)` дорожка нулевая (…-item-minmax-
                        // img-002), правило «потолок ниже базы поднимается»
                        // работает только для ЧИСЛОВОЙ базы.
                        Track::Auto | Track::MinContent | Track::MaxContent => contrib[i].min(hi),
                        _ => lo.max(contrib[i].min(hi.max(lo))),
                    };
                    *t = TrackSize::Single(Track::Px(w.max(0.0)));
                }
                _ => {}
            }
        }
        // fr при НЕОПРЕДЕЛЁННОМ контейнере (css-grid-2 §11.8): свободного
        // места нет, и доля ведёт себя как max-content — единый коэффициент
        // = max(вклад/долю), дорожка = f * коэффициент.
        if room.is_none() {
            let ratio = tracks
                .iter()
                .enumerate()
                .filter_map(|(i, t)| match t {
                    TrackSize::Single(Track::Fr(f)) if *f > 0.0 => Some(contrib[i] / f),
                    _ => None,
                })
                .fold(0.0f32, f32::max);
            if ratio > 0.0 {
                for t in tracks.iter_mut() {
                    if let TrackSize::Single(Track::Fr(f)) = t {
                        *t = TrackSize::Single(Track::Px(*f * ratio));
                    }
                }
            }
        }
    }
    let count = if !tracks.is_empty() {
        tracks.len()
    } else {
        // Неявные дорожки от ЯВНОГО размещения (css-grid-3: сетка лунок
        // растёт как обычная, §8.5): наибольшая занятая линия по оси лунок
        // задаёт число дорожек. Раньше без шаблона всё сваливалось в одну
        // лунку (row-explicit-placement-001 и родня); и фолбэк читал
        // счётчик КОЛОНОК даже при укладке рядами — чужую ось.
        let mut implicit = 0usize;
        for nd in &e.children {
            let Node::Element(item) = nd else { continue };
            let across = if row_dir {
                item.style.grid_row
            } else {
                item.style.grid_col
            };
            let Some((a, b)) = across else { continue };
            if let crate::computed::Placement::Line(n) = a
                && n > 0
            {
                implicit = implicit.max(n as usize);
            }
            if let crate::computed::Placement::Line(n) = b
                && n > 1
            {
                implicit = implicit.max(n as usize - 1);
            }
        }
        let fallback = if row_dir {
            1
        } else {
            merged.grid_cols.unwrap_or(1).max(1) as usize
        };
        implicit.max(fallback)
    };
    // Реверсы направления (css-grid-3): `track-reverse` меняет только порядок
    // АВТО-перебора дорожек — при равной высоте побеждает ПОСЛЕДНЯЯ; ширины и
    // заданные линии НЕ зеркалятся (сверено с двумя ref: column-align-items-008
    // — авто-элементы уходят вправо; row-track-reverse-dense-...-multi-span-001
    // — `grid-row: 1` остаётся визуально ПЕРВЫМ; попытка зеркалить fixed и
    // ширины ЗАМЕРЕНА: la 99 против 102 без неё).
    // `fill-reverse` разворачивает ЗАПОЛНЕНИЕ: те же слоты, но лунка зеркалится
    // (дети в обратном порядке, прижаты к низу), а элемент с выравниванием
    // прижат к ДАЛЬНЕМУ краю своего слота.
    let fill_reverse = merged.lanes_fill_reverse;
    let track_rev = merged.lanes_track_reverse;
    // Порог «равных» лунок (`flow-tolerance`, css-grid-3): в его пределах от
    // самой короткой заполнение идёт ПОРЯДКОМ ДОКУМЕНТА. Дефолт normal = 1em;
    // `infinite` — строгий документный порядок. ~500 WPT-пар семьи полагаются
    // на дефолт — до этого наш выбор был фактически flow-tolerance: 0.
    // Дефолт normal = 1em (css-grid-3; initial-flow-tolerance: 11.33→0.00).
    // ЗАМЕРЕНО: гейт «только при явном свойстве» даёт 525 против 527 по всей
    // семье — дефолт 1em и спековее, и нетто-лучше. ЦЕНА: 16 пар align-items/
    // justify-items 007-016 упали (их Chrome-ref размещает строго в минимум
    // при разницах < 1em) — расхождение tied-семантики вскрывать отдельным
    // разбором (дамп 010 бок о бок с initial-flow-tolerance).
    let tolerance = match merged.lanes_tolerance {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) => room.unwrap_or(0.0) * k,
        _ => match merged.font_size {
            Some(Len::Px(v)) => v,
            _ => 16.0,
        },
    };
    let mut tracks = tracks;
    // Процентный размер — от контейнера: элемент `width:100%` занимает ВЕСЬ
    // ряд, и следующий уходит в другой
    // (grid-lanes-align-content-refinalize-row-geometry-001). Ветка выбирается
    // по САМОМУ значению, а не по «нулю» итога: margin-box с отрицательным
    // полем меньше нуля, и подмена нулём теряла клэмп-пад следующего
    // (column-negative-margin-001: пятый накрывал третьего).
    let extent = |item: &Element| -> f32 {
        if row_dir {
            match (item.style.width, px_of_size(merged.width)) {
                (Some(Len::Pct(k)), Some(cw)) => cw * k,
                _ => item_width(item),
            }
        } else {
            match (item.style.height, px_of_size(merged.height)) {
                (Some(Len::Pct(k)), Some(ch)) => ch * k,
                _ => {
                    let mut h = item_height(item, merged, opts);
                    // Многострочный текст: одна строка в probe раздаёт floors
                    // не так, как браузер, и элементы падают в чужие лунки
                    // (column-justify-items-end-justify-self-start-001).
                    // Перенос считается ПОСЛОВНО жадно по ширине знака (ch) —
                    // посимвольная оценка ЗАМЕРЕНА И ОТКАЧЕНА (94→93: она
                    // врёт против пословного переноса браузера).
                    if item.style.height.is_none() && h > 0.0 {
                        if let Some(TrackSize::Single(Track::Px(w))) = tracks.first() {
                            let st = crate::inline::inherit(merged, &item.style);
                            let size = match st.font_size {
                                Some(Len::Px(v)) => v,
                                _ => 16.0,
                            };
                            let family = st.font_family.clone().unwrap_or_else(|| {
                                if st.monospace == Some(true) {
                                    crate::metrics::mono_family().to_string()
                                } else {
                                    String::new()
                                }
                            });
                            let ch = crate::metrics::ch_ex_px(&family, size).0;
                            if ch > 0.0 && *w > ch {
                                let per_line = (*w / ch).floor().max(1.0) as usize;
                                let mut lines = 0usize;
                                let mut used_now = 0usize;
                                for word in words(&item.children) {
                                    let need = word.min(per_line);
                                    if used_now == 0 {
                                        lines += 1;
                                        used_now = need;
                                    } else if used_now + 1 + need <= per_line {
                                        used_now += 1 + need;
                                    } else {
                                        lines += 1;
                                        used_now = need;
                                    }
                                }
                                if lines > 1 {
                                    h += (lines as f32 - 1.0) * line_height_px(&st, opts);
                                }
                            }
                        }
                    }
                    h
                }
            }
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
    // своих лунках, — то есть заполняет дыры от многолуночных соседей, В ТОМ
    // ЧИСЛЕ раньше уже уложенных соседей своей лунки: сборка бакетов сортирует
    // слоты по координате, порядок пушей значения не имеет
    // (row-dense-packing-justify-self-multi-span-001: пятый — в дыру второго
    // ряда). Обычная укладка ставит его под всем, что уже уложено.
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
        // Безразмерный элемент (авто вдоль оси) ничего не «пересекает» и
        // падал в занятую первую лунку — у содержимого ширина всегда есть
        // (row-dense-packing-justify-self-002: третий вставал в ряд 1).
        let height = height.max(0.01);
        let mut y = 0.0f32;
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
    // `order` переставляет элементы ДО размещения (css-grid-3 / css-flexbox
    // §5.4): оба прохода идут по одному переупорядоченному списку индексов.
    let order_ix: Vec<usize> = {
        let mut ix: Vec<usize> = (0..e.children.len()).collect();
        let key = |n: &Node| match n {
            Node::Element(el) => el.style.order.unwrap_or(0),
            _ => 0,
        };
        if e.children.iter().any(|n| key(n) != 0) {
            ix.sort_by_key(|i| key(&e.children[*i]));
        }
        ix
    };
    let mut reach: Vec<(usize, f32)> = vec![];
    // Конец слота элемента: верх СЛЕДУЮЩЕГО по разметке соседа его лунок минус
    // зазор. Нужен и растяжке (reach), и обратному заполнению (`fill-reverse`).
    let mut slot_end: Vec<(usize, f32)> = vec![];
    {
        let mut probe: Vec<Vec<(f32, f32)>> = vec![vec![]; count];
        // Курсор авто-размещения: «равные» лунки берутся вперёд по кругу.
        let mut cursor = 0usize;
        let mut placed: Vec<(usize, usize, usize, f32, f32)> = vec![];
        for &idx in &order_ix {
            let child = &e.children[idx];
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
            let at = match fixed {
                Some(f) => f,
                None => {
                    let (at, pos) = shortest_lane_free(
                        &probe, count, span, height, &free_top, track_rev, tolerance, cursor,
                    );
                    // Плотная укладка курсор не двигает: каждый элемент ищет
                    // дыру с начала (аналог dense в css-grid-1 §8.5). Курсор —
                    // линия ЗА элементом: span-3 через все лунки даёт wrap, и
                    // следующий tied берётся с начала (column-align-items-010:
                    // четвёртый в первую лунку, не во вторую).
                    if !dense {
                        cursor = pos + span;
                    }
                    at
                }
            }
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
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("LA_DBG").is_ok());
            *ON
        } {
            eprintln!("LA placed={placed:?} reach0={reach:?}");
        }
        // Верх следующего элемента той же лунки — ПО КООРДИНАТЕ, не по порядку
        // разметки: плотная укладка ставит элемент в дыру ПЕРЕД уже уложенными
        // соседями, и его нижний сосед размечен раньше него
        // (column-dense-packing-align-self-001: третий не видел второго).
        let next_top = |idx: usize| -> Option<f32> {
            let (_, at, span, top, _) = *placed.iter().find(|p| p.0 == idx)?;
            placed
                .iter()
                .filter(|(j, a, sp, t, _)| {
                    *j != idx
                        && *a < at + span
                        && at < *a + *sp
                        && (*t > top + 0.01 || (*j > idx && *t > top - 0.01))
                })
                .map(|p| p.3)
                .min_by(f32::total_cmp)
        };
        slot_end = placed
            .iter()
            .filter_map(|(idx, ..)| next_top(*idx).map(|t| (*idx, t - along_gap)))
            .collect();
        reach = reach
            .into_iter()
            .filter_map(|(idx, top)| {
                // Предел не выбрасывается и нулевым: сама ЗАПИСЬ запрещает
                // рост (row-justify-items-004: пятый span-3 с нулевым
                // пределом от шестого растягивался на весь остаток).
                let height = (next_top(idx)? - along_gap - top).max(0.0);
                Some((idx, height))
            })
            .collect();
        if {
            static ON: std::sync::LazyLock<bool> =
                std::sync::LazyLock::new(|| std::env::var("LA_DBG").is_ok());
            *ON
        } {
            eprintln!("LA reach={reach:?}");
        }
    }
    // Слот лунки: координата и коробка. Бакеты собираются ПОСЛЕ раскладки
    // сортировкой по координате — плотная укладка ставит элемент в дыру
    // РАНЬШЕ уже уложенных соседей, и порядок пушей перестаёт быть порядком
    // отрисовки. `along` — выравнивание вдоль оси укладки (из стиля его потом
    // не достать: проработанная ось гасится на клоне, иначе CSS `align-self`
    // утекает в flex как ПОПЕРЕЧНЫЙ и рушит растяжку по ширине —
    // `column-align-self-003`: первый сжимался в столбик).
    struct SlotNode {
        top: f32,
        height: f32,
        node: Node,
        along: Option<Align>,
        real: bool,
    }
    let mut slots: Vec<Vec<SlotNode>> = (0..count).map(|_| vec![]).collect();
    let used_sizes = lane_used_sizes(&tracks);
    // Позиционированные — вне потока: в конец первой лунки, без падов.
    let mut extras: Vec<Node> = vec![];
    // Курсор авто-размещения основного прохода (зеркало probe).
    let mut cursor = 0usize;
    for &idx in &order_ix {
        let child = &e.children[idx];
        let Node::Element(item) = child else {
            continue;
        };
        // Позиционированный элемент лунок не занимает: он вне потока. Но с
        // заданными линиями его containing block — СВОИ ДОРОЖКИ (css-grid-3,
        // row-grid-lanes-alignment-positioned-items-*): absolute-обёртка по
        // геометрии дорожек, self-оси выравнивают содержимое внутри неё.
        if matches!(
            item.style.position,
            Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
        ) {
            let wrapped = (|| -> Option<Node> {
                // Свои инсеты сильнее дорожек — обёртка их не перекрывает.
                let ins = item.style.inset;
                if ins.top.is_some()
                    || ins.bottom.is_some()
                    || ins.left.is_some()
                    || ins.right.is_some()
                {
                    return None;
                }
                let (fixed, span) = lane_span(item, count, row_dir);
                let a = fixed?;
                let span = span.clamp(1, count);
                let a = if track_rev {
                    count.saturating_sub(a + span)
                } else {
                    a
                }
                .min(count - span);
                let px_of = |t: Option<&TrackSize>| match t {
                    Some(TrackSize::Single(Track::Px(w))) => Some(*w),
                    _ => None,
                };
                let mut off = 0.0f32;
                for i in 0..a {
                    off += px_of(tracks.get(i))? + cross_gap;
                }
                let mut size = cross_gap * (span as f32 - 1.0);
                for i in a..a + span {
                    size += px_of(tracks.get(i))?;
                }
                use crate::computed::{FlexDir, Justify, Position};
                // ★ ЗАМЕРЕНО И ОТКАЧЕНО: прибавка паддинга контейнера к
                // инсетам обёртки (гипотеза «инсет от padding box, дорожки от
                // content box») — positioned-items 001 0.23→1.71, 002
                // 0.36→4.08, la 94→92. Инсет тут уже попадает в content box;
                // остаток 003/004 (~1.1-3.5) — не паддинг, вскрывать
                // поэлементным пиксельным диффом.
                let mut style = Computed::default();
                style.position = Some(Position::Absolute);
                style.display = Some(Display::Flex);
                style.flex_dir = Some(FlexDir::Row);
                if row_dir {
                    style.inset.top = Some(Len::Px(off));
                    style.height = Some(Len::Px(size));
                    style.inset.left = Some(Len::Px(0.0));
                    style.inset.right = Some(Len::Px(0.0));
                } else {
                    style.inset.left = Some(Len::Px(off));
                    style.width = Some(Len::Px(size));
                    style.inset.top = Some(Len::Px(0.0));
                    style.inset.bottom = Some(Len::Px(0.0));
                }
                // Инлайн-ось — justify-self (rtl зеркалит), блочная — align-self;
                // без своих значений — *-items контейнера.
                let flip = merged.rtl == Some(true);
                style.justify_content =
                    Some(match item.style.justify_self.or(merged.justify_items) {
                        Some(Align::End) => {
                            if flip {
                                Justify::Start
                            } else {
                                Justify::End
                            }
                        }
                        Some(Align::Center) => Justify::Center,
                        _ => {
                            if flip {
                                Justify::End
                            } else {
                                Justify::Start
                            }
                        }
                    });
                style.align_items = Some(match item.style.align_self.or(merged.align_items) {
                    Some(a @ (Align::End | Align::Center | Align::Stretch)) => a,
                    _ => Align::Start,
                });
                let mut freed = item.clone();
                freed.style.position = None;
                Some(Node::Element(Element {
                    list_item: None,
                    node_id: 0,
                    anim: None,
                    tag: "div".into(),
                    style,
                    hover: None,
                    first_letter: None,
                    first_line: None,
                    children: vec![Node::Element(freed)],
                    attrs: vec![],
                    inline: false,
                }))
            })();
            let ins = item.style.inset;
            let has_insets = ins.top.is_some()
                || ins.bottom.is_some()
                || ins.left.is_some()
                || ins.right.is_some();
            match wrapped {
                Some(w) => extras.push(w),
                // Заданные края — от контейнера; без краёв и линий элемент
                // стоит на СТАТИЧЕСКОЙ позиции: в потоке первой лунки, после
                // уже уложенного (row-grid-lanes-out-of-flow-003).
                None if has_insets => extras.push(child.clone()),
                None => {
                    let top = used[0].iter().map(|(_, e)| *e).fold(0.0f32, f32::max);
                    slots[0].push(SlotNode {
                        top,
                        height: 0.0,
                        node: child.clone(),
                        along: None,
                        real: false,
                    });
                }
            }
            continue;
        }
        // Заданные линии сильнее раздачи: элемент встаёт именно между ними и
        // может ЗАНЯТЬ НЕСКОЛЬКО лунок.
        let (fixed, span) = lane_span(item, count, row_dir);
        let span = span.clamp(1, count);
        let mut height = extent(item);
        let at = match fixed {
            Some(f) => f,
            None => {
                let (at, pos) = shortest_lane_free(
                    &used, count, span, height, &free_top, track_rev, tolerance, cursor,
                );
                if !dense {
                    cursor = pos + span;
                }
                at
            }
        }
        .min(count - span);
        // Верх элемента — низ самой заполненной из перекрытых лунок; при
        // плотной укладке — верхняя свободная отметка (дыры заполняются).
        let mut top = free_top(&used, at, span, height);
        // Выравнивание элемента ВДОЛЬ лунки: `stretch` растит его на остаток,
        // остальные значения оставляют ему свой размер.
        let free = along_free(item);
        let along = along_align(item);
        // Выравнивание НЕ-хвостового элемента в его слоте (от floor до верха
        // следующего): `center` — середина, `end` — дальний край. При обратном
        // заполнении ось зеркальна: дальним краем становится `start` (ref
        // column-align-items-008: первый элемент [20,60] от низа, а не [0,40]).
        if let Some((_, end)) = slot_end.iter().find(|(j, _)| *j == idx) {
            let far = match along {
                Some(Align::Start) => fill_reverse,
                Some(Align::End) => !fill_reverse,
                _ => false,
            };
            let shifted = if far {
                end - height
            } else if along == Some(Align::Center) {
                top + (end - top - height) / 2.0
            } else {
                top
            };
            if shifted > top {
                top = shifted;
            }
        }
        // Наличие записи в reach само по себе значит: предел элемента — верх
        // следующего в лунке, и рост до низа контейнера ему ЗАПРЕЩЁН, даже
        // когда расти некуда (рост равен контентной высоте —
        // column-align-items-004: пятый заливал низ вместо полосы в 16).
        let reach_entry = reach.iter().find(|(j, _)| *j == idx).map(|(_, h)| *h);
        let grown = reach_entry.filter(|h| *h > height);
        if let Some(h) = grown {
            height = h;
        }
        for lane in at..at + span {
            if lane != at {
                // Место, занятое чужим элементом: своей коробки тут нет, но
                // следующий элемент лунки обязан начаться ПОД ним.
                slots[lane].push(SlotNode {
                    top,
                    height,
                    node: spacer(height, row_dir),
                    along: None,
                    real: false,
                });
            }
            used[lane].push((top, top + height));
        }
        // Элемент без заданной высоты ТЯНЕТСЯ вдоль лунки до низа контейнера
        // (`align-items: stretch` по оси укладки).
        let mut item = item.clone();
        // Попытка №3 (после Px-ификации дорожек contrib/step-правками):
        // сабгрид-ребёнок получает ТОЧНЫЙ Px-кусок родительских дорожек — те
        // самые «разрешённые ширины», которых не было в откатах №1 (свои
        // дорожки по линиям: −17 baseline) и №2 (сырой кусок: −4 gap).
        // ЗАМЕРЕНО И ОТКАЧЕНО: отменять подсетку у элемента, образующего
        // НЕЗАВИСИМЫЙ контекст форматирования (css-grid-2 §9), по признакам
        // `contain: layout|paint` и внепоточности. Полный свод CSS3: 0 и 0 —
        // `container-type` движок не разбирает вовсе, а по двум оставшимся
        // признакам ни одна пара свода не проходит.
        if item.style.subgrid {
            let slice: Vec<TrackSize> = (at..at + span)
                .filter_map(|i| tracks.get(i).cloned())
                .collect();
            if slice.len() == span
                && slice
                    .iter()
                    .all(|t| matches!(t, TrackSize::Single(Track::Px(_))))
            {
                // ПРОБОВАНО: замораживать и РАЗМЕР элемента суммой куска
                // (спека: свой размер в сабгридной оси игнорируется) —
                // subgrid-stretch-001 19.8→20.4, нетто 0: реверт, только
                // дорожки.
                // Собственные края субгрида (margin+border+padding) ВЫЧИТАЮТСЯ
                // из первой и последней дорожки куска (css-grid-2 §subgrids):
                // дорожка 100px у субгрида с margin/padding/border по 10 —
                // это внутренние 100−(10+10+10)=70 с каждой стороны краёв.
                let side = |m: Option<Len>, b: Option<Len>, p: Option<Len>| -> f32 {
                    let px = |l: Option<Len>| match l {
                        Some(Len::Px(v)) => v,
                        _ => 0.0,
                    };
                    px(m) + px(b) + px(p)
                };
                let bs = item.style.borders();
                let (lead, trail) = if row_dir {
                    (
                        side(item.style.margin.top, bs.top, item.style.padding.top),
                        side(
                            item.style.margin.bottom,
                            bs.bottom,
                            item.style.padding.bottom,
                        ),
                    )
                } else {
                    (
                        side(item.style.margin.left, bs.left, item.style.padding.left),
                        side(item.style.margin.right, bs.right, item.style.padding.right),
                    )
                };
                let mut slice = slice;
                if let Some(TrackSize::Single(Track::Px(w))) = slice.first_mut() {
                    *w = (*w - lead).max(0.0);
                }
                if let Some(TrackSize::Single(Track::Px(w))) = slice.last_mut() {
                    *w = (*w - trail).max(0.0);
                }
                // Разница зазоров — тем же правилом, что и на разборе:
                // иначе стороны разъезжаются, когда тест написан на лунках, а
                // эталон на обычной сетке (замерено: 0 и −3).
                let (prow, pcol) = merged.gap.unwrap_or((None, None));
                let (crow, ccol) = item.style.gap.unwrap_or((None, None));
                let par = if row_dir { prow } else { pcol };
                // Незаданный зазор подсетки — `normal`, то есть «как у
                // родителя» (css-grid-2 §subgrid-gaps), разница ноль.
                let own = if row_dir { crow } else { ccol }.or(par);
                if crow.is_none() && ccol.is_none() {
                    item.style.gap = Some(if row_dir { (par, ccol) } else { (crow, par) });
                    if !row_dir {
                        item.style.column_gap = par;
                    }
                }
                crate::dom::subgrid_gap_slice(&mut slice, par, own);
                // В сабгридной оси SELF-выравнивание НЕ действует: субгрид
                // держит ВСЮ дорожку (все четыре js/je/jc/jb варианта
                // subgrid-alignment-in-subgridded-axis-001 обязаны совпасть).
                if row_dir {
                    item.style.grid_rows = Some(slice);
                    item.style.align_self = None;
                } else {
                    item.style.grid_tracks = Some(slice);
                    item.style.grid_cols = Some(span as u16);
                    item.style.justify_self = None;
                }
            }
        }
        if let Some(h) = grown {
            // Размер вдоль лунки известен — рост не нужен, иначе элемент
            // съест и остаток лунки.
            if row_dir {
                item.style.width = Some(Len::Px(h));
            } else {
                item.style.height = Some(Len::Px(h));
            }
        } else if free
            && reach_entry.is_none()
            && (matches!(along, Some(Align::Stretch))
                || (along.is_none()
                    && !merged.lanes_inline
                    && px_of_size(if row_dir { merged.width } else { merged.height }).is_some()))
        {
            // Рост до низа — только у ХВОСТОВОГО элемента лунки (без
            // следующего). ЯВНЫЙ stretch растит всегда; `normal` (пусто)
            // вдоль оси укладки растит лишь при ОПРЕДЕЛЁННОМ размере
            // контейнера — у авто-контейнера низ задаёт сам контент, и рост
            // раздувал элементы (grid-lanes-justify-content-001), а явному
            // stretch авто-контейнер не помеха (column-align-items-001).
            // ПРОБОВАЛИ И ОТКАТИЛИ снятие роста целиком (замер по
            // target/la.txt): column-align-items-001/003/007 и
            // row-justify-items 0.0 -> 1.2..15, вверх ноль.
            item.style.flex_grow = Some(1.0);
        }
        // Размер по числу занятых лунок: коробка выходит за свою лунку в
        // соседние, а их место держат распорки. СВОЙ поперечный размер
        // перезаписывать нельзя — элемент выравнивается ВНУТРИ области
        // span-дорожек (row-align-items-end-align-self-start-001: розовый
        // 80px раздувался на две дорожки и end не работал) — тогда область
        // строится обёрткой при укладке в слот.
        let mut span_area: Option<f32> = None;
        if span > 1 {
            let width: f32 = (at..at + span)
                .filter_map(|i| used_sizes.get(i).copied().flatten())
                .sum();
            if width > 0.0 {
                let area = width + cross_gap * (span as f32 - 1.0);
                let own = if row_dir {
                    item.style.height
                } else {
                    item.style.width
                };
                if own.is_none() {
                    if row_dir {
                        item.style.height = Some(Len::Px(area));
                    } else {
                        item.style.width = Some(Len::Px(area));
                    }
                } else {
                    span_area = Some(area);
                }
            }
        }
        // Ось укладки проработана здесь (сдвиг в слоте / коробка хвостового) —
        // до флекса лунки она дойти не должна: там та же ось уже ПОПЕРЕЧНАЯ.
        // А ПОПЕРЕЧНОЕ выравнивание (justify-* в колонках, align-* в рядах) —
        // это cross-ось флекса лунки, то есть taffy align_self: в колонках
        // туда переносится CSS justify-self/-items (в taffy justify_self во
        // флексе мёртв — column-grid-lanes-justify-self-002/003).
        // Модификатор `safe` при элементе БОЛЬШЕ дорожки глушит center/end в
        // start (css-align §5.3, `column-overflow-alignment-001`); без него
        // элемент честно вылезает.
        let zone: f32 = (at..at + span)
            .filter_map(|i| used_sizes.get(i).copied().flatten())
            .sum::<f32>()
            + cross_gap * (span as f32 - 1.0);
        let cross_safe = |own: Option<Align>, own_safe: bool, items_safe: bool| {
            if own.is_some() { own_safe } else { items_safe }
        };
        if row_dir {
            item.style.justify_self = None;
            let safe = cross_safe(
                item.style.align_self,
                item.style.align_self_safe,
                merged.align_items_safe,
            );
            let mut cross = item.style.align_self.or(merged.align_items);
            if safe
                && matches!(cross, Some(Align::End | Align::Center))
                && zone > 0.0
                && item_height(&item, merged, opts) > zone + 0.01
            {
                cross = Some(Align::Start);
            }
            item.style.align_self = cross;
        } else {
            let safe = cross_safe(
                item.style.justify_self,
                item.style.justify_self_safe,
                merged.justify_items_safe,
            );
            let mut cross = item.style.justify_self.or(merged.justify_items);
            if safe
                && matches!(cross, Some(Align::End | Align::Center))
                && zone > 0.0
                && item_width(&item) > zone + 0.01
            {
                cross = Some(Align::Start);
            }
            item.style.align_self = cross;
            item.style.justify_self = None;
        }
        // Слот, ОГРАНИЧЕННЫЙ соседом (есть конец слота), уже выровнен
        // сдвигом top — коробка хвостового на весь остаток ему не положена
        // (column-fill-reverse-dense-packing-align-items-multi-span-001:
        // пятый с соседом сверху уезжал коробкой к верху всей лунки).
        let along = if slot_end.iter().any(|(j, _)| *j == idx) {
            None
        } else {
            along
        };
        // `order` уже отработал ПРИ РАЗМЕЩЕНИИ (порядок обхода детей);
        // в бакете лунки сортировать больше нечего — рядом лежат распорки
        // с order=0, и элемент прыгал бы через свою распорку.
        item.style.order = None;
        // Область span-дорожек: обёртка поперечного размера области, элемент
        // выравнивается внутри своим (перенесённым) align_self.
        let node = match span_area {
            Some(area) => {
                let mut style = Computed::default();
                style.display = Some(Display::Flex);
                style.flex_dir = Some(if row_dir {
                    crate::computed::FlexDir::Row
                } else {
                    crate::computed::FlexDir::Col
                });
                if row_dir {
                    style.height = Some(Len::Px(area));
                } else {
                    style.width = Some(Len::Px(area));
                }
                Node::Element(Element {
                    list_item: None,
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
            None => Node::Element(item),
        };
        slots[at].push(SlotNode {
            top,
            height,
            node,
            along,
            real: true,
        });
    }
    // Сборка бакетов из слотов: сортировка по координате (плотная укладка
    // ставит элемент раньше уже уложенных), пады из разниц координат.
    // Распорка — ЛИШНИЙ ребёнок гибкой лунки с собственным `gap`: каждая
    // добавляет одну щель, поэтому её размер уменьшается на зазор, а распорка
    // не толще зазора не ставится вовсе (её роль играет сама щель) —
    // column-align-items-004: четвёртый элемент сидел на 10 ниже.
    let mut buckets: Vec<Vec<Node>> = Vec::with_capacity(count);
    // Протяжённость содержимого лунки вдоль оси — для `safe` content-раздачи.
    let mut lane_extents: Vec<f32> = Vec::with_capacity(count);
    for lane in slots {
        let mut lane = lane;
        lane.sort_by(|a, b| a.top.total_cmp(&b.top));
        lane_extents.push(lane.iter().map(|s| s.top + s.height).fold(0.0f32, f32::max));
        let last_real = lane.iter().rposition(|s| s.real);
        let mut nodes: Vec<Node> = Vec::with_capacity(lane.len() * 2);
        let mut cursor = 0.0f32;
        let last_idx = lane.len().saturating_sub(1);
        for (j, mut s) in lane.into_iter().enumerate() {
            let pad = s.top - cursor;
            // Щель РОВНО в зазор выражается НУЛЕВЫМ спейсером (двойной
            // флекс-гэп): прежний гейт `pad > gap` её выбрасывал, и реверсные
            // ряды съезжали на зазор (row-fill-reverse-definite-size-001:
            // группа на 10px правее эталона). Щель меньше полузазора
            // остаётся самим гэпом — точнее флексом не выразить.
            if pad > along_gap * 0.5 + 0.01 {
                nodes.push(spacer((pad - along_gap).max(0.0), row_dir));
            }
            cursor = s.top + s.height + along_gap;
            // Тянется ТОЛЬКО последний элемент лунки: свободное место копится
            // в хвосте, у остальных рост снимается (`column-align-items-003`).
            let tail = last_real == Some(j);
            if s.real && !tail {
                if let Node::Element(el) = &mut s.node {
                    el.style.flex_grow = None;
                }
            }
            // Дети лунки НЕ сжимаются: содержимое шире лунки переполняет её,
            // как блочный поток (row-fill-reverse-justify-content-safe-001:
            // два по 40px в лунке 60px сжимались до 27 вместо вылета).
            // Узел с ОТРИЦАТЕЛЬНЫМ полем вдоль оси — исключение: запрет
            // сжатия ломал его поток и съедал клэмп-пад соседа
            // (column-negative-margin-001: пятый снова накрывал третьего).
            if let Node::Element(el) = &mut s.node {
                let neg = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v < 0.0);
                let m = el.style.margin;
                let neg_along = if row_dir {
                    neg(m.left) || neg(m.right)
                } else {
                    neg(m.top) || neg(m.bottom)
                };
                if el.style.flex_shrink.is_none() && !neg_along {
                    el.style.flex_shrink = Some(0.0);
                }
            }
            // Свободное место лунки достаётся ПОСЛЕДНЕМУ её элементу: по CSS
            // его область тянется до конца контейнера, и `align-items`
            // выравнивает его внутри неё. Растяжка уже учтена ростом;
            // остальные значения требуют коробки на весь остаток
            // (`column-align-items-001`). Распорка чужого элемента ПОСЛЕ
            // хвостового съедает остаток — тогда коробки нет.
            if tail && j == last_idx {
                if let (
                    Some(along @ (Align::Center | Align::End | Align::Start)),
                    Node::Element(el),
                ) = (s.along, &s.node)
                {
                    nodes.push(lane_align_box(el.clone(), along, row_dir));
                    continue;
                }
            }
            nodes.push(s.node);
        }
        // `fill-reverse` зеркалит лунку: дети в обратном порядке, прижаты к
        // концу. Пады оказываются ПОД своими элементами, коробка хвостового —
        // сверху и тянет элемент к дальнему краю (ref column-align-items-008:
        // 6 и 7 у потолка, 1 — [20,60] от низа).
        if fill_reverse {
            nodes.reverse();
        }
        buckets.push(nodes);
    }
    // `auto-fit` схлопывает ПУСТЫЕ дорожки: место, которое им причиталось,
    // делят между собой непустые (`column-auto-repeat-auto-012`: две дорожки
    // по 150 вместо трёх по 100).
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
    // После auto-fit-схлопывания список дорожек другой — размеры заново.
    let used_sizes = lane_used_sizes(&tracks);
    let mut row = styled_div_with(e, merged).flex();
    row = if row_dir {
        row.flex_col().gap_y(gpui::px(cross_gap))
    } else {
        row.flex_row().gap_x(gpui::px(cross_gap))
    };
    // Сырые content-свойства утекали из styled_div на флекс контейнера и
    // двигали ЛУНКИ по чужой оси (row-fill-reverse-justify-content-001:
    // center/end роняли ряды вниз на половину/весь остаток). Раздачей здесь
    // управляют только across- и along-ветки ниже.
    row.style().justify_content = None;
    row.style().align_content = None;
    // Строчный контейнер лунок обнимает свои дорожки, а не строку
    // (grid-lanes-align-content-001: блоки на всю страницу вместо ширины
    // четырёх дорожек).
    // `width: min-content/max-content` — та же обтяжка по дорожкам: точечная
    // мера контейнера и есть их сумма (intrinsic-sizing-cols-*).
    let hug_width =
        merged.width.is_none() || matches!(merged.width, Some(Len::MinContent | Len::MaxContent));
    if merged.lanes_inline && !row_dir && hug_width {
        let total: f32 = tracks
            .iter()
            .map(|t| match t {
                TrackSize::Single(Track::Px(w)) => *w,
                _ => f32::NAN,
            })
            .sum();
        if total.is_finite() && total > 0.0 {
            // gpui-размер — BORDER-BOX: свои паддинги и рамки контейнер несёт
            // сверх дорожек (grid-lanes-subgrid-001b: с голой суммой контейнер
            // ужимался на паддинг, и вся сетка съезжала).
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                _ => 0.0,
            };
            let b = merged.borders();
            let extra = side(merged.padding.left)
                + side(merged.padding.right)
                + side(b.left)
                + side(b.right);
            row = row.w(gpui::px(
                total + cross_gap * (tracks.len().saturating_sub(1)) as f32 + extra,
            ));
        }
    }
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
            // `left`/`right` — физические края (лунки).
            Justify::Left => Some(gpui::JustifyContent::Start),
            Justify::Right => Some(gpui::JustifyContent::End),
            Justify::WmStart | Justify::WmEnd => {
                let flip = merged.rtl == Some(true);
                let end = (j? == Justify::WmEnd) != flip;
                Some(if end {
                    gpui::JustifyContent::End
                } else {
                    gpui::JustifyContent::Start
                })
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
    // Позиционированные дети живут на САМОМ контейнере (он их содержащий
    // блок), а не в лунке: внутри лунки absolute не рисовался вовсе
    // (grid-lanes/abspos/*: красные абспосы пропадали с картинки).
    for (i, items) in buckets.into_iter().enumerate() {
        let mut lane = div().flex();
        lane = if row_dir {
            lane.flex_row().min_h_0().gap_x(gpui::px(along_gap))
        } else {
            lane.flex_col().min_w_0().gap_y(gpui::px(along_gap))
        };
        // Раздача СОДЕРЖИМОГО лунки вдоль оси укладки — это `align-content`
        // в колонках и `justify-content` в рядах; стороны ФИЗИЧЕСКИЕ и при
        // обратном заполнении (column-fill-reverse-align-content-001: start
        // тянет группы к верху). Без заданной раздачи зеркальная лунка
        // прижата к концу.
        let along_content = if row_dir {
            merged.justify_content
        } else {
            merged.align_content
        };
        // `safe` у content-раздачи: при переполнении лунки содержимым раздача
        // отставляется в start (css-align 5.3; у зеркальной лунки start —
        // её End-дефолт: row-fill-reverse-justify-content-safe-001).
        let along_safe = if row_dir {
            merged.justify_content_safe
        } else {
            merged.align_content_safe
        };
        let along_size = px_of_size(if row_dir { merged.width } else { merged.height });
        let overflowed = matches!(
            (lane_extents.get(i), along_size),
            (Some(c), Some(a)) if *c > a + 0.01
        );
        match content(along_content) {
            Some(j) if !(along_safe && overflowed) => lane.style().justify_content = Some(j),
            _ if fill_reverse => {
                lane.style().justify_content = Some(gpui::JustifyContent::End);
            }
            _ => {}
        }
        match (used_sizes.get(i).copied().flatten(), tracks.get(i)) {
            (Some(w), _) if row_dir => lane = lane.h(gpui::px(w)).flex_shrink_0(),
            (Some(w), _) => lane = lane.w(gpui::px(w)).flex_shrink_0(),
            (None, t) => match t {
                Some(TrackSize::Single(Track::Pct(k))) if row_dir => {
                    lane = lane.h(gpui::relative(*k)).flex_shrink_0()
                }
                Some(TrackSize::Single(Track::Pct(k))) => {
                    lane = lane.w(gpui::relative(*k)).flex_shrink_0()
                }
                // Дорожка по содержимому шире содержимого не бывает: ширину
                // ей задаёт самый широкий элемент лунки, а не равная доля.
                Some(TrackSize::Single(Track::Auto | Track::MinContent | Track::MaxContent)) => {
                    lane = lane.flex_shrink_0()
                }
                Some(TrackSize::Single(Track::Fr(f))) => {
                    lane.style().flex_grow = Some(*f);
                    lane = lane.flex_basis(px(0.));
                }
                _ => lane = lane.flex_1(),
            },
        }
        row = row.child(lane.children(blocks(&items, merged, opts)));
    }
    if !extras.is_empty() {
        let mut ctx = merged.clone();
        ctx.display = Some(Display::Block);
        row = row.children(blocks(&extras, &ctx, opts));
    }
    row.into_any_element()
}

/// Между какими лунками стоит элемент: начало (если задано) и сколько занимает.
/// Used-размеры дорожек лунок (css-grid-3 §track-sizing, каркас):
/// пока в точки превращаются только заданные `Px`; интрин/minmax/fr
/// остаются None и живут прежними ветками. Единая точка последующей
/// Px-ификации всех дорожек (план target/scout-lanes-spec.md).
fn lane_used_sizes(tracks: &[crate::computed::TrackSize]) -> Vec<Option<f32>> {
    use crate::computed::{Track, TrackSize};
    tracks
        .iter()
        .map(|t| match t {
            TrackSize::Single(Track::Px(w)) => Some(*w),
            _ => None,
        })
        .collect()
}

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
            (
                Some(s.min(t)),
                (t as i32 - s as i32).unsigned_abs() as usize,
            )
        }
        Some((Placement::Line(a), Placement::Span(k))) => (Some(line(a)), k as usize),
        Some((Placement::Line(a), Placement::Auto)) => (Some(line(a)), 1),
        // `span 3 / 4` — конец задан ЛИНИЕЙ, начало отсчитывается от неё
        // назад (intrinsic-sizing-cols: шестой span3/4 сидит в 0..2, а не в
        // авто-выборе).
        Some((Placement::Span(k), Placement::Line(b))) => {
            // line(b) — индекс дорожки, начинающейся на линии b; конец на b
            // значит последняя занятая дорожка line(b)−1, начало — line(b)−k.
            (Some(line(b).saturating_sub(k as usize)), k as usize)
        }
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
        list_item: None,
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
        list_item: None,
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
    tolerance: f32,
    cursor: usize,
) -> (usize, usize) {
    // Побеждает лунка в пределах ПОРОГА от самой короткой, ПЕРВАЯ в порядке
    // обхода (css-grid-3 `flow-tolerance`: близкие лунки «равны», заполнение
    // идёт порядком документа; дефолт normal = 1em). Обычно первая — левая,
    // при `track-reverse` — правая (дорожки перечислены от конца). `min_by`
    // при равенстве отдаёт ПОСЛЕДНИЙ минимум — tie-break уезжал в другую
    // сторону (`column-align-items-008`: шестой вставал правее эталона).
    // Из «равных» берётся первая линия НЕ РАНЬШЕ КУРСОРА авто-размещения
    // (движение вперёд, css-grid-3 §4.4); нет таких — первая равная вообще.
    let pick = |it: &mut dyn Iterator<Item = usize>| -> (usize, usize) {
        let order: Vec<usize> = it.collect();
        let mut best_top = f32::INFINITY;
        for &i in &order {
            let t = top_of(used, i, span, height);
            if t < best_top {
                best_top = t;
            }
        }
        let tied = |i: usize| top_of(used, i, span, height) <= best_top + tolerance + 0.01;
        let pos = order
            .iter()
            .enumerate()
            .position(|(p, i)| p >= cursor && tied(*i))
            .or_else(|| order.iter().position(|i| tied(*i)))
            .unwrap_or(0);
        (order.get(pos).copied().unwrap_or(0), pos)
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
/// Длины слов текста поддерева (в знаках) — для пословной оценки переноса.
fn words(nodes: &[Node]) -> Vec<usize> {
    let mut out = vec![];
    for n in nodes {
        match n {
            Node::Text(t) => out.extend(t.split_whitespace().map(|w| w.chars().count())),
            Node::Element(e) => out.extend(words(&e.children)),
        }
    }
    out
}

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
