//! Обрезка строк `line-clamp`.
// owner: A

use crate::interact::*;

/// Бюджет строк обрезки (`line-clamp`, css-overflow-3/4): точка среза —
/// низ N-й СЧИТАЕМОЙ строки. Строки потомков в собственном контексте
/// форматирования (BFC: overflow, флоат, корень потока) видимы, но НЕ
/// считаются; блок, пересекающий точку среза, прячется целиком — срез
/// поднимается к его верху. Точка меряется пробами построенного кадра и
/// применяется потолком высоты на СЛЕДУЮЩЕМ (перестройка каждый кадр).
#[derive(Clone)]
pub struct ClampEntry {
    pub bounds: Bounds<Pixels>,
    /// Высота строки в точках; 0 — блок без собственного текста.
    pub line: f32,
    /// Строки не считаются (элемент внутри вложенного BFC).
    pub skip_count: bool,
    /// Коробка с ЗАДАННОЙ высотой: фрагментировать нечего, пересечённая
    /// точкой среза она прячется целиком.
    pub fixed_height: bool,
    /// Нижние рамка и паддинг коробки в точках. Проба меряет ПАДДИНГ-БОКС,
    /// а фрагментированная коробка своих нижних рамки и паддинга не теряет
    /// (css-overflow-4 §5.3): на них укорачивается бюджет строк и на них же
    /// удлиняется итоговый срез. Для строчных проб — ноль.
    pub bp_after: f32,
    /// Порядковый номер абзаца ВНУТРИ клэмп-контейнера, выданный при
    /// построении. `None` — проба коробки, а не абзаца: знак обрыва на
    /// неё не садится. Номер, а не совпадение по геометрии, потому что
    /// сопоставлять надо КАДРЫ: срез считается по прошлому кадру, а
    /// применяется на следующем.
    pub seq: Option<u32>,
    /// Сколько строк этому абзацу оставлено УЖЕ на этом кадре. Признак
    /// того, что абзац укорочен нами: его собственный низ за срез больше
    /// не выходит, и без этой защёлки «что-то срезано» пропало бы, а
    /// кадр запросил бы себя заново (css-overflow-4 §5.3: вставка знака
    /// «must not cause a reevaluation of the effects of `continue`»).
    pub clamped: Option<usize>,
    /// Пустая поточная блочная коробка (без содержимого): сама по себе —
    /// возможная точка среза МЕЖДУ блоками (css-overflow-4 §5.3).
    pub empty: bool,
}

pub type ClampLines = std::rc::Rc<std::cell::RefCell<Vec<ClampEntry>>>;

thread_local! {
    pub(crate) static CLAMP_LINES: std::cell::RefCell<std::collections::HashMap<u64, ClampLines>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Вычисленные точки среза (высота от верха контейнера) прошлого кадра.
    pub(crate) static CLAMP_CUTS: std::cell::RefCell<std::collections::HashMap<u64, f32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Стек активных clamp-контейнеров при ПОСТРОЕНИИ дерева:
    /// (ключ, граница BFC уже пройдена).
    pub(crate) static CLAMP_STACK: std::cell::RefCell<Vec<(u64, bool)>> =
        std::cell::RefCell::new(Vec::new());
    /// Знак обрыва АВТО-режима, посчитанный на прошлом кадре:
    /// ключ контейнера → (номер абзаца, сколько его строк остаётся).
    /// Абзац в контейнере ровно один — тот, на чьей последней строке
    /// перед точкой среза стоит знак (css-overflow-4 §5.3).
    pub(crate) static CLAMP_PARA: std::cell::RefCell<std::collections::HashMap<u64, (u32, usize)>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Счётчик абзацев контейнера при ПОСТРОЕНИИ поддерева.
    pub(crate) static CLAMP_SEQ: std::cell::RefCell<std::collections::HashMap<u64, u32>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
}

pub fn clamp_lines_for(key: u64) -> ClampLines {
    CLAMP_LINES.with(|m| m.borrow_mut().entry(key).or_default().clone())
}

/// Выдать следующему абзацу клэмп-контейнера его номер. Счётчик сбрасывает
/// вход в сам контейнер (`ClampGuard::enter`), поэтому нумерация одна и та
/// же на каждом кадре, пока не меняется дерево.
pub fn clamp_next_seq(key: u64) -> u32 {
    CLAMP_SEQ.with(|m| {
        let mut m = m.borrow_mut();
        let n = m.entry(key).or_insert(0);
        let v = *n;
        *n += 1;
        v
    })
}

