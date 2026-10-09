//! Буквица `initial-letter` как флоат.
// owner: A

use crate::render::*;

/// `initial-letter` (css-inline-3 §initial-letter): буквица — не кусок
/// текста, а коробка В НАЧАЛЕ БЛОКА, которую строки обтекают. Эталоны WPT
/// пишут её плавающим блоком (`initial-letter-drop-initial-ref`: `float:
/// left; width: 80px; height: 80px; margin-top: 2px` при `font: 20px/24px
/// Ahem` и `initial-letter: 3`), и наше обтекание (`wrap_floats` →
/// `kamin-float` → `float_flow`) такой узел уже ведёт — поэтому буквица
/// расшивается в синтетический флоат ДО `wrap_floats`, и обе стороны пары
/// идут одним путём.
///
/// Числа (§sizing-initial-letter; Blink `ComputeInitialLetterFont` и
/// `initial_letter_utils.cc::ComputeInitialLetterBoxBlockOffset`):
/// * прописная буквицы C = (N − 1)·line-height + cap(абзаца);
/// * кегль F = C / доля прописной шрифта буквицы (Ahem: 64 / 0.8 = 80);
/// * коробка высотой ascent(F) + descent(F), строка той же высоты;
/// * верх коробки = N·line-height − ascent(F) − (descent(абзаца) +
///   полулидинг) — у Ahem/20/24/3 ровно 2;
/// * осадка M < N: строки под буквицей уходят на (N − M) строк вниз — как
///   `<br>` перед текстом в эталонах `raise`/`sunk`.
///
/// Собственные `font-size` и `line-height` слоя НЕ действуют
/// (§initial-letter-properties). Шаг 1: горизонтальное письмо, буква —
/// первый текстовый узел блока (перед ним допустимы только флоаты и пустой
/// текст), ширина коробки — продвижение нуля семейства (у Ahem равно
/// кеглю; текстовым шрифтам нужен щуп продвижения знака — шаг 2).
pub(crate) fn initial_letter_float(nodes: Vec<Node>, inherited: &Computed, opts: &RenderOpts) -> Vec<Node> {
    let Some(first) = inherited.first_letter.as_deref() else {
        return nodes;
    };
    let Some((size_lines, sink)) = first.initial_letter else {
        return nodes;
    };
    // Вертикальное письмо больше НЕ отсекается. Эталоны семейства пишут в
    // вертикали ту же плавающую коробку 80×80 с тем же `float: left`
    // (`initial-letter-drop-initial-vrl-ref` и ещё пятнадцать), меняются
    // ровно две вещи: поле сдвига стоит на БЛОК-СТАРТЕ (`margin-right` при
    // `*-rl`, `margin-left` при `*-lr` — css-writing-modes-4 §6.3, строка
    // `block-start`), а величина сдвига у `vertical-*` считается
    // центрированием, а не по алфавитной базовой.
    let vert = inherited.vertical == Some(true);
    // `sideways-*` типографски ГОРИЗОНТАЛЕН: у него алфавитная базовая и та
    // же формула, что в горизонтали. Blink `initial_letter_utils.cc:81-83`
    // разводит ветки условием
    // `IsHorizontalTypographicMode() || text-orientation: sideways`, а
    // `sideways-rl`/`sideways-lr` попадают во вторую половину этого «или».
    let sideways = inherited.sideways == Some(true);
    // Блок-старт вертикали: правый край при `vertical-rl`/`sideways-rl`,
    // левый при `vertical-lr`/`sideways-lr`.
    let block_rl = inherited.vertical_rl == Some(true);
    let at = nodes.iter().position(|n| match n {
        Node::Text(t) => !blank_text(t),
        Node::Element(e) => !e.style.float.is_some_and(|f| f != 0),
    });
    let Some(at) = at else {
        return nodes;
    };
    let Node::Text(text) = &nodes[at] else {
        return nodes;
    };
    let Some((pos, ch)) = text.char_indices().find(|(_, c)| !c.is_whitespace()) else {
        return nodes;
    };
    let end = pos + ch.len_utf8();
    let font_px = match inherited.font_size {
        Some(Len::Px(v)) => v,
        _ => opts.base_size(),
    };
    let family = inherited.font_family.clone().unwrap_or_default();
    let (asc, desc, cap) = crate::metrics::vmetrics_px(&family, font_px);
    let line = match inherited.line_height {
        Some(Len::Px(v)) => v,
        Some(Len::Pct(k)) | Some(Len::Em(k)) => k * font_px,
        _ => font_px * normal_fraction(inherited, opts),
    };
    // Доля прописной — у шрифта БУКВИЦЫ: слой может сменить семейство.
    let letter_family = first.font_family.clone().unwrap_or_else(|| family.clone());
    let (_, _, cap_frac) = crate::metrics::vmetrics_px(&letter_family, 1.0);
    if cap_frac <= 0.0 || line <= 0.0 {
        return nodes;
    }
    let want_cap = (size_lines - 1.0) * line + cap;
    let letter_px = want_cap / cap_frac;
    let (l_asc, l_desc, _) = crate::metrics::vmetrics_px(&letter_family, letter_px);
    let box_h = l_asc + l_desc;
    let half_leading = (line - (asc + desc)) / 2.0;
    // Размер меньше осадки (`3 5`) — выравнивание по верху
    // (§initial-letter-block-position): коробка опускается на sink строк.
    let rows = size_lines.ceil() as u32;
    let top = if rows < sink {
        line * sink as f32 - box_h
    } else if vert && !sideways {
        // Вертикальное письмо со СМЕШАННОЙ ориентацией: базовая линия
        // центральная, и коробка выравнивается по центру строки. Blink
        // `initial_letter_utils.cc:99-101` дословно:
        //   // In vertical writing mode, `block_offset` will be physical
        //   // offset x. Align initial letter box in center.
        //   return (line_height * size - block_size) / 2;
        // Ahem 20px/24px и `initial-letter: 3`: (3·24 − 80)/2 = −4 — ровно
        // `margin-right: -4px` эталона `initial-letter-drop-initial-vrl-ref`
        // и `margin-left: -4px` эталона `-vlr-ref`.
        (size_lines * line - box_h) / 2.0
    } else {
        size_lines * line - l_asc - (desc + half_leading)
    };
    let shift = rows.saturating_sub(sink);
    // Слой — копия стиля блока плюс объявления `::first-letter`, поэтому
    // «своё» у слоя — то, что отличается от блока: поля, цвет, фон.
    let own = |layer: Option<Len>, base: Option<Len>| match layer {
        Some(Len::Px(v)) if layer != base => v,
        _ => 0.0,
    };
    let color = match first.color {
        Some(c) if Some(c) != inherited.color => Some(c),
        // `::first-letter` наследует у `::first-line`
        // (`initial-letter-with-first-line`: `color: inherit` → цвет строки).
        _ => inherited
            .first_line
            .as_deref()
            .and_then(|l| l.color)
            .or(inherited.color),
    };
    let rtl = inherited.rtl == Some(true);
    // Отступ первой строки (css-inline-3 §initial-letter-indentation:
    // «'text-indent' … cause a shift in the start of the line's contents
    // including the initial letter itself»). Blink сдвигает КОРОБКУ буквицы
    // на отступ (`inline_layout_algorithm.cc`: `bfc_line_offset +=
    // TextIndent()` до `PostPlaceInitialLetterBox`), а своя строка внутри
    // коробки наследует тот же `text-indent`, и ширина коробки его включает
    // (`CalculateInitialLetterBoxInlineSize`). Итог — эталон
    // `initial-letter-indentation-ref`: при `text-indent: 10px` квадрат стоит
    // с `margin-left: 20px`, строки обтекают от 100. У нас знак уже сдвигался
    // унаследованным отступом, а коробка — нет: знак вылезал на 10 точек за
    // флоат, строки 2-4 обтекали по 80 с нахлёстом. Только горизонталь ltr:
    // `-indentation-rtl` зелёный (0.41) на зеркальной ошибке `float: right`
    // в rtl, и менять его сторону нечем.
    let para_indent = match inherited.text_indent {
        Some(Len::Px(v)) if inherited.text_indent_hanging != Some(true) => v,
        _ => 0.0,
    };
    let indent = if !vert && !rtl && para_indent > 0.0 {
        para_indent
    } else {
        0.0
    };
    // Сохранённые пробелы ПЕРЕД буквой входят в буквицу: Blink
    // `FirstLetterPseudoElement::FirstLetterLength` сперва забирает ведущие
    // пробелы, и при `white-space: pre` табуляция остаётся в коробке
    // буквицы. Эталон `initial-letter-with-tab-ref` пишет перед квадратом
    // 80×80 жёлтый (фон слоя) флоат шириной 160 = шаг табуляции АБЗАЦА
    // (8 × 20px Ahem), и текст идёт с 240; раньше ведущий `\t` просто
    // выбрасывался. Шаг — та же формула, что у `tab_stop` абзаца
    // (`tab-size` × ширина `0`, css-text-3 §tab-size); пробел меряется той же
    // шириной — точной ширины пробела здесь нет, у Ahem они равны.
    let lead = &text[..pos];
    let lead_w = if !vert
        && inherited.keep_spaces == Some(true)
        && !lead.contains(|c: char| c == '\n' || c == '\r')
    {
        let space = crate::metrics::ch_ex_px(&family, font_px).0;
        let stop = match inherited.tab_size_len {
            Some(Len::Px(v)) if v > 0.0 => v,
            _ => inherited.tab_size.unwrap_or(8.0).max(0.0) * space,
        };
        lead.chars().fold(0.0f32, |x, c| match c {
            '\t' if stop > 0.0 => ((x / stop).floor() + 1.0) * stop,
            ' ' => x + space,
            _ => x,
        })
    } else {
        0.0
    };
    let mut style = Computed {
        // Сторона — начало строки: rtl отправляет буквицу вправо.
        float: Some(if rtl { 1 } else { -1 }),
        // Оба размера в точках: без них `float_flow` откатывается на плоский
        // ряд, и строки под буквицей не возвращаются к левому краю. Ширина
        // несёт и внутренний отступ строки буквицы, и ведущие пробелы:
        // `float_flow` меряет обтекание по ней (`float.rs` `narrow`).
        width: Some(Len::Px(
            crate::metrics::ch_ex_px(&letter_family, letter_px).0 + indent + lead_w,
        )),
        height: Some(Len::Px(box_h)),
        font_size: Some(Len::Px(letter_px)),
        line_height: Some(Len::Px(box_h)),
        font_family: Some(letter_family),
        font_weight: first.font_weight,
        italic: first.italic,
        color,
        background: (first.background != inherited.background)
            .then_some(first.background)
            .flatten(),
        ..Computed::default()
    };
    // Картинки фона слоя первой буквы (css-pseudo-4 §3.6: к `::first-letter`
    // применимы все свойства фона) — со своими размером, положением,
    // повтором и списками слоёв. Прежде переносился только цвет, и вместо
    // зелёных картинок проступал красный цвет фона (`background-image-007`).
    if first.bg_image.is_some() || first.gradient.is_some() || !first.bg_lists.is_empty() {
        style.bg_image = first.bg_image.clone();
        style.gradient = first.gradient.clone();
        style.gradient_raw = first.gradient_raw.clone();
        style.bg_size = first.bg_size;
        style.bg_pos = first.bg_pos;
        style.bg_repeat = first.bg_repeat;
        style.bg_origin = first.bg_origin;
        style.bg_clip = first.bg_clip;
        style.bg_lists = first.bg_lists.clone();
    }
    // Сдвиг ложится на поле БЛОК-СТАРТА и СКЛАДЫВАЕТСЯ с полем слоя — ровно
    // так же, как в эталонах: `block-position-margins-vrl` задаёт слою
    // `margin-right: 45px`, а его эталон пишет `margin-right: 41px`
    // = 45 + (−4); у `-vlr` то же на левом краю — `margin-left: 11px`
    // = 15 + (−4).
    let (mt, ml, mr) = (
        own(first.margin.top, inherited.margin.top),
        own(first.margin.left, inherited.margin.left),
        own(first.margin.right, inherited.margin.right),
    );
    style.margin.top = Some(Len::Px(if vert { mt } else { mt + top }));
    style.margin.bottom = Some(Len::Px(own(first.margin.bottom, inherited.margin.bottom)));
    style.margin.left = Some(Len::Px(if vert && !block_rl {
        ml + top
    } else {
        ml + indent
    }));
    style.margin.right = Some(Len::Px(if vert && block_rl { mr + top } else { mr }));
    // Знак встаёт ЗА ведущими пробелами: внутренний отступ строки флоата =
    // отступ абзаца + их ширина (в rtl отступ идёт от правого края коробки,
    // и пробелы остаются у начала строки). Без ведущих пробелов отступ
    // наследуется, как раньше.
    if lead_w > 0.0 {
        style.text_indent = Some(Len::Px(para_indent + lead_w));
    }
    let synthetic = |tag: &str, style: Computed, children: Vec<Node>, inline: bool| {
        Node::Element(Element {
            list_item: None,
            node_id: 0,
            anim: None,
            tag: tag.into(),
            style,
            hover: None,
            first_letter: None,
            first_line: None,
            children,
            attrs: vec![],
            inline,
        })
    };
    let mut out: Vec<Node> = Vec::with_capacity(nodes.len() + 2 + shift as usize);
    out.extend(nodes[..at].iter().cloned());
    let mut letter = synthetic("div", style, vec![Node::Text(text[pos..end].to_string())], false);
    // Метка буквицы: её место — исключение строки (css-inline-3
    // §initial-letter, Blink `initial_letter_utils.cc`), а не флоат полос:
    // измеряемый хост ставит её `FloatBands::add_initial_letter` (шаг F11);
    // прогон с руби хост по-прежнему не берёт (`initial-letter-*-ruby`).
    if let Node::Element(e) = &mut letter {
        e.attrs.push(("initial-letter".into(), "1".into()));
    }
    out.push(letter);
    for _ in 0..shift {
        out.push(synthetic("br", Computed::default(), vec![], true));
    }
    if end < text.len() {
        out.push(Node::Text(text[end..].to_string()));
    }
    out.extend(nodes[at + 1..].iter().cloned());
    out
}

