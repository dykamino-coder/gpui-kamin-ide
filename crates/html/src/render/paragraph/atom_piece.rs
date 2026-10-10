//! Строчный атом абзаца в кусок (`Piece::Atom`): тело замыкания `atom` из `paragraph_pieces_routed`.
mod static_overlay;
pub(super) use static_overlay::static_overlay;

mod decoration;
pub(super) use decoration::decorate_atom;

// owner: A

use crate::animation::frames::bake_frozen;
use crate::dom::Element;
use crate::layout::atom::atom_element;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::replaced::{replaced_content, svg_percentage_size};
use crate::layout::writing_mode::rotated_atom;
use crate::render::*;
use crate::style::cascade::inherit::inherit;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_transform;
use crate::text::vertical::combined_text;
use gpui::{IntoElement, ParentElement};

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
        return static_overlay(e, inherited, opts, rot_block);
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
    built.map(|el| decorate_atom(el, e, inherited, original_margin, abs_cb))
}
