//! Своя строчная раскладка: разбиение абзаца на строки по правилам CSS.
//!
//! Переносчик GPUI знает одно правило — рвать по границам слов — и обойти его
//! снаружи можно лишь подсказками (нулевой пробел, словосоединитель). Этого
//! хватает на простые случаи и НЕ хватает на те, где ширина и точка разрыва
//! связаны:
//!
//! * `white-space: pre-wrap` — пробел в конце строки ВИСИТ за краем: место
//!   занимает, а перенос не вызывает;
//! * `white-space: break-spaces` — тот же пробел место занимает И даёт точку
//!   разрыва после себя;
//! * `overflow-wrap: break-word` — слово рвётся ТОЛЬКО если иначе не влезает;
//! * `line-break: anywhere` — разрыв где угодно, поверх запретов типографики;
//! * двунаправленный текст — знаки набираются в логическом порядке, а на экран
//!   идут в видимом, причём переставлять надо ГОТОВЫЕ прогоны, иначе рвётся
//!   арабская вязь.
//!
//! Поэтому строки считаются здесь: один раз меряется вся строка (`layout_line`
//! даёт положение каждого знака), по мере накопления ширины выбираются точки
//! разрыва, а на отрисовке каждая строка набирается своим `shape_line` и
//! рисуется на своём месте.

use gpui::{
    AnyElement, App, Bounds, Element, ElementId, GlobalElementId, Hitbox, HitboxBehavior, Hsla,
    InspectorElementId, IntoElement, LayoutId, MouseButton, MouseDownEvent, MouseMoveEvent,
    MouseUpEvent, Pixels, Point, SharedString, TextRun, Window, point, px, size,
};

/// Правила переноса, собранные из CSS.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Wrap {
    /// `white-space: nowrap`/`pre` — мягких переносов нет вовсе.
    pub nowrap: bool,
    /// `white-space: break-spaces` — пробел занимает место и даёт разрыв.
    pub break_spaces: bool,
    /// `word-break: break-all` — разрыв между любыми знаками слова.
    pub break_all: bool,
    /// `line-break: anywhere` — разрыв где угодно, поверх запретов.
    pub anywhere: bool,
    /// `word-break: keep-all` — иероглифы не рвутся, только по пробелам.
    pub keep_all: bool,
    /// `overflow-wrap: break-word` — рвать слово, только если иначе не влезает.
    pub break_word: bool,
    /// `overflow-wrap: anywhere` — рвёт слово И при подсчёте размера по
    /// минимальному содержимому, в отличие от `break-word`.
    pub wrap_anywhere: bool,
    /// `direction: rtl` — основное направление абзаца.
    pub rtl: bool,
    /// `text-wrap: balance` — строки абзаца выравниваются по длине.
    pub balance: bool,
    /// `white-space: pre*` — пробелы сохраняются, в начале строки не срезаются.
    pub keep_spaces: bool,
}

/// Куда прижимать строку.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum Align {
    #[default]
    Left,
    Center,
    Right,
    Justify,
}

/// Мягкий перенос: разрешение разорвать слово, своей ширины он не имеет.
const SOFT_HYPHEN: char = '\u{00ad}';

/// Знак обрыва строки по `line-clamp`.
const ELLIPSIS: &str = "…";

/// Абзац со своей раскладкой строк.
pub struct Paragraph {
    text: SharedString,
    runs: Vec<TextRun>,
    font_size: Pixels,
    line_height: Pixels,
    align: Align,
    /// Выключка последней строки (`text-align-last`), если задана.
    align_last: Option<Align>,
    /// `unicode-bidi: plaintext` — сторона письма выбирается для КАЖДОГО
    /// абзаца между жёсткими разрывами по его первому сильному знаку. В
    /// преформате такой абзац — это строка, поэтому и `start`/`end` у каждой
    /// строки свои (HTML ставит это правило на `dir="auto"`).
    plaintext: Option<crate::computed::TextAlign>,
    /// Строки в ОБРАТНОМ порядке (снизу вверх): у `vertical-lr` колонки идут
    /// слева направо, а поворот по часовой кладёт ПЕРВУЮ строку правой —
    /// подача снизу вверх возвращает ей левую колонку.
    lines_reversed: bool,
    /// `line-clamp`: сколько строк показывать, остальные обрываются.
    clamp: Option<usize>,
    /// `text-overflow: ellipsis` контейнера с обрезкой.
    text_overflow: bool,
    /// `text-fit`: подбор кегля под ширину коробки.
    fit: Option<crate::computed::TextFit>,
    /// Шаг позиций табуляции (`tab-size` в точках).
    tab_stop: Pixels,
    /// Чем показывать перенос слова (`hyphenate-character`).
    hyphen: SharedString,
    /// Ширина этого знака — считается при раскладке, где есть окно.
    hyphen_w: std::cell::Cell<Pixels>,
    /// Куски ВНЕ потока: байтовое место в тексте → элемент. Рисуются поверх
    /// строк, места в них не занимают.
    overlays: Vec<(usize, AnyElement)>,
    /// Трекинг (`letter-spacing`): добавка к каждому знаку.
    letter_spacing: Pixels,
    /// `word-spacing` — добавка к КАЖДОМУ пробелу. Шейпер о ней не знает,
    /// поэтому она добавляется к положению знака: сколько пробелов позади,
    /// столько добавок.
    word_spacing: Pixels,
    /// Вертикальное письмо: строка идёт СВЕРХУ ВНИЗ, а строки набегают по
    /// горизонтали. Абзац при этом остаётся обычным элементом раскладки —
    /// ограничение приходит от родителя по нужной оси, а не выдумывается.
    vertical: bool,
    /// `vertical-rl` — строки набегают справа налево.
    vertical_rl: bool,
    /// Предел строки для ОРТОГОНАЛЬНОГО потока: ось строки абзаца совпала с
    /// осью потока родителя, а та не ограничена. По CSS Writing Modes §7.3
    /// предел берётся от ближайшего предка-контейнера прокрутки, а при его
    /// отсутствии — от начального содержащего блока, то есть от окна.
    ortho_limit: Option<Pixels>,
    /// Какая пунктуация свисает за край (`hanging-punctuation`).
    hanging: crate::computed::Hanging,
    /// Отступ первой строки (`text-indent`).
    indent: Indent,
    /// Места знаков-распорок (`inline::SPACER`) — байтовые смещения по
    /// возрастанию. Точки переноса считаются по тексту без них.
    spacers: Vec<usize>,
    /// Опознание абзаца для памяти выделения. Без него абзац не выделяется:
    /// состояние между кадрами хранит раскладка по этому ключу.
    id: Option<ElementId>,
    /// Цвет подложки выделенного куска.
    highlight: Hsla,
    wrap: Wrap,
    /// Правила переноса ПО КУСКАМ: `word-break` или `overflow-wrap`, заданные
    /// на вложенном `<span>`, действуют только на его байты. Пусто — значит
    /// весь абзац живёт по одному правилу.
    spans: Vec<(std::ops::Range<usize>, Wrap)>,
    /// Межсловный интервал ПО КУСКАМ: `word-spacing` на вложенном `<span>`
    /// действует только на пробелы внутри него. Пусто — значит на весь абзац
    /// один интервал.
    word_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Трекинг ПО КУСКАМ: `letter-spacing` на вложенном `<span>` действует
    /// только на его знаки. Набор принимает трекинг скаляром, поэтому разница
    /// с общим значением добавляется к положению знака, а слово набирается
    /// своим трекингом.
    letter_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Сдвиг куска по вертикали (`vertical-align: super`/`sub`): смещение
    /// базовой линии в точках, вниз положительное.
    shift_spans: Vec<(std::ops::Range<usize>, Pixels)>,
    /// Границы строк в байтах — считаются на замере, переиспользуются на
    /// отрисовке.
    lines: Vec<Line>,
}

/// Строка: что рисовать и сколько она занимает.
#[derive(Clone, Debug)]
struct Line {
    /// Байтовый отрезок исходного текста.
    range: std::ops::Range<usize>,
    /// Ширина без хвостовых пробелов — по ней идёт выключка.
    width: Pixels,
    /// Строка оборвана `line-clamp`: за её текстом рисуется многоточие.
    ellipsis: bool,
    /// Строка кончилась мягким переносом: за ней рисуется знак переноса.
    hyphen: bool,
    /// Отступ строки (`text-indent`). Отрицательный выводит строку за край.
    indent: Pixels,
}

/// `text-indent`: отступ первой строки блока.
///
/// Значение хранится ДВУМЯ частями. Абсолютную (`px`) считает разбор стилей,
/// а доля (`pct`) берётся от ширины содержащего блока и известна только там,
/// где ширина уже решена, — в раскладке строк.
#[derive(Default, Clone, Copy, PartialEq)]
pub struct Indent {
    /// Абсолютная часть в точках; отрицательная выводит строку за край.
    pub px: f32,
    /// Доля ширины строки: `10%` — это `0.1`.
    pub pct: f32,
    /// `each-line`: отступ повторяется после каждого жёсткого разрыва.
    pub each_line: bool,
    /// `hanging`: отступ получают все строки, КРОМЕ той, что получила бы его.
    pub hanging: bool,
}

impl Paragraph {
    pub fn new(
        text: SharedString,
        runs: Vec<TextRun>,
        font_size: Pixels,
        line_height: Pixels,
        align: Align,
        wrap: Wrap,
    ) -> Self {
        Paragraph {
            text,
            runs,
            font_size,
            line_height,
            align,
            align_last: None,
            plaintext: None,
            lines_reversed: false,
            letter_spacing: px(0.),
            word_spacing: px(0.),
            vertical: false,
            vertical_rl: false,
            ortho_limit: None,
            hanging: crate::computed::Hanging::default(),
            indent: Indent::default(),
            spacers: Vec::new(),
            id: None,
            highlight: Hsla::default(),
            wrap,
            spans: Vec::new(),
            word_spans: Vec::new(),
            letter_spans: Vec::new(),
            shift_spans: Vec::new(),
            lines: Vec::new(),
            clamp: None,
            text_overflow: false,
            fit: None,
            tab_stop: px(8. * 8.),
            hyphen: SharedString::from("\u{2010}"),
            hyphen_w: std::cell::Cell::new(px(0.)),
            overlays: Vec::new(),
        }
    }

    /// Пустой абзац — только чтобы на миг занять место настоящего, пока тот
    /// рисуется в повёрнутой системе координат.
    fn empty() -> Self {
        Paragraph::new(
            SharedString::default(),
            Vec::new(),
            px(0.),
            px(0.),
            Align::Left,
            Wrap::default(),
        )
    }

