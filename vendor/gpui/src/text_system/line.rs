use crate::{
    App, Bounds, Half, Hsla, LineLayout, Pixels, Point, Result, SharedString, StrikethroughStyle,
    TextAlign, UnderlineStyle, Window, WrapBoundary, WrappedLineLayout, black, fill, point, px,
    size,
};
use derive_more::{Deref, DerefMut};
use smallvec::SmallVec;
use std::sync::Arc;

/// Set the text decoration for a run of text.
#[derive(Debug, Clone)]
pub struct DecorationRun {
    /// The length of the run in utf-8 bytes.
    pub len: u32,

    /// The color for this run
    pub color: Hsla,

    /// The background color for this run
    pub background_color: Option<Hsla>,
    /// KaminIDE patch: поля вокруг фона прогона (строчный бокс).
    pub background_pad: Point<Pixels>,
    /// KaminIDE patch: скругление фона прогона (строчный бокс).
    pub background_radius: Pixels,
    /// KaminIDE patch: рамка строчного бокса — цвет и толщина. Рисуется по
    /// КУСКАМ строк вместе с фоном: строчная коробка в браузере разрезается
    /// переносом, и рамка каждого куска своя.
    pub background_border: Option<(Hsla, [Pixels; 4])>,

    /// The underline style for this run
    pub underline: Option<UnderlineStyle>,

    /// The strikethrough style for this run
    pub strikethrough: Option<StrikethroughStyle>,
}

/// A line of text that has been shaped and decorated.
#[derive(Clone, Default, Debug, Deref, DerefMut)]
pub struct ShapedLine {
    #[deref]
    #[deref_mut]
    pub(crate) layout: Arc<LineLayout>,
    /// The text that was shaped for this line.
    pub text: SharedString,
    pub(crate) decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl ShapedLine {
    /// The length of the line in utf-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len
    }

    /// Override the len, useful if you're rendering text a
    /// as text b (e.g. rendering invisibles).
    pub fn with_len(mut self, len: usize) -> Self {
        let layout = self.layout.as_ref();
        self.layout = Arc::new(LineLayout {
            font_size: layout.font_size,
            width: layout.width,
            ascent: layout.ascent,
            descent: layout.descent,
            runs: layout.runs.clone(),
            len,
        });
        self
    }

    /// Paint the line of text to the window.
    pub fn paint(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        paint_line(
            origin,
            &self.text,
            &self.layout,
            line_height,
            TextAlign::default(),
            None,
            &self.decoration_runs,
            &[],
            window,
            cx,
        )?;

        Ok(())
    }

    /// Paint the background of the line to the window.
    pub fn paint_background(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        paint_line_background(
            origin,
            &self.text,
            &self.layout,
            line_height,
            TextAlign::default(),
            None,
            &self.decoration_runs,
            &[],
            window,
            cx,
        )?;

        Ok(())
    }
}

/// A line of text that has been shaped, decorated, and wrapped by the text layout system.
#[derive(Clone, Default, Debug, Deref, DerefMut)]
pub struct WrappedLine {
    #[deref]
    #[deref_mut]
    pub(crate) layout: Arc<WrappedLineLayout>,
    /// The text that was shaped for this line.
    pub text: SharedString,
    pub(crate) decoration_runs: SmallVec<[DecorationRun; 32]>,
}

impl WrappedLine {
    /// The length of the underlying, unwrapped layout, in utf-8 bytes.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.layout.len()
    }

    /// Paint this line of text to the window.
    pub fn paint(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        align: TextAlign,
        bounds: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        let align_width = match bounds {
            Some(bounds) => Some(bounds.size.width),
            None => self.layout.wrap_width,
        };

        paint_line(
            origin,
            &self.text,
            &self.layout.unwrapped_layout,
            line_height,
            align,
            align_width,
            &self.decoration_runs,
            &self.wrap_boundaries,
            window,
            cx,
        )?;

        Ok(())
    }

    /// Paint the background of line of text to the window.
    pub fn paint_background(
        &self,
        origin: Point<Pixels>,
        line_height: Pixels,
        align: TextAlign,
        bounds: Option<Bounds<Pixels>>,
        window: &mut Window,
        cx: &mut App,
    ) -> Result<()> {
        let align_width = match bounds {
            Some(bounds) => Some(bounds.size.width),
            None => self.layout.wrap_width,
        };

        paint_line_background(
            origin,
            &self.text,
            &self.layout.unwrapped_layout,
            line_height,
            align,
            align_width,
            &self.decoration_runs,
            &self.wrap_boundaries,
            window,
            cx,
        )?;

        Ok(())
    }
}

