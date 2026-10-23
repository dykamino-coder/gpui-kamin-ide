//! Строчный атом абзаца в кусок (`Piece::Atom`): тело замыкания `atom` из
//! `paragraph_pieces_routed`.
use crate::layout::box_kind::inline_level;
use crate::layout::box_kind::out_of_flow;
use crate::layout::box_kind::replaced_tag;
use crate::layout::options::RenderOpts;
use crate::paint::box_tree::styled::styled_div_with;
use crate::render::blocks::blocks;
mod static_overlay;
#[cfg(test)]
mod tests;
pub(super) use static_overlay::static_overlay;
mod decoration;
pub(super) use decoration::decorate_atom;
mod atom_cx;
use crate::animation::frames::bake_frozen;
use crate::dom::Element;
use crate::hooks::IntoHooked;
use crate::layout::atom::atom_element;
use crate::layout::positioned::static_position::at_static_position;
use crate::layout::replaced::{replaced_content, svg_percentage_size};
use crate::layout::writing_mode::rotated_atom;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use crate::text::inline;
use crate::text::ruby::ruby_transform;
use crate::text::vertical::combined_text;
pub(crate) use atom_cx::AtomRuby;
use gpui::{IntoElement, ParentElement};

pub(crate) fn atom_piece(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    flow_text: bool,
    atom_cx: &inline::atom_parent::AtomCx,
    ruby: &AtomRuby,
) -> Option<inline::Piece> {
    // Атом внутри строчного предка наследует от НЕГО (`atom_parent`).
    // Пока только атомы В ПОТОКЕ: абсолют внутри `rtl`-спана, унаследовав
    // его направление, ставит статическую позицию от правого края, а наш
    // расчёт этого не умеет (`static-position/htb-ltr-rtl.tentative`
    // краснел) — внепоточным прежний путь.
    let parent_style = atom_cx
        .parent
        .as_ref()
        .filter(|_| !out_of_flow(&e.style.computed()));
    let inherited = parent_style.unwrap_or(inherited);
    let child_opts = opts.inner(|cx| {
        cx.clamp_atom_depth =
            if e.style.computed().clamp_lines().is_some() || e.style.computed().clamp_by_height() {
                0
            } else {
                cx.clamp_atom_depth + 1
            };
    });
    atom_piece_in(
        e,
        inherited,
        &child_opts,
        flow_text,
        atom_cx.in_inline_cb,
        ruby,
    )
}