/// Флоат, ради которого строчная коробка и существует, — и ничего кроме него.
///
/// Содержащий блок флоата — ближайший БЛОЧНЫЙ предок (§10.1: «the containing
/// block is formed by the content edge of the nearest block container ancestor
/// box»), а не строчная коробка, внутри которой он записан. Правило 1 §9.5.1
/// держит его внешний край у края СОДЕРЖАЩЕГО БЛОКА, поэтому отбивка, рамка и
/// поле `<span>` флоат не двигают ни на точку. Сегодня двигают: `wrap_floats`
/// смотрит только на список братьев (проверка `floated` ниже не рекурсивная),
/// а разделение на строчное и блочное (`:3742`) исключает из прогона лишь
/// ПРЯМОГО плавающего ребёнка. Завёрнутый в `<span>` флоат уезжает в абзац и
/// встаёт от содержательного края строчной коробки — в `float-in-inline-001`
/// это ровно 30 + 30 + 40 = 100 точек вправо и вниз.
///
/// Возвращается сам флоат; строчная обёртка выбрасывается. Терять с ней
/// нечего: своего содержимого у неё нет, а рамку и отбивку строчной коробки
/// БЕЗ фона мы и так не рисуем (корень `INLINE-BOX-PAINT`) — поэтому гейт
/// требует отсутствия фона.
///
/// Проба (`target/scout-floatline-2026-09.md` §5.4): дерево ПОСЛЕ снятия
/// обёртки сходится с настоящими эталонами `ref-filled-green-200px-square` и
/// `ref-filled-green-100px-square` в 0.00 на всех трёх парах подкорня.
///
/// Гейт узкий нарочно: обёртка — НАСТОЯЩАЯ строчная (`display: inline` либо
/// строчный по тегу и без своего `display`), не позиционированная, без
/// `clear`, ничего не красящая и не образующая ГРУППУ, без стилей
/// `:hover`/`::first-letter`/`::first-line`, а внутри неё — только пустой
/// текст и РОВНО ОДИН элемент: флоат либо такая же обёртка
/// (`float-in-inline-002`: `<span><span><span style="float:left">`).
/// Leading float of an inline wrapper (see `wrap_floats`): the wrapper (or a
/// chain of such wrappers) must be a genuine, non-positioned inline that
/// forms no group (opacity, filter, transform, … act on the float through
/// it), and the float must be its first in-flow content — only blank text
/// before it. Returns the float and the wrapper without it.
pub(crate) fn split_leading_float(e: &Element) -> Option<(Element, Element)> {
    let genuine_inline =
        (e.inline && e.style.display.is_none()) || e.style.inline_display == Some(true);
    // Ruby boxes are not plain inline wrappers: their content is paired into
    // bases and annotations (css-ruby-1 §2.2) and laid out by the ruby path.
    if !genuine_inline
        || matches!(e.tag.as_str(), "br" | "ruby" | "rb" | "rt" | "rtc" | "rp")
        || e.style.float.is_some_and(|f| f != 0)
        || e.style.position.is_some()
        || e.hover.is_some()
        || e.style.opacity.is_some()
        || e.style.filter.is_some()
        || e.style.transform.is_some()
        || e.style.blend.is_some()
        || e.style.clip_polygon.is_some()
        || e.style.mask_image.is_some()
    {
        return None;
    }
    for (i, n) in e.children.iter().enumerate() {
        match n {
            Node::Text(t) if blank_text(t) => continue,
            Node::Text(_) => return None,
            Node::Element(c) => {
                if c.style.float.is_some_and(|f| f != 0)
                    && c.style.position.is_none()
                    && c.style.display != Some(Display::None)
                {
                    let mut rest = e.clone();
                    rest.children.remove(i);
                    // The float leaves its wrapper but keeps what it
                    // inherited through it (`run-in-contains-inline-007`:
                    // bold of the run-in).
                    let mut float = c.clone();
                    carry_inherited(&e.style, &mut float.style);
                    return Some((float, rest));
                }
                let (mut float, inner) = split_leading_float(c)?;
                carry_inherited(&e.style, &mut float.style);
                let mut rest = e.clone();
                rest.children[i] = Node::Element(inner);
                return Some((float, rest));
            }
        }
    }
    None
}