/// KaminIDE patch: добавка на один межсловный пробел в каждой строке.
///
/// Выключка растягивает не буквы, а промежутки между словами, и только в
/// строках, которые переносятся: последняя строка абзаца остаётся как есть.
/// Пробел на самом переносе не растягивается — в браузере он схлопнут.
fn justify_extras(
    text: &str,
    layout: &LineLayout,
    wrap_boundaries: &[WrapBoundary],
    align_width: Pixels,
) -> Vec<Pixels> {
    let mut extras = Vec::with_capacity(wrap_boundaries.len() + 1);
    let mut wraps = wrap_boundaries.iter().peekable();
    let mut line_start = px(0.);
    let mut gaps = 0usize;
    let mut prev_space = false;
    for (run_ix, run) in layout.runs.iter().enumerate() {
        for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
            if wraps.peek() == Some(&&WrapBoundary { run_ix, glyph_ix }) {
                wraps.next();
                let width = glyph.position.x - line_start;
                extras.push(if gaps > 0 {
                    ((align_width - width) / gaps as f32).max(px(0.))
                } else {
                    px(0.)
                });
                line_start = glyph.position.x;
                gaps = 0;
                prev_space = false;
            }
            let space = text.as_bytes().get(glyph.index) == Some(&b' ');
            if prev_space && !space {
                gaps += 1;
            }
            prev_space = space;
        }
    }
    // Последняя строка не растягивается.
    extras.push(px(0.));
    extras
}

