//! Строчный атом абзаца в кусок (`Piece::Atom`): тело замыкания `atom` из `paragraph_pieces_routed`.
// owner: A

use crate::animation::frames::bake_frozen;
use crate::dom::Element;
use crate::layout::atom::atom_element;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::replaced::image::image;
use crate::layout::replaced::{replaced_content, svg_percentage_size};
use crate::layout::writing_mode::rotated_atom;
use crate::paint::effects::grouped::grouped;
use crate::paint::effects::transform::transformed;
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_transform;
use crate::text::vertical::combined_text;
use gpui::{IntoElement, ParentElement, Styled, div};

pub(crate) fn atom_piece(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    flow_text: bool,
) -> Option<inline::Piece> {
    let in_inline_cb = crate::text::inline::take_atom_cb();
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    // CSS 2.1 sections 10.3.8/10.6.5 use the replaced default size
    // before solving absolute insets; an empty frame is still replaced.
    let iframe_sized = (replaced_content::default_iframe(e)
        && matches!(
            e.style.position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        ))
    .then(|| replaced_content::empty_iframe_size(e, inherited, opts.viewport));
    let e = iframe_sized.as_ref().unwrap_or(e);
    // Абсолютный элемент на статической позиции ВНУТРИ строки — кусок вне
    // потока: место в строке он не занимает, поэтому абзац остаётся
    // текстовым и не теряет пробелы (`line-breaking-018`).
    // Вертикальное письмо сюда по-прежнему не пускается: там строчная ось
    // вертикальна, `point_of` отдаёт ДО-поворотные координаты, и класс
    // берётся отдельно (подкорень A3, три отката на `render.rs` «отдать
    // ось строки самому абзацу»).
    // ★ ЗАМЕРЕНО: без оговорки про вертикаль текстовый путь открывался
    // и ГОРИЗОНТАЛЬНОМУ rtl, и две зелёные пары уходили в «красное
    // видно» (`htb-rtl-ltr.tentative`, `htb-rtl-rtl` 0.16 -> 99.00) при
    // 24 приобретениях. Все приобретения — вертикальные
    // (`abs-pos-non-replaced-v{lr,rl}-*`), поэтому послабление
    // ограничено абзацем, чей содержащий блок пишет вертикально —
    // признак этого у нас `ortho_limit`, он ставится только внутри
    // вертикального содержащего блока (`element()`, `inline.rs:1012`).
    let plain_flow = inherited.vertical != Some(true)
        && (inherited.rtl != Some(true) || (flow_text && inherited.ortho_limit.is_some()));
    // Статическая позиция считается для ГИПОТЕТИЧЕСКОГО статического
    // элемента (§10.3.7: «if position had been static») — блокификация
    // `display: inline` под absolute на неё не влияет, метка
    // inline_display возвращает такой элемент в строчный путь.
    // БЛОЧНЫЙ абсолют в ПОВЁРНУТОМ абзаце вертикального письма — тоже
    // кусок вне потока, но с местом в начале СЛЕДУЮЩЕЙ строки
    // (`lines.rs: next_line_point`). Прежде он шёл атомом со щупом
    // `Spot`: атом выводил абзац из текстового пути в ряд слов, щуп
    // мерил до-поворотные координаты, а заместитель верхнего слоя
    // ложился горизонтальной полосой поперёк вертикального контейнера —
    // красное проступало у всех `css-position/static-position/v{lr,rl}-*`.
    // В повёрнутом абзаце коробка поворачивается вместе со строками, и её
    // до-поворотное место — ровно гипотетическая коробка §10.6.4.
    let rot_block = inherited.rotated_line == Some(true)
        && !inline_level(e)
        && e.style.inline_display != Some(true);
    if plain_flow
        && at_static_position(&e.style)
        && (inline_level(e) || e.style.inline_display == Some(true) || rot_block)
    {
        let mut merged = inherit(inherited, &e.style);
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
        // Сторона, которой коробка вешается на статическую точку. При
        // `direction: ltr` — левый край (текстовый путь так и кладёт,
        // `lines.rs: prepaint_at(origin)`), при `rtl` — ПРАВЫЙ:
        // css-position-3 §abs-non-replaced-width (строки 1035-1043,
        // перепись CSS 2.1 §10.3.7) — «…if the 'direction' property of the
        // element establishing the static-position containing block is
        // 'ltr' set 'left' to the static position …; otherwise, set
        // 'right' to the static-position». Точка у обеих сторон ОДНА И ТА
        // ЖЕ: замер по снимкам — ltr-двойники `-v{lr,rl}-{004,005,028,029,
        // 104,105,136,137}` ставят левый край ровно на 168.0 и все восемь
        // 0.00, а эталон rtl-пар требует 88.0..168.0, то есть ту же 168.0
        // правым краем.
        //
        // Blink разводит это на два шага: `geometry/static_position.h:86`
        // даёт `kInlineEnd` при `!IsLtr()`, а `absolute_utils.cc:27-37`
        // `GetStaticPositionInsetBias` переводит его в `InsetBias::kEnd`.
        // Сторону задаёт направление СОДЕРЖАЩЕГО блока, а не собственное
        // письмо коробки (css-writing-modes-4 §7.1, строки 1926-1931).
        let rotated_rtl = inherited.rtl == Some(true) && inherited.rotated_line == Some(true);
        let inner = if inherited.rtl == Some(true) && !rot_block && !rotated_rtl {
            crate::layout::positioned::containing_block::InlineStartHang::new(inner)
                .into_any_element()
        } else {
            inner
        };
        // Кусок кладётся КОРНЕМ в статическую точку (`lines.rs`), а корень
        // своих полей не кладёт: от статической позиции коробку отодвигает
        // её поле (CSS 2.1 §10.3.7, `margin-left` в уравнении ширины). Под
        // обёрткой коробка — обычный ребёнок, и поле на месте
        // (`CSS2/text/text-indent-013-ref`: `margin-left: -10em` — чёрная
        // полоса на 328 вместо 168 закрывала PASS).
        let nz = |l: Option<Len>| matches!(l, Some(Len::Px(v)) if v.abs() > 0.001);
        let inner = if nz(merged.margin.left) || nz(merged.margin.top) {
            div()
                .flex()
                .flex_row()
                .items_start()
                .child(inner)
                .into_any_element()
        } else {
            inner
        };
        return Some(inline::Piece::Overlay(
            inner,
            inline::OverlayAt {
                next_line: rot_block,
                bidi_hang: rotated_rtl && !rot_block,
                ..Default::default()
            },
        ));
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
        let mut merged = inherit(inherited, &e.style);
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
            crate::text::vertical::CombinedUpright::upright_box(inner, em).into_any_element(),
        ));
    }
    if let Some(piece) = combined_text::piece(e, inherited, opts) {
        return Some(piece);
    }
    // Остановленная анимация атома — запечённым кадром, как у блока в
    // `animated()`, но ВМЕСТЕ с `rotate`/`scale`/`transform`: матрицу
    // атома строит `transformed()` ниже от `e.style`. В `animated()` атомы
    // не заходят вовсе, и кадр `individual-transform-combine` (шесть
    // `inline-block` под `animation-delay: -500000s`) не доходил ни до
    // сдвига, ни до матрицы (css-transforms-1 §transformable-element).
    let frozen_atom;
    let e = match bake_frozen(e, true) {
        Some(b) => {
            frozen_atom = b;
            &frozen_atom
        }
        None => e,
    };
    let ruby_atom;
    let e = match ruby_transform::used(e) {
        Some(used) => {
            ruby_atom = used;
            &ruby_atom
        }
        None => e,
    };
    // Боковые поля атома с собственным прижимом несёт ОБЁРТКА: внутри
    // неё они сдвигают коробку, но в продвижение строки не входят —
    // следующий кусок наезжал на предыдущий ровно на его поле
    // (эталоны `fixed-table-layout-021..023`: `img{vertical-align:top}`
    // плюс `margin-left`).
    let wrapped = matches!(
        e.style.vertical_align,
        Some(crate::style::computed::Align::Start)
            | Some(crate::style::computed::Align::End)
            | Some(crate::style::computed::Align::Center)
    );
    let original_margin = rotated_atom::margin(&e.style, inherited);
    let bare;
    let e = if wrapped {
        let mut copy = e.clone();
        copy.style.margin = Default::default();
        bare = copy;
        &bare
    } else {
        e
    };
    // An absolutely positioned box inside a rotated vertical paragraph is
    // blockified (CSS 2.1 §9.7) and inherits the vertical writing mode
    // (css-writing-modes-4 §2.1): it is its own vertical block, not a
    // piece of the horizontal pre-rotation paragraph, whose clone has
    // `vertical` cleared. Carry the writing mode on the box itself.
    let vertical_abs;
    let e = if (inherited.rotated_line == Some(true) || inherited.upright_stack)
        && e.style.vertical.is_none()
        && matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && !at_static_position(&e.style)
        && !replaced_tag(e)
    {
        let mut copy = e.clone();
        copy.style.vertical = Some(true);
        copy.style.vertical_rl = Some(inherited.vertical_rl == Some(true));
        copy.style.sideways = Some(inherited.sideways == Some(true));
        vertical_abs = copy;
        &vertical_abs
    } else {
        e
    };
    crate::text::inline::set_atom_cb(in_inline_cb);
    let built = atom_element(e, inherited, opts);
    crate::text::inline::set_atom_cb(false);
    let abs_cb = crate::text::inline::take_abs_cb();
    built.map(|el| {
        // `mix-blend-mode` на ЗАМЕЩАЕМОМ атоме строки: блочный путь,
        // атом с коробкой и флоат смешивают через `grouped`, а `<svg>`,
        // `<iframe>`, `<img>` в строке шли мимо, и режим пропадал
        // (`mix-blend-mode-svg`, `-iframe-parent`, `-iframe-sibling`:
        // красный квадрат вместо зелёного). Носитель несёт ТОЛЬКО режим:
        // маска и обрезка у атомов живут своими путями, полный стиль
        // применил бы их второй раз.
        let el = if replaced_tag(e) && e.style.blend.is_some_and(|b| b != 0) {
            let mut only = Computed::default();
            only.blend = e.style.blend;
            grouped(el, &only)
        } else {
            el
        };
        // Атомарный строчный — transformable element (css-transforms-1
        // §transformable-element: всё по блочной модели, «except for
        // non-replaced inline boxes»): `img`, `iframe`, `inline-block`,
        // `inline-table`, строчный `<svg>`. Обёртку получали только поля
        // форм — они и сейчас заворачиваются в `atom_element`, здесь их
        // пропускаем. Абсолюты через пустышку статической позиции не
        // трогаем: обёртка на нулевой пустышке взяла бы origin от нуля.
        let el = if matches!(
            e.tag.as_str(),
            "input" | "textarea" | "select" | "progress" | "meter"
        ) || matches!(
            e.style.position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) {
            el
        } else {
            transformed(el, &e.style, inherited)
        };
        // `vertical-align` НА САМОМ куске (`img { vertical-align: top }`):
        // ряд строит базовую линию, а кускам с top/middle/bottom нужен
        // собственный прижим (wm-propagation-body-033-ref: полоса-картинка
        // в строке с квадратом прижата к верху, у нас висела на базовой).
        // ПРОБОВАЛИ И ОТКАТИЛИ: выражать `text-top`/`text-bottom` у
        // атомарного куска прижимом к краю строки. Замерено по семьям
        // linebox/*, css1/*, *vertical*: 0 и 0 — этим парам нужен сдвиг
        // относительно ТЕКСТОВОЙ области родителя, а не край строки.
        use crate::style::computed::Align;
        let self_align = match e.style.vertical_align {
            Some(Align::Start) => Some(gpui::AlignItems::FlexStart),
            Some(Align::End) => Some(gpui::AlignItems::FlexEnd),
            Some(Align::Center) => Some(gpui::AlignItems::Center),
            _ => None,
        };
        let el = match self_align {
            Some(a) => {
                let mut w = crate::style::apply::margins(div().flex_shrink_0(), &original_margin);
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
            && e.style.position == Some(crate::style::computed::Position::Relative)
        {
            crate::paint::effects::underlay::Underlay::new(el).into_any_element()
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
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        ) && !at_static_position(&e.style)
            && !matches!(
                e.tag.as_str(),
                "svg" | "img" | "canvas" | "video" | "embed" | "object" | "iframe"
            )
        {
            return inline::Piece::Overlay(
                el,
                inline::OverlayAt {
                    edges: abs_cb,
                    ..Default::default()
                },
            );
        }
        inline::Piece::Atom(el)
    })
}