    /// Сдвиг кусков по вертикали: отрезок байт → смещение базовой линии.
    pub fn shift_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.shift_spans = spans;
        self
    }

    /// Трекинг по кускам: отрезок байт → своё значение.
    pub fn letter_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.letter_spans = spans;
        self
    }

    /// Межсловный интервал по кускам: отрезок байт → своя добавка.
    pub fn word_spans(mut self, spans: Vec<(std::ops::Range<usize>, Pixels)>) -> Self {
        self.word_spans = spans;
        self
    }

    /// Правила переноса по кускам: отрезок байт → своё правило.
    pub fn spans(mut self, spans: Vec<(std::ops::Range<usize>, Wrap)>) -> Self {
        self.spans = spans;
        self
    }

    /// Выключка последней строки — своя, если разметка её задала.
    /// `unicode-bidi: plaintext`: логическая выключка, которую надо решать по
    /// стороне КАЖДОЙ строки.
    pub fn plaintext(mut self, align: Option<crate::computed::TextAlign>) -> Self {
        self.plaintext = align;
        self
    }

    /// Рисовать строки снизу вверх (см. поле `lines_reversed`).
    pub fn reversed_lines(mut self, on: bool) -> Self {
        self.lines_reversed = on;
        self
    }

    pub fn align_last(mut self, align: Option<Align>) -> Self {
        self.align_last = align;
        self
    }

    /// Трекинг: добавка к каждому знаку (`letter-spacing`).
    pub fn letter_spacing(mut self, extra: Pixels) -> Self {
        self.letter_spacing = extra;
        self
    }

    pub fn word_spacing(mut self, extra: Pixels) -> Self {
        self.word_spacing = extra;
        self
    }

    /// Предел ортогонального потока (см. поле `ortho_limit`).
    pub fn ortho_limit(mut self, limit: Option<Pixels>) -> Self {
        self.ortho_limit = limit;
        self
    }

    /// Вертикальное письмо и сторона набегания строк.
    pub fn vertical(mut self, on: bool, rl: bool) -> Self {
        self.vertical = on;
        self.vertical_rl = rl;
        self
    }

    /// Разрешить выделение мышью: абзац заводит своё состояние и область
    /// попадания.
    pub fn selectable(mut self, id: ElementId, highlight: Hsla) -> Self {
        self.id = Some(id);
        self.highlight = highlight;
        self
    }

    /// Свисающая пунктуация (`hanging-punctuation`).
    pub fn hanging(mut self, hanging: Option<crate::computed::Hanging>) -> Self {
        self.hanging = hanging.unwrap_or_default();
        self
    }

    /// Места знаков-распорок строчных коробок.
    pub fn spacers(mut self, spacers: Vec<usize>) -> Self {
        self.spacers = spacers;
        self
    }

    /// Отступ первой строки (`text-indent`).
    pub fn indent(mut self, indent: Indent) -> Self {
        self.indent = indent;
        self
    }

    /// Отступ ЭТОЙ строки в точках.
    ///
    /// Доля считается от ширины строки (css-text-3 §7.1: процент берётся от
    /// ширины содержащего блока), поэтому предел приходит сюда: при замере по
    /// содержимому его нет, и доля обращается в ноль — как в браузере.
    fn indent_of(&self, head_of_part: bool, first_part: bool, limit: Option<Pixels>) -> Pixels {
        let own = if self.indent.each_line {
            head_of_part
        } else {
            head_of_part && first_part
        };
        if own == self.indent.hanging {
            return px(0.);
        }
        let pct = self.indent.pct * f32::from(limit.unwrap_or(px(0.)));
        px(self.indent.px + pct)
    }

    /// Сколько байт в начале строки свисает за левый край.
    ///
    /// Свисает только открывающий знак и только в начале ПЕРВОЙ строки
    /// абзаца: место он занимает в поле, а не в колонке, поэтому в ширину
    /// строки не входит.
    fn hang_first(&self, start: usize) -> usize {
        if !self.hanging.first || start != 0 {
            return 0;
        }
        match self.text[start..].chars().next() {
            Some(ch) if is_opening(ch) => ch.len_utf8(),
            _ => 0,
        }
    }

    /// Сколько байт в конце строки свисает за правый край.
    fn hang_last(&self, end: usize, closing_line: bool, over: bool) -> usize {
        let Some(ch) = self.text[..end].chars().next_back() else {
            return 0;
        };
        let hangs = (self.hanging.last && closing_line && is_closing(ch))
            || (self.hanging.force_end && is_stop(ch))
            // `allow-end` свисает ТОЛЬКО когда строка иначе не влезает —
            // в отличие от `force-end`, который свисает всегда. Пока разницы
            // не было, строки рвались на знак позже, чем надо.
            || (self.hanging.allow_end && over && is_stop(ch));
        if hangs { ch.len_utf8() } else { 0 }
    }

    /// Куски между обязательными разрывами: набор не принимает перевод строки,
    /// поэтому мерить приходится по кускам, а положения знаков сшивать.
    ///
    /// Результат запоминается: раскладка спрашивает размер абзаца по многу раз
    /// за кадр (перебор ширин в гибком контейнере), а набор строки — самая
    /// дорогая операция здесь.
    fn measure(&self, window: &mut Window) -> Vec<Seg> {
        let key = self.measure_key();
        if let Some(hit) = MEASURED.with(|c| {
            c.borrow()
                .iter()
                .find(|(k, _)| *k == key)
                .map(|(_, v)| v.clone())
        }) {
            return hit;
        }
        let out = self.measure_uncached(window);
        MEASURED.with(|c| {
            let mut cache = c.borrow_mut();
            if cache.len() >= MEASURE_CACHE {
                cache.remove(0);
            }
            cache.push((key, out.clone()));
        });
        out
    }

    /// Ключ памяти замера: от чего зависит положение знаков.
    fn measure_key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.text.hash(&mut h);
        f32::from(self.font_size).to_bits().hash(&mut h);
        f32::from(self.letter_spacing).to_bits().hash(&mut h);
        f32::from(self.word_spacing).to_bits().hash(&mut h);
        f32::from(self.tab_stop).to_bits().hash(&mut h);
        for run in &self.runs {
            run.len.hash(&mut h);
            run.font_size.map(|s| f32::from(s).to_bits()).hash(&mut h);
            run.font.family.hash(&mut h);
            run.font.weight.0.to_bits().hash(&mut h);
            (run.font.style as u8).hash(&mut h);
            // Возможности OpenType меняют и подстановку, и продвижение
            // (`vert`, `hwid`) — без них кэш отдавал чужой набор.
            for (tag, value) in run.font.features.tag_value_list() {
                tag.hash(&mut h);
                value.hash(&mut h);
            }
        }
        h.finish()
    }

    /// Ключ РАЗРЕЗА: замер плюс всё, от чего зависит перенос и ширины строк.
    fn split_key(&self, limit: Option<Pixels>) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut h = std::collections::hash_map::DefaultHasher::new();
        self.measure_key().hash(&mut h);
        limit.map(|l| f32::from(l).to_bits()).hash(&mut h);
        let w = &self.wrap;
        [
            w.nowrap,
            w.break_spaces,
            w.break_all,
            w.anywhere,
            w.keep_all,
            w.break_word,
            w.wrap_anywhere,
            w.rtl,
            w.balance,
            w.keep_spaces,
        ]
        .hash(&mut h);
        self.indent.px.to_bits().hash(&mut h);
        self.indent.pct.to_bits().hash(&mut h);
        self.indent.each_line.hash(&mut h);
        self.indent.hanging.hash(&mut h);
        let hg = &self.hanging;
        [hg.first, hg.last, hg.force_end, hg.allow_end].hash(&mut h);
        self.clamp.hash(&mut h);
        self.text_overflow.hash(&mut h);
        self.hyphen.hash(&mut h);
        self.spacers.hash(&mut h);
        for (r, v) in self.word_spans.iter().chain(&self.letter_spans) {
            r.start.hash(&mut h);
            r.end.hash(&mut h);
            f32::from(*v).to_bits().hash(&mut h);
        }
        h.finish()
    }

    fn measure_uncached(&self, window: &mut Window) -> Vec<Seg> {
        let mut out = Vec::new();
        let mut start = 0usize;
        let mut offset = px(0.);
        loop {
            // Кусок кончается переводом строки ИЛИ табуляцией: табуляция — не
            // знак со своей шириной, а прыжок к следующей позиции табуляции, и
            // отдавать её набору нечего (`break-spaces-tab`).
            let end = self.text[start..]
                .find(['\n', '\t', SOFT_HYPHEN])
                .map(|i| start + i)
                .unwrap_or(self.text.len());
            let runs = slice_runs(&self.runs, &(start..end));
            let layout = window.text_system().layout_line_spaced(
                &self.text[start..end],
                self.font_size,
                &runs,
                None,
                self.letter_spacing,
            );
            let width = layout.width;
            out.push(Seg {
                start,
                end,
                layout,
                offset,
            });
            if end >= self.text.len() {
                break;
            }
            let mark = self.text[end..].chars().next().unwrap_or('\n');
            match mark {
                '\n' => offset = px(0.),
                // Мягкий перенос своей ширины не имеет: он лишь ПОЗВОЛЯЕТ
                // разрыв. Пока он доезжал до набора, шрифт давал ему ширину
                // дефиса, и слово рвалось раньше времени (`hyphens-manual-011`:
                // «Deoxy-ribo-» вместо «Deoxyribo-»).
                SOFT_HYPHEN => offset += width,
                _ => {
                    let x = f32::from(offset + width);
                    let step = f32::from(self.tab_stop).max(1.0);
                    offset = px((x / step).floor() * step + step);
                }
            }
            start = end + mark.len_utf8();
        }
        out
    }

    /// Ширина отрезка строки.
    ///
    /// Положения знаков считаются от начала своего куска, поэтому границу
    /// надо толковать по её роли: КОНЕЦ отрезка сразу за переводом строки —
    /// это конец прошлого куска, а НАЧАЛО с тем же индексом — начало нового.
    /// Иначе строка, открывающая новый кусок, получала ширину со знаком минус
    /// и уезжала за край коробки.
    fn span(&self, segs: &[Seg], from: usize, to: usize) -> Pixels {
        if to <= from {
            return px(0.);
        }
        let width = self.x_at(segs, to, Edge::End) - self.x_at(segs, from, Edge::Start);
        if width < px(0.) { px(0.) } else { width }
    }

    /// Положение знака от начала своего куска.
    fn x_at(&self, segs: &[Seg], i: usize, edge: Edge) -> Pixels {
        let i = i.min(self.text.len());
        let after_break = edge == Edge::End && i > 0 && self.text.as_bytes()[i - 1] == b'\n';
        let seg = if after_break {
            segs.iter().find(|s| s.end + 1 == i)
        } else {
            segs.iter().find(|s| i <= s.end)
        };
        let Some(seg) = seg.or_else(|| segs.last()) else {
            return px(0.);
        };
        let base = if i >= seg.end {
            seg.layout.width
        } else if i <= seg.start {
            px(0.)
        } else {
            seg.layout.x_for_index(i - seg.start)
        };
        // Сдвиг куска внутри строки: его задаёт табуляция перед ним.
        let mut base = base + seg.offset;
        if !self.letter_spans.is_empty() {
            let upto = i.min(seg.end);
            for (off, _) in self.text[seg.start..upto].char_indices() {
                let at = seg.start + off;
                if let Some((_, v)) = self.letter_spans.iter().find(|(r, _)| r.contains(&at)) {
                    base += *v - self.letter_spacing;
                }
            }
        }
        if self.word_spacing == px(0.) && self.word_spans.is_empty() {
            return base;
        }
        // Набор про `word-spacing` не знает: знак сдвинут на столько добавок,
        // сколько пробелов осталось позади него внутри куска. Добавка у
        // каждого пробела СВОЯ — заданная на том куске, в который он попал.
        let upto = i.min(seg.end);
        let mut extra = px(0.);
        for (off, _) in self.text[seg.start..upto]
            .char_indices()
            .filter(|(_, c)| word_separator(*c))
        {
            let at = seg.start + off;
            extra += self
                .word_spans
                .iter()
                .find(|(r, _)| r.contains(&at))
                .map(|(_, v)| *v)
                .unwrap_or(self.word_spacing);
        }
        base + extra
    }

    /// Ширина по минимальному содержимому — самый широкий кусок, который
    /// разорвать нельзя.
    ///
    /// Нужна раскладке: под неё она меряет высоту, когда ширина ещё не
    /// решена. Ноль тут не годится — по нулю строка рвётся на каждом знаке, и
    /// коробка выходит во много раз выше настоящей.
    fn min_content(&self, window: &mut Window) -> Pixels {
        let segs = self.measure(window);
        let mut best = px(0.);
        let mut start = 0usize;
        let mut chunk = |from: usize, to: usize, this: &Self| {
            let end = if this.wrap.break_spaces {
                to
            } else {
                trim_hanging(&this.text[from..to]) + from
            };
            let w = this.span(&segs, from, end);
            if w > best {
                best = w;
            }
        };
        // `overflow-wrap: anywhere` — единственное из семейства, что меняет
        // размер по минимальному содержимому: слово рвётся и здесь, поэтому
        // точками счёта становятся ВСЕ границы знаков (css-text-3 §5.5).
        let stops: Vec<Stop> = if self.wrap.wrap_anywhere {
            self.text
                .char_indices()
                .skip(1)
                .map(|(at, _)| Stop {
                    at,
                    mandatory: false,
                })
                .collect()
        } else {
            // Куски со своим `overflow-wrap: anywhere` добавляют границы
            // знаков только внутри себя.
            let mut stops = self.opportunities();
            for (range, w) in &self.spans {
                if !w.wrap_anywhere {
                    continue;
                }
                for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                    stops.push(Stop {
                        at: range.start + i,
                        mandatory: false,
                    });
                }
            }
            stops.sort_by_key(|s| (s.at, !s.mandatory));
            stops.dedup_by_key(|s| s.at);
            stops
        };
        for stop in stops {
            if stop.at <= start {
                continue;
            }
            chunk(start, stop.at, self);
            start = stop.at;
        }
        chunk(start, self.text.len(), self);
        best
    }

    /// Разбить текст на строки под заданную ширину и оборвать по `line-clamp`.
    ///
    /// Порядок важен: сперва обрыв, потом выравнивание длин. Выровнять надо
    /// то, что ОСТАЛОСЬ видимым, и с учётом места, отнятого многоточием
    /// (`text-wrap-balance-line-clamp-003`).
    /// Разрез с памятью: раскладка гоняет его по 3-5 раз на абзац за кадр
    /// (min/max/definite у гибкого родителя + подготовка), а разрез — самое
    /// дорогое место резчика. Ключ обязан покрывать ВСЁ, что читает
    /// `split_uncached`, иначе устаревшие переносы сдвинут пиксели.
    fn split(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        // Ширина знака переноса взводится ДО обращения в память: при
        // попадании она нужна отрисовке, а считалась только внутри разреза —
        // свежий экземпляр абзаца оставался с нулём.
        if self.hyphen_w.get() == px(0.)
            && !self.hyphen.is_empty()
            && self.text.contains('\u{00ad}')
        {
            let mark = self.hyphen.clone();
            self.hyphen_w.set(self.suffix_width(&mark, 0, window));
        }
        // Подбор кегля мутирует абзац между вызовами — ключ это видит
        // (font_size в ключе замера).
        let key = self.split_key(limit);
        if let Some(hit) = SPLITS.with(|c| c.borrow().get(&key).cloned()) {
            return (*hit).clone();
        }
        let lines = self.split_uncached(limit, window);
        SPLITS.with(|c| {
            let mut m = c.borrow_mut();
            // Прямолинейный сброс при переполнении: страница с тысячами
            // абзацев дороже промахов одного сброса.
            if m.len() >= 2048 {
                m.clear();
            }
            m.insert(key, std::rc::Rc::new(lines.clone()));
        });
        lines
    }

    fn split_uncached(&self, limit: Option<Pixels>, window: &mut Window) -> Vec<Line> {
        let segs = self.measure(window);
        let mut lines = self.lay(limit, &segs);
        // Обрезка строк контейнером с `text-overflow: ellipsis`: не влезшая
        // строка усекается с многоточием (css-overflow-3 §text-overflow).
        if self.text_overflow
            && let Some(limit) = limit
        {
            for line in lines.iter_mut() {
                if line.width > limit + px(0.5) && !line.ellipsis {
                    self.ellipsize(line, limit, &segs, window);
                }
            }
        }
        let cut = self
            .clamp
            .filter(|n| *n > 0 && lines.len() > *n)
            .zip(limit)
            .filter(|_| self.wrap.balance);
        let Some((max, limit)) = cut else {
            return self.clamp_lines(self.balanced(lines, limit, &segs), limit, window);
        };
        // Видимый текст — тот, что уместился в обрезанные строки. Его и
        // раскладываем заново, ища самую узкую колонку, в которой он всё ещё
        // помещается в те же строки.
        let end = self
            .clamp_lines(lines, Some(limit), window)
            .last()
            .map(|l| l.range.end)
            .unwrap_or(0);
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            let probe = self.lay(Some(middle), &segs);
            let fits = probe
                .get(max - 1)
                .is_some_and(|l: &Line| l.range.end >= end);
            if fits {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.clamp_lines(self.lay(Some(wide), &segs), Some(limit), window)
    }

    /// `line-clamp`: строк остаётся не больше заданного числа, а на последней
    /// появляется многоточие. Место под него отбирается у текста — иначе
    /// строка вылезала бы за коробку (`text-wrap-balance-line-clamp-003`).
    fn clamp_lines(
        &self,
        mut lines: Vec<Line>,
        limit: Option<Pixels>,
        window: &mut Window,
    ) -> Vec<Line> {
        let Some(max) = self.clamp.filter(|n| *n > 0) else {
            return lines;
        };
        if lines.len() <= max {
            return lines;
        }
        lines.truncate(max);
        let Some(last) = lines.pop() else {
            return lines;
        };
        let segs = self.measure(window);
        let ell = self.suffix_width(ELLIPSIS, last.range.start, window);
        let head = last.range.start;
        let mut end = head + trim_hanging(&self.text[last.range.clone()]);
        // Место под многоточие отбирается ЦЕЛЫМИ кусками: строка обрывается по
        // точке переноса, а не посреди слова. Слово, которое с многоточием уже
        // не влезает, уходит со строки целиком — как в браузере.
        if let Some(room) = limit.map(|w| w - ell) {
            if self.span(&segs, head, end) > room {
                end = self
                    .opportunities()
                    .iter()
                    .map(|s| s.at)
                    .filter(|at| *at > head && *at <= end)
                    .map(|at| head + trim_hanging(&self.text[head..at]))
                    .filter(|at| self.span(&segs, head, *at) <= room)
                    .max()
                    .unwrap_or(head);
            }
        }
        let width = self.span(&segs, head, end) + ell;
        lines.push(Line {
            range: head..end,
            width,
            ellipsis: true,
            hyphen: false,
            indent: last.indent,
        });
        lines
    }

    /// `line-clamp`: сколько строк оставить.
    pub fn line_clamp(mut self, lines: Option<usize>) -> Self {
        self.clamp = lines;
        self
    }

    /// `text-overflow: ellipsis` контейнера: строка, не влезшая в колонку
    /// (nowrap/pre — переносов нет), усекается и получает многоточие.
    pub fn text_ellipsis(mut self, on: bool) -> Self {
        self.text_overflow = on;
        self
    }

    /// Усечь строку под многоточие: место отбирается целыми кусками по
    /// точкам переноса — как у `line-clamp` (общая механика).
    fn ellipsize(&self, line: &mut Line, limit: Pixels, segs: &[Seg], window: &mut Window) {
        let ell = self.suffix_width(ELLIPSIS, line.range.start, window);
        let head = line.range.start;
        let mut end = head + trim_hanging(&self.text[line.range.clone()]);
        let room = limit - ell;
        if self.wrap.rtl {
            // Письмо справа налево: строка прижата вправо, контейнер режет
            // ЛЕВЫЙ край — усечение с ЛОГИЧЕСКОГО НАЧАЛА, многоточие там же.
            let mut start = head;
            if self.span(segs, head, end) > room {
                start = self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, *at, end) <= room)
                    .min()
                    .unwrap_or(end);
            }
            line.width = self.span(segs, start, end) + ell;
            line.range = start..end;
            line.ellipsis = true;
            return;
        }
        if self.span(segs, head, end) > room {
            let by_break = self
                .opportunities()
                .iter()
                .map(|s| s.at)
                .filter(|at| *at > head && *at <= end)
                .map(|at| head + trim_hanging(&self.text[head..at]))
                .filter(|at| self.span(segs, head, *at) <= room)
                .max();
            // Непереносимое слово режется ПО ЗНАКАМ: обрезка контейнером
            // не ждёт точки переноса (в отличие от line-clamp).
            end = by_break.unwrap_or_else(|| {
                self.text[head..end]
                    .char_indices()
                    .map(|(i, _)| head + i)
                    .filter(|at| *at > head && self.span(segs, head, *at) <= room)
                    .max()
                    .unwrap_or(head)
            });
        }
        line.width = self.span(segs, head, end) + ell;
        line.range = head..end;
        line.ellipsis = true;
    }

    /// Чем показывать перенос слова.
    pub fn hyphen_char(mut self, mark: Option<String>) -> Self {
        if let Some(mark) = mark {
            self.hyphen = SharedString::from(mark);
        }
        self
    }

    /// Шаг позиций табуляции.
    pub fn tab_stop(mut self, step: Pixels) -> Self {
        self.tab_stop = step;
        self
    }

    /// Куски вне потока: место в тексте → элемент.
    pub fn overlays(mut self, overlays: Vec<(usize, AnyElement)>) -> Self {
        self.overlays = overlays;
        self
    }

    /// Трекинг, который добавлен ПОСЛЕДНЕМУ знаку отрезка.
    ///
    /// По css-text-3 §8.2 межбуквенный интервал в конце строки не действует:
    /// он свисает за край и в ширину строки не входит. Пока входил, коробка
    /// шириной ровно в текст рвала последнее слово (`letter-spacing-200`).
    fn tail_spacing(&self, end: usize) -> Pixels {
        if end == 0 {
            return px(0.);
        }
        // Знак-распорка несёт ПОЛЕ строчной коробки, а не трекинг: вычитать
        // его на конце строки нельзя — иначе коробка теряет своё правое поле
        // и выходит уже на целый em (`word-space-transform-010`).
        if self.text[..end].chars().next_back() == Some('\u{feff}') {
            return px(0.);
        }
        let at = self.text[..end]
            .char_indices()
            .next_back()
            .map(|(i, _)| i)
            .unwrap_or(0);
        self.letter_spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, v)| *v)
            .unwrap_or(self.letter_spacing)
    }

    /// Где в коробке стоит байт текста: левый верхний угол его знака.
    fn point_of(&self, segs: &[Seg], at: usize, bounds: Bounds<Pixels>) -> Point<Pixels> {
        let row = self
            .lines
            .iter()
            .position(|l| at < l.range.end)
            .unwrap_or(self.lines.len().saturating_sub(1));
        let Some(line) = self.lines.get(row) else {
            return bounds.origin;
        };
        let from = self.x_at(segs, line.range.start, Edge::Start);
        let x = self.x_at(segs, at.max(line.range.start), Edge::Start) - from;
        point(
            bounds.origin.x + x,
            bounds.origin.y + self.line_height * row as f32,
        )
    }

    /// `text-fit`: подбирать ли кегль под ширину коробки.
    pub fn text_fit(mut self, fit: Option<crate::computed::TextFit>) -> Self {
        self.fit = fit;
        self
    }

    /// Подобрать кегль так, чтобы строки заполнили коробку (css-text-5).
    ///
    /// Считается по САМОЙ ШИРОКОЙ строке: увеличивать до тех пор, пока она не
    /// упрётся в край. Множитель идёт на всё, что задаёт размер набора, —
    /// кегль, интерлиньяж и разрядки, иначе строка растёт непропорционально.
    fn apply_fit(&mut self, limit: Pixels, window: &mut Window) {
        let Some(f) = self.fit.filter(|f| f.grow || f.shrink) else {
            return;
        };
        let lines = self.split(Some(limit), window);
        let widest = lines
            .iter()
            .map(|l| l.width)
            .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
        if widest <= px(0.) || limit <= px(0.) {
            return;
        }
        let k = f32::from(limit) * f.target / f32::from(widest);
        if !k.is_finite() || (k > 1.0 && !f.grow) || (k < 1.0 && !f.shrink) {
            return;
        }
        if (k - 1.0).abs() < 0.001 {
            return;
        }
        remember_fit(self.measure_key(), k);
        self.scale_by(k);
    }

    /// Множитель, найденный ЗАМЕРОМ для этого же абзаца.
    ///
    /// Подбирать кегль имеет смысл только под заданный размер строки, а
    /// известен он лишь замеру: отрисовке коробка достаётся уже посчитанной, и
    /// по ней подбор пошёл бы по кругу (`text-fit/writing-mode`: кегль
    /// вырастал в размер окна).
    fn apply_measured_fit(&mut self) {
        if self.fit.is_none() {
            return;
        }
        let key = self.measure_key();
        if let Some(k) = FITTED.with(|c| {
            c.borrow()
                .iter()
                .find(|(hit, _)| *hit == key)
                .map(|(_, k)| *k)
        }) {
            self.scale_by(k);
        }
        self.fit = None;
    }

    /// Помножить всё, что задаёт размер набора.
    fn scale_by(&mut self, k: f32) {
        self.font_size = px(f32::from(self.font_size) * k);
        self.line_height = px(f32::from(self.line_height) * k);
        self.letter_spacing = px(f32::from(self.letter_spacing) * k);
        self.word_spacing = px(f32::from(self.word_spacing) * k);
        for run in &mut self.runs {
            if let Some(size) = run.font_size {
                run.font_size = Some(px(f32::from(size) * k));
            }
        }
    }

    /// Ширина многоточия в наборе того куска, где оборвана строка.
    fn suffix_width(&self, mark: &str, at: usize, window: &mut Window) -> Pixels {
        let mut runs = slice_runs(&self.runs, &(at..at + 1));
        let Some(run) = runs.first_mut() else {
            return px(0.);
        };
        run.len = mark.len();
        let piece = vec![run.clone()];
        window
            .text_system()
            .shape_line_spaced(
                SharedString::from(mark.to_string()),
                self.font_size,
                &piece,
                None,
                self.letter_spacing,
            )
            .width
    }

    /// `text-wrap: balance` — те же строки, но одной длины.
    fn balanced(&self, lines: Vec<Line>, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        let Some(limit) = limit else { return lines };
        if !self.wrap.balance || lines.len() < 2 {
            return lines;
        }
        // Группы строк, разделённые ЖЁСТКИМ разрывом, выравниваются по
        // отдельности (css-text-4 §7.1): у каждой своя ширина, одной на весь
        // абзац не хватает (`text-wrap-balance-004`).
        let mut out: Vec<Line> = Vec::new();
        let mut i = 0usize;
        while i < lines.len() {
            let last = lines[i..]
                .iter()
                .position(|l| self.text[l.range.clone()].ends_with('\n'))
                .map(|k| i + k)
                .unwrap_or(lines.len() - 1);
            let part = lines[i].range.start..lines[last].range.end;
            out.extend(self.balanced_part(part, last + 1 - i, limit, segs));
            i = last + 1;
        }
        out
    }

    /// Выравнивание длин ОДНОЙ группы строк: поиск самой узкой колонки, в
    /// которой строк не прибавилось. Тогда последняя строка перестаёт быть
    /// коротким огрызком.
    fn balanced_part(
        &self,
        part: std::ops::Range<usize>,
        target: usize,
        limit: Pixels,
        segs: &[Seg],
    ) -> Vec<Line> {
        if target < 2 {
            return self.lay_in(part, Some(limit), segs);
        }
        let (mut narrow, mut wide) = (px(0.), limit);
        for _ in 0..12 {
            let middle = (narrow + wide) / 2.;
            if self.lay_in(part.clone(), Some(middle), segs).len() <= target {
                wide = middle;
            } else {
                narrow = middle;
            }
        }
        self.lay_in(part, Some(wide), segs)
    }

    /// Набор строк под заданную ширину — без выравнивания их длин.
    fn lay(&self, limit: Option<Pixels>, segs: &[Seg]) -> Vec<Line> {
        self.lay_in(0..self.text.len(), limit, segs)
    }

    /// То же для ЧАСТИ текста: выравнивание длин идёт по группам между
    /// жёсткими разрывами, и каждая группа набирается своей ширины.
    fn lay_in(
        &self,
        part: std::ops::Range<usize>,
        limit: Option<Pixels>,
        segs: &[Seg],
    ) -> Vec<Line> {
        let x = |i: usize| -> Pixels { self.x_at(segs, i, Edge::End) };
        let mut out: Vec<Line> = Vec::new();
        // Начало строки: схлопываемые пробелы после переноса не рисуются и в
        // ширину не входят. При сохранённых пробелах (`pre*`) они значимы.
        // Правило берётся В ЭТОМ МЕСТЕ, а не у абзаца целиком: `white-space`
        // на вложенном `<span>`/`display: inline` действует на свои знаки, и
        // абзац об этом не знает. Пока смотрели правило абзаца, сохранённые
        // пробелы вложенного куска исчезали с начала перенесённой строки
        // (`ws-break-spaces-applies-to-001`).
        let bol = |at: usize| -> usize {
            if self.wrap_at(at).keep_spaces {
                at
            } else {
                at + skip_leading(&self.text[at..])
            }
        };
        let mut start = bol(part.start);
        // Отступ первой строки (`text-indent`) — свойство СТРОКИ, а не абзаца:
        // его получает первая строка блока, при `each-line` — первая после
        // каждого жёсткого разрыва, при `hanging` — все остальные. Поэтому
        // здесь ведётся, начинает ли строка кусок и первый ли это кусок блока:
        // группы между жёсткими разрывами набираются и по отдельности
        // (выравнивание длин), и подряд в одном проходе.
        let mut head_of_part = true;
        let mut first_part = part.start == 0;
        let mut last_fit: Option<usize> = None;
        let mut opportunities: Vec<Stop> = self
            .opportunities()
            .into_iter()
            .filter(|s| s.at > part.start && s.at <= part.end)
            .collect();
        // Конец текста — тоже точка проверки: без него хвост последней строки
        // никто не мерил и она оставалась во всю длину, сколько бы ни
        // переполняла коробку.
        if opportunities.last().is_none_or(|s| s.at < part.end) {
            opportunities.push(Stop {
                at: part.end,
                mandatory: false,
            });
        }
        let mut i = 0usize;
        while i < opportunities.len() {
            let Stop { at, mandatory } = opportunities[i];
            if at <= start {
                i += 1;
                continue;
            }
            // Отступ отбирает место у СВОЕЙ строки: на неё остаётся уже
            // меньшая ширина, а отрицательный отступ, наоборот, добавляет.
            let ind = self.indent_of(head_of_part, first_part, limit);
            let limit = limit.map(|w| w - ind);
            // Хвостовые пробелы висят за краем: в ширину строки они не входят.
            // Хвостовые пробелы висят за краем СТРОКИ — то есть когда край
            // вообще есть. При замере по максимальному содержимому предела
            // нет, и сохранённый пробел в ширину ВХОДИТ (`pre-wrap-017`:
            // коробка `width: max-content` выходила на знак уже).
            let measured = if self.wrap.break_spaces
                || (limit.is_none() && self.wrap.keep_spaces)
            {
                at
            } else {
                trim_hanging(&self.text[start..at]) + start
            };
            // Свисающее за края в ширину строки не входит — ни открывающий
            // знак в начале, ни точка с запятой в конце.
            let head = start + self.hang_first(start);
            // Свисает ли знак — зависит от того, влезает ли строка БЕЗ него;
            // поэтому ширина считается дважды: сначала без свисания.
            let bare = self.span(&segs, head, measured);
            let tight = limit.is_some_and(|w| bare > w);
            let tail_hang = self.hang_last(measured, at >= part.end, tight);
            let mut width = self.span(&segs, head, measured - tail_hang)
                - self.tail_spacing(measured - tail_hang);
            // Строка, кончающаяся мягким переносом, несёт ещё и знак переноса.
            if self.text[..measured].ends_with('\u{00ad}') {
                width += self.hyphen_w.get();
            }
            // Допуск в сотую точки: ширина строки складывается из замеров
            // кусков и знака переноса, и на ТОЧНОМ совпадении с коробкой
            // накопленная ошибка решала исход (`hyphens-manual-011`: строка,
            // влезающая ровно, уходила на перенос).
            let over = limit.is_some_and(|w| f32::from(width) > f32::from(w) + 0.01);
            // Обязательный разрыв проверяется ПОСЛЕ переполнения: до него
            // строка может не влезать, и тогда сперва переносится она.
            // Раньше кусок перед переводом строки уходил в строку целиком,
            // сколько бы ни переполнял коробку (`pre-wrap-leading-spaces`).
            if mandatory && !over {
                out.push(Line {
                    range: start..at,
                    width,
                    ellipsis: false,
                    hyphen: false,
                    indent: ind,
                });
                start = bol(at);
                // За жёстким разрывом начинается новый кусок: при `each-line`
                // отступ повторяется, но «первым куском блока» он уже не будет.
                head_of_part = true;
                first_part = false;
                last_fit = None;
                i += 1;
                continue;
            }
            if over {
                // Переносим по последней подошедшей точке; если её нет —
                // рвём по знакам, но только когда это разрешено.
                let cut = last_fit.filter(|c| *c > start).unwrap_or_else(|| {
                    // Разрешение рвать слово берётся ПО МЕСТУ переполнения:
                    // `overflow-wrap` на вложенном `<span>` действует только
                    // на его знаки.
                    // Разрешение берётся ПО МЕСТУ, где строка переполнилась,
                    // а не по её началу: `overflow-wrap` на вложенном
                    // `<span>` действует на свои знаки, и кусок этот обычно
                    // начинается посреди строки
                    // (`overflow-wrap-anywhere-inline-*`).
                    // `white-space: nowrap` запрещает и аварийный разрыв:
                    // `overflow-wrap` действует, только когда перенос вообще
                    // разрешён (`overflow-wrap-002`).
                    if self.emergency_ok(start) || self.emergency_ok(at.saturating_sub(1)) {
                        self.cut_by_char(start, at, limit, &x)
                    } else {
                        at
                    }
                });
                let tail = if self.wrap.break_spaces {
                    cut
                } else {
                    trim_hanging(&self.text[start..cut]) + start
                };
                let tail = tail - self.hang_last(tail, false, true);
                // Разрыв по мягкому переносу: на строке остаётся знак
                // переноса, и он же входит в её ширину.
                let hyphen = self.text[..cut].ends_with('\u{00ad}');
                let extra = if hyphen { self.hyphen_w.get() } else { px(0.) };
                out.push(Line {
                    range: start..cut,
                    width: self.span(&segs, head, tail) - self.tail_spacing(tail) + extra,
                    ellipsis: false,
                    hyphen,
                    indent: ind,
                });
                start = bol(cut);
                // Мягкий перенос кусок не кончает: следующая строка отступа
                // не получает (кроме `hanging`, где его получают именно они).
                head_of_part = false;
                last_fit = None;
                // Ту же точку проверяем заново от нового начала строки: за
                // одним переносом может идти следующий.
                continue;
            }
            last_fit = Some(at);
            i += 1;
        }
        if start < part.end || out.is_empty() {
            let end = part.end;
            // Тот же довод, что и в цикле: висеть пробелу можно только за
            // КРАЕМ, а при замере по максимальному содержимому края нет
            // (`pre-wrap-017`).
            let tail = if self.wrap.break_spaces || (limit.is_none() && self.wrap.keep_spaces) {
                end
            } else {
                trim_hanging(&self.text[start..end]) + start
            };
            let head = start + self.hang_first(start);
            let tail = tail - self.hang_last(tail, true, true);
            out.push(Line {
                range: start..end,
                width: self.span(&segs, head, tail) - self.tail_spacing(tail),
                ellipsis: false,
                hyphen: false,
                indent: self.indent_of(head_of_part, first_part, limit),
            });
        }
        // Печать разреза строк: `HTML_LINES=1`. Себя окупила — ею нашлось,
        // что узел из идеографических пробелов не доезжает до раскладки
        // ВООБЩЕ (отбрасывался разбором). Когда след ведёт «строка пропала»,
        // смотреть надо сюда, а не в саму раскладку.
        if { static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("HTML_LINES").is_ok()); *ON } {
            eprintln!(
                "LINES fonts={:?} {:?} -> {:?}",
                self.runs
                    .iter()
                    .map(|r| r.font.family.to_string())
                    .collect::<Vec<_>>(),
                self.text,
                out.iter()
                    .map(|l| (l.range.clone(), f32::from(l.width)))
                    .collect::<Vec<_>>()
            );
        }
        out
    }

    /// Место разрыва внутри неразрывного куска — по знакам, до последнего
    /// влезающего.
    fn cut_by_char(
        &self,
        start: usize,
        end: usize,
        limit: Option<Pixels>,
        x: &dyn Fn(usize) -> Pixels,
    ) -> usize {
        let Some(limit) = limit else { return end };
        let from = x(start);
        let mut last = start;
        // Конец отрезка — тоже граница знака, и проверять его ОБЯЗАТЕЛЬНО:
        // без него разрез, у которого не влезал только последний знак,
        // возвращал весь отрезок целиком. При `break-spaces` это съедало
        // ведущий пробел следующей строки — он уезжал в конец предыдущей.
        let bounds = self.text[start..end]
            .char_indices()
            .map(|(i, _)| start + i)
            .chain(std::iter::once(end));
        for at in bounds {
            if at == start {
                continue;
            }
            // Рвать ВНУТРИ грозди знаков нельзя: огласовка, соединитель и
            // знак вариации принадлежат своей букве и в другую строку не
            // уходят (`overflow-wrap-cluster`: देवनागरी рвалась пополам).
            if !cluster_edge(&self.text, at) {
                continue;
            }
            // И только там, где аварийный разрыв РАЗРЕШЁН: у соседнего куска
            // правила могут быть другими.
            if !self.emergency_ok(at.saturating_sub(1)) && !self.emergency_ok(at) {
                continue;
            }
            // `word-break: break-all` рвёт между БУКВАМИ и запретов типографики
            // не отменяет: перед точкой и после знака-приставки строка не
            // рвётся даже в аварийном разрезе (`word-break-break-all-inline-008`
            // — «X» и «.» обязаны остаться вместе и вылезти за коробку).
            // Семейство `anywhere` — наоборот, рвёт где угодно.
            if !self.loose_at(at.saturating_sub(1)) && !self.loose_at(at) {
                let after = self.text[at..].chars().next();
                let before = self.text[..at].chars().next_back();
                if after.is_some_and(no_break_before) || before.is_some_and(no_break_after) {
                    continue;
                }
            }
            if x(at) - from > limit {
                return if last > start { last } else { at };
            }
            last = at;
        }
        end
    }

    /// Неперносима ли точка МЕЖДУ двумя знаками.
    ///
    /// Решает её общий предок (css-text-3 §5.1). У нас предки выражены
    /// диапазонами кусков: если оба знака в ОДНОМ куске, правило его; если в
    /// разных (или один вне кусков) — общий предок это сам абзац.
    fn nowrap_between(&self, at: usize) -> bool {
        let which = |i: usize| self.spans.iter().position(|(r, _)| r.contains(&i));
        let left = which(at.saturating_sub(1));
        let right = which(at);
        match (left, right) {
            (Some(a), Some(b)) if a == b => self.spans[a].1.nowrap,
            _ => self.wrap.nowrap,
        }
    }

    /// Рвётся ли на этом месте что угодно и где угодно — без оглядки на
    /// типографику (`line-break: anywhere`, `overflow-wrap: anywhere`,
    /// `word-wrap: break-word`).
    fn loose_at(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        w.anywhere || w.break_word || w.wrap_anywhere
    }

    /// Разрешён ли на этом месте аварийный разрыв по знакам.
    fn emergency_ok(&self, at: usize) -> bool {
        let w = self.wrap_at(at);
        !w.nowrap && (w.break_all || w.anywhere || w.break_word || w.wrap_anywhere)
    }

    /// Точки, где строку РАЗРЕШЕНО разорвать.
    /// Правила переноса, действующие на байте `at`: сначала свой кусок, потом
    /// абзац целиком.
    fn wrap_at(&self, at: usize) -> Wrap {
        self.spans
            .iter()
            .find(|(r, _)| r.contains(&at))
            .map(|(_, w)| *w)
            .unwrap_or(self.wrap)
    }

    /// Точки переноса по UAX-14 — по тексту БЕЗ знаков-распорок.
    ///
    /// Распорка (`inline::SPACER`) — не знак документа, а место под поля
    /// строчной коробки. Класс WJ запрещает разрыв и перед собой, поэтому
    /// пробел ПЕРЕД `<span>` с отступом переставал быть точкой переноса, и
    /// строка уходила за край коробки вместо переноса. Разрыв возвращается на
    /// место распорки: поле уезжает на новую строку вместе со своим текстом.
    fn linebreaks(&self) -> Vec<(usize, unicode_linebreak::BreakOpportunity)> {
        if self.spacers.is_empty() {
            return unicode_linebreak::linebreaks(&self.text).collect();
        }
        let mut clean = String::with_capacity(self.text.len());
        let mut map: Vec<usize> = Vec::with_capacity(self.text.len() + 1);
        let mut pending: Option<usize> = None;
        for (at, ch) in self.text.char_indices() {
            if self.spacers.binary_search(&at).is_ok() {
                pending.get_or_insert(at);
                continue;
            }
            map.push(pending.take().unwrap_or(at));
            for k in 1..ch.len_utf8() {
                map.push(at + k);
            }
            clean.push(ch);
        }
        map.push(self.text.len());
        unicode_linebreak::linebreaks(&clean)
            .map(|(at, kind)| (map.get(at).copied().unwrap_or(self.text.len()), kind))
            .collect()
    }

    fn opportunities(&self) -> Vec<Stop> {
        let mut out: Vec<Stop> = Vec::new();
        // Обязательные разрывы есть всегда, даже при `nowrap`.
        for (i, ch) in self.text.char_indices() {
            if ch == '\n' {
                out.push(Stop {
                    at: i + 1,
                    mandatory: true,
                });
            }
        }
        {
            if self.wrap.anywhere {
                for (i, _) in self.text.char_indices().skip(1) {
                    out.push(Stop {
                        at: i,
                        mandatory: false,
                    });
                }
            } else {
                // `line-break: anywhere` на вложенном куске: точки ставятся
                // только внутри него, остальной абзац живёт по UAX-14.
                for (range, w) in &self.spans {
                    if !w.anywhere {
                        continue;
                    }
                    for (i, _) in self.text[range.clone()].char_indices().skip(1) {
                        out.push(Stop {
                            at: range.start + i,
                            mandatory: false,
                        });
                    }
                }
                for (at, kind) in self.linebreaks() {
                    // Обязательные разрывы Юникода — это не только перевод
                    // строки: подача страницы, вертикальная табуляция,
                    // разделители строки и абзаца, NEL. Все они заканчивают
                    // строку принудительно (`line-breaking-022`).
                    match kind {
                        unicode_linebreak::BreakOpportunity::Allowed => out.push(Stop {
                            at,
                            mandatory: false,
                        }),
                        // Конец текста переносчик тоже зовёт обязательным
                        // разрывом — но переносить там нечего, а лишняя точка
                        // ломает счёт строк.
                        unicode_linebreak::BreakOpportunity::Mandatory
                            if at < self.text.len() =>
                        {
                            out.push(Stop {
                                at,
                                mandatory: true,
                            })
                        }
                        unicode_linebreak::BreakOpportunity::Mandatory => {}
                    }
                }
                {
                    // Разрыв разрешён между знаками слова, но запреты
                    // типографики он не отменяет: перед точкой, скобкой или
                    // знаком препинания рвать всё равно нельзя (это отличает
                    // `break-all` от `line-break: anywhere`). Правило берётся
                    // ПО МЕСТУ: заданное на вложенном `<span>`, оно действует
                    // только на его байты.
                    for (i, ch) in self.text.char_indices().skip(1) {
                        if !self.wrap_at(i).break_all {
                            continue;
                        }
                        let before = self.text[..i].chars().next_back();
                        let allowed = !ch.is_whitespace()
                            && !no_break_before(ch)
                            && before.is_none_or(|c| !no_break_after(c));
                        if allowed {
                            out.push(Stop {
                                at: i,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Мягкий перенос — ЯВНАЯ точка переноса: она сильнее
                    // запретов типографики. UAX-14 держит вместе перенос и
                    // следующую за ним кавычку (`hyphens-i18n-manual-003`:
                    // «tú­’àn» не рвалось вовсе).
                    for (i, ch) in self.text.char_indices() {
                        if ch == SOFT_HYPHEN {
                            out.push(Stop {
                                at: i + ch.len_utf8(),
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // После КАЖДОГО сохранённого пробела — своя точка разрыва.
                    // Табуляция тоже пробел: `break-spaces` рвёт и после неё,
                    // хотя UAX-14 держит подряд идущие табуляции вместе
                    // (`break-spaces-tab-003`).
                    for (i, ch) in self.text.char_indices() {
                        if matches!(ch, ' ' | '\t') && self.wrap_at(i).break_spaces {
                            out.push(Stop {
                                at: i + 1,
                                mandatory: false,
                            });
                        }
                    }
                }
                {
                    // Запрет действует только МЕЖДУ буквенными единицами:
                    // иероглифы друг от друга не отрываются, а после запятой
                    // или дефиса строка рвётся по-прежнему.
                    out.retain(|s| {
                        if !self.wrap_at(s.at).keep_all {
                            return true;
                        }
                        let before = self.text[..s.at].chars().next_back();
                        let after = self.text[s.at..].chars().next();
                        s.mandatory
                            || !(before.is_some_and(letter_unit) && after.is_some_and(letter_unit))
                    });
                }
            }
        }
        // `white-space: nowrap`/`pre` гасит МЯГКИЕ точки — но по месту, а не по
        // абзацу целиком: вложенный `<span>` со своим `white-space` переносится
        // внутри неперносимого абзаца и наоборот (`white-space-pre-031`).
        //
        // Точку между ДВУМЯ знаками решает их общий предок (css-text-3
        // §5.1), поэтому гасится она, только если неперносимы ОБЕ стороны.
        // Пока смотрели один знак слева, `<span style="white-space:pre">口</span>口`
        // не рвался на границе куска, хотя рвать там велит div-родитель
        // (`line-breaking-ic-001`).
        out.retain(|s| s.mandatory || !self.nowrap_between(s.at));
        // Внутри грозди знаков рвать нельзя НИКОГДА: составной знак (флаг,
        // смайлик с модификатором) — одна буква, и переносчик UAX-14 о его
        // устройстве не знает (`line-breaking-014`: радужный флаг рассыпался
        // на четыре строки).
        // Обязательный разрыв не снимается НИКОГДА: перевод строки обязан
        // закончить строку, иначе набор получает строку с переводом внутри и
        // падает на проверке (`text argument should not contain newlines`).
        // `line-break: anywhere` рвёт между ЛЮБЫМИ знаками, включая склеенные
        // соединителем нулевой ширины: класс ZWJ он тоже перекрывает
        // (`line-break-anywhere-overrides-uax-behavior-015`).
        out.retain(|s| {
            s.mandatory || cluster_edge_at(&self.text, s.at, self.wrap_at(s.at).anywhere)
        });
        // Знак перед числом держит следующий за собой: `$`, `£`, `\`. Таблица
        // пар UAX-14 в переносчике этого не знает и рвёт «XX XX\\\» между
        // обратными косыми (`word-break-break-all-023`), тогда как рвать
        // разрешено только ПЕРЕД первой из них.
        // `line-break: anywhere` и `overflow-wrap: anywhere` снимают запреты
        // типографики целиком — их точки остаются.
        out.retain(|s| {
            let w = self.wrap_at(s.at);
            s.mandatory
                || w.anywhere
                || w.wrap_anywhere
                || self.text[..s.at]
                    .chars()
                    .next_back()
                    .is_none_or(|c| !no_break_after(c))
        });
        out.sort_by_key(|s| (s.at, !s.mandatory));
        out.dedup_by_key(|s| s.at);
        out
    }
}

/// Сторона письма по ПЕРВОМУ СИЛЬНОМУ знаку куска: `Some(true)` — справа
/// налево, `None` — сильных знаков нет вовсе.
fn first_strong_rtl(text: &str) -> Option<bool> {
    for ch in text.chars() {
        match unicode_bidi::bidi_class(ch) {
            unicode_bidi::BidiClass::L => return Some(false),
            unicode_bidi::BidiClass::R | unicode_bidi::BidiClass::AL => return Some(true),
            _ => {}
        }
    }
    None
}

/// Открывающий знак — скобка или кавычка.
fn is_opening(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | Quotation
    )
}

/// Закрывающий знак — скобка или кавычка.
fn is_closing(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation | CloseParenthesis | Quotation
    )
}

/// Точка или запятая — то, что свисает по `force-end`/`allow-end`.
fn is_stop(ch: char) -> bool {
    matches!(
        ch,
        '.' | ','
            | '\u{060C}'
            | '\u{06D4}'
            | '、'
            | '。'
            | '，'
            | '．'
            | '\u{FE50}'
            | '\u{FE51}'
            | '\u{FE52}'
            | '\u{FF61}'
            | '\u{FF64}'
    )
}

/// Буквенная единица письма: между такими знаками `word-break: keep-all`
/// запрещает разрыв. Знаки препинания сюда не входят — после них рвать можно.
///
/// Пробел единицей письма не является НИКАКОЙ, даже идеографический: по
/// классу переноса он иероглиф (ID), и запрет заодно снимал перенос по нему
/// (`word-space-transform-013`: коробка шла одной строкой за край).
fn letter_unit(ch: char) -> bool {
    if ch.is_whitespace() {
        return false;
    }
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        Alphabetic
            | Numeric
            | Ambiguous
            | Ideographic
            | ConditionalJapaneseStarter
            | HebrewLetter
            | ComplexContext
            | CombiningMark
            | HangulLvSyllable
            | HangulLvtSyllable
            | HangulLJamo
            | HangulVJamo
            | HangulTJamo
    )
}

/// Знак, перед которым рвать нельзя: закрывающая скобка, знак препинания,
/// разделитель разрядов, неразрывный пробел (классы UAX-14).
fn no_break_before(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        ClosePunctuation
            | CloseParenthesis
            | Exclamation
            | InfixSeparator
            | NonStarter
            | Symbol
            | NonBreakingGlue
            | WordJoiner
            | ZeroWidthJoiner
    )
}

/// Знак, после которого рвать нельзя: открывающая скобка, склейка, знак
/// перед числом (`$`, `\`, `£`).
///
/// Соединитель нулевой ширины держит составные знаки вместе — на нём собраны
/// целые эмодзи (человек + компьютер = «программист»). Разрыв по нему
/// рассыпал бы один знак на составные части.
fn no_break_after(ch: char) -> bool {
    use unicode_linebreak::BreakClass::*;
    matches!(
        unicode_linebreak::break_property(ch as u32),
        OpenPunctuation | NonBreakingGlue | WordJoiner | ZeroWidthJoiner | Prefix
    )
}

/// Забыть замеры. Зовётся при разборе новой страницы: одно и то же имя
/// семейства на разных страницах означает РАЗНЫЕ шрифты (`@font-face`), и
/// старые положения знаков стали бы чужими.
pub fn forget_measures() {
    MEASURED.with(|c| c.borrow_mut().clear());
    SPLITS.with(|c| c.borrow_mut().clear());
}

thread_local! {
    /// Память разрезов: ключ разреза → готовые строки.
    static SPLITS: std::cell::RefCell<
        std::collections::HashMap<u64, std::rc::Rc<Vec<Line>>>,
    > = std::cell::RefCell::new(std::collections::HashMap::new());
}

/// Сколько замеров абзацев помнить между кадрами.
const MEASURE_CACHE: usize = 64;

/// Память подбора кегля: ключ абзаца → найденный множитель.
fn remember_fit(key: u64, k: f32) {
    FITTED.with(|c| {
        let mut cache = c.borrow_mut();
        if let Some(hit) = cache.iter_mut().find(|(hit, _)| *hit == key) {
            hit.1 = k;
            return;
        }
        if cache.len() >= MEASURE_CACHE {
            cache.remove(0);
        }
        cache.push((key, k));
    });
}

thread_local! {
    /// Найденные множители `text-fit`: ключ абзаца → во сколько раз крупнее.
    static FITTED: std::cell::RefCell<Vec<(u64, f32)>> =
        const { std::cell::RefCell::new(Vec::new()) };
    /// Память замеров: ключ стиля и текста → положения знаков по кускам.
    static MEASURED: std::cell::RefCell<Vec<(u64, Vec<Seg>)>> =
        const { std::cell::RefCell::new(Vec::new()) };
}

/// Знак-указание, у которого нет своего изображения: словосоединитель,
/// нулевой пробел, метка порядка байтов, мягкий перенос, знаки управления
/// встроенностью.
fn invisible(ch: char) -> bool {
    matches!(ch as u32,
        0x00AD | 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

/// Прогоны без невидимых знаков: длины считаются по оставшимся байтам.
fn trim_runs(runs: &[TextRun], text: &str) -> Vec<TextRun> {
    let mut out = Vec::with_capacity(runs.len());
    let mut at = 0usize;
    for run in runs {
        let end = (at + run.len).min(text.len());
        let kept: usize = text
            .get(at..end)
            .map(|s| {
                s.chars()
                    .filter(|c| !invisible(*c))
                    .map(char::len_utf8)
                    .sum()
            })
            .unwrap_or(0);
        if kept > 0 {
            let mut piece = run.clone();
            piece.len = kept;
            out.push(piece);
        }
        at = end;
    }
    out
}

/// Зеркальная пара знака: в правой стороне письма скобка смотрит в другую
/// сторону. Список короткий — это те знаки, что встречаются в тексте, а не
/// весь набор Юникода.
fn mirror(ch: char) -> char {
    match ch {
        '(' => ')',
        ')' => '(',
        '[' => ']',
        ']' => '[',
        '{' => '}',
        '}' => '{',
        '<' => '>',
        '>' => '<',
        '«' => '»',
        '»' => '«',
        '‹' => '›',
        '›' => '‹',
        other => other,
    }
}

/// Слово строки и сколько пробелов стоит перед ним от начала строки.
struct Word {
    range: std::ops::Range<usize>,
    spaces_before: usize,
}

/// Точка возможного разрыва.
#[derive(Clone, Copy, Debug)]
struct Stop {
    at: usize,
    mandatory: bool,
}

/// Какой край отрезка спрашивают: у конца строки индекс сразу за переводом
/// строки принадлежит прошлому куску, у начала — новому.
#[derive(Clone, Copy, PartialEq)]
enum Edge {
    Start,
    End,
}

/// Кусок текста между обязательными разрывами вместе со своим набором.
#[derive(Clone)]
struct Seg {
    start: usize,
    end: usize,
    layout: std::sync::Arc<gpui::LineLayout>,
    /// Сдвиг начала куска внутри своей строки. Нужен табуляции: она рвёт
    /// набор на куски, и каждый следующий начинается со своей позиции табуляции.
    offset: Pixels,
}

/// Длина куска без хвостовых пробелов — они висят за краем строки.
/// Сколько байт схлопываемых пробелов в НАЧАЛЕ строки: по CSS они удаляются
/// вместе с переносом, иначе следующая строка начинается с отступа в пробел.
fn skip_leading(chunk: &str) -> usize {
    chunk.len() - chunk.trim_start_matches([' ', '\t']).len()
}

/// Разделитель, который висит за краем строки. Неразрывный пробел сюда НЕ
/// входит: он держит слова вместе и место занимает всегда.
fn hangs(ch: char) -> bool {
    matches!(
        ch as u32,
        0x20 | 0x09 | 0x0A | 0x1680 | 0x2000..=0x200A | 0x202F | 0x205F | 0x3000
    )
}

/// Разделитель слов по css-text-3 §8.2: к нему прибавляется `word-spacing`,
/// и по нему же выключка раздаёт остаток строки.
///
/// Обычным пробелом набор не исчерпывается: пока неразрывный в него не
/// входил, `word-spacing` на строке из `&nbsp;` не действовал вовсе
/// (`word-spacing-001`).
fn word_separator(ch: char) -> bool {
    matches!(
        ch as u32,
        0x20 | 0xA0 | 0x1361 | 0x3000 | 0x10100 | 0x10101 | 0x1039F | 0x1091F
    )
}

/// Знак управления нулевой ширины: сам не висит, но обрезка хвоста смотрит
/// сквозь него — иначе пробел перед ним перестаёт висеть.
fn zero_width(ch: char) -> bool {
    matches!(ch as u32, 0x200B | 0x2060 | 0xFEFF | 0x202A..=0x202E | 0x2066..=0x2069)
}

fn trim_hanging(chunk: &str) -> usize {
    // Идеографический пробел тоже не тянет за собой перенос: место он
    // занимает и рисуется, но строку из-за него не рвут — иначе он один
    // уезжал бы на следующую строку.
    //
    // Соединитель слов (U+FEFF) из обрезки ИСКЛЮЧЁН: им помечены распорки
    // строчных коробок, и в них лежит поле — обрезав хвостовую, коробка
    // теряла своё правое поле целиком (`word-space-transform-010`).
    chunk
        .trim_end_matches(|c| c != '\u{feff}' && (hangs(c) || zero_width(c)))
        .len()
}

impl Element for Paragraph {
    type RequestLayoutState = ();
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
        _cx: &mut App,
    ) -> (LayoutId, ()) {
        // Ширина известна только раскладке, поэтому строки считаются в замере:
        // сколько дали места — столько строк и получилось.
        let text = self.text.clone();
        let runs = self.runs.clone();
        let font_size = self.font_size;
        let line_height = self.line_height;
        let wrap = self.wrap;
        let align = self.align;
        let vertical = self.vertical;
        let ortho_limit = self.ortho_limit;
        // Правила КУСКОВ обязаны доехать и до замера: без них щуп считает
        // абзац по общим правилам и отдаёт другое число строк, чем потом
        // рисуется. Коробка тогда выходит по замеру, а текст по отрисовке —
        // и лишние строки вылезают за рамку (`white-space-pre-031`).
        let spans = self.spans.clone();
        let word_spans = self.word_spans.clone();
        let letter_spans = self.letter_spans.clone();
        // Обрыв по `line-clamp` обязан доехать и до замера: иначе коробка
        // считается по ПОЛНОМУ числу строк, а рисуются обрезанные, и рамка
        // выходит выше текста (`text-wrap-balance-line-clamp-004`).
        let clamp = self.clamp;
        let fit = self.fit;
        let tab_stop = self.tab_stop;
        // Отступ первой строки решает и число строк, и ширину коробки —
        // без него щуп мерил абзац по чужой раскладке.
        let indent = self.indent;
        let hanging = self.hanging;
        let spacers = self.spacers.clone();
        let id = window.request_measured_layout_with_baseline(
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
                probe.spans = spans.clone();
                probe.word_spans = word_spans.clone();
                probe.letter_spans = letter_spans.clone();
                probe.clamp = clamp;
                probe.fit = fit;
                probe.tab_stop = tab_stop;
                probe.indent = indent;
                probe.hanging = hanging;
                probe.spacers = spacers.clone();
                // Предел переноса берётся ПО ОСИ СТРОКИ: по горизонтали это
                // ширина коробки, по вертикали — её высота. Уже решённая
                // родителем сторона сильнее доступной.
                let (known_along, space_along) = if vertical {
                    (known.height, available.height)
                } else {
                    (known.width, available.width)
                };
                let limit = known_along.or(match space_along {
                    gpui::AvailableSpace::Definite(w) => Some(w),
                    // Ось потока родителя не ограничена — она и растёт под
                    // содержимое. Ортогональному потоку предел приходит
                    // СНАРУЖИ (§7.3): от ближайшего предка-контейнера
                    // прокрутки, а без него — от окна. Брать вместо этого
                    // другую сторону родителя нельзя: она про ширину колонки,
                    // а не про длину строки (проверено, было в минус).
                    // Ось потока родителя не ограничена и растёт под
                    // содержимое. Ортогональному потоку предел приходит
                    // СНАРУЖИ (§7.3): от контейнера прокрутки или от окна.
                    gpui::AvailableSpace::MaxContent if vertical => ortho_limit,
                    gpui::AvailableSpace::MaxContent => None,
                    // По минимальному содержимому строка рвётся не где попало,
                    // а по самому широкому неразрывному куском.
                    gpui::AvailableSpace::MinContent => Some(probe.min_content(window)),
                });
                // Кегль подбирается ДО замера: коробка считается уже по
                // подобранному, иначе её высота не сойдётся с отрисовкой.
                // Подбирать есть смысл только под ЗАДАННЫЙ размер строки:
                // когда его нет, коробка растёт под текст, и заполнять нечего
                // (иначе кегль улетал в размер окна — `text-fit/writing-mode`).
                if let Some(w) = known_along {
                    probe.apply_fit(w, window);
                }
                let lines = probe.split(limit, window);
                if { static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("HTML_MEASURE").is_ok()); *ON } {
                    eprintln!(
                        "MEASURE {:?} known={:?}x{:?} avail={:?}x{:?} limit={:?} lines={:?}",
                        probe.text,
                        known.width,
                        known.height,
                        available.width,
                        available.height,
                        limit,
                        lines.iter().map(|l| f32::from(l.width)).collect::<Vec<_>>()
                    );
                }
                // Ширина — по самой длинной строке. Вся отведённая ширина
                // берётся только под выключку по ширине: там остаток строки
                // раздаётся пробелам, и без полной колонки раздавать нечего.
                // В остальных случаях абзац обтягивает текст, иначе ломается
                // размер по содержимому у родителя.
                // Отступ строки входит в её место в колонке: коробка по
                // содержимому обязана вместить и его. Отрицательный уходит в
                // поле и ширины не требует, поэтому в ноль он и упирается.
                let content = lines
                    .iter()
                    .map(|l| l.width + l.indent.max(px(0.)))
                    .fold(px(0.), |a: Pixels, b| if b > a { b } else { a });
                let width = known_along.unwrap_or(content);
                // Шире отведённого коробка не бывает: у абзаца блочного уровня
                // ширина ограничена содержащим блоком, и без этого предела
                // длинная сохранённая строка растягивала коробку и вылезала
                // за неё вместо переноса.
                let width = match space_along {
                    gpui::AvailableSpace::Definite(w) if width > w => w,
                    _ => width,
                };
                let across = line_height * lines.len() as f32;
                if vertical {
                    // Строка идёт вниз: её длина — это ВЫСОТА коробки, а
                    // строки набегают вбок и занимают ширину. Стороны, уже
                    // решённые родителем, не перебиваются — иначе сетка
                    // считает дорожки по чужому числу.
                    // Базовой линии у вертикального абзаца нет: у раскладки она
                    // только по вертикальной оси, а строка идёт вниз.
                    return (
                        size(known.width.unwrap_or(across), known.height.unwrap_or(width)),
                        None,
                    );
                }
                // Первая базовая линия абзаца — она нужна выравниванию
                // `align-items: baseline`. Считается ПОСЛЕ подбора кегля:
                // `apply_fit` меняет и кегль, и высоту строки.
                let baseline = probe.runs.first().map(|run| {
                    let font = run.font.clone();
                    let size = run.font_size.unwrap_or(probe.font_size);
                    let id = window.text_system().resolve_font(&font);
                    window.text_system().baseline_offset(id, size, line_height)
                });
                (
                    size(known.width.unwrap_or(width), known.height.unwrap_or(across)),
                    baseline,
                )
            },
        );
        (id, ())
    }

    fn prepaint(
        &mut self,
        _id: Option<&GlobalElementId>,
        _inspector_id: Option<&InspectorElementId>,
        bounds: Bounds<Pixels>,
        _state: &mut (),
        window: &mut Window,
        _cx: &mut App,
    ) -> Option<Hitbox> {
        // Предел переноса — длина СТРОКИ: по горизонтали это ширина коробки,
        // по вертикали её высота.
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
        self.apply_measured_fit();
        self.lines = self.split(Some(limit), window);
        // Куски вне потока встают на своё место в строке: раскладываются
        // по содержимому и подготавливаются от угла своего знака.
        if !self.overlays.is_empty() {
            let segs = self.measure(window);
            let mut placed = std::mem::take(&mut self.overlays);
            for (at, el) in placed.iter_mut() {
                let origin = self.point_of(&segs, *at, bounds);
                // Ширина абсолютного элемента — «по содержимому» (CSS 2.1
                // §10.3.7): по МИНИМАЛЬНОМУ содержимому он рвался бы по
                // словам (`static-position/htb-*`).
                el.layout_as_root(
                    gpui::size(
                        gpui::AvailableSpace::MaxContent,
                        gpui::AvailableSpace::MaxContent,
                    ),
                    window,
                    _cx,
                );
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
        _state: &mut (),
        hitbox: &mut Option<Hitbox>,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Вертикальное письмо: строки рисуются в ПОВЁРНУТОЙ системе координат
        // самого абзаца. Раскладка при этом остаётся честной — коробка уже
        // получила свои размеры от родителя по нужным осям, а поворот меняет
        // только то, как в неё ложатся строки. Прежде поворачивался блок
        // целиком, и ограничение родителя до текста не доходило вовсе.
        if self.vertical {
            let scale = window.scale_factor();
            let dev = |v: Pixels| v.scale(scale);
            let corner = if self.vertical_rl {
                bounds.origin.x + bounds.size.width
            } else {
                bounds.origin.x
            };
            let matrix = gpui::TransformationMatrix::unit()
                .translate(gpui::point(dev(corner), dev(bounds.origin.y)))
                .rotate(gpui::Radians(std::f32::consts::FRAC_PI_2))
                .translate(gpui::point(dev(-bounds.origin.x), dev(-bounds.origin.y)));
            // Внутри поворота коробка «горизонтальная»: её ширина — это
            // высота настоящей, и наоборот.
            let flat = Bounds {
                origin: bounds.origin,
                size: size(bounds.size.height, bounds.size.width),
            };
            let mut inner = std::mem::replace(self, Paragraph::empty());
            inner.vertical = false;
            window.with_transformation(matrix, |window| {
                inner.paint(id, _inspector_id, flat, _state, hitbox, window, cx);
            });
            inner.vertical = true;
            *self = inner;
            return;
        }
        let segs = self.measure(window);
        let count = self.lines.len();
        let mut y = if self.lines_reversed && count > 0 {
            bounds.origin.y + self.line_height * (count as f32 - 1.0)
        } else {
            bounds.origin.y
        };
        let selection = id
            .map(|global| {
                window.with_element_state::<Selection, _>(global, |state, _| {
                    let st = state.unwrap_or_default();
                    (st.range(), st)
                })
            })
            .unwrap_or((0, 0));
        let runs = self.runs_with_selection(selection.0, selection.1);
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
            let last_line = i + 1 == count || body.len() < line.range.len();
            // Строка с СОХРАНЁННОЙ табуляцией не растягивается (позиции
            // табуляции обязаны совпасть с нерастянутой строкой), но выключку
            // ПОСЛЕДНЕЙ строки (`text-align-last`) она не получает: к
            // табуляции та отношения не имеет.
            let no_stretch = last_line;
            // При `plaintext` сторона письма своя у каждого АБЗАЦА между
            // жёсткими разрывами (не у строки: мягкий перенос сторону не
            // меняет). От неё же зависят `start` и `end`.
            let own_align = match self.plaintext {
                Some(logical) => {
                    let start = self.text[..line.range.start]
                        .rfind('\n')
                        .map(|i| i + 1)
                        .unwrap_or(0);
                    let end = self.text[start..]
                        .find('\n')
                        .map(|i| start + i)
                        .unwrap_or(self.text.len());
                    // База направления — `direction` ЭЛЕМЕНТА (css-writing-modes
                    // §2: bidi paragraph level из свойства, не из содержимого);
                    // авто-детект по первому сильному знаку (UAX9 P3) — только
                    // когда направления нет вовсе (text-align-end-001: `end` при
                    // rtl обязан уйти влево и с латинским текстом).
                    let _ = (start, end);
                    align_of_value(logical.physical(self.wrap.rtl))
                }
                None => self.align,
            };
            // Нерастянутая выключка: `justify` прижимает строку к НАЧАЛУ, а
            // начало у письма справа налево — правый край, не левый.
            let flat = |a: Align| match a {
                Align::Justify if self.wrap.rtl => Align::Right,
                Align::Justify => Align::Left,
                other => other,
            };
            let align = if last_line {
                self.align_last.unwrap_or(flat(own_align))
            } else if no_stretch {
                flat(own_align)
            } else {
                own_align
            };
            // Отступ первой строки занимает место В колонке: остаток на
            // выключку считается уже без него.
            let free = bounds.size.width - line.width - line.indent;
            let free = if free < px(0.) { px(0.) } else { free };
            // Свисающий открывающий знак уходит ЗА край: строка сдвигается
            // влево на его ширину. Считается до выбора пути отрисовки —
            // выключенная строка свисает так же, как обычная.
            let hang = self.hang_first(line.range.start);
            let shift = self.span(&segs, line.range.start, line.range.start + hang);
            let lead = line.indent - shift;
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
                || !self.letter_spans.is_empty()
                || !self.shift_spans.is_empty()
            {
                let (free, dx) = if align == Align::Justify {
                    (free, lead)
                } else {
                    let dx = match align {
                        Align::Center => free / 2.,
                        Align::Right => free,
                        _ => px(0.),
                    };
                    (px(0.), dx + lead)
                };
                self.paint_justified(&range, &segs, free, bounds, y, dx, window, cx);
                if line.ellipsis {
                    let text = self.span(&segs, line.range.start, range.end);
                    self.paint_suffix(
                        ELLIPSIS,
                        line.range.start,
                        point(bounds.origin.x + dx + text, y),
                        window,
                        cx,
                    );
                }
                y += if self.lines_reversed {
                    -self.line_height
                } else {
                    self.line_height
                };
                continue;
            }
            let dx = match align {
                Align::Center => free / 2.,
                Align::Right => free,
                _ => px(0.),
            } + lead;
            if { static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("TCA_DBG").is_ok()); *ON } {
                eprintln!(
                    "TCA para {:?} align={:?} bw={:?} lw={:?} free={:?}",
                    &self.text[..self.text.len().min(6)],
                    align,
                    bounds.size.width,
                    line.width,
                    free
                );
            }
            let at = point(bounds.origin.x + dx, y);
            // Висящие пробелы конца строки при письме справа налево уходят по
            // правилу L1 на ЛЕВЫЙ край и отодвигали бы текст от края коробки.
            // Рисовать их незачем: они пустые.
            let visible = if self.wrap.rtl && !self.wrap.break_spaces {
                range.start..range.start + trim_hanging(&self.text[range.clone()])
            } else {
                range.clone()
            };
            // Знак обрыва и знак переноса набираются вместе со строкой.
            let mark = if line.ellipsis {
                ELLIPSIS.to_string()
            } else if line.hyphen {
                self.hyphen.to_string()
            } else {
                String::new()
            };
            self.paint_line(&visible, &runs, at, &mark, window, cx);
            y += if self.lines_reversed {
                -self.line_height
            } else {
                self.line_height
            };
        }
        for (_, el) in self.overlays.iter_mut() {
            el.paint(window, cx);
        }
        if let (Some(global), Some(hitbox)) = (id, hitbox.clone()) {
            self.track_selection(global, &segs, bounds, hitbox, window);
        }
    }
}

/// Память выделения между кадрами: границы в байтах текста абзаца.
#[derive(Default, Clone, Copy)]
struct Selection {
    anchor: usize,
    head: usize,
    dragging: bool,
}

impl Selection {
    fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

impl Paragraph {
    /// Прогоны с подложкой на выделенном куске: прогон нельзя раскрасить
    /// наполовину, поэтому попавшие на границу режутся надвое.
    fn runs_with_selection(&self, from: usize, to: usize) -> Vec<TextRun> {
        if from >= to {
            return self.runs.clone();
        }
        let mut out = Vec::with_capacity(self.runs.len() + 2);
        let mut at = 0usize;
        for run in &self.runs {
            let end = at + run.len;
            let mut cut = |start: usize, stop: usize, selected: bool| {
                if stop <= start {
                    return;
                }
                let mut piece = run.clone();
                piece.len = stop - start;
                piece.background_color = selected.then_some(self.highlight);
                out.push(piece);
            };
            cut(at, end.min(from), false);
            cut(at.max(from), end.min(to), true);
            cut(at.max(to), end, false);
            at = end;
        }
        out
    }

    /// Байтовый индекс под точкой: сначала строка по высоте, потом знак по
    /// ширине внутри неё.
    fn index_at(&self, segs: &[Seg], bounds: Bounds<Pixels>, at: Point<Pixels>) -> usize {
        if self.lines.is_empty() {
            return 0;
        }
        let row = ((at.y - bounds.origin.y) / self.line_height)
            .floor()
            .max(0.) as usize;
        let line = &self.lines[row.min(self.lines.len() - 1)];
        let want = at.x - bounds.origin.x + self.x_at(segs, line.range.start, Edge::Start);
        let mut best = line.range.start;
        for (i, _) in self.text[line.range.clone()].char_indices() {
            let idx = line.range.start + i;
            if self.x_at(segs, idx, Edge::End) > want {
                break;
            }
            best = idx;
        }
        best
    }

    /// Тянуть выделение мышью.
    fn track_selection(
        &self,
        global: &GlobalElementId,
        segs: &[Seg],
        bounds: Bounds<Pixels>,
        hitbox: Hitbox,
        window: &mut Window,
    ) {
        let inside = hitbox.is_hovered(window);
        if inside {
            window.set_cursor_style(gpui::CursorStyle::IBeam, &hitbox);
        }
        // Замыкания живут дольше кадра, поэтому берут СВОЙ снимок раскладки.
        let probe = Paragraph {
            plaintext: self.plaintext,
            lines_reversed: self.lines_reversed,
            text: self.text.clone(),
            spans: self.spans.clone(),
            word_spans: self.word_spans.clone(),
            letter_spans: self.letter_spans.clone(),
            shift_spans: self.shift_spans.clone(),
            ortho_limit: self.ortho_limit,
            runs: Vec::new(),
            font_size: self.font_size,
            line_height: self.line_height,
            align: self.align,
            align_last: self.align_last,
            letter_spacing: self.letter_spacing,
            word_spacing: self.word_spacing,
            vertical: self.vertical,
            vertical_rl: self.vertical_rl,
            hanging: self.hanging,
            indent: self.indent,
            spacers: self.spacers.clone(),
            id: None,
            highlight: self.highlight,
            wrap: self.wrap,
            lines: self.lines.clone(),
            clamp: self.clamp,
            text_overflow: false,
            fit: self.fit,
            tab_stop: self.tab_stop,
            hyphen: self.hyphen.clone(),
            hyphen_w: std::cell::Cell::new(self.hyphen_w.get()),
            overlays: Vec::new(),
        };
        let segs = segs.to_vec();
        window.with_element_state::<Selection, _>(global, |state, window| {
            let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
            let index_at = {
                let probe = std::rc::Rc::new(probe);
                let segs = std::rc::Rc::new(segs);
                move |p: Point<Pixels>| probe.index_at(&segs, bounds, p)
            };

            let down = st.clone();
            let at_down = index_at.clone();
            window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
                if !phase.bubble() || e.button != MouseButton::Left || !inside {
                    return;
                }
                let i = at_down(e.position);
                down.set(Selection {
                    anchor: i,
                    head: i,
                    dragging: true,
                });
                window.refresh();
            });

            let mv = st.clone();
            let at_move = index_at.clone();
            window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = mv.get();
                if !s.dragging {
                    return;
                }
                let i = at_move(e.position);
                if s.head != i {
                    s.head = i;
                    mv.set(s);
                    window.refresh();
                }
            });

            let up = st.clone();
            window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
                if !phase.bubble() {
                    return;
                }
                let mut s = up.get();
                if s.dragging {
                    s.dragging = false;
                    up.set(s);
                }
            });
            ((), st.get())
        });
    }

    /// Набрать кусок строки и поставить его прогоны в ВИДИМОМ порядке.
    ///
    /// Двунаправленный текст набирается в логическом порядке, а на экран идёт
    /// в видимом. Переставлять ЗНАКИ нельзя — рвётся арабская вязь, поэтому
    /// строка режется на прогоны по уровням встроенности: порядок прогонов
    /// считаем сами, а каждый прогон набирает сам набор. Правому прогону
    /// сторона сообщается знаком управления: внутри него набор и переставит
    /// знаки, и развернёт парные скобки.
    fn paint_line(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        at: Point<Pixels>,
        suffix: &str,
        window: &mut Window,
        cx: &mut App,
    ) {
        // Пустой отрезок разбору двунаправленности отдавать нельзя: он берёт
        // уровень по первому знаку и падает на конце текста. Пустая строка
        // бывает у абзаца из одних пробелов и после жёсткого разрыва в конце.
        if range.start >= range.end || range.end > self.text.len() {
            return;
        }
        let base = if self.wrap.rtl {
            unicode_bidi::Level::rtl()
        } else {
            unicode_bidi::Level::ltr()
        };
        // `unicode-bidi: plaintext`: базу КАЖДОГО абзаца (между жёсткими
        // разрывами) выбирает первый сильный знак (UAX9 P2/P3) — разбор без
        // навязанного уровня делает ровно это. Выключка уже решается так же
        // построчно (см. own_align выше).
        let forced = if self.plaintext.is_some() {
            None
        } else {
            Some(base)
        };
        let info = unicode_bidi::BidiInfo::new(&self.text, forced);
        let Some(para) = info
            .paragraphs
            .iter()
            .find(|p| p.range.start <= range.start && range.start < p.range.end)
            .or_else(|| info.paragraphs.first())
        else {
            return;
        };
        let (levels, visual) = info.visual_runs(para, range.clone());
        let mut x = at.x;
        for run in visual.into_iter() {
            let rtl = levels.get(run.start).is_some_and(|l| l.is_rtl());
            // Знак обрыва — у обрезанного КРАЯ: обычно это логический
            // конец строки; при письме справа налево контейнер режет левый
            // край, то есть логическое НАЧАЛО — знак идёт префиксом
            // первого прогона.
            let (tail, at_start) = if self.wrap.rtl {
                (if run.start == range.start { suffix } else { "" }, true)
            } else {
                (if run.end == range.end { suffix } else { "" }, false)
            };
            let Some(shaped) = self.shape_with_mark(&run, runs, rtl, tail, at_start, window) else {
                continue;
            };
            let width = shaped.width;
            if at_start && !tail.is_empty() {
                // Знак обрыва СЛЕВА от куска: рисуется на своём месте, а
                // кусок сдвигается на его ширину.
                let ell = self.suffix_width(tail, run.start, window);
                self.paint_suffix(tail, run.start, point(x, at.y), window, cx);
                x += ell;
            }
            // Подложка прогона (`background` на `<span>`) рисуется ОТДЕЛЬНЫМ
            // вызовом: `paint` кладёт только глифы. Пока его не звали, фон
            // строчного элемента не появлялся вовсе — проверено пробой, где
            // `background: green; color: transparent` давал пустую страницу.
            let _ = shaped.paint_background(point(x, at.y), self.line_height, window, cx);
            let _ = shaped.paint(point(x, at.y), self.line_height, window, cx);
            x += width;
        }
    }

    /// Многоточие обрыва: набирается стилем того куска, на котором строка
    /// оборвана, и рисуется сразу за её текстом.
    fn paint_suffix(
        &self,
        mark: &str,
        at: usize,
        origin: Point<Pixels>,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut runs = slice_runs(&self.runs, &(at..at + 1));
        let Some(run) = runs.first_mut() else {
            return;
        };
        run.len = mark.len();
        let piece = vec![run.clone()];
        let shaped = window.text_system().shape_line_spaced(
            SharedString::from(mark.to_string()),
            self.font_size,
            &piece,
            None,
            self.letter_spacing,
        );
        let _ = shaped.paint(origin, self.line_height, window, cx);
    }

    /// Набор с знаком обрыва в начале или в конце куска.
    fn shape_with_mark(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        at_start: bool,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        if !at_start || suffix.is_empty() {
            return self.shape(range, runs, rtl, suffix, window);
        }
        // Префикс: знак дорисовывается отдельным вызовом слева, а сам кусок
        // набирается без него (вплетение в шейп меняло бы кернинг начала).
        self.shape(range, runs, rtl, "", window)
    }

    /// Набрать кусок текста; правый прогон — со знаком стороны письма.
    fn shape(
        &self,
        range: &std::ops::Range<usize>,
        runs: &[TextRun],
        rtl: bool,
        suffix: &str,
        window: &mut Window,
    ) -> Option<gpui::ShapedLine> {
        let mut piece = slice_runs(runs, range);
        if piece.is_empty() {
            return None;
        }
        // Управляющие знаки не рисуются: своей ширины у них нет, но подмена
        // шрифта может подставить вместо них пустой квадрат и раздвинуть
        // строку. Разрывы по ним УЖЕ решены — здесь остаётся только показ.
        //
        // ПРОБОВАЛИ И ОТКАТИЛИ: не выбрасывать их, чтобы замер и показ считали
        // один и тот же текст (замер берёт его целиком). Счёт не изменился,
        // а `trim_runs` и `invisible` становились мёртвым кодом.
        let body: String = self.text[range.clone()]
            .chars()
            .filter(|c| !invisible(*c))
            .collect();
        if body.len() != range.len() {
            piece = trim_runs(&piece, &self.text[range.clone()]);
        }
        // Знак переноса набирается ВМЕСТЕ со строкой, а не отдельным вызовом:
        // отдельный набор садится на свою базовую линию и сдвигает строку
        // (`hyphens-manual-011`: текст уезжал на три точки вниз).
        let body = if suffix.is_empty() {
            body
        } else {
            if let Some(last) = piece.last_mut() {
                last.len += suffix.len();
            }
            format!("{body}{suffix}")
        };
        let body = body.as_str();
        if !rtl {
            return Some(window.text_system().shape_line_spaced(
                body.to_string().into(),
                self.font_size,
                &piece,
                None,
                self.letter_spacing,
            ));
        }
        // В правом прогоне парный знак смотрит в другую сторону: скобка,
        // кавычка-ёлочка, знаки сравнения.
        let mirrored: String = body.chars().map(mirror).collect();
        Some(window.text_system().shape_line_rtl(
            mirrored.into(),
            self.font_size,
            &piece,
            self.letter_spacing,
        ))
    }

    /// Выключка по ширине: остаток строки раздаётся её пробелам.
    ///
    /// Слова набираются по отдельности и ставятся каждое на своё место —
    /// иначе растянуть промежутки нечем: набор отдаёт готовую строку одним
    /// куском. Внутри слова набор остаётся сплошным, поэтому лигатуры и вязь
    /// не рвутся.
    #[allow(clippy::too_many_arguments)]
    fn paint_justified(
        &self,
        range: &std::ops::Range<usize>,
        segs: &[Seg],
        free: Pixels,
        bounds: Bounds<Pixels>,
        y: Pixels,
        dx: Pixels,
        window: &mut Window,
        cx: &mut App,
    ) {
        let mut words = self.words(range);
        // Слово режется по границам кусков с трекингом: набор принимает
        // трекинг скаляром, поэтому кусок с другим значением обязан идти
        // отдельным вызовом. Без этого `letter-spacing` на `<span>` внутри
        // слова не действовал вовсе.
        if !self.letter_spans.is_empty() || !self.shift_spans.is_empty() {
            let mut cuts: Vec<usize> = Vec::new();
            let mut cut = |edge: usize| {
                if edge > range.start && edge < range.end {
                    cuts.push(edge);
                }
            };
            for (r, _) in self.letter_spans.iter() {
                // Трекинг знака — это промежуток ПОСЛЕ него, поэтому у
                // последнего знака отрезка он на набор внутри отрезка не
                // влияет. Резать по началу нужно только там, где знаков в
                // диапазоне несколько: одиночный (зазор `text-autospace`)
                // спокойно доживает в общем отрезке, а свой разрез оставлял
                // между половинками слова шов в точку — соседние отрезки
                // округляются независимо (`text-autospace-001`: `XX`
                // расходились).
                if self.text[r.clone()].chars().nth(1).is_some() {
                    cut(r.start);
                }
                cut(r.end);
            }
            // Сдвиг по вертикали — свойство самого глифа: он обязан ехать
            // отдельным вызовом целиком.
            for (r, _) in self.shift_spans.iter() {
                cut(r.start);
                cut(r.end);
            }
            cuts.sort_unstable();
            cuts.dedup();
            let mut split: Vec<Word> = Vec::with_capacity(words.len());
            for w in words {
                let mut at = w.range.start;
                let mut spaces = w.spaces_before;
                for cut in cuts.iter().copied().filter(|c| w.range.contains(c)) {
                    split.push(Word {
                        range: at..cut,
                        spaces_before: spaces,
                    });
                    at = cut;
                    spaces = 0;
                }
                split.push(Word {
                    range: at..w.range.end,
                    spaces_before: spaces,
                });
            }
            words = split;
        }
        // Растягивается КАЖДЫЙ пробел, а не промежуток между словами: там, где
        // подряд стоят два сохранённых пробела, добавка идёт дважды.
        //
        // Пробелы ЛЕВЕЕ последней табуляции добавки не получают: табуляция
        // доводит строку до своей позиции и всё лишнее место слева от себя
        // поглощает, поэтому позиции табуляции совпадают с нерастянутой
        // строкой (css-text-4 §8.1). Отсюда оба поведения сразу: строка, где
        // все пробелы левее табуляции, не растягивается вовсе
        // (`text-align-justify-tabs-001`, обе коробки обязаны совпасть), а
        // остаток достаётся только пробелам правее (`-002`: их ровно два, и
        // каждый вырастает на пробел).
        let absorbed = self.text[range.clone()].rfind('\u{9}').map_or(0, |at| {
            self.text[range.start..range.start + at]
                .chars()
                .filter(|c| word_separator(*c))
                .count()
        });
        if absorbed > 0 {
            for w in words.iter_mut() {
                w.spaces_before = w.spaces_before.saturating_sub(absorbed);
            }
        }
        let opportunities = words.last().map(|w| w.spaces_before).unwrap_or(0);
        let step = if opportunities > 0 {
            free / opportunities as f32
        } else {
            px(0.)
        };
        let from = self.x_at(segs, range.start, Edge::Start);
        for (wi, word) in words.iter().enumerate() {
            let slice: SharedString = self.text[word.range.clone()].to_string().into();
            let runs = slice_runs(&self.runs, &word.range);
            let shaped = window.text_system().shape_line_spaced(
                slice,
                self.font_size,
                &runs,
                None,
                self.letter_spans
                    .iter()
                    .find(|(r, _)| r.contains(&word.range.start))
                    .map(|(_, v)| *v)
                    .unwrap_or(self.letter_spacing),
            );
            let logical = (self.x_at(segs, word.range.start, Edge::Start) - from)
                + step * word.spaces_before as f32;
            // При письме справа налево строка раздаётся от ПРАВОГО края:
            // первое слово встаёт справа, последнее — слева. Раздача слева
            // направо переворачивала порядок слов на выключенной строке.
            let x = if self.wrap.rtl {
                bounds.origin.x + bounds.size.width - dx - logical - shaped.width
            } else {
                bounds.origin.x + dx + logical
            };
            // Сдвиг куска по вертикали: надстрочный и подстрочный знак стоят
            // выше и ниже базовой линии, оставаясь в той же строке.
            let dy = self
                .shift_spans
                .iter()
                .find(|(r, _)| r.contains(&word.range.start))
                .map(|(_, v)| *v)
                .unwrap_or(px(0.));
            // Подложка прогона — отдельным вызовом, см. выше.
            let _ = shaped.paint_background(point(x, y + dy), self.line_height, window, cx);
            let _ = shaped.paint(point(x, y + dy), self.line_height, window, cx);
            // Растянутый выключкой пробел тоже принадлежит прогону, и его
            // подложка обязана быть сплошной. Красим ТОЛЬКО когда пробел
            // целиком внутри одного прогона с фоном — иначе фон соседнего
            // куска растекается по чужому месту (замерено: css-text 1015 →
            // 691 при покраске каждого промежутка).
            if step > px(0.)
                && !self.wrap.rtl
                && let Some(next) = words.get(wi + 1)
                && next.range.start > word.range.end
                && let Some(bg) = self.gap_background(word, next)
            {
                let after = (self.x_at(segs, next.range.start, Edge::Start) - from)
                    + step * next.spaces_before as f32;
                let right = bounds.origin.x + dx + after;
                let left = x + shaped.width;
                if right > left {
                    window.paint_quad(gpui::fill(
                        Bounds {
                            origin: point(left, y + dy),
                            size: gpui::size(right - left, self.line_height),
                        },
                        bg,
                    ));
                }
            }
        }
    }

    /// Подложка промежутка между словами — только если оба соседа и сам
    /// промежуток лежат в ОДНОМ прогоне, и у него есть фон.
    fn gap_background(&self, left: &Word, right: &Word) -> Option<gpui::Hsla> {
        let run_at = |at: usize| -> Option<usize> {
            let mut start = 0usize;
            for (i, run) in self.runs.iter().enumerate() {
                if at < start + run.len {
                    return Some(i);
                }
                start += run.len;
            }
            None
        };
        let a = run_at(left.range.end.saturating_sub(1))?;
        let b = run_at(right.range.start)?;
        let gap = run_at(left.range.end)?;
        if a != b || a != gap {
            return None;
        }
        self.runs[a].background_color
    }

    /// Слова строки — куски между пробелами, каждое со счётом пробелов слева.
    fn words(&self, range: &std::ops::Range<usize>) -> Vec<Word> {
        let mut out: Vec<Word> = Vec::new();
        let mut start = None;
        let mut spaces = 0usize;
        for (i, ch) in self.text[range.clone()].char_indices() {
            let at = range.start + i;
            // Разделитель слов для выключки — не любой пробел. По css-text-3
            // это пробел, неразрывный и идеографический; ТАБУЛЯЦИЯ в него не
            // входит: она доводит строку до своей позиции, и растягивать её
            // нечем. Пока табуляция раскрывалась в пробелы и каждый считался
            // точкой раздачи, остаток размазывался по ней вместо слов.
            if word_separator(ch) {
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
                spaces += 1;
            } else if ch == '\u{9}' {
                // Табуляция — ГРАНИЦА слова, хотя точкой раздачи и не служит.
                // Её продвижение задаёт позиция табуляции (`Seg::offset`), и
                // внутри слова оно пропадало: строка без пробелов уходила в
                // набор одним куском, и табуляция рисовалась глифом шрифта
                // (`text-indent-tab-positions-001`: `a⇥b⇥c` выходило `abc`).
                if let Some(s) = start.take() {
                    out.push(Word {
                        range: s..at,
                        spaces_before: spaces,
                    });
                }
            } else if start.is_none() {
                start = Some(at);
            }
        }
        if let Some(s) = start {
            out.push(Word {
                range: s..range.end,
                spaces_before: spaces,
            });
        }
        out
    }
}