fn paint_line(
    origin: Point<Pixels>,
    text: &str,
    layout: &LineLayout,
    line_height: Pixels,
    align: TextAlign,
    align_width: Option<Pixels>,
    decoration_runs: &[DecorationRun],
    wrap_boundaries: &[WrapBoundary],
    window: &mut Window,
    cx: &mut App,
) -> Result<()> {
    let line_bounds = Bounds::new(
        origin,
        size(
            layout.width,
            line_height * (wrap_boundaries.len() as f32 + 1.),
        ),
    );
    window.paint_layer(line_bounds, |window| {
        // KaminIDE patch: выключка по ширине — остаток строки раздаётся её
        // межсловным промежуткам (см. `justify_extras`).
        let justify = (align == TextAlign::Justify).then(|| {
            justify_extras(
                text,
                layout,
                wrap_boundaries,
                align_width.unwrap_or(layout.width),
            )
        });
        let padding_top = (line_height - layout.ascent - layout.descent) / 2.;
        let baseline_offset = point(px(0.), padding_top + layout.ascent);
        let mut decoration_runs = decoration_runs.iter();
        let mut wraps = wrap_boundaries.iter().peekable();
        let mut run_end = 0;
        let mut color = black();
        let mut current_underline: Option<(Point<Pixels>, UnderlineStyle)> = None;
        let mut current_strikethrough: Option<(Point<Pixels>, StrikethroughStyle)> = None;
        let text_system = cx.text_system().clone();
        let mut glyph_origin = point(
            aligned_origin_x(
                origin,
                align_width.unwrap_or(layout.width),
                px(0.0),
                &align,
                layout,
                wraps.peek(),
            ),
            origin.y,
        );
        let mut prev_glyph_position = Point::default();
        let mut max_glyph_size = size(px(0.), px(0.));
        let mut first_glyph_x = origin.x;
        // KaminIDE patch: выключка — добавка на каждый пройденный промежуток.
        let mut line_ix = 0usize;
        let mut gaps_passed = 0usize;
        let mut prev_space = false;
        for (run_ix, run) in layout.runs.iter().enumerate() {
            max_glyph_size = text_system.bounding_box(run.font_id, run.font_size).size;

            for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                glyph_origin.x += glyph.position.x - prev_glyph_position.x;
                if glyph_ix == 0 && run_ix == 0 {
                    first_glyph_x = glyph_origin.x;
                }

                let space = text.as_bytes().get(glyph.index) == Some(&b' ');
                if let Some(extras) = justify.as_ref() {
                    // Добавка отдаётся ОДИН раз на промежуток: положение
                    // глифа копится от предыдущего, и повторное прибавление
                    // растащило бы буквы внутри слова.
                    if prev_space && !space {
                        gaps_passed += 1;
                        glyph_origin.x += extras.get(line_ix).copied().unwrap_or(px(0.));
                    }
                }
                prev_space = space;

                if wraps.peek() == Some(&&WrapBoundary { run_ix, glyph_ix }) {
                    wraps.next();
                    line_ix += 1;
                    gaps_passed = 0;
                    prev_space = false;
                    if let Some((underline_origin, underline_style)) = current_underline.as_mut() {
                        if glyph_origin.x == underline_origin.x {
                            underline_origin.x -= max_glyph_size.width.half();
                        };
                        window.paint_underline(
                            *underline_origin,
                            glyph_origin.x - underline_origin.x,
                            underline_style,
                        );
                        underline_origin.x = origin.x;
                        underline_origin.y += line_height;
                    }
                    if let Some((strikethrough_origin, strikethrough_style)) =
                        current_strikethrough.as_mut()
                    {
                        if glyph_origin.x == strikethrough_origin.x {
                            strikethrough_origin.x -= max_glyph_size.width.half();
                        };
                        window.paint_strikethrough(
                            *strikethrough_origin,
                            glyph_origin.x - strikethrough_origin.x,
                            strikethrough_style,
                        );
                        strikethrough_origin.x = origin.x;
                        strikethrough_origin.y += line_height;
                    }

                    glyph_origin.x = aligned_origin_x(
                        origin,
                        align_width.unwrap_or(layout.width),
                        glyph.position.x,
                        &align,
                        layout,
                        wraps.peek(),
                    );
                    glyph_origin.y += line_height;
                }
                prev_glyph_position = glyph.position;

                let mut finished_underline: Option<(Point<Pixels>, UnderlineStyle)> = None;
                let mut finished_strikethrough: Option<(Point<Pixels>, StrikethroughStyle)> = None;
                if glyph.index >= run_end {
                    let mut style_run = decoration_runs.next();

                    // ignore style runs that apply to a partial glyph
                    while let Some(run) = style_run {
                        if glyph.index < run_end + (run.len as usize) {
                            break;
                        }
                        run_end += run.len as usize;
                        style_run = decoration_runs.next();
                    }

                    if let Some(style_run) = style_run {
                        if let Some((_, underline_style)) = &mut current_underline
                            && style_run.underline.as_ref() != Some(underline_style)
                        {
                            finished_underline = current_underline.take();
                        }
                        if let Some(run_underline) = style_run.underline.as_ref() {
                            current_underline.get_or_insert((
                                point(
                                    glyph_origin.x,
                                    glyph_origin.y + baseline_offset.y + (layout.descent * 0.618),
                                ),
                                UnderlineStyle {
                                    color: Some(run_underline.color.unwrap_or(style_run.color)),
                                    thickness: run_underline.thickness,
                                    wavy: run_underline.wavy,
                                },
                            ));
                        }
                        if let Some((_, strikethrough_style)) = &mut current_strikethrough
                            && style_run.strikethrough.as_ref() != Some(strikethrough_style)
                        {
                            finished_strikethrough = current_strikethrough.take();
                        }
                        if let Some(run_strikethrough) = style_run.strikethrough.as_ref() {
                            current_strikethrough.get_or_insert((
                                point(
                                    glyph_origin.x,
                                    glyph_origin.y
                                        + (((layout.ascent * 0.5) + baseline_offset.y) * 0.5),
                                ),
                                StrikethroughStyle {
                                    color: Some(run_strikethrough.color.unwrap_or(style_run.color)),
                                    thickness: run_strikethrough.thickness,
                                },
                            ));
                        }

                        run_end += style_run.len as usize;
                        color = style_run.color;
                    } else {
                        run_end = layout.len;
                        finished_underline = current_underline.take();
                        finished_strikethrough = current_strikethrough.take();
                    }
                }

                if let Some((mut underline_origin, underline_style)) = finished_underline {
                    if underline_origin.x == glyph_origin.x {
                        underline_origin.x -= max_glyph_size.width.half();
                    };
                    window.paint_underline(
                        underline_origin,
                        glyph_origin.x - underline_origin.x,
                        &underline_style,
                    );
                }

                if let Some((mut strikethrough_origin, strikethrough_style)) =
                    finished_strikethrough
                {
                    if strikethrough_origin.x == glyph_origin.x {
                        strikethrough_origin.x -= max_glyph_size.width.half();
                    };
                    window.paint_strikethrough(
                        strikethrough_origin,
                        glyph_origin.x - strikethrough_origin.x,
                        &strikethrough_style,
                    );
                }

                let max_glyph_bounds = Bounds {
                    origin: glyph_origin,
                    size: max_glyph_size,
                };

                let content_mask = window.content_mask();
                if max_glyph_bounds.intersects(&content_mask.bounds) {
                    if glyph.is_emoji {
                        window.paint_emoji(
                            glyph_origin + baseline_offset,
                            run.font_id,
                            glyph.id,
                            run.font_size,
                        )?;
                    } else {
                        window.paint_glyph(
                            glyph_origin + baseline_offset,
                            run.font_id,
                            glyph.id,
                            // KaminIDE patch: кегль ПРОГОНА — иначе кусок
                            // другого размера рисовался чужими глифами.
                            run.font_size,
                            color,
                        )?;
                    }
                }
            }
        }

        let mut last_line_end_x = first_glyph_x + layout.width;
        if let Some(boundary) = wrap_boundaries.last() {
            let run = &layout.runs[boundary.run_ix];
            let glyph = &run.glyphs[boundary.glyph_ix];
            last_line_end_x -= glyph.position.x;
        }

        if let Some((mut underline_start, underline_style)) = current_underline.take() {
            if last_line_end_x == underline_start.x {
                underline_start.x -= max_glyph_size.width.half()
            };
            window.paint_underline(
                underline_start,
                last_line_end_x - underline_start.x,
                &underline_style,
            );
        }

        if let Some((mut strikethrough_start, strikethrough_style)) = current_strikethrough.take() {
            if last_line_end_x == strikethrough_start.x {
                strikethrough_start.x -= max_glyph_size.width.half()
            };
            window.paint_strikethrough(
                strikethrough_start,
                last_line_end_x - strikethrough_start.x,
                &strikethrough_style,
            );
        }

        Ok(())
    })
}

