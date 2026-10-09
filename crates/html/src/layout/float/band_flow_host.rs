//! Поток полос: дети и охранники полос.
// owner: A

use crate::render::*;

thread_local! {
    /// Письмо содержащего блока, для которого `wrap_floats` собирает хост:
    /// 0 — горизонтальное, 1 — `vertical-rl`, 2 — `vertical-lr`. Им гейты
    /// измеряемого хоста отличают ортогональный поток от своего.
    pub(crate) static BAND_WM: std::cell::Cell<u8> = const { std::cell::Cell::new(0) };
}

thread_local! {
    /// Высота содержащего блока хоста в точках, если задана: от неё доли
    /// высоты детей (§10.5). Хост несёт её атрибутом `cbh`.
    pub(crate) static BAND_CBH: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

thread_local! {
    /// Слой `::first-line` содержащего блока (`inherited.first_line` в
    /// `blocks()`): измеряемый хост — синтетический узел, своего слоя у него
    /// нет, и `element` отдал бы детям `None`. Хост, с которого начинается
    /// содержимое блока, несёт слой узлом (`wrap_floats`).
    pub(crate) static BAND_FL: std::cell::RefCell<Option<Computed>> = const { std::cell::RefCell::new(None) };
}

/// Вернуть прежний слой первой строки по выходе из `blocks()`.
pub(crate) struct BandFlGuard(pub(crate) Option<Computed>);

impl Drop for BandFlGuard {
    fn drop(&mut self) {
        BAND_FL.with(|f| *f.borrow_mut() = self.0.take());
    }
}

thread_local! {
    /// Ширина содержащего блока хоста в точках, если задана. В вертикальном
    /// письме это его БЛОЧНЫЙ размер: от неё доли `block-size` флоатов
    /// (`width` после перевода логических свойств, §10.5 по блочной оси).
    /// Хост несёт её атрибутом `cbw`.
    pub(crate) static BAND_CBW: std::cell::Cell<Option<f32>> = const { std::cell::Cell::new(None) };
}

/// Вернуть прежнюю ширину содержащего блока хоста по выходе из `blocks()`.
pub(crate) struct BandCbwGuard(pub(crate) Option<f32>);

impl Drop for BandCbwGuard {
    fn drop(&mut self) {
        BAND_CBW.with(|w| w.set(self.0));
    }
}

/// Вернуть прежнюю высоту содержащего блока хоста по выходе из `blocks()`.
pub(crate) struct BandCbhGuard(pub(crate) Option<f32>);

impl Drop for BandCbhGuard {
    fn drop(&mut self) {
        BAND_CBH.with(|h| h.set(self.0));
    }
}

/// Вернуть прежнее письмо хоста по выходе из `blocks()`.
pub(crate) struct BandWmGuard(pub(crate) u8);

impl Drop for BandWmGuard {
    fn drop(&mut self) {
        BAND_WM.with(|w| w.set(self.0));
    }
}

/// Письмо коробки отличается от письма содержащего блока хоста —
/// ортогональный поток (css-writing-modes-4 §7.3): shrink-to-fit и место по
/// чужой оси каркас пробы не считает.
pub(crate) fn band_orthogonal(c: &Computed) -> bool {
    // Письмо наследуется: незаданное у коробки — письмо содержащего блока,
    // ортогональна только коробка, ЗАДАВШАЯ другое.
    let wm = BAND_WM.with(std::cell::Cell::get);
    c.vertical.is_some_and(|v| v != (wm != 0))
        || (c.vertical == Some(true) && c.vertical_rl.is_some_and(|r| r != (wm == 1)))
}

/// Есть ли в поддереве непустой текст.
pub(crate) fn subtree_has_text(e: &Element) -> bool {
    e.children.iter().any(|n| match n {
        Node::Text(t) => !t.trim().is_empty(),
        Node::Element(c) => subtree_has_text(c),
    })
}

/// Блок обычного потока для измеряемого хоста (шаг F4): блочного уровня,
/// в потоке, своего контекста не заводит, поля разрешимы.
pub(crate) fn band_flow_block(c: &Element, em: f32) -> bool {
    block_level_in_flow(c)
        && !own_context(c)
        && !replaced_tag(c)
        && !c.children.iter().any(has_ruby)
        // Заголовок таблицы вне таблицы — анонимная таблица (§17.2.1), то
        // есть коробка, флоаты не перекрывающая (`clear-applies-to-015`).
        && c.style.is_caption != Some(true)
        && c.tag != "caption"
        && flow_interior_plain(c)
        && (c.style.clear.is_none() || band_clear_supported(c))
        && matches!(
            c.style.display,
            None | Some(Display::Block) | Some(Display::ListItem)
        )
        && band_margins(&c.style, em).is_some()
}

/// Внутри блока потока нет ни флоатов, ни блоков своего контекста — на
/// любой глубине обычного потока. Блок хоста раскладывается своим корнем, и
/// полосы доходят только до ЕГО строк (`flow_shapes`); вложенный флоат или
/// коробка своего контекста внешних флоатов не увидели бы вовсе — им нужен
/// один `FloatBands` на весь БФК (шаг F7: `floats-rule7-outside-left-001`,
/// `floats-wrap-bfc-with-margin-008`, `second-float-inside-empty-cleared-block`).
pub(crate) fn flow_interior_plain(c: &Element) -> bool {
    c.children.iter().all(|n| match n {
        Node::Text(_) => true,
        Node::Element(k) => {
            if k.style.float.is_some_and(|f| f != 0) {
                return false;
            }
            // `<br style="clear">` под срезом конца строки по НЕ-текстовому
            // краю (`text-box-trim: trim-end` + `text-box-edge: … alphabetic`):
            // css-inline-3 §text-box-trim — срез не трогает clearance, конец
            // блока = max(срезанная строка, низ флоатов). Блок потока хоста
            // срезает строку, а clearance разрыва не видит — такой блок
            // уходит на прежний путь (`text-box-trim-float-clear-br-003`).
            // Срез по краю `text` (у Ahem он нулевой) хост держит верно:
            // гейт на любой `clear` отправлял и его на прежний путь, и
            // `text-box-trim-float-clear-br-001` терял 0.00 → 16.21.
            if k.style.clear.is_some()
                && c.style.text_box_trim_end
                && c.style.text_box_under != crate::computed::TextEdge::Text
            {
                return false;
            }
            if block_level_in_flow(k) {
                // Вложенный `clear` тоже упирается во ВНЕШНИЕ флоаты
                // (`clear-applies-to-009`: `<div><span display:block;
                // clear:both>`).
                !own_context(k)
                    && k.tag != "table"
                    && k.style.clear.is_none()
                    && flow_interior_plain(k)
            } else {
                // Строчный: атом в строке (инлайн-блок) режется вырезом как
                // целое; флоат внутри строчного всплывает к блоку (§10.1).
                inline_level(k) || flow_interior_plain(k)
            }
        }
    })
}

/// Хвост измеряемого хоста с шагом F4: куски `band_piece_m`, блоки обычного
/// потока и строчные прогоны. Подряд идущее строчное содержимое (текст,
/// строчные элементы, атомы) собирается в АНОНИМНЫЙ блок (CSS 2.1 §9.2.1.1)
/// — у него своя строка и свои вырезы. Внепоточный сосед хост отменяет
/// (как у `band_piece`).
pub(crate) fn band_flow_rest(rest: Vec<Node>, em: f32) -> Option<Vec<Node>> {
    band_flow_rest_lift(rest, em, None)
}

/// Абсолютная коробка с заданными вставками по обеим осям: её место от
/// статической позиции не зависит (CSS 2.1 §10.3.7/§10.6.4 — `auto` нет ни
/// у `left`/`right`, ни у `top`/`bottom` разом), и её можно вынести из
/// хоста в поток содержащего блока, ничего не сдвинув.
pub(crate) fn abs_pinned(c: &Computed) -> bool {
    let set = |l: Option<Len>| !matches!(l, None | Some(Len::Auto));
    matches!(
        c.position,
        Some(crate::computed::Position::Absolute) | Some(crate::computed::Position::Fixed)
    ) && (set(c.inset.top) || set(c.inset.bottom))
        && (set(c.inset.left) || set(c.inset.right))
}

/// `band_flow_rest`, где абсолюты с заданными вставками (`abs_pinned`) не
/// отменяют хост, а уходят в `lift` — вызывающий кладёт их за хостом.
/// Свой содержащий блок они находят снаружи хоста: внутри его отдельного
/// дерева абсолют встал бы от держателя (`floats-placement-001`: зелёная
/// заплатка `left: 50px` у `position: relative` контейнера).
pub(crate) fn band_flow_rest_lift(
    rest: Vec<Node>,
    em: f32,
    mut lift: Option<&mut Vec<Node>>,
) -> Option<Vec<Node>> {
    let mut out: Vec<Node> = vec![];
    let mut run: Vec<Node> = vec![];
    // Прогон без текста: сплошь атомы известного размера — строчный поток
    // атомов `FlowRow` с вырезами полос (как у статического хоста); иначе
    // (`<img>` без размеров, пустые строчные) — хост отменяется.
    let mut bad = false;
    // Прогон после `<br>` — продолжение того же абзаца (`cont`): без отступа
    // первой строки (`band_kids`).
    let mut cont = false;
    let mut flush = |run: &mut Vec<Node>, out: &mut Vec<Node>, cont: bool| {
        if run.iter().any(|n| !is_blank(n)) {
            // `<br>` — строка, пусть и пустая: прогон `[<br>]` после разреза
            // по `<br>` — строчный, а не «пустой» (иначе хост отменялся).
            // Строчный элемент с текстом внутри (`<span>Inline box</span>`)
            // — тоже строки: прогон без ГОЛОГО текста отменял хост, и флоат
            // за таким прогоном уходил на следующую строку вместе с блоком
            // (`box-generation-002`: флоат обязан встать на строку прогона
            // слева от неё, §9.5.1 правило 6).
            let text = run.iter().any(|n| match n {
                Node::Text(t) => !t.trim().is_empty(),
                Node::Element(c) => {
                    inline_level(c)
                        && band_piece(n) != Some(BandPiece::Atom)
                        && !replaced_tag(c)
                        && subtree_has_text(c)
                }
            }) || run
                    .iter()
                    .all(|n| is_blank(n) || matches!(n, Node::Element(e) if e.tag == "br"));
            let atoms = !text
                && run
                    .iter()
                    .all(|n| is_blank(n) || band_piece(n) == Some(BandPiece::Atom));
            if !text && !atoms {
                bad = true;
            }
            if atoms && BAND_WM.with(std::cell::Cell::get) != 0 {
                bad = true;
            }
            // Вырезы строк (`lines.rs` `flow_cut`) считают строки равной
            // высоты `line_no × line-height`; руби поднимает строку на
            // аннотацию, и вырез уезжает с неё
            // (`initial-letter-block-position-raise-over-ruby-ref`).
            if run.iter().any(has_ruby) {
                bad = true;
            }
            let mut attrs = vec![("anon".to_string(), "1".to_string())];
            if atoms {
                attrs.push(("atoms".into(), "1".into()));
            }
            if cont {
                attrs.push(("cont".into(), "1".into()));
            }
            out.push(Node::Element(Element {
                list_item: None,
                node_id: 0,
                anim: None,
                tag: "div".into(),
                style: Computed::default(),
                hover: None,
                first_letter: None,
                first_line: None,
                children: std::mem::take(run),
                attrs,
                inline: false,
            }));
        } else {
            run.clear();
        }
    };
    for n in rest {
        match &n {
            Node::Text(_) => run.push(n),
            Node::Element(c) => {
                if out_of_flow(&c.style) {
                    if let Some(l) = lift.as_deref_mut()
                        && abs_pinned(&c.style)
                    {
                        l.push(n);
                        continue;
                    }
                    return None;
                }
                if !block_level_in_flow(c) {
                    // Строчный `<br clear>` — разрыв с очисткой: прогон уходит
                    // под флоаты, чего строки хоста не умеют.
                    if c.style.clear.is_some() {
                        return None;
                    }
                    // Прогон режется по `<br>` верхнего уровня: §9.5 сдвигает
                    // под флоат СТРОКУ, в которую ничего не влезло, а план
                    // умеет сдвигать только прогон целиком. Строка `<br>`
                    // остаётся рядом с флоатом, а слово за ним, не влезшее в
                    // окно, уходит под флоат своим прогоном
                    // (`float-no-content-beside-001-ref`: `<span float>` +
                    // `<br>` + длинное слово — прогон целиком съезжал под
                    // флоат вместе с пустой строкой `<br>`, на строку ниже).
                    let br = c.tag == "br";
                    run.push(n);
                    if br {
                        flush(&mut run, &mut out, cont);
                        cont = true;
                    }
                    continue;
                }
                flush(&mut run, &mut out, cont);
                cont = false;
                if band_piece_m(&n, em).is_some()
                    || band_flow_block(c, em)
                    || band_nest_ok(c, em)
                {
                    out.push(n);
                } else {
                    return None;
                }
            }
        }
    }
    flush(&mut run, &mut out, cont);
    (!bad).then_some(out)
}

/// Сборка измеряемого хоста: каждому ребёнку — построитель, который
/// `band_flow` зовёт на каждую пробу и на `prepaint`.
pub(crate) fn band_flow_host(e: &Element, inherited: &Computed, opts: &RenderOpts) -> AnyElement {
    let count: usize = e.attr("count").and_then(|c| c.parse().ok()).unwrap_or(0);
    let em: f32 = e
        .attr("em")
        .and_then(|c| c.parse().ok())
        .unwrap_or(opts.base_size());
    let cbh: Option<f32> = e.attr("cbh").and_then(|c| c.parse().ok());
    let cbw: Option<f32> = e.attr("cbw").and_then(|c| c.parse().ok());
    let mut inherited = inherited.clone();
    if let Some(h) = cbh {
        inherited.height = Some(Len::Px(h));
    }
    if let Some(w) = cbw {
        inherited.width = Some(Len::Px(w));
    }
    let inherited = &inherited;
    let kids = band_kids(
        &e.children, count, inherited, opts, em, e.attr("adjoining-start") == Some("1"),
    );
    let flow = crate::band_flow::BandFlow::new(kids, e.attr("inflow-height") != Some("1"));
    if inherited.vertical == Some(true) {
        flow.vertical(inherited.vertical_rl == Some(true))
            .into_any_element()
    } else {
        flow.into_any_element()
    }
}

/// Дети одного содержащего блока измеряемого хоста. `count` первых
/// элементов — флоаты пробега; дальше флоатом считается всякий элемент с
/// `float` (дети `Kind::Nest`, шаг F7).
pub(crate) fn band_kids(
    nodes: &[Node],
    count: usize,
    inherited: &Computed,
    opts: &RenderOpts,
    em: f32,
    start_open: bool,
) -> Vec<crate::band_flow::Kid> {
    use crate::band_flow::{Kid, Kind, Nest};
    let depth = defer_depth();
    let mut kids: Vec<Kid> = vec![];
    // Письмо содержащего блока: план хоста — в логических осях, поля
    // переводятся в (block-start, inline-end, block-end, inline-start)
    // по таблице css-writing-modes-4 :1877-1888 (`vertical-rl`: block-start
    // — право, inline-start — верх; `vertical-lr`: block-start — лево).
    let vert = (inherited.vertical == Some(true)).then_some(inherited.vertical_rl == Some(true));
    // Был ли уже ребёнок потока со строками (не флоат и не распорка).
    let mut seen_inflow = false;
    // Строчное содержимое перед флоатами пробега (`lead-probe`,
    // `band_host_m`): щуп ширины, сам он не рисуется — его узлы идут в
    // начале первого прогона.
    // При `white-space: nowrap` мягких разрывов нет, и строка флоата — весь
    // первый прогон: щуп — он целиком, флоат влезает рядом, только если
    // влезает вся строка, иначе уходит под неё (Blink откладывает флоат до
    // возможности разрыва; `float-nowrap-8` против эталона
    // `float-nowrap-1`: флоат после всей строки).
    let has_lead = nodes
        .iter()
        .any(|n| matches!(n, Node::Element(p) if p.attr("lead-probe") == Some("1")));
    let nowrap = inherited.nowrap == Some(true);
    let mk_build = |p: &Element| -> crate::band_flow::Build {
        let p = p.clone();
        let inherited = inherited.clone();
        let opts = opts.clone();
        std::rc::Rc::new(move |_cb: f32, _avail: f32, _shapes, _h: Option<f32>| {
            let _depth = DepthScope::enter(depth);
            element(&p, &inherited, &opts)
        })
    };
    // Первый прогон — щуп для всех флоатов при `nowrap`.
    let nowrap_probe: Option<crate::band_flow::Build> = (nowrap && has_lead)
        .then(|| {
            nodes.iter().find_map(|n| match n {
                Node::Element(p)
                    if p.attr("anon") == Some("1") && p.attr("lead-probe").is_none() =>
                {
                    Some(mk_build(p))
                }
                _ => None,
            })
        })
        .flatten();
    // Щупы по номерам флоатов (`lead-for`/`lead-base` = «a-b»).
    let probes_of = |key: &str| -> Vec<(usize, usize, crate::band_flow::Build)> {
        nodes
            .iter()
            .filter_map(|n| match n {
                Node::Element(p) if p.attr("lead-probe") == Some("1") => {
                    let (a, b) = p.attr(key)?.split_once('-')?;
                    Some((a.parse().ok()?, b.parse().ok()?, mk_build(p)))
                }
                _ => None,
            })
            .collect()
    };
    let lead_probes = probes_of("lead-for");
    let base_probes = probes_of("lead-base");
    let base_for = |idx: usize| -> Option<crate::band_flow::Build> {
        base_probes
            .iter()
            .find(|(a, b, _)| (*a..*b).contains(&idx))
            .map(|(_, _, b)| b.clone())
    };
    let lead_for = |idx: usize| -> Option<crate::band_flow::Build> {
        if let Some(b) = nowrap_probe.as_ref() {
            return Some(b.clone());
        }
        lead_probes
            .iter()
            .find(|(a, b, _)| (*a..*b).contains(&idx))
            .map(|(_, _, b)| b.clone())
    };
    for (idx, n) in nodes.iter().enumerate() {
        let Node::Element(c) = n else {
            continue;
        };
        if c.attr("lead-probe") == Some("1") {
            continue;
        }
        let Some(margin) = band_margins(&c.style, em) else {
            continue;
        };
        let [t, r, b, l] = margin;
        let margin = match vert {
            None => margin,
            Some(true) => [r, b, l, t],
            Some(false) => [l, b, r, t],
        };
        // Распорка держит физическую высоту — в вертикальном письме это не
        // блочный размер.
        if vert.is_some() && band_piece_m(n, em) == Some(false) {
            continue;
        }
        let float = idx < count || c.style.float.is_some_and(|f| f != 0);
        // `::first-line` содержащего блока — первой строке его потока
        // (CSS 2.1 §5.12.1); флоаты строк не образуют. Анонимный прогон
        // своего `first_line` не несёт (`element` берёт псевдоэлементы только
        // у самого узла), и первая строка хоста теряла свой кегль
        // (`below-float3`: `::first-line { font-size: 50px }` у `x` под
        // флоатом). Первый прогон получает слой хоста.
        let first_line = !float
            && !seen_inflow
            && c.attr("anon") == Some("1")
            && c.attr("cont").is_none();
        if !float && band_piece_m(n, em) != Some(false) {
            seen_inflow = true;
        }
        let first_layer: Option<Computed> = if first_line {
            inherited.first_line.as_deref().cloned()
        } else {
            None
        };
        let mut nest: Option<Nest> = None;
        let kind = if float {
            Kind::Float {
                side: c.style.float.unwrap_or(-1),
                clear: c.style.clear,
                // Строчный размер `auto` — shrink-to-fit; в вертикальном
                // письме строчный размер — высота.
                shrink: if vert.is_some() {
                    matches!(c.style.height, None | Some(Len::Auto))
                } else {
                    matches!(c.style.width, None | Some(Len::Auto))
                },
                letter: c.attr("initial-letter") == Some("1"),
            }
        } else {
            match band_piece_m(n, em) {
                Some(true) => Kind::Piece {
                    rtl: inherited.rtl == Some(true),
                    table: c.tag == "table" || matches!(c.style.display, Some(Display::Table)),
                },
                Some(false) => Kind::Strut(px_margin_box(&c.style).map_or(0.0, |(_, h)| h)),
                None if c.attr("anon") == Some("1") || band_flow_block(c, em) => Kind::Flow,
                None => match band_nest(c, inherited, opts, em) {
                    Some(nn) => {
                        nest = Some(nn);
                        Kind::Nest
                    }
                    None => continue,
                },
            }
        };
        // Голова анонимного прогона — его первое слово (до первой мягкой
        // возможности разрыва): по ней план решает, влезает ли ПЕРВАЯ строка
        // в окно рядом с флоатами (§9.5). min-content всего прогона — самое
        // длинное слово где-то дальше — опускал прогон под флоат, хотя
        // первые строки рядом помещались (`shape-image-012-ref`: флоат 100px
        // в 200px, строки `XXXXX` по 100px рядом, а `XXXXXXXXXX` — ниже).
        // При `nowrap` и значимых пробелах слово не граница строки — голову
        // не заводим, мерится весь прогон.
        let head: Option<crate::band_flow::Build> = if c.attr("anon") == Some("1")
            && inherited.nowrap != Some(true)
            && inherited.keep_spaces != Some(true)
        {
            match c.children.iter().find(|n| !is_blank(n)) {
                Some(Node::Text(t)) => t.split_whitespace().next().map(|word| {
                    let mut hn = c.clone();
                    cont_indent(&mut hn, inherited);
                    if first_layer.is_some() {
                        hn.first_line = first_layer.clone();
                    }
                    hn.children = vec![Node::Text(word.to_string())];
                    let inherited = inherited.clone();
                    let opts = opts.clone();
                    let b: crate::band_flow::Build =
                        std::rc::Rc::new(move |_cb: f32, _avail: f32, _shapes, _h: Option<f32>| {
                            let _depth = DepthScope::enter(depth);
                            element(&hn, &inherited, &opts)
                        });
                    b
                }),
                _ => None,
            }
        } else {
            None
        };
        let mut node = c.clone();
        if first_layer.is_some() {
            node.first_line = first_layer;
        }
        let inherited = inherited.clone();
        let opts = opts.clone();
        let is_nest = nest.is_some();
        let vertical = vert.is_some();
        let cb_height = inherited.height;
        let cb_block_w = inherited.width;
        let build: crate::band_flow::Build =
            std::rc::Rc::new(move |cb: f32, avail: f32, shapes, height: Option<f32>| {
                let _depth = DepthScope::enter(depth);
                // Ширина содержащего блока — та, что намерил хост: замещаемым
                // без размеров (§10.3.2) и долям внутри (`CB_WIDTH`), блочным
                // детям куска — ширина окна (`AVAIL_W`).
                let cb_prev = CB_WIDTH.get();
                let avail_prev = AVAIL_W.get();
                if cb > 0.0 {
                    CB_WIDTH.set(Some(cb));
                }
                AVAIL_W.set((avail > 0.0).then_some(avail));
                let _cb_guard = scopeguard_cb(cb_prev);
                let _avail_guard = AvailWGuard(avail_prev);
                let mut copy = node.clone();
                cont_indent(&mut copy, &inherited);
                // Сторону, очистку и поля несёт хост (позиция от полос), на
                // самой коробке они сдвинули бы её ещё раз — как у
                // статического хоста.
                copy.style.float = None;
                copy.style.clear = None;
                copy.style.margin = crate::computed::Sides::default();
                // Флоат заводит свой контекст форматирования (§9.4.1), а
                // `float` с копии снят — метка остаётся: по ней дети флоата
                // узнают корень БФК (`parent_bfc` у `wrap_floats` — §10.6.7:
                // высота флоата охватывает флоаты внутри него;
                // `letter-spacing-206`).
                if float {
                    copy.style.flow_root = Some(true);
                    native_vertical::claim_float_inline_size(&mut copy.style, &inherited, &copy.children);
                }
                if is_nest {
                    // Коробка `Kind::Nest` — без детей (их кладут полосы) и
                    // высотой содержимого из плана.
                    copy.children.clear();
                    copy.style.height = height.map(Len::Px);
                    copy.style.border_box = None;
                    return element(&copy, &inherited, &opts);
                }
                // Доля ширины — от СОДЕРЖАЩЕГО БЛОКА (§10.2), а каркас пробы
                // шириной в окно: решаем её здесь.
                // `Pct(1.0)` — это и `100%`, и `stretch` (`value.rs` пишет
                // ключевое слово долей): `stretch` заполняет ОКНО рядом с
                // флоатом (css-sizing-4 §4.1, Blink — доступный размер из
                // возможности, `block_layout_algorithm.cc`
                // `child_available_inline_size`), и его оставляем каркасу
                // (`bfc-next-to-float-1`); `100%` рядом с флоатом не влез бы
                // ни в какое окно, а ниже флоатов окно и есть содержащий блок.
                if let Some(Len::Pct(k)) = copy.style.width
                    && cb > 0.0
                    && (float || k != 1.0)
                    && !vertical
                {
                    copy.style.width = Some(Len::Px(k * cb));
                }
                // Доля ВЫСОТЫ — от высоты содержащего блока, когда она задана
                // точками (§10.5). Ребёнок хоста раскладывается своим корнем
                // с неопределённой высотой, и доля там вырождалась в `auto`:
                // плавающий `height: 100%` в блоке `height: 200px` выходил
                // высотой в свой текст (`flexbox-align-self-horiz-001-ref`).
                if let Some(Len::Pct(k)) = copy.style.height
                    && let Some(Len::Px(h)) = cb_height
                {
                    copy.style.height = Some(Len::Px(k * h));
                }
                // В вертикальном письме блочный размер — физическая ширина:
                // её доля — от ширины содержащего блока (§10.5 по блочной
                // оси). Каркас пробы ширины не задаёт, и `block-size: 100%`
                // у флоата вырождалась в ноль — флоат с детьми пропадал
                // (эталон `css-break/background-image-001`: колонки-флоаты
                // `block-size: 100%` во `flow-root` `vertical-rl`).
                if vertical
                    && let Some(Len::Pct(k)) = copy.style.width
                    && let Some(Len::Px(w)) = cb_block_w
                {
                    copy.style.width = Some(Len::Px(k * w));
                }
                // Вырезы полос — строкам ЭТОЙ коробки, от её верха (шаг F4):
                // `inline::inherit` начинает слитый стиль с собственного, и
                // вырезы доезжают до прямых строк коробки.
                if shapes.is_some() {
                    copy.style.flow_shapes = shapes;
                }
                let table = copy.tag == "table"
                    || matches!(
                        copy.style.display,
                        Some(Display::Table) | Some(Display::InlineTable)
                    );
                if copy.attr("atoms") == Some("1") {
                    // Прогон атомов: `FlowRow` режет строки вырезами полос.
                    let atoms: Vec<crate::flow::FlowChild> = copy
                        .children
                        .iter()
                        .filter_map(|n| match n {
                            Node::Element(a) => band_atom(a, &inherited, &opts),
                            Node::Text(_) => None,
                        })
                        .collect();
                    let shapes = copy
                        .style
                        .flow_shapes
                        .clone()
                        .unwrap_or_else(|| std::sync::Arc::new((Vec::new(), Vec::new())));
                    return crate::flow::FlowRow::new(atoms, shapes, inherited.rtl == Some(true)).into_any_element();
                }
                // Замещаемый флоат, кроме `<img>` (`embed`, `object`,
                // `video`…), — своей веткой `element` ниже: каркас блока со
                // `blocks(детей)` рисовал вместо картинки пустую коробку, и
                // `object-fit-*-00Ne/o/p` (88 пар `css-images`) теряли
                // содержимое.
                let replaced = replaced_tag(&copy) && copy.tag != "img";
                // Вертикальный флоат — общим путём `element`: только там блок
                // вертикального письма раскладывает детей рядом по
                // горизонтальной оси блочного потока. Каркас `styled_div_with`
                // + `blocks` клал их горизонтальным блоком, строчный размер
                // пустого ребёнка выходил нулём, и флоат с детьми не
                // рисовался вовсе (эталон `css-break/background-image-001`:
                // колонка-флоат с `<div style="block-size:100%; background">`).
                // CSS Lists 3 §2: a floated list item (a `::before`/`::after`
                // with `display: list-item` too) keeps its marker, which only
                // `element`'s list-item painter draws.
                let list_item = copy.style.display == Some(Display::ListItem);
                if float && !table && !replaced && !vertical && !list_item {
                    // Флоат — блочная коробка (§9.7) каким бы ни был тег: тем
                    // же путём, что у статического хоста (`shape_flow`).
                    // Таблица — своей веткой `element` ниже: каркас блока её
                    // не соберёт.
                    let mut merged = inline::inherit(&inherited, &copy.style);
                    merged.margin = crate::computed::Sides::default();
                    if copy.tag == "img" {
                        grouped(image(&copy), &copy.style)
                    } else {
                        grouped(
                            styled_div_with(&copy, &merged)
                                .children(blocks(&copy.children, &merged, &opts))
                                .into_any_element(),
                            &copy.style,
                        )
                    }
                } else {
                    // Общий путь отрисовки узла — тот же, что в потоке:
                    // таблица, замещаемый, список строятся своими ветками
                    // `element`.
                    let el = element(&copy, &inherited, &opts);
                    // Эффекты группы (`clip-path`, маска, фильтр, смешивание)
                    // `element` не накладывает — их кладёт поток (`blocks`:
                    // `grouped(transformed(animated(..)))`). Вертикальный флоат
                    // идёт сюда мимо потока, и `clip-path` у него не резал
                    // ничего (`shape-outside-circle-048-ref`: флоат целым
                    // прямоугольником при абсолютах с вставками по обеим осям).
                    if float {
                        grouped(el, &copy.style)
                    } else {
                        content_wrapper::for_element(el, &copy, &inherited, (None, None))
                    }
                }
            });
        let clear = if float { None } else { c.style.clear };
        kids.push(Kid {
            kind,
            clear,
            margin,
            build,
            nest,
            anon: c.attr("anon") == Some("1"),
            head,
            lead: if float { lead_for(idx) } else { None },
            lead_base: if float { base_for(idx) } else { None },
            margin_offset: c.style.float_margin_offset.unwrap_or(0.0),
            start_open,
        });
    }
    kids
}