pub fn clamp_cut(key: u64) -> Option<f32> {
    CLAMP_CUTS.with(|m| m.borrow().get(&key).copied())
}

/// Бюджет строк абзаца со знаком обрыва: (номер абзаца, сколько строк
/// оставить). Считает `ClampCut::paint` прошлого кадра.
pub fn clamp_para(key: u64) -> Option<(u32, usize)> {
    CLAMP_PARA.with(|m| m.borrow().get(&key).copied())
}

pub fn forget_clamp_buffers() {
    CLAMP_LINES.with(|m| m.borrow_mut().clear());
    CLAMP_CUTS.with(|m| m.borrow_mut().clear());
    CLAMP_STACK.with(|st| st.borrow_mut().clear());
    CLAMP_PARA.with(|m| m.borrow_mut().clear());
    CLAMP_SEQ.with(|m| m.borrow_mut().clear());
    PARA_BUDGET.with(|c| c.set(None));
    PARA_ROWS.with(|m| m.borrow_mut().clear());
    PARA_TAG.with(|c| c.set(None));
}

/// Сторож стека clamp-контекста на время построения поддерева.
pub struct ClampGuard(pub(crate) bool);

impl ClampGuard {
    /// Вход в сам clamp-контейнер.
    pub fn enter(key: u64) -> Self {
        CLAMP_STACK.with(|st| st.borrow_mut().push((key, false)));
        // Нумерация абзацев контейнера начинается заново на каждом кадре:
        // иначе номер рос бы от кадра к кадру и бюджет прошлого кадра
        // никогда не находил бы своего абзаца.
        CLAMP_SEQ.with(|m| {
            m.borrow_mut().insert(key, 0);
        });
        ClampGuard(true)
    }

    /// Вход в элемент с собственным контекстом форматирования: строки
    /// глубже не считаются.
    pub fn enter_bfc() -> Self {
        let pushed = CLAMP_STACK.with(|st| {
            let mut st = st.borrow_mut();
            match st.last().copied() {
                Some((key, false)) => {
                    st.push((key, true));
                    true
                }
                _ => false,
            }
        });
        ClampGuard(pushed)
    }
}

impl Drop for ClampGuard {
    fn drop(&mut self) {
        if self.0 {
            CLAMP_STACK.with(|st| {
                st.borrow_mut().pop();
            });
        }
    }
}

/// Текущий clamp-контекст построения: (ключ, внутри вложенного BFC).
pub fn clamp_context() -> Option<(u64, bool)> {
    CLAMP_STACK.with(|st| st.borrow().last().copied())
}

thread_local! {
    /// Настоящие строки абзацев клэмп-контейнера на этом кадре: (ключ,
    /// номер абзаца) → (верх, низ) строк в координатах окна. Пишет их
    /// отрисовка абзаца (`lines::Paragraph::paint`), забирает `ClampCut`
    /// (он рисуется последним ребёнком контейнера). Без них строки
    /// абзаца делились бы поровну, а строка с крупным кеглем или руби
    /// выше прочих (css-overflow-4 §5.3: точка среза — между строчными
    /// коробками, их высоты свои).
    pub(crate) static PARA_ROWS: std::cell::RefCell<std::collections::HashMap<(u64, u32), Vec<(f32, f32)>>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// (ключ, номер) абзаца, который сейчас будет собран.
    pub(crate) static PARA_TAG: std::cell::Cell<Option<(u64, u32)>> = const { std::cell::Cell::new(None) };
}

pub fn set_para_tag(v: Option<(u64, u32)>) {
    PARA_TAG.with(|c| c.set(v));
}

pub fn take_para_tag() -> Option<(u64, u32)> {
    PARA_TAG.with(|c| c.take())
}

/// Отрисовка абзаца сообщает его строки (см. `PARA_ROWS`).
pub fn publish_para_rows(tag: (u64, u32), rows: Vec<(f32, f32)>) {
    PARA_ROWS.with(|m| {
        m.borrow_mut().insert(tag, rows);
    });
}