fn paint_line_background(
    origin: Point<Pixels>,
    text: &str,
    layout: &LineLayout,
    line_height: Pixels,
    align: TextAlign,
    align_width: Option<Pixels>,
    decoration_runs: &[DecorationRun],
    wrap_boundaries: &[WrapBoundary],
    window: &mut Window,
    cx: &mut App,
) -> Result<()> {
    let line_bounds = Bounds::new(
        origin,
        size(
            layout.width,
            line_height * (wrap_boundaries.len() as f32 + 1.),
        ),
    );
        // KaminIDE patch: выключка по ширине — остаток строки раздаётся её
        // межсловным промежуткам (см. `justify_extras`).
        let justify = (align == TextAlign::Justify).then(|| {
            justify_extras(
                text,
                layout,
                wrap_boundaries,
                align_width.unwrap_or(layout.width),
            )
        });
    window.paint_layer(line_bounds, |window| {
        let mut decoration_runs = decoration_runs.iter();
        let mut wraps = wrap_boundaries.iter().peekable();
        let mut run_end = 0;
        // Пятое поле — рамка строчного бокса: она живёт вместе с фоном и
        // режется переносом так же, как он.
        let mut current_background: Option<(
            Point<Pixels>,
            (Hsla, Point<Pixels>, Pixels, bool, Option<(Hsla, [Pixels; 4])>),
        )> = None;
        let text_system = cx.text_system().clone();
        let mut glyph_origin = point(
            aligned_origin_x(
                origin,
                align_width.unwrap_or(layout.width),
                px(0.0),
                &align,
                layout,
                wraps.peek(),
            ),
            origin.y,
        );
        let mut prev_glyph_position = Point::default();
        let mut max_glyph_size = size(px(0.), px(0.));
        // KaminIDE patch: выключка — добавка на каждый пройденный промежуток.
        let mut line_ix = 0usize;
        let mut gaps_passed = 0usize;
        let mut prev_space = false;
        for (run_ix, run) in layout.runs.iter().enumerate() {
            max_glyph_size = text_system.bounding_box(run.font_id, run.font_size).size;

            for (glyph_ix, glyph) in run.glyphs.iter().enumerate() {
                glyph_origin.x += glyph.position.x - prev_glyph_position.x;

                let space = text.as_bytes().get(glyph.index) == Some(&b' ');
                if let Some(extras) = justify.as_ref() {
                    // Та же добавка, что и в проходе глифов: подложки прогонов
                    // обязаны стоять там же, где буквы.
                    if prev_space && !space {
                        gaps_passed += 1;
                        glyph_origin.x += extras.get(line_ix).copied().unwrap_or(px(0.));
                    }
                }
                prev_space = space;

                if wraps.peek() == Some(&&WrapBoundary { run_ix, glyph_ix }) {
                    wraps.next();
                    line_ix += 1;
                    gaps_passed = 0;
                    prev_space = false;
                    if let Some((background_origin, background_color)) = current_background.as_mut()
                    {
                        if glyph_origin.x == background_origin.x {
                            background_origin.x -= max_glyph_size.width.half()
                        }
                        window.paint_quad(run_background_quad(
                            *background_origin,
                            glyph_origin.x - background_origin.x,
                            line_height,
                            layout.font_size,
                            background_color.0,
                            background_color.1,
                            background_color.2,
                            background_color.3,
                            false,
                            background_color.4,
                        ));
                        background_color.3 = false;
                        background_origin.x = origin.x;
                        background_origin.y += line_height;
                    }

                    glyph_origin.x = aligned_origin_x(
                        origin,
                        align_width.unwrap_or(layout.width),
                        glyph.position.x,
                        &align,
                        layout,
                        wraps.peek(),
                    );
                    glyph_origin.y += line_height;
                }
                prev_glyph_position = glyph.position;

                let mut finished_background: Option<(
                    Point<Pixels>,
                    (Hsla, Point<Pixels>, Pixels, bool, Option<(Hsla, [Pixels; 4])>),
                )> = None;
                if glyph.index >= run_end {
                    let mut style_run = decoration_runs.next();

                    // ignore style runs that apply to a partial glyph
                    while let Some(run) = style_run {
                        if glyph.index < run_end + (run.len as usize) {
                            break;
                        }
                        run_end += run.len as usize;
                        style_run = decoration_runs.next();
                    }

                    if let Some(style_run) = style_run {
                        if let Some((_, background_color)) = &mut current_background
                            && style_run.background_color.as_ref() != Some(&background_color.0)
                        {
                            finished_background = current_background.take();
                        }
                        if let Some(run_background) = style_run.background_color {
                            current_background.get_or_insert((
                                point(glyph_origin.x, glyph_origin.y),
                                (
                                    run_background,
                                    style_run.background_pad,
                                    style_run.background_radius,
                                    // Поле слева — только у начала прогона:
                                    // на переносе подсветка продолжается.
                                    true,
                                    style_run.background_border,
                                ),
                            ));
                        }
                        run_end += style_run.len as usize;
                    } else {
                        run_end = layout.len;
                        finished_background = current_background.take();
                    }
                }

                if let Some((mut background_origin, background_color)) = finished_background {
                    let mut width = glyph_origin.x - background_origin.x;
                    if background_origin.x == glyph_origin.x {
                        background_origin.x -= max_glyph_size.width.half();
                    };
                    window.paint_quad(run_background_quad(
                        background_origin,
                        width,
                        line_height,
                        layout.font_size,
                        background_color.0,
                        background_color.1,
                        background_color.2,
                        background_color.3,
                        true,
                        background_color.4,
                    ));
                }
            }
        }

        let mut last_line_end_x = origin.x + layout.width;
        if let Some(boundary) = wrap_boundaries.last() {
            let run = &layout.runs[boundary.run_ix];
            let glyph = &run.glyphs[boundary.glyph_ix];
            last_line_end_x -= glyph.position.x;
        }

        if let Some((mut background_origin, background_color)) = current_background.take() {
            if last_line_end_x == background_origin.x {
                background_origin.x -= max_glyph_size.width.half()
            };
            window.paint_quad(run_background_quad(
                background_origin,
                last_line_end_x - background_origin.x,
                line_height,
                layout.font_size,
                background_color.0,
                background_color.1,
                background_color.2,
                background_color.3,
                true,
                background_color.4,
            ));
        }

        Ok(())
    })
}