fn atom_piece_in(
    e: &Element,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    flow_text: bool,
    in_inline_cb: bool,
    ruby: &AtomRuby,
) -> Option<inline::Piece> {
    let vcx = opts.cx.vcx();
    let svg_sized = svg_percentage_size::resolve(e, inherited);
    let e = svg_sized.as_ref().unwrap_or(e);
    let media_sized =
        crate::layout::replaced::media::resolve(e, inherited, opts.viewport, opts.cx.avail_w, vcx);
    let e = media_sized.as_ref().unwrap_or(e);
    // CSS 2.1 sections 10.3.8/10.6.5 use the replaced default size
    // before solving absolute insets; an empty frame is still replaced.
    let iframe_sized = (replaced_content::default_iframe(e)
        && matches!(
            e.style.computed().position,
            Some(
                crate::style::computed::Position::Absolute
                    | crate::style::computed::Position::Fixed
            )
        ))
    .then(|| replaced_content::empty_iframe_size(e, inherited, opts.viewport, vcx));
    let e = iframe_sized.as_ref().unwrap_or(e);
    // Абсолютный элемент на статической позиции ВНУТРИ строки — кусок вне
    // потока: место в строке он не занимает, поэтому абзац остаётся
    // текстовым и не теряет пробелы (`line-breaking-018`).
    // Вертикальное письмо сюда не пускается (`point_of` отдаёт до-поворотные
    // координаты). Горизонтальный rtl — только в абзаце внутри вертикального
    // содержащего блока (`ortho_limit`), иначе ломаются `htb-rtl-*`; блок общей
    // полосы (`band_kid`) без потокового текста хранит статическую точку.
    let plain_flow = !inherited.wm().vertical()
        && (!inherited.wm().rtl()
            || (flow_text && inherited.ortho_limit.is_some())
            || (inherited.band_kid && !flow_text));
    // Статическая позиция — для гипотетической статической коробки (§10.3.7):
    // метка `inline_display` возвращает блокифицированный `display: inline` в
    // строчный путь. БЛОЧНЫЙ абсолют в повёрнутом абзаце — кусок вне потока в
    // начале следующей строки (`next_line_point`): поворачивается вместе со
    // строками, и его до-поворотное место — гипотетическая коробка §10.6.4.
    let rot_block = inherited.wm().turned()
        && !inline_level(e)
        && e.style.computed().inline_display != Some(true);
    if plain_flow
        && (!in_inline_cb || inherited.wm().turned())
        && at_static_position(&e.style.computed())
        && (inline_level(e)
            || e.style.computed().inline_display == Some(true)
            || rot_block
            || inherited.band_kid)
    {
        return static_overlay(e, inherited, opts, rot_block);
    }
    // Не контр-поворачивать физические четвёрки краёв (`margin`/`padding`/
    // `border-width` и четвёрки цвета, стиля, видимости) у КАЖДОГО куска
    // повёрнутого абзаца, хотя поворот уносит их с собой, а стороны письмом
    // не переставляются (§3.2): `grid-self-baseline-*` держит отсутствие
    // физической высоты повёрнутого блока, а `wm-propagation-body-049`
    // ломается. Вернуть можно только вместе с высотой повёрнутого блока
    // по содержимому (верное направление — по часовой: верх←право,
    // право←низ, низ←лево, лево←верх).
    // Стоячая коробка в повёрнутом абзаце: `inline-block` с явным
    // ГОРИЗОНТАЛЬНЫМ письмом контр-поворачивается — его содержимое
    // обязано стоять прямо (эмуляция tcy в эталонах compression-*).
    if inherited.wm().turned()
        && e.style.computed().display == Some(Display::InlineBlock)
        && e.style.cascaded().vertical_decl() == Some(false)
    {
        // An orthogonal atomic inline keeps its physical box and own writing
        // mode (css-writing-modes-4 §4.3); the line synthesizes its baseline
        // from the box's line-under edge, or its centre for a central
        // baseline (css-align-3 §9.1; Blink LogicalBoxFragment synthesis).
        // Measured against Chrome for alphabetic (sideways) lines only; a
        // central-baseline line keeps the tcy emulation below
        // (text-combine-upright-compression-007 moved its CJK glyphs).
        let alphabetic = inherited.wm().alphabetic_baseline();
        if alphabetic && let Some(physical) = rotated_atom::physical(e, inherited, opts) {
            return Some(inline::Piece::Atom(physical));
        }
        let mut merged = crate::style::cascade::materialize::box_style(inherited, &e.style, vcx);
        merged.turned_line = false;
        let em = match merged.width {
            Some(Len::Px(v)) => v,
            _ => match merged.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            },
        };
        let inner = styled_div_with(e, &merged, &opts.cx)
            .children(blocks(&e.children, &merged, opts))
            .into_any_element();
        return Some(inline::Piece::Atom(
            crate::text::vertical::CombinedUpright::upright_box(inner, em).hooked(),
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
    let e = match bake_frozen(e, true, vcx) {
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
        e.style.own_vertical_align(),
        Some(crate::style::computed::Align::Start)
            | Some(crate::style::computed::Align::End)
            | Some(crate::style::computed::Align::Center)
    );
    let original_margin = rotated_atom::margin(&e.style.computed(), inherited);
    let bare;
    let e = if wrapped {
        let mut copy = e.clone();
        copy.style.used_mut().margin = Default::default();
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
    let e = if (inherited.wm().turned() || inherited.upright_stack)
        && e.style.cascaded().writing_mode.is_none()
        && matches!(
            e.style.computed().position,
            Some(crate::style::computed::Position::Absolute)
                | Some(crate::style::computed::Position::Fixed)
        )
        && !at_static_position(&e.style.computed())
        && !replaced_tag(e)
    {
        let mut copy = e.clone();
        copy.style.used_mut().writing_mode =
            Some(crate::style::computed::writing_mode::WritingModeKw::new(
                true,
                inherited.wm().block_rl(),
                inherited.wm().sideways(),
            ));
        vertical_abs = copy;
        &vertical_abs
    } else {
        e
    };
    // Внепоточный атом может стать распоркой статической позиции в строке:
    // строчную коробку ей передаёт абзац (`LineBoxSlot`).
    // Позиционированный строчный предок предлагается атому содержащим
    // блоком (`InlineCbOffer`); предложение — только этому атому.
    let line_box = out_of_flow(&e.style.computed())
        .then(crate::layout::positioned::containing_block::LineBoxSlot::default);
    let offer = in_inline_cb.then(crate::text::inline::InlineCbOffer::offered);
    let opts = &*atom_cx::atom_opts(opts, ruby, &line_box, &offer);
    let built = atom_element(e, inherited, opts);
    let abs_cb = offer.as_ref().is_some_and(|o| o.taken());
    built.map(|el| decorate_atom(el, e, inherited, original_margin, abs_cb, line_box, vcx))
}