pub(crate) fn take_para_rows(key: u64) -> std::collections::HashMap<u32, Vec<(f32, f32)>> {
    PARA_ROWS.with(|m| {
        let mut m = m.borrow_mut();
        let tags: Vec<(u64, u32)> = m.keys().filter(|k| k.0 == key).copied().collect();
        tags.into_iter()
            .filter_map(|t| m.remove(&t).map(|mut v| {
                v.sort_by(|a, b| a.0.total_cmp(&b.0));
                (t.1, v)
            }))
            .collect()
    })
}

thread_local! {
    /// Бюджет строк ТЕКУЩЕГО собираемого абзаца (только авто-режим).
    pub(crate) static PARA_BUDGET: std::cell::Cell<Option<usize>> = const { std::cell::Cell::new(None) };
}

/// Положить бюджет строк для абзаца, который сейчас будет собран.
pub fn set_para_budget(v: Option<usize>) {
    PARA_BUDGET.with(|c| c.set(v));
}

/// Забрать бюджет (и опустошить ячейку). Опустошение обязательно: куски
/// абзаца строят вложенные абзацы (`inline-block`), и им чужой бюджет
/// доставаться не должен.
pub fn take_para_budget() -> Option<usize> {
    PARA_BUDGET.with(|c| c.take())
}

/// Проба строк: канвас в элементе с текстом (или блоке), пишет границы и
/// высоту строки в prepaint своего кадра.
pub fn clamp_probe(
    lines: ClampLines,
    line: f32,
    skip_count: bool,
    fixed_height: bool,
    bp_after: f32,
    seq: Option<u32>,
    clamped: Option<usize>,
) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line,
                skip_count,
                fixed_height,
                bp_after,
                seq,
                clamped,
                empty: false,
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