/// KaminIDE patch: прямоугольник фона прогона (`<span>` с фоном внутри строки).
///
/// Браузер рисует такой фон по коробке содержимого — высотой в кегль с
/// выносными, а не во всю строку, — и раздвигает её внутренними отступами.
/// Прежний квад занимал строку целиком, поэтому подсветка получалась выше и
/// уже браузерной.
#[allow(clippy::too_many_arguments)]
fn run_background_quad(
    origin: Point<Pixels>,
    width: Pixels,
    line_height: Pixels,
    font_size: Pixels,
    color: Hsla,
    pad: Point<Pixels>,
    radius: Pixels,
    pad_left: bool,
    pad_right: bool,
    border: Option<(Hsla, [Pixels; 4])>,
) -> crate::PaintQuad {
    // 1.16 кегля — высота коробки содержимого у типовых интерфейсных
    // шрифтов; она же центрируется в строке, как половинный интерлиньяж.
    let band = font_size * 1.16 + pad.y * 2.0;
    let top = origin.y + (line_height - band).half();
    // Поля стоят на КОНЦАХ прогона: на переносе подсветка идёт впритык, иначе
    // она вылезала бы за край колонки с обеих сторон каждой строки.
    let left = if pad_left { pad.x } else { px(0.) };
    let right = if pad_right { pad.x } else { px(0.) };
    let quad = crate::fill(
        Bounds {
            origin: point(origin.x - left, top),
            size: size(width + left + right, band),
        },
        color,
    )
    .corner_radii(crate::Corners::all(radius));
    // Рамка строчной коробки: у КУСКА строки она своя, поэтому рисуется тем же
    // прямоугольником, что и фон. На переносе боковые грани не ставятся —
    // коробка продолжается на следующей строке.
    match border {
        // KaminIDE patch: ширины по сторонам [верх, право, низ, лево] —
        // строчная коробка бывает с частичной рамкой (`border-left` у
        // первого куска). На переносе боковые грани не ставятся.
        Some((border_color, w)) => crate::PaintQuad {
            border_widths: crate::Edges {
                top: w[0],
                right: if pad_right { w[1] } else { px(0.) },
                bottom: w[2],
                left: if pad_left { w[3] } else { px(0.) },
            },
            border_color,
            ..quad
        },
        None => quad,
    }
}

fn aligned_origin_x(
    origin: Point<Pixels>,
    align_width: Pixels,
    last_glyph_x: Pixels,
    align: &TextAlign,
    layout: &LineLayout,
    wrap_boundary: Option<&&WrapBoundary>,
) -> Pixels {
    let end_of_line = if let Some(WrapBoundary { run_ix, glyph_ix }) = wrap_boundary {
        layout.runs[*run_ix].glyphs[*glyph_ix].position.x
    } else {
        layout.width
    };

    let line_width = end_of_line - last_glyph_x;

    match align {
        // Выключка начинается от левого края: остаток раздают пробелы внутри
        // строки, а не сдвиг всей строки (см. `justify_extras`).
        TextAlign::Left | TextAlign::Justify => origin.x,
        TextAlign::Center => (origin.x * 2.0 + align_width - line_width) / 2.0,
        TextAlign::Right => origin.x + align_width - line_width,
    }
}
