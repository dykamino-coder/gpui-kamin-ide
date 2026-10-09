//! impl Element/IntoElement for Paragraph: раскладка, prepaint, отрисовка.

use crate::text::paragraph::*;
use gpui::{
    App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, InspectorElementId,
    IntoElement, LayoutId, Pixels, Window, point, px, size,
};

impl Element for Paragraph {
    type RequestLayoutState = LayoutId;
    /// Область попадания заводится только у выделяемого абзаца.
    type PrepaintState = Option<Hitbox>;

    fn id(&self) -> Option<ElementId> {
        self.id.clone()
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
    ) -> (LayoutId, LayoutId) {
        // Атомы раскладываются ДО замера: их ширина — продвижение распорки,
        // высота растит строку (см. `lay_atoms`).
        if !self.atoms.is_empty() {
            self.lay_atoms(window, cx);
        }
        // Рост строки под знак акцента меряется подъёмом и спуском его
        // прогона (`line_padding`); без замера при раскладке строка не росла
        // и рост доставался только сдвигу набора на отрисовке.
        if !self.emph_spans.is_empty() && self.run_metrics.len() != self.runs.len() {
            self.run_metrics = self.measure_runs(window);
        }
        if !self.box_spans.is_empty() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            if let Some((font, size)) = self.strut_run.clone() {
                let ts = window.text_system();
                let id = ts.resolve_font(&font);
                self.strut_box = (
                    f32::from(ts.ascent(id, size)),
                    f32::from(ts.descent(id, size)).abs(),
                );
            }
        }
        let atom_boxes = self.atom_boxes.clone();
        let edge_spans = self.edge_spans.clone();
        let ruby_trim = self.ruby_trim;
        let emph_spans = self.emph_spans.clone();
        let box_spans = self.box_spans.clone();
        let strut_box = self.strut_box;
        let run_metrics = self.run_metrics.clone();
        let strut = self.strut;
        // Ширина известна только раскладке, поэтому строки считаются в замере:
        // сколько дали места — столько строк и получилось.
        let text = self.text.clone();
        let runs = self.runs.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let wrap = self.wrap;
        let align = self.align;
        let vertical = self.vertical;
        let lines_reversed = self.lines_reversed;
        let vertical_central_baseline = self.vertical_central_baseline;
        let vertical_ccw = self.vertical_ccw;
        let vertical_inline = self.vertical_inline;
        let ortho_limit = self.ortho_limit;
        // Правила КУСКОВ обязаны доехать и до замера: без них щуп считает
        // абзац по общим правилам и отдаёт другое число строк, чем потом
        // рисуется. Коробка тогда выходит по замеру, а текст по отрисовке —
        // и лишние строки вылезают за рамку (`white-space-pre-031`).
        let spans = self.spans.clone();
        let word_spans = self.word_spans.clone();
        let letter_spans = self.letter_spans.clone();
        let shift_spans = self.shift_spans.clone();
        let lh_spans = self.lh_spans.clone();
        // Обрыв по `line-clamp` обязан доехать и до замера: иначе коробка
        // считается по ПОЛНОМУ числу строк, а рисуются обрезанные, и рамка
        // выходит выше текста (`text-wrap-balance-line-clamp-004`).
        let clamp = self.clamp;
        let clamp_force = self.clamp_force;
        let fit = self.fit;
        let tab_stop = self.tab_stop.clone();
        // Отступ первой строки решает и число строк, и ширину коробки —
        // без него щуп мерил абзац по чужой раскладке.
        let indent = self.indent;
        let hanging = self.hanging;
        let spacers = self.spacers.clone();
        let spacer_edges = self.spacer_edges.clone();
        let box_extents = self.box_extents.clone();
        let flow = self.flow.clone();
        let atom_fit = self.atom_fit.clone();
        let ruby_base_sink = self.ruby_base_sink.clone();
        let id = window.request_measured_layout_with_physical_baselines(
            gpui::Style::default(),
            move |known, available, window, _cx| {
                // Заданная ширина сильнее доступной: раскладка уже решила, в
                // какую коробку абзац ставится, и переносы считаются по ней.
                let mut probe = Paragraph::new(
                    text.clone(),
                    runs.clone(),
                    font_size,
                    line_height,
                    align,
                    wrap,
                );
                probe.vertical = vertical;
                probe.lines_reversed = lines_reversed;
                probe.vertical_central_baseline = vertical_central_baseline;
                probe.vertical_ccw = vertical_ccw;
                probe.vertical_inline = vertical_inline;
                probe.spans = spans.clone();
                probe.word_spans = word_spans.clone();
                probe.letter_spans = letter_spans.clone();
                probe.shift_spans = shift_spans.clone();
                probe.lh_spans = lh_spans.clone();
                probe.clamp = clamp;
                probe.clamp_force = clamp_force;
                probe.fit = fit;
                probe.tab_stop = tab_stop.clone();
                probe.indent = indent;
                probe.hanging = hanging;
                probe.flow = flow.clone();
                probe.atom_boxes = atom_boxes.clone();
                probe.edge_spans = edge_spans.clone();
                probe.ruby_trim = ruby_trim;
                probe.emph_spans = emph_spans.clone();
                probe.box_spans = box_spans.clone();
                probe.strut_box = strut_box;
                probe.run_metrics = run_metrics.clone();
                probe.strut = strut;
                probe.spacers = spacers.clone();
                probe.spacer_edges = spacer_edges.clone();
                probe.box_extents = box_extents.clone();
                // Ширина атома «по содержимому» — от содержащего блока, то
                // есть от ширины самого абзаца (CSS 2.1 §10.3.9), см. `atom_fit`.
                if !vertical && !atom_fit.borrow().is_empty() {
                    let avail = match (known.width, available.width) {
                        (Some(w), _) | (None, gpui::AvailableSpace::Definite(w)) => f32::from(w),
                        (None, gpui::AvailableSpace::MinContent) => 0.0,
                        (None, gpui::AvailableSpace::MaxContent) => f32::INFINITY,
                    };
                    let fitted = atom_fit.borrow_mut().fit(avail, window, _cx);
                    probe.atom_fit = atom_fit.clone();
                    probe.apply_atom_fit(&fitted);
                }
                // Предел переноса берётся ПО ОСИ СТРОКИ: по горизонтали это
                // ширина коробки, по вертикали — её высота. Уже решённая
                // родителем сторона сильнее доступной.
                let (known_along, space_along) = if vertical {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let limit =
                    probe.measured_inline_limit(known_along, space_along, ortho_limit, window);
                // Кегль подбирается ДО замера: коробка считается уже по
                // подобранному, иначе её высота не сойдётся с отрисовкой.
                // Подбирать есть смысл только под ЗАДАННЫЙ размер строки:
                // когда его нет, коробка растёт под текст, и заполнять нечего
                // (иначе кегль улетал в размер окна — `text-fit/writing-mode`).
                if let Some(w) = known_along {
                    probe.apply_fit(w, window);
                }
                if vertical {
                    probe.run_metrics = probe.measure_runs(window);
                }
                let lines = probe.split(limit, window);
                // Ширина — по самой длинной строке. Вся отведённая ширина
                // берётся только под выключку по ширине: там остаток строки
                // раздаётся пробелам, и без полной колонки раздавать нечего.
                // В остальных случаях абзац обтягивает текст, иначе ломается
                // размер по содержимому у родителя.
                // Отступ строки входит в её место в колонке: коробка по
                // содержимому обязана вместить и его. Отрицательный уходит в
                // поле и ширины не требует, поэтому в ноль он и упирается.
                // У абзаца с атомами отрицательный отступ ширину по содержимому
                // УМЕНЬШАЕТ (css-text-3 §7.1: отступ входит в строку; доля при
                // замере — ноль): `text-indent: calc(50% - 3px)` у флоата с
                // атомом 10px даёт 7px (`calc-text-indent-intrinsic-1`). Так
                // мерил и прежний ряд слов; у текстового абзаца — как было.
                let atoms_in = !probe.atom_boxes.is_empty();
                // Under a min-content constraint a NEGATIVE indent of a
                // paragraph with atoms narrows only the first piece; the lines
                // split at the min-content width packed more onto the
                // indented first line (`text-indent-intrinsic-003/004`).
                let neg_indent = atoms_in
                    && !vertical
                    && known_along.is_none()
                    && probe.indent.px < 0.0
                    && !probe.indent.hanging;
                let min_indented = neg_indent
                    .then(|| match space_along {
                        gpui::AvailableSpace::MaxContent => None,
                        _ => Some(probe.min_content_indented(window)),
                    })
                    .flatten();
                let content = lines
                    .iter()
                    .map(|l| {
                        if atoms_in {
                            (l.width + l.indent).max(px(0.))
                        } else {
                            l.width + l.indent.max(px(0.))
                        }
                    })
                    .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
                // A ruby base unit (nowrap) reports its content width for the
                // annotation overhang (`lay_atoms`).
                if let Some(sink) = &ruby_base_sink {
                    sink.set(Some(f32::from(content)));
                }
                // Fit-content is max(min-content, min(max-content, available))
                // (css-sizing-3 §5.1): with a negative indent the max-content
                // line can be NARROWER than the widest piece of a later line,
                // and the min-content then wins (`text-indent-intrinsic-004`).
                let content = match (min_indented, space_along) {
                    (Some(min), gpui::AvailableSpace::MinContent) => min,
                    (Some(min), _) if min > content => min,
                    _ => content,
                };
                // Native vertical lines report their inline extent to the
                // band host's intrinsic probe, like `VerticalText` does: the
                // box itself stretches to the window along the line axis.
                if vertical {
                    crate::text::vertical::VT_INLINE_MAX.with(|c| {
                        if let Some(v) = c.get() {
                            c.set(Some(v.max(f32::from(content))));
                        }
                    });
                }
                let width = known_along.unwrap_or(content);
                // Шире отведённого коробка не бывает: у абзаца блочного уровня
                // ширина ограничена содержащим блоком, и без этого предела
                // длинная сохранённая строка растягивала коробку и вылезала
                // за неё вместо переноса.
                let width = match space_along {
                    gpui::AvailableSpace::Definite(w) if width > w => w,
                    _ => width,
                };
                // Абзац с атомами, перенесённый МЯГКО, занимает всё отведённое
                // место: ширина «по содержимому» — это min(max-content,
                // max(min-content, доступное)) (CSS 2.1 §10.3.5, css-sizing-3
                // §5.1 fit-content), а не самая длинная строка после переноса.
                // Так мерил и прежний гибкий ряд с переносом, и коробка
                // `width: fit-content(100px)` из двух `inline-block` по 60px
                // выходила 60 вместо 100 (`fit-content-length-percentage-*`).
                let width = match space_along {
                    gpui::AvailableSpace::Definite(w)
                        if known_along.is_none() && !probe.atom_boxes.is_empty() && width < w =>
                    {
                        let full = probe
                            .split(None, window)
                            .iter()
                            .map(|l| l.width + l.indent.max(px(0.)))
                            .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
                        if full > w { w } else { width }
                    }
                    _ => width,
                };
                // Высота абзаца — сумма ШАГОВ строк: обычно это ровно
                // `line_height`, но строка со сдвинутым по вертикали куском
                // выше на его вылет (CSS 2.1 §10.8).
                // Сдвиг ПОСЛЕДНЕЙ строки от первой: шаги всех строк перед ней
                // плюс разница их надбавок сверху (у первой базовой надбавка
                // своей строки не учтена — последняя считается тем же отсчётом,
                // и у однострочного абзаца обе совпадают).
                let mut last_shift = px(0.);
                // Верхняя надбавка ПЕРВОЙ строки опускает её базовую линию:
                // атом выше струта сдвигает текст строки вниз, и базовая
                // абзаца (для `inline-block` — его собственная, §10.8.1)
                // обязана уехать вместе с ним.
                let mut first_above = px(0.);
                let across = {
                    probe.lines = lines.clone();
                    let pads = probe.line_padding();
                    if !probe.atom_boxes.is_empty() || !probe.box_spans.is_empty() {
                        first_above = px(pads.first().map_or(0.0, |p| p.0));
                    }
                    if pads.len() > 1 {
                        let before: f32 = pads[..pads.len() - 1].iter().map(|(a, b)| a + b).sum();
                        let own = pads[pads.len() - 1].0 - pads[0].0;
                        last_shift = line_height * (pads.len() - 1) as f32 + px(before + own);
                    }
                    let extra: f32 = pads.iter().map(|(a, b)| a + b).sum();
                    (if vertical {
                        probe.line_height
                    } else {
                        line_height
                    }) * lines.len() as f32
                        + px(extra)
                };
                if vertical {
                    let width = if vertical_inline.is_some() {
                        limit.unwrap_or(width)
                    } else {
                        width
                    };
                    return probe.vertical_content_baselines(size(
                        known.width.unwrap_or(across),
                        known.height.unwrap_or(width),
                    ));
                }
                let baseline = probe.measured_first_baseline(line_height, first_above, window);
                let last_baseline = baseline.map(|b| b + last_shift);
                gpui::MeasuredContent {
                    first_y: baseline,
                    last_y: last_baseline,
                    lines_y: Some(probe.content_line_baselines(baseline)),
                    ..gpui::MeasuredContent::new(size(
                        known.width.unwrap_or(width),
                        known.height.unwrap_or(across),
                    ))
                }
            },
        );
        (id, id)
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        state: &mut LayoutId,
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        // Предел переноса — длина строки по её физической оси.
        self.vertical_layout_origin =
            window.layout_origin_unrounded(*state) - window.element_offset();
        self.width_nudge = {
            let dw = window.layout_size_unrounded(*state).width - bounds.size.width;
            let one = 1.0 / window.scale_factor().max(0.01) + 1e-4;
            if self.vertical || f32::from(dw).abs() > one {
                px(0.0)
            } else {
                dw
            }
        };
        // Snapping moves an edge by at most half a device pixel; anything
        // larger means the node was placed outside its tree (`prepaint_at`).
        self.glyph_nudge = {
            let d = window.layout_origin_unrounded(*state) - bounds.origin;
            let half = 0.5 / window.scale_factor().max(0.01) + 1e-4;
            let ok = |v: Pixels| f32::from(v).abs() <= half;
            if self.vertical || !(ok(d.x) && ok(d.y)) {
                point(px(0.0), px(0.0))
            } else {
                d
            }
        };
        let limit = if self.vertical {
            bounds.size.height
        } else {
            bounds.size.width
        };
        // Коробка приходит округлённой ВНИЗ до точки устройства, а мерили её
        // по дробной ширине: строка, влезавшая ровно, на отрисовке уже не
        // влезала и рвалась заново (`hyphens-manual-011`). Возвращаем себе эту
        // одну точку устройства — иначе раскладка кадра расходится с замером.
        let scale = window.scale_factor().max(1.0);
        let limit = limit + px(1.0 / scale);
        self.indent_basis = {
            let exact = window.layout_size_unrounded(*state);
            let exact = if self.vertical {
                exact.height
            } else {
                exact.width
            };
            let snapped = if self.vertical {
                bounds.size.height
            } else {
                bounds.size.width
            };
            (f32::from(exact - snapped).abs() <= 1.0 / scale + 1e-4).then_some(exact)
        };
        self.apply_measured_fit();
        if !self.vertical {
            let avail = f32::from(window.layout_size_unrounded(*state).width);
            self.refit_atoms(avail, window, _cx);
        }
        self.lines = self.split(Some(limit), window);
        self.unbalanced_steps = None;
        if self.clamp_tag.is_some() && self.wrap.balance && self.clamp.is_none() {
            if self.run_metrics.len() != self.runs.len() {
                self.run_metrics = self.measure_runs(window);
            }
            let segs = self.measure(window);
            let unbalanced = self.lay(Some(limit), &segs);
            let balanced = std::mem::replace(&mut self.lines, unbalanced);
            let lh = f32::from(self.line_height);
            self.unbalanced_steps = Some(
                self.line_padding()
                    .iter()
                    .map(|(a, b)| lh + a + b)
                    .collect(),
            );
            self.lines = balanced;
        }
        self.place_atoms(*state, window, _cx);
        // Куски вне потока встают на своё место в строке: раскладываются
        // по содержимому и подготавливаются от угла своего знака.
        if !self.overlays.is_empty() {
            let segs = self.measure(window);
            let mut placed = std::mem::take(&mut self.overlays);
            let rotated = crate::text::vertical::in_rotated_frame();
            let scale = window.scale_factor().max(0.01);
            for (at, el, how) in placed.iter_mut() {
                // Абсолют от строчного содержащего блока: края считает
                // раскладка от коробки размером в этот блок (`inline_cb_rect`).
                if let Some(cb) = how.cb.filter(|_| !rotated) {
                    let s = (*at as isize + cb.start).max(0) as usize;
                    let e = (*at as isize + cb.end).max(0) as usize;
                    if let Some(r) = self.inline_cb_rect(&segs, s, e, cb.pad, bounds) {
                        use gpui::{ParentElement, Styled};
                        let inner = std::mem::replace(el, gpui::Empty.into_any_element());
                        *el = gpui::div()
                            .relative()
                            .w(r.size.width)
                            .h(r.size.height)
                            .child(inner)
                            .into_any_element();
                        el.layout_as_root(
                            gpui::size(
                                gpui::AvailableSpace::Definite(r.size.width),
                                gpui::AvailableSpace::Definite(r.size.height),
                            ),
                            window,
                            _cx,
                        );
                        let o = point(r.origin.x + px(cb.shift.0), r.origin.y + px(cb.shift.1));
                        el.prepaint_at(o, window, _cx);
                        continue;
                    }
                }
                let next = &how.next_line;
                let origin = if *next {
                    self.next_line_point(*at, bounds)
                } else {
                    self.point_of(&segs, *at, bounds)
                };
                // Повёрнутый абзац: до-поворотная y — блочная ось экрана.
                // Коробка ложится краем туда же, куда глиф соседнего текста:
                // глиф — целая часть физической точки (`paint_glyph`), а
                // раскладка округлила бы до ближайшей. Расхождение в точку
                // оставляло столбец красного (`static-position/vlr-*`).
                let origin = if rotated {
                    let y = f32::from(origin.y + px(how.rot_dy)) * scale;
                    point(origin.x + px(how.rot_dx), px((y + 1e-3).floor() / scale))
                } else {
                    origin
                };
                // Ширина абсолютного элемента — «по содержимому» (CSS 2.1
                // §10.3.7): по МИНИМАЛЬНОМУ содержимому он рвался бы по
                // словам (`static-position/htb-*`).
                let size = el.layout_as_root(
                    gpui::size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    _cx,
                );
                // Блочная коробка при `rtl` вешается ПРАВЫМ краем на правый
                // край содержимого (§10.3.7: `right` = статическая позиция).
                let origin = if *next && self.wrap.rtl {
                    // `origin.x` здесь — край содержимого плюс сдвиг предков.
                    point(origin.x + bounds.size.width - size.width, origin.y)
                } else if how.bidi_hang && self.rtl_level_at(*at) {
                    point(origin.x - size.width, origin.y)
                } else {
                    origin
                };
                el.prepaint_at(origin, window, _cx);
            }
            self.overlays = placed;
        }
        self.id
            .is_some()
            .then(|| window.insert_hitbox(bounds, HitboxBehavior::Normal))
    }