/// Проба пустой поточной блочной коробки клэмп-контейнера: только её
/// положение. Точка среза после неё (§5.3 «a point between two in-flow
/// block-level sibling boxes») отделяет предыдущую строку от точки, и знака
/// обрыва на той строке нет (`line-clamp-auto-039/032`).
pub fn clamp_empty_probe(lines: ClampLines) -> AnyElement {
    gpui::canvas(
        move |bounds: Bounds<Pixels>, _, _| {
            lines.borrow_mut().push(ClampEntry {
                bounds,
                line: 0.0,
                skip_count: false,
                fixed_height: false,
                bp_after: 0.0,
                seq: None,
                clamped: None,
                empty: true,
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

/// Вычислитель точки среза: абсолютный элемент В КОНЦЕ clamp-контейнера,
/// его границы — весь контейнер. Считает низ N-й считаемой строки,
/// поднимает срез к верху пересечённого блока и просит новый кадр, когда
/// точка изменилась.
pub struct ClampCut {
    pub(crate) key: u64,
    pub(crate) lines: ClampLines,
    /// Число считаемых строк; None — `line-clamp: auto` (срез только по
    /// потолку высоты, но пересечённый блок всё равно прячется целиком).
    pub(crate) limit: Option<u32>,
    /// Потолок высоты контейнера в точках (max-height), если задан.
    pub(crate) max_h: Option<f32>,
    /// `text-box-trim: trim-end` контейнера в точках: последняя строка
    /// ПЕРЕД точкой обрыва — последняя отформатированная, и её конец
    /// срезается (`text-box-trim-line-clamp-*`). Ноль — среза нет.
    pub(crate) trim_end: f32,
}

impl ClampCut {
    pub fn new(key: u64, lines: ClampLines, limit: Option<u32>, max_h: Option<f32>) -> Self {
        ClampCut {
            key,
            lines,
            limit,
            max_h,
            trim_end: 0.0,
        }
    }

    /// Срез конца последней видимой строки (`text-box-trim`).
    pub fn trim_end(mut self, v: f32) -> Self {
        self.trim_end = v.max(0.0);
        self
    }
}

impl Element for ClampCut {
    type RequestLayoutState = ();
    type PrepaintState = ();

    fn id(&self) -> Option<ElementId> {
        None
    }

    fn source_location(&self) -> Option<&'static core::panic::Location<'static>> {
        None
    }

    fn request_layout(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        window: &mut Window,
        cx: &mut App,
    ) -> (LayoutId, ()) {
        let mut style = gpui::Style::default();
        style.position = gpui::Position::Absolute;
        style.size.width = gpui::relative(1.0).into();
        style.size.height = gpui::relative(1.0).into();
        (window.request_layout(style, [], cx), ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        _bounds: Bounds<Pixels>,
        _state: &mut (),
        _window: &mut Window,
        _cx: &mut App,
    ) {
    }

    fn paint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        _prepaint: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) {
        let all_entries = std::mem::take(&mut *self.lines.borrow_mut());
        // Пустые коробки — только для выбора знака обрыва (ниже); в строки,
        // блоки и признак «за точкой есть содержимое» они не входят.
        let empties: Vec<ClampEntry> = all_entries.iter().filter(|e| e.empty).cloned().collect();
        let entries: Vec<ClampEntry> = all_entries.into_iter().filter(|e| !e.empty).collect();
        let para_rows = take_para_rows(self.key);
        // Строки текстового вклада: настоящие, если абзац их сообщил и они
        // лежат в его коробке; иначе — высота пробы поровну на строки.
        let split = |e: &ClampEntry| -> Vec<(f32, f32)> {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if let Some(r) = e.seq.and_then(|s| para_rows.get(&s))
                && !r.is_empty()
                && r.iter().all(|(a, b)| *a >= y0 - 0.5 && *b <= y0 + h + 0.5)
            {
                return r.clone();
            }
            let n = (h / e.line).round().max(1.0) as usize;
            let step = h / n as f32;
            (0..n)
                .map(|i| (y0 + i as f32 * step, y0 + (i + 1) as f32 * step))
                .collect()
        };
        let top = f32::from(bounds.origin.y);
        // Строки: у текстового вклада их bounds.height / line штук.
        let mut rows: Vec<(f32, f32, bool)> = vec![]; // (верх, низ, считается)
        // (верх, низ, заданная высота, нижние рамка+паддинг)
        let mut blocks: Vec<(f32, f32, bool, f32)> = vec![];
        for e in &entries {
            let y0 = f32::from(e.bounds.origin.y);
            let h = f32::from(e.bounds.size.height);
            if e.line > 0.0 && h > 0.0 {
                for (a, b) in split(e) {
                    rows.push((a, b, !e.skip_count));
                }
            } else if h > 0.0 {
                blocks.push((y0, y0 + h, e.fixed_height, e.bp_after));
            }
        }
        rows.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        let mut cut: Option<f32> = self.max_h.map(|m| top + m);
        let trim_end = self.trim_end;
        // Счётный режим (css-overflow-4 §5.3): точка среза — СРАЗУ после N-й
        // считаемой строки. Это готовая строка, а не бюджет высоты, поэтому
        // правила авто-режима ниже (вычет нижних рамки и паддинга, посадка на
        // верх пересечённой строки) к ней не применяются: после `c2 -= bp`
        // точка уходила внутрь N-й строки, и строка пропадала
        // (`line-clamp-012/022`, `webkit-line-clamp-050`). Абзацу N-й строки
        // отдаётся бюджет строк — тот же `CLAMP_PARA`, что у авто-режима, и
        // «…» ставит строчный слой. Высоту даёт УКОРОЧЕННОЕ содержимое — с
        // полями, схлопыванием и заданной высотой предков (Blink: всё за
        // точкой `is_hidden_for_paint`, размер — по видимому); потолок нужен,
        // только когда за точкой есть другое содержимое.
        let mut num_para: Option<(u32, usize)> = None;
        let mut by_count = false;
        if let Some(limit) = self.limit.filter(|n| *n > 0) {
            // (верх строки, низ строки, номер абзаца, номер строки в абзаце)
            let mut marks: Vec<(f32, f32, u32, usize)> = vec![];
            for e in entries.iter().filter(|e| e.line > 0.0 && !e.skip_count) {
                let Some(seq) = e.seq else { continue };
                let h = f32::from(e.bounds.size.height);
                if h <= 0.0 {
                    continue;
                }
                for (i, (a, b)) in split(e).into_iter().enumerate() {
                    marks.push((a, b, seq, i + 1));
                }
            }
            marks.sort_by(|a, b| a.0.total_cmp(&b.0));
            match marks.get(limit as usize - 1).copied() {
                Some((_, bottom, seq, k)) if cut.is_none_or(|c| bottom <= c + 0.5) => {
                    // Остаток СВОЕГО абзаца — знак нужен, потолок нет: абзац
                    // укоротит бюджет.
                    let own_rest = entries.iter().any(|e| {
                        e.line > 0.0
                            && e.seq == Some(seq)
                            && f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height)
                                > bottom + 0.5
                    });
                    // Другое содержимое за точкой: чужой абзац, выходящий за
                    // неё (в том числе несчитаемый), или коробка, начатая после.
                    let follows = entries.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        let h = f32::from(e.bounds.size.height);
                        h > 0.0
                            && if e.line > 0.0 {
                                e.seq != Some(seq) && y0 + h > bottom + 0.5
                            } else {
                                y0 >= bottom - 0.5
                            }
                    });
                    if own_rest || follows || entries.iter().any(|e| e.clamped.is_some()) {
                        num_para = Some((seq, k));
                    }
                    by_count = true;
                    if follows {
                        // Коробки, содержащие точку, фрагментированы в ней и
                        // уносят свои нижние рамку и паддинг (§5.3); коробка с
                        // заданной высотой не фрагментируется — видна целиком.
                        let mut add = 0.0f32;
                        let mut floor = bottom;
                        for (y0, y1, fixed, bp) in &blocks {
                            if *y0 < bottom && bottom < *y1 {
                                if *fixed {
                                    floor = floor.max(*y1);
                                } else {
                                    add += *bp;
                                }
                            }
                        }
                        cut = Some((bottom + add).max(floor));
                    }
                }
                // `max-height` теснее N строк — дальше как в авто-режиме.
                Some(_) => {}
                None => {
                    if self.max_h.is_none() {
                        // Строк меньше предела — среза нет.
                        cut = None;
                    }
                }
            }
        }
        // Блок, СОДЕРЖАЩИЙ точку среза, фрагментируется по последней
        // влезающей строке — ПРЯЧЕТСЯ целиком только коробка с заданной
        // высотой: её не фрагментировать (css-overflow-4 §line-clamp).
        // Строка, пересечённая точкой, не показывается половинкой:
        // срез поднимается к её верху.
        if let Some(c) = cut.filter(|_| !by_count) {
            // Коробка с ЗАДАННОЙ высотой не фрагментируется: пересечённая
            // точкой среза, она прячется целиком (css-overflow-4 §5.3).
            let mut c2 = c;
            for (y0, y1, fixed, _) in &blocks {
                if *fixed && *y0 < c2 && c2 < *y1 {
                    c2 = *y0;
                }
            }
            // Коробка БЕЗ заданной высоты фрагментируется по последней
            // влезающей строке, но нижние рамку и паддинг с собой уносит:
            // бюджет строк на них укорачивается, а итоговый срез — на
            // столько же удлиняется (`line-clamp-auto-019`: 2+14+4×32+14+2
            // = 160 = ровно потолок `max-height: 5lh`).
            let crossed: Vec<(f32, f32)> = blocks
                .iter()
                .filter(|(y0, y1, fixed, _)| !*fixed && *y0 < c2 && c2 < *y1)
                .map(|(y0, _, _, bp)| (*y0, *bp))
                .collect();
            let bp: f32 = crossed.iter().map(|(_, bp)| *bp).sum();
            c2 -= bp;
            // Строка, которая влезает только СРЕЗАННОЙ (`text-box-trim:
            // trim-end` — последняя строка перед обрывом срезается), остаётся:
            // `max-height: 285px` при строке 100 и срезе 25 — три строки
            // (3·100 − 25 = 275), а не две (`line-clamp-auto-001/002`).
            for (y0, y1, _) in &rows {
                if *y0 < c2 && c2 < *y1 - trim_end - 0.5 {
                    c2 = *y0;
                }
            }
            // И точка обрыва садится на срезанный низ последней видимой
            // строки: при числовом пределе — всегда, в авто-режиме — только
            // когда строки идут дальше точки (иначе обрыва нет и срез уже
            // сделал хвост `blocks()`).
            if trim_end > 0.0
                && (self.limit.is_some() || rows.iter().any(|r| r.1 > c2 + 0.5))
                && let Some(b) = rows
                    .iter()
                    .map(|r| r.1)
                    .filter(|y1| *y1 - trim_end <= c2 + 0.5)
                    .max_by(|a, b| a.total_cmp(b))
            {
                c2 = c2.min(b - trim_end);
            }
            // Пересечённая коробка, в которую с её верхними рамкой и паддингом
            // не влезло НИ ОДНОЙ строки, не фрагментируется: точка среза — между
            // ней и предыдущим соседом (§5.3 «between two in-flow block-level
            // sibling boxes»; `line-clamp-auto-024`: 224, а не 240). Её нижние
            // рамка и паддинг не нужны; охватывающие непустые свои уносят.
            // Строка «влезла» — по СРЕЗАННОМУ низу (`trim_end`, как в цикле
            // выше): без поправки последняя видимая строка под `text-box-trim`
            // не считалась бы, и срез уходил бы к верху её коробки.
            let holds = |y0: f32| {
                rows.iter().any(|(r0, r1, countable)| {
                    *countable && *r0 >= y0 - 0.5 && *r1 - trim_end <= c2 + 0.5
                })
            };
            match crossed
                .iter()
                .filter(|(y0, _)| !holds(*y0))
                .map(|(y0, _)| *y0)
                .reduce(f32::min)
            {
                Some(empty_top) => {
                    let keep: f32 = crossed
                        .iter()
                        .filter(|(y0, _)| holds(*y0) && *y0 < empty_top)
                        .map(|(_, bp)| *bp)
                        .sum();
                    cut = Some(empty_top + keep);
                }
                None => cut = Some(c2 + bp),
            }
        }
        // ★ ЗАМЕРЕНО (10.09, `scout-clampmarker-2026-09c.md`, 21 хунк):
        // срез 416 пар (семья `line-clamp` + схлопывание полей) 259 ->
        // 260. Взяты `line-clamp-auto-003` 1.82 -> 0.00 и `-047`
        // 1.62 -> 0.12; четвёрка `-018`…`-021` стояла на 0.39-0.40 и
        // встала РОВНО на 0.00 — знак наконец в конце текста строки.
        // Потеря одна: `-022` 0.39 -> 2.61 — тот же тест, что `-021`,
        // но `max-height: 5.5lh` вместо `5lh` при том же эталоне;
        // полстроки сверх бюджета у нас пускают лишнюю строку. Долг
        // отдельным подкорнем CLAMP-HALF-LINE.
        // Знак обрыва в АВТО-режиме (`limit == None`). Рисует его НЕ этот
        // слой: сюда возвращается только БЮДЖЕТ строк — номер абзаца в
        // контейнере и сколько его строк остаётся выше среза, — а «…»
        // ставит уже проверенный `lines::clamp_lines`/`paint_line`: в
        // конце ТЕКСТА строки, с выключкой и направлением письма
        // (css-overflow-4: знак «is placed at the end of the line box
        // reducing the space available to the other contents of the
        // line», а для bidi — анонимный строчный с уровнем bidi-абзаца).
        // Так же развязан и Blink: блочный слой отдаёт признак
        // `IsAtClampPoint`, а ширину знака получает разрыватель строк
        // (`inline_layout_algorithm.cc:1247` `SetupLineClampEllipsis` →
        // `SetLineClampEllipsisWidth`). Прежний набросок рисовал знак у
        // ПРАВОГО края коробки — мимо конца текста, мимо выключки и
        // мимо rtl.
        // Числовой предел, который `max-height` перехватил раньше N-й строки
        // (`line-clamp: 4 auto` при `max-height: 3lh`), — та же точка
        // обрыва, что в авто-режиме: §5.3 берёт ПЕРВУЮ из двух точек, и знак
        // встаёт на последнюю строку перед ней (`line-clamp-041`).
        let para = cut.filter(|_| !by_count).and_then(|c| {
            // Есть ли что резать. Как только бюджет применён, абзац УЖЕ
            // укорочен и сам за срез не выходит — признак защёлкивается
            // применённым бюджетом, иначе кадры зациклились бы:
            // обрезали → влезло → сняли → снова не влезло.
            let overflow = entries.iter().any(|e| e.clamped.is_some())
                || entries.iter().any(|e| {
                    f32::from(e.bounds.origin.y) + f32::from(e.bounds.size.height) > c + 0.5
                });
            if !overflow {
                return None;
            }
            // Знак садится на ПОСЛЕДНЮЮ строку перед точкой среза — в том
            // числе когда точка стоит МЕЖДУ блоками и сам абзац видим
            // целиком. Абзацы в своём контексте форматирования
            // пропускаются: точкой среза их строки быть не могут.
            // Последняя строка перед точкой — во ВЛОЖЕННОМ контексте
            // форматирования (несчитаемая): знак не ставится вовсе, а не
            // уходит на предыдущий считаемый абзац (css-overflow-4 §5.3:
            // многоточие — на последней строке ПЕРЕД точкой среза в этом
            // BFC; `line-clamp-auto-034/039`: «Line 4» без знака).
            entries
                .iter()
                .filter(|e| e.line > 0.0 && (e.skip_count || e.seq.is_some()))
                .filter_map(|e| {
                    let seq = if e.skip_count { None } else { e.seq };
                    let h = f32::from(e.bounds.size.height);
                    if h <= 0.0 {
                        return None;
                    }
                    let rows = split(e);
                    // Точка `c` уже стоит на СРЕЗАННОМ низу последней
                    // строки — полный низ этой строки ниже на `trim_end`.
                    let k = rows
                        .iter()
                        .filter(|(_, b)| *b <= c + trim_end + 0.5)
                        .count();
                    (k >= 1).then(|| (rows[k - 1].1, seq, k))
                })
                .max_by(|a, b| a.0.total_cmp(&b.0))
                // Пустая блочная коробка между последней строкой и точкой
                // среза: точка — после неё (последняя возможная), и строка
                // точке уже не предшествует — знака нет (§5.3).
                .filter(|(bottom, _, _)| {
                    !empties.iter().any(|e| {
                        let y0 = f32::from(e.bounds.origin.y);
                        e.empty
                            && f32::from(e.bounds.size.height) <= 0.5
                            && y0 >= *bottom - 0.5
                            && y0 <= c + 0.5
                    })
                })
                .and_then(|(_, seq, k)| seq.map(|s| (s, k)))
        });
        // Бюджет одного и того же абзаца только УЖИМАЕТСЯ: рост числа
        // строк на следующем кадре — это отражение нашей же правки, а не
        // новое измерение. Правило конечно (бюджет строго убывает и не
        // меньше единицы), поэтому кадр не может просить себя без конца.
        // Счётный режим несёт свой бюджет (см. выше); авто-режим — свой.
        let para = if by_count { num_para } else { para };
        let prev_para = clamp_para(self.key);
        let para = match (prev_para, para) {
            (Some((ps, pk)), Some((s, k))) if ps == s && k > pk => Some((ps, pk)),
            (_, v) => v,
        };
        if prev_para != para {
            CLAMP_PARA.with(|m| {
                let mut m = m.borrow_mut();
                match para {
                    Some(v) => {
                        m.insert(self.key, v);
                    }
                    None => {
                        m.remove(&self.key);
                    }
                }
            });
            window.request_animation_frame();
        }
        let rel = cut.map(|c| (c - top).max(0.0));
        // Гистерезис: мелкие колебания точки (обрезка двигает схлопнутые
        // поля, точка плывёт на доли строки) не перезаписывают её — иначе
        // пары мигали между прогонами. Крупный сдвиг — честный пересчёт.
        // «Измерено: резать нечего» хранится БЕСКОНЕЧНОСТЬЮ, а не пустотой:
        // пустота значит «ещё не мерили», и `styled_div_with` подставлял бы
        // запасной потолок `N × line-height` навсегда (`webkit-line-clamp-029`:
        // все строки в своём контексте, считать нечего, а коробка резалась на
        // три строки из пяти). Запасной потолок остаётся только первому кадру.
        let v = rel.unwrap_or(f32::INFINITY);
        let prev = clamp_cut(self.key);
        let changed = match prev {
            Some(a) if a.is_finite() && v.is_finite() => (a - v).abs() > 4.0,
            Some(a) => a.is_finite() != v.is_finite(),
            None => true,
        };
        if changed {
            CLAMP_CUTS.with(|m| {
                m.borrow_mut().insert(self.key, v);
            });
            window.request_animation_frame();
        }
    }
}

impl IntoElement for ClampCut {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