/// Куски оформления, попавшие в отрезок строки.
fn slice_runs(runs: &[TextRun], range: &std::ops::Range<usize>) -> Vec<TextRun> {
    let mut out = Vec::new();
    let mut at = 0usize;
    for run in runs {
        let end = at + run.len;
        let from = at.max(range.start);
        let to = end.min(range.end);
        if from < to {
            let mut piece = run.clone();
            piece.len = to - from;
            out.push(piece);
        }
        at = end;
        if at >= range.end {
            break;
        }
    }
    out
}

impl IntoElement for Paragraph {
    type Element = Self;

    fn into_element(self) -> Self::Element {
        self
    }
}

/// Правила переноса из стиля — и признак, нужна ли своя раскладка вовсе.
///
/// Пока своя раскладка не умеет выделение мышью, поэтому обычный текст
/// остаётся на выделяемом элементе движка. Сюда уходит только то, что иначе
/// не выразить.
/// Правила переноса из стиля.
///
/// Своя раскладка считает ВЕСЬ текст: перенос, выключка и свисающая
/// пунктуация должны решаться одним алгоритмом, иначе соседние абзацы одной
/// страницы ломаются по-разному. Поэтому правила есть всегда — отбор «кому
/// своя раскладка нужна, а кому нет» отсюда снят.
pub fn rules(c: &crate::computed::Computed) -> Option<Wrap> {
    Some(wrap_of(c))
}