    fn paint(
        &mut self,
        id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut LayoutId,
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Поворачивается текст внутри уже рассчитанной по физическим осям коробки.
        if self.vertical {
            let bounds = self.vertical_paint_bounds(
                bounds,
                self.vertical_layout_origin,
                window.scale_factor(),
            );
            let matrix = self.vertical_transform(bounds, window.scale_factor());
            // Flat inline/block axes match the painted physical extent.
            let flat = Bounds {
                origin: bounds.origin,
                size: size(bounds.size.height, bounds.size.width),
            };
            let mut inner = std::mem::replace(self, Paragraph::empty());
            inner.vertical = false;
            let selection = inner
                .selection_vertical
                .replace((bounds, inner.vertical_ccw));
            window.with_transformation(matrix, |window| {
                inner.paint(id, _inspector_id, flat, _state, hitbox, window, cx);
            });
            inner.selection_vertical = selection;
            inner.vertical = true;
            *self = inner;
            return;
        }
        let outer_nudge = window.replace_glyph_offset(self.glyph_nudge);
        let bounds = Bounds {
            origin: bounds.origin,
            size: size(bounds.size.width + self.width_nudge, bounds.size.height),
        };
        let segs = self.measure(window);
        if self.run_metrics.len() != self.runs.len() {
            self.run_metrics = self.measure_runs(window);
        }
        let count = self.lines.len();
        // Надбавки строк от сдвинутых кусков: шаг до следующей строки и
        // сдвиг набора внутри своей.
        let pads = self.line_padding();
        let step = |i: usize| -> Pixels {
            let (a, b) = pads.get(i).copied().unwrap_or((0.0, 0.0));
            self.line_height + px(a + b)
        };
        // Надбавка сверху опускает НАБОР строки: поднятый знак занимает её,
        // а базовая линия остаётся на своём месте относительно кегля.
        let above = |i: usize| -> Pixels { px(pads.get(i).copied().unwrap_or((0.0, 0.0)).0) };
        if let Some(tag) = self.clamp_tag
            && !self.lines_reversed
        {
            let mut y0 = f32::from(bounds.origin.y);
            let rows = match &self.unbalanced_steps {
                Some(steps) => steps
                    .iter()
                    .map(|h| {
                        let r = (y0, y0 + h);
                        y0 = r.1;
                        r
                    })
                    .collect(),
                None => (0..count)
                    .map(|i| {
                        let r = (y0, y0 + f32::from(step(i)));
                        y0 = r.1;
                        r
                    })
                    .collect(),
            };
            crate::text::clamp::publish_para_rows(tag, rows);
        }
        // Строки снизу вверх: место строки считается ОТ ВЕРХА коробки одним
        // сложением (`origin + px(смещение)`), как у `point_of`. Прежде
        // `origin + total - line_height` в f32 расходился с `point_of` в
        // последнем знаке, а глиф (`paint_glyph`: `floor` физической точки)
        // на ровной точке от этого падает на целую точку: текст стоял на
        // точку от щупа статической позиции — столбец красного в
        // `static-position/vlr-*` (замер по снимку: глиф x=49, коробка 48;
        // после правки оба 48).
        let mut rev_off: f32 = if self.lines_reversed && count > 0 {
            let total: f32 = (0..count).map(|i| f32::from(step(i))).sum();
            total - f32::from(self.line_height)
        } else {
            0.0
        };
        let mut y = bounds.origin.y + px(rev_off);
        let selection = id
            .map(|global| {
                window.with_element_state::<Selection, _>(global, |state, _| {
                    let st = state.unwrap_or_default();
                    (st.range(), st)
                })
            })
            .unwrap_or((0, 0));
        let mut runs = self.runs_with_selection(selection.0, selection.1);
        if self.decor_on() {
            for r in runs.iter_mut() {
                r.underline = None;
                r.strikethrough = None;
            }
        }
        for (i, line) in self.lines.clone().into_iter().enumerate() {
            // Перевод строки в набор не отдаём: он уже сработал разрывом.
            let body = self.text[line.range.clone()].trim_end_matches('\n');
            let range = line.range.start..line.range.start + body.len();
            // Последняя строка абзаца и строка, оборванная жёстким разрывом,
            // по ширине не растягиваются: иначе абзац из одного слова разъехался
            // бы во всю колонку. Для них своя выключка (`text-align-last`).
            // Строка с СОХРАНЁННОЙ табуляцией не растягивается: позиции
            // табуляции обязаны совпасть с нерастянутой строкой
            // (css-text-4 §8.1, `text-align-justify-tabs-001`), а раздача
            // остатка их бы сдвинула.
            let align = self.line_align(i, &line);
            // Отступ первой строки занимает место В колонке: остаток на
            // выключку считается уже без него. Правый вырез обтекания
            // (`shape-outside`) — тоже: прижатая вправо строка упирается в
            // форму, а не в край коробки (circle-024: text-align right).
            let free_raw = bounds.size.width - line.width - line.indent - px(self.flow_cut(i).1);
            // Обрезание отрицательного остатка нужно только РАЗДАЧЕ
            // (`Justify`): растягивать переполненную строку нечем. Сдвиг по
            // `text-align` берёт остаток СО ЗНАКОМ — иначе широкая строка
            // всегда вылезает вправо, то есть по-ltr при любом письме
            // (см. `line_offset`).
            let free = if free_raw < px(0.) { px(0.) } else { free_raw };
            // Свисающий открывающий знак уходит ЗА край: строка сдвигается
            // влево на его ширину. Считается до выбора пути отрисовки —
            // выключенная строка свисает так же, как обычная.
            let hang = self.hang_first(line.range.start);
            let shift = self.span(&segs, line.range.start, line.range.start + hang);
            // Отступ первой строки идёт от НАЧАЛЬНОГО края (§16.1): в rtl это
            // правый край, и место ему уже отдано вычетом из остатка выше.
            // Прибавка слева считала бы его второй раз, а при выключке вправо
            // и вовсе гасила: `(W - w - indent) + indent = W - w`.
            let lead = if self.wrap.rtl { px(0.) } else { line.indent } - shift;
            // Строка с межсловным интервалом рисуется ПО СЛОВАМ: одним
            // набором промежутки не показать — шейпер о них не знает. Раздача
            // остатка при этом нулевая, слова просто встают по своим местам.
            // Межсловный интервал ставит слова по местам сам, поэтому строка
            // с ним рисуется тем же путём, что и выключенная. Интервал бывает
            // задан и НА КУСКЕ — тогда общего значения нет, а путь нужен тот
            // же (иначе `word-spacing` на `<span>` не действовал вовсе).
            // Строка с СОХРАНЁННОЙ табуляцией рисуется тоже по словам:
            // продвижение табуляции задаёт её позиция (`Seg::offset`), а
            // сплошной набор строки о ней не знает и кладёт глиф шрифта —
            // нарисованное выходило короче замеренного на целую позицию
            // (`text-align-justify-tabs-002`). Раздача остатка при этом
            // нулевая: растягивать такую строку нельзя (см. `no_stretch`),
            // слова просто встают по своим местам.
            if align == Align::Justify
                || body.contains('\u{9}')
                || self.word_spacing != px(0.)
                || !self.word_spans.is_empty()
                || self.letter_spans_diverge()
                || !self.shift_spans.is_empty()
                || !self.rel_spans.is_empty()
                || !self.edge_spans.is_empty()
            {
                let (free, dx) = if align == Align::Justify {
                    (free, lead)
                } else {
                    let dx = line_offset(align, self.wrap.rtl, free_raw);
                    (px(0.), dx + lead)
                };
                // Набор строки опускается на её верхнюю надбавку: поднятый
                // кусок занимает добавленное место, а остальной текст
                // остаётся на своей базовой линии.
                self.paint_justified(
                    &range,
                    &segs,
                    free,
                    line.width,
                    bounds,
                    y + above(i),
                    pads.get(i).copied().unwrap_or((0.0, 0.0)),
                    dx,
                    window,
                    cx,
                );
                if line.ellipsis {
                    let text = self.span(&segs, line.range.start, range.end);
                    self.paint_suffix(
                        self.line_mark(&line),
                        line.range.start,
                        point(bounds.origin.x + dx + text, y + above(i)),
                        // На базовую линию строки — ту же, на которую
                        // `paint_justified` опускает слова (`block-ellipsis-005`:
                        // знак кегля блока за `<span>` 1.5em стоял выше).
                        self.base_of(&range).map(px),
                        false,
                        window,
                        cx,
                    );
                }
                if self.lines_reversed {
                    rev_off -= f32::from(step(i));
                    y = bounds.origin.y + px(rev_off);
                } else {
                    y += step(i);
                }
                continue;
            }
            let dx = line_offset(align, self.wrap.rtl, free_raw) + lead;
            let at = point(bounds.origin.x + dx, y + above(i));
            // Висящие пробелы конца строки при письме справа налево уходят по
            // правилу L1 на ЛЕВЫЙ край и отодвигали бы текст от края коробки.
            // Рисовать их незачем: они пустые.
            // ★ ЗАМЕРЕНО: рисовать их и при `pre` — `trailing-space-and-
            // text-alignment-rtl-002` 0.02 -> 1.67 (пробел вставал справа от
            // текста и сдвигал его); место в ширине строки они держат.
            let visible = if self.wrap.rtl && !self.wrap.break_spaces {
                range.start..range.start + trim_hanging(&self.text[range.clone()])
            } else if !self.wrap.keep_spaces {
                // Схлопываемый пробел конца строки УДАЛЯЕТСЯ (CSS 2.1 §16.6.1),
                // а не висит: рисовать его незачем, а подложка `<span>` под ним
                // вылезала за край коробки квадратом кегля (`c548-leadin-000`:
                // красный 25×25 справа от первой строки). Отличие от откаченной
                // правки у `trim_hanging`: там резалась ПОДЛОЖКА прогонов по
                // всему `hangs` (U+3000, U+2000..200A и пр.), здесь — только сам
                // отрезок набора, только U+0020/U+0009 и только при схлопывающем
                // `white-space`; прочие Zs-разделители висят как прежде.
                // CSS Text §4.1.3 preserves each inline span's non-collapsible tail.
                range.start..self.drop_collapsible_tail(range.start, range.end)
            } else {
                range.clone()
            };
            // Знак обрыва и знак переноса набираются вместе со строкой.
            let mark = if line.ellipsis && line.clamped && line.hyphen {
                // Обрыв на мягком переносе: знак переноса, затем многоточие.
                format!("{}{}", self.hyphen, self.line_mark(&line))
            } else if line.ellipsis {
                self.line_mark(&line).to_string()
            } else if line.hyphen {
                self.hyphen.to_string()
            } else {
                String::new()
            };
            if let Some(cut) = line.vis_cut.filter(|_| line.ellipsis) {
                // Строка целиком в видимом порядке, скрытые с конечного края
                // знаки отсекает маска; знак обрыва — сразу за видимой частью.
                let full = self.span(&segs, visible.start, visible.end);
                let (x0, mask_x, mark_x) = if self.wrap.rtl {
                    (at.x + line.width - full, at.x + line.width - cut, at.x)
                } else {
                    (at.x, at.x, at.x + cut)
                };
                let mask = Bounds {
                    origin: point(mask_x, bounds.origin.y - px(1000.)),
                    size: size(cut, bounds.size.height + px(2000.)),
                };
                let (base, exact) = window
                    .with_content_mask(Some(gpui::ContentMask { bounds: mask }), |window| {
                        self.paint_line(&visible, &runs, point(x0, at.y), "", window, cx)
                    });
                self.paint_suffix(
                    &mark,
                    line.range.start,
                    point(mark_x, at.y),
                    base,
                    exact,
                    window,
                    cx,
                );
            } else {
                self.paint_line(&visible, &runs, at, &mark, window, cx);
            }
            if self.lines_reversed {
                rev_off -= f32::from(step(i));
                y = bounds.origin.y + px(rev_off);
            } else {
                y += step(i);
            }
        }
        window.replace_glyph_offset(outer_nudge);
        for slot in self.atoms.iter_mut().filter(|s| !s.hidden) {
            slot.el.paint(window, cx);
        }
        for (_, el, _) in self.overlays.iter_mut() {
            el.paint(window, cx);
        }
        if let (Some(global), Some(hitbox)) = (id, hitbox.clone()) {
            self.track_selection(global, &segs, bounds, hitbox, window);
        }
    }
}

impl IntoElement for Paragraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}