/// Inherited values a hoisted float takes from the inline wrapper it left
/// (only those the wrapper sets itself; `inline::inherit` would also resolve
/// font-relative units against the bare wrapper style).
pub(crate) fn carry_inherited(wrapper: &Computed, own: &mut Computed) {
    own.color = own.color.or(wrapper.color);
    own.font_weight = own.font_weight.or(wrapper.font_weight);
    own.italic = own.italic.or(wrapper.italic);
    if own.font_family.is_none() {
        own.font_family = wrapper.font_family.clone();
    }
}

pub(crate) fn inline_float_host(e: &Element) -> Option<Element> {
    // `display: inline` после каскада — это `InlineBlock` с пометкой
    // `inline_display` (`computed.rs`), поэтому одного взгляда на `display`
    // мало; тег без своего `display` даёт строчность через `e.inline`.
    let genuine_inline =
        (e.inline && e.style.display.is_none()) || e.style.inline_display == Some(true);
    if !genuine_inline
        || e.style.float.is_some_and(|f| f != 0)
        || e.style.clear.is_some()
        || e.style.position.is_some()
        || e.hover.is_some()
        || e.first_letter.is_some()
        || e.first_line.is_some()
        // Обёртка КРАСИТ: фон, картинка и градиент ушли бы вместе с ней.
        || e.style.background.is_some()
        || e.style.bg_image.is_some()
        || e.style.gradient.is_some()
        // Обёртка образует ГРУППУ: прозрачность, фильтр, трансформ,
        // смешивание, обрезка и маска действуют на плавающего ребёнка ЧЕРЕЗ
        // неё. `css-color/inline-opacity-float-child` (зелёная, 0.00) держится
        // ровно на этом: `opacity: 0` на `<span>` гасит красный флоат внутри,
        // и снятая обёртка проявила бы красное.
        || e.style.opacity.is_some()
        || e.style.filter.is_some()
        || e.style.transform.is_some()
        || e.style.blend.is_some()
        || e.style.clip_polygon.is_some()
        || e.style.mask_image.is_some()
    {
        return None;
    }
    // Ровно один элемент и сколько угодно пустого текста. Непустой текст,
    // второй элемент, `<br>` — обёртка несёт СВОЁ содержимое, снимать её
    // нельзя: строчный прогон разъедется. Этим же условием из-под патча
    // выведены `below-float`, `float-nowrap-3/9` и `block-in-inline-margins-004`.
    let mut only: Option<&Element> = None;
    for n in &e.children {
        match n {
            Node::Text(t) if blank_text(t) => {}
            Node::Element(c) if only.is_none() => only = Some(c),
            _ => return None,
        }
    }
    let inner = only?;
    if inner.style.float.is_some_and(|f| f != 0) {
        return Some(inner.clone());
    }
    inline_float_host(inner)
}

/// Ширина margin-box по строчной оси в точках (`auto`-поле — ноль, как в
/// `px_margin`). Нижняя оценка для флоата без своей ширины: shrink-to-fit не
/// меньше нуля.
pub(crate) fn px_margin_w(c: &Computed) -> Option<f32> {
    let b = c.borders();
    Some(
        px_of2(&c.width)?
            + px_of2(&c.padding.left)?
            + px_of2(&c.padding.right)?
            + px_of2(&b.left)?
            + px_of2(&b.right)?
            + px_margin(&c.margin.left)?
            + px_margin(&c.margin.right)?,
    )
}