/// Выключка из стиля.
pub fn align_of(a: Option<crate::computed::TextAlign>) -> Align {
    a.map(align_of_value).unwrap_or(Align::Left)
}

/// Выключка абзаца с разворотом логических краёв по стороне письма.
pub fn align_for(c: &crate::computed::Computed) -> Align {
    let rtl = c.rtl == Some(true);
    if { static ON: std::sync::LazyLock<bool> = std::sync::LazyLock::new(|| std::env::var("TA_DBG").is_ok()); *ON } {
        eprintln!("TA align_for rtl={rtl} ta={:?}", c.text_align);
    }
    let value = c
        .text_align
        .unwrap_or(crate::computed::TextAlign::Start)
        .physical(rtl);
    let align = align_of_value(value);
    // `text-justify: none` — растягивать запрещено, и строка идёт к началу:
    // у письма справа налево началом служит правый край.
    if align == Align::Justify && c.no_justify == Some(true) {
        return if rtl { Align::Right } else { Align::Left };
    }
    align
}

/// Выключка из заданного значения.
pub fn align_of_value(a: crate::computed::TextAlign) -> Align {
    match a {
        crate::computed::TextAlign::Center => Align::Center,
        crate::computed::TextAlign::Right => Align::Right,
        crate::computed::TextAlign::Justify => Align::Justify,
        _ => Align::Left,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Абзац без набора: точки разрыва считаются по тексту и правилам, а
    /// система шрифтов для этого не нужна.
    fn para(text: &str, wrap: Wrap) -> Paragraph {
        Paragraph::new(
            SharedString::from(text.to_string()),
            vec![],
            px(16.),
            px(16.),
            Align::Left,
            wrap,
        )
    }

    fn stops(text: &str, wrap: Wrap) -> Vec<usize> {
        para(text, wrap)
            .opportunities()
            .iter()
            .map(|s| s.at)
            .collect()
    }

    #[test]
    fn a_hard_break_is_a_stop_even_without_wrapping() {
        let wrap = Wrap {
            nowrap: true,
            ..Default::default()
        };
        assert_eq!(stops("a\nb", wrap), vec![2]);
    }

    #[test]
    fn break_spaces_stops_after_every_kept_space() {
        let wrap = Wrap {
            break_spaces: true,
            ..Default::default()
        };
        // Пробел даёт точку разрыва ПОСЛЕ себя — и первый, и второй.
        assert_eq!(stops("a  b", wrap), vec![2, 3]);
    }

    #[test]
    fn break_all_stops_between_letters_but_not_before_a_space() {
        let wrap = Wrap {
            break_all: true,
            ..Default::default()
        };
        // Между буквами — точка, перед пробелом — нет: рвать там нечего,
        // пробел и так свисает за край. После пробела точка от типографики.
        assert_eq!(stops("ab c", wrap), vec![1, 3]);
    }

    #[test]
    fn anywhere_stops_before_every_character() {
        let wrap = Wrap {
            anywhere: true,
            ..Default::default()
        };
        assert_eq!(stops("a b", wrap), vec![1, 2]);
    }

    #[test]
    fn keep_all_keeps_ideographs_together() {
        let wrap = Wrap {
            keep_all: true,
            ..Default::default()
        };
        // Между иероглифами разрыва нет, около пробела — есть.
        assert_eq!(stops("中文 中文", wrap), vec![7]);
    }

    #[test]
    fn hanging_spaces_are_cut_off_the_measured_part() {
        assert_eq!(trim_hanging("ab  "), 2);
        assert_eq!(trim_hanging("ab"), 2);
        assert_eq!(trim_hanging("  "), 0);
    }

    /// Распорка строчной коробки не должна съедать точку переноса ПЕРЕД собой:
    /// её класс по UAX-14 (WJ) запрещает разрыв с обеих сторон, и пробел
    /// перед `<span>` с отступом переставал быть точкой переноса.
    #[test]
    fn spacer_keeps_the_break_before_it() {
        let text = "aaa \u{feff}bbb";
        let mut para = para(text, Wrap::default());
        assert!(
            !para.opportunities().iter().any(|s| s.at == 4),
            "пока распорка не объявлена, разрыв по пробелу запрещён"
        );
        para.spacers = vec![4];
        let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
        // Разрыв встаёт НА распорку: поле коробки уходит на новую строку
        // вместе со своим текстом.
        assert_eq!(stops, vec![4]);
    }

    /// Отступ первой строки: кому он достаётся при `each-line` и `hanging`.
    #[test]
    fn indent_goes_to_the_right_lines() {
        let mut para = para("a", Wrap::default());
        let of = |p: &Paragraph, head, first| f32::from(p.indent_of(head, first, None));
        para.indent = Indent {
            px: 40.,
            ..Default::default()
        };
        assert_eq!(of(&para, true, true), 40., "первая строка блока");
        assert_eq!(of(&para, true, false), 0., "первая строка ВТОРОГО куска");
        assert_eq!(of(&para, false, true), 0., "перенесённая строка");
        para.indent.each_line = true;
        assert_eq!(of(&para, true, false), 40., "each-line: каждый кусок");
        assert_eq!(of(&para, false, true), 0., "each-line: не перенос");
        para.indent = Indent {
            px: 40.,
            hanging: true,
            ..Default::default()
        };
        assert_eq!(of(&para, true, true), 0., "hanging: кроме первой");
        assert_eq!(of(&para, false, true), 40.);
        assert_eq!(of(&para, true, false), 40.);
    }

    /// Доля берётся от ширины строки, а при замере по содержимому её нет.
    #[test]
    fn indent_share_needs_a_limit() {
        let mut para = para("a", Wrap::default());
        para.indent = Indent {
            pct: 0.1,
            ..Default::default()
        };
        assert_eq!(f32::from(para.indent_of(true, true, Some(px(300.)))), 30.);
        assert_eq!(f32::from(para.indent_of(true, true, None)), 0.);
    }
}

#[cfg(test)]
mod break_spaces_tests {
    use super::*;

    fn wrap() -> Wrap {
        Wrap {
            break_spaces: true,
            keep_spaces: true,
            ..Default::default()
        }
    }

    /// `white-space: break-spaces` даёт точку разрыва ПОСЛЕ каждого пробела.
    /// Пока их не было, строка рвалась только по правилам UAX-14, и
    /// сохранённый пробел уходил в конец строки вместо начала следующей.
    #[test]
    fn every_preserved_space_gives_a_stop() {
        let para = Paragraph::new(
            SharedString::from("X XX X".to_string()),
            vec![],
            px(25.),
            px(25.),
            Align::Left,
            wrap(),
        );
        let stops: Vec<usize> = para.opportunities().iter().map(|s| s.at).collect();
        assert!(stops.contains(&2), "после первого пробела: {stops:?}");
        assert!(stops.contains(&5), "после второго пробела: {stops:?}");
    }
}

/// Правила переноса из стиля БЕЗ вопроса, нужна ли своя раскладка.
pub fn wrap_of(c: &crate::computed::Computed) -> Wrap {
    Wrap {
        nowrap: c.nowrap == Some(true),
        break_spaces: c.break_after_spaces == Some(true),
        break_all: c.break_anywhere == Some(true) && c.break_anywhere_strict != Some(true),
        anywhere: c.break_anywhere_strict == Some(true),
        keep_all: c.keep_all == Some(true),
        break_word: c.break_word == Some(true),
        wrap_anywhere: c.wrap_anywhere == Some(true),
        rtl: c.rtl == Some(true),
        balance: c.balance_lines == Some(true),
        keep_spaces: c.keep_spaces == Some(true),
    }
}

/// Можно ли разорвать текст ровно на этом месте — граница ли это грозди.
///
/// Смотрятся ОБЕ стороны: знак справа не должен быть продолжением
/// (огласовка, модификатор, знак-тег), а знак слева не должен быть
/// соединителем — после нулевого соединителя гроздь продолжается следующим
/// знаком (`line-breaking-014`: радужный флаг рвался по соединителю).
fn cluster_edge(text: &str, at: usize) -> bool {
    if at >= text.len() {
        return true;
    }
    // Огласовка ПОСЛЕ пробела ни к чему не приросла: по UAX-14 (правило LB9)
    // знак-продолжение после разделителя считается обычной буквой, и рвать
    // перед ним можно.
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    if !cluster_start(&text[at..]) {
        return false;
    }
    !matches!(text[..at].chars().next_back(), Some('\u{200d}'))
}

/// То же, но с учётом `line-break: anywhere`.
///
/// `anywhere` перекрывает класс ZWJ по css-text-4, то есть рвать РЯДОМ с
/// соединителем можно. Саму гроздь он не разбирает: огласовка, знак вариации,
/// модификатор тона и знак-тег остаются при своём знаке, иначе эмодзи-цепочка
/// рассыпается по строкам (`line-breaking-014`).
fn cluster_edge_at(text: &str, at: usize, anywhere: bool) -> bool {
    if !anywhere {
        return cluster_edge(text, at);
    }
    if at >= text.len() {
        return true;
    }
    let before = text[..at].chars().next_back();
    if before.is_none_or(|c| matches!(c, ' ' | '\t' | '\r' | '\n')) {
        return true;
    }
    let next = text[at..].chars().next();
    next == Some('\u{200d}') || before == Some('\u{200d}') || cluster_start(&text[at..])
}

/// Начинается ли с этого места ГРОЗДЬ знаков — то есть можно ли тут рвать.
///
/// Знаки-продолжения грозди: соединительная огласовка (класс CM по UAX-14),
/// нулевой соединитель и знаки вариации.
fn cluster_start(rest: &str) -> bool {
    let Some(ch) = rest.chars().next() else {
        return true;
    };
    // Продолжения грозди: нулевой соединитель, знаки вариации, знаки-теги
    // (флаги вроде уэльского), модификаторы тона кожи. Все они принадлежат
    // предыдущему знаку и в другую строку не уходят (`line-breaking-014`).
    if matches!(
        ch as u32,
        0x200D
            | 0xFE00..=0xFE0F
            | 0xE0100..=0xE01EF
            | 0xE0020..=0xE007F
            | 0x1F3FB..=0x1F3FF
    ) {
        return false;
    }
    !matches!(
        unicode_linebreak::break_property(ch as u32),
        unicode_linebreak::BreakClass::CombiningMark
    )
}

