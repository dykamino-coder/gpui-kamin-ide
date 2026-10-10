//! Коробка элемента: базовый стиль div, обрезка и покраска фона за вычетом области.

mod clipping;
pub(super) use clipping::clip_layer;
pub(crate) use clipping::paint_rect_minus;
use clipping::shows_inside;

use crate::dom::Element;
use crate::layout::block::margin_height;
use crate::layout::multicol::spanner::multicol_container;
use crate::layout::positioned::absolute_overflow;
use crate::layout::replaced::limits::auto_clamp_limit;
use crate::layout::writing_mode::native_intrinsic;
use crate::paint::decorations::decorations;
use crate::paint::effects::mask::mask_def;
use crate::render::*;
use crate::style::apply::{apply, apply_hover};
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{ParentElement, Styled, div, px};

/// Базовый стиль элемента плюс слой наведения и дорисовка того, чего в
/// `gpui::Style` нет: обводки, размытия подложки, разноцветных сторон рамки.
pub(crate) fn styled_div(e: &Element) -> gpui::Div {
    styled_div_with(e, &e.style)
}

pub(crate) fn styled_div_with(e: &Element, style: &Computed) -> gpui::Div {
    // Скрытая коробка с видимым потомком не прячется целиком: раскладка та
    // же, гаснет только СВОЯ краска — иначе ранний возврат из отрисовки
    // уносит и потомка (`visufx/visibility-005`).
    let bare;
    // Скрытая коробка с `filter: url()`: исходная картинка прозрачна, но
    // примитивы без источника (`feFlood`) свой выход дают — фильтр остаётся
    // (`visibility-hidden-element-with-filter-001`: зелёная заливка видна).
    let c = if style.hidden == Some(true)
        && (shows_inside(&e.children) || style.filter_ref.is_some())
    {
        bare = style.paint_off();
        &bare
    } else {
        style
    };
    // Коробка под слоем `filter: url(#id)` (`interact::FilterLayer`, только
    // пустая — `dom::filter_ref_only_empty`) несёт свой фон ВНУТРИ SVG:
    // результат фильтра ЗАМЕНЯЕТ картинку элемента, вне области фильтра —
    // прозрачно (filter-effects-1 §filter region). Фон самой коробки под
    // слоем проступал там, куда фильтр краски не кладёт
    // (`empty-element-with-filter`, `filter-region-calc-001`).
    // Под преобразованием (своим или предка) фон остаётся: растр слоя
    // считается по масштабу окна, а не преобразования, и под `scale(10)`
    // его край расплывается на десяток точек — чёткий фон коробки под ним
    // держит край (`filter-scale-001`, `filter-scaling-001`).
    let unfilled;
    let paint = if c.background.is_some()
        && !c.transform_ancestor
        && c.transform.is_none()
        && c.filter_ref
            .as_deref()
            .is_some_and(|id| mask_def(&format!("filter:{id}")).is_some())
        && !e.children.iter().any(|n| !is_blank(n))
    {
        unfilled = Computed {
            background: None,
            ..c.clone()
        };
        &unfilled
    } else {
        c
    };
    let auto_height = margin_height::used_style(e, paint);
    let mut d = apply(div(), auto_height.as_ref().unwrap_or(paint));
    if native_intrinsic::eligible(e) {
        d.style().sizing_keywords = Some(crate::style::apply::intrinsic_size::keywords(c));
    }
    d = crate::interactive::scroll_target::attach(d, e, c);
    // Проба якоря (css-anchor-position-1 §anchor-name) и содержащего блока
    // (§position-area): канвас во всю коробку пишет её рамку в реестр кадра
    // на подготовке — позже по дереву её прочтёт `anchor::AnchorPlace`
    // позиционированной коробки. Ставится здесь, потому что через
    // `styled_div_with` проходят и блоки, и атомы строки (`inline-block` из
    // `anchor-position-005`), и держатели. `visibility: hidden` якоря —
    // из `style`: `c` может быть `paint_off()` без этого флага
    // (§position-visibility: anchor-visible, «anchor box is invisible»).
    if let Some(probe) =
        crate::layout::positioned::anchor::probe_for(e, c, style.hidden == Some(true))
    {
        d = d.child(probe);
    }
    if let Some(probe) = absolute_overflow::probe(c) {
        d = d.child(probe);
    }
    // `pointer-events: none` — элемент не реагирует на курсор, значит и слой
    // наведения к нему не применяется.
    if c.pointer_events_none != Some(true)
        && let Some(h) = &e.hover
    {
        d = apply_hover(d, h);
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
    // Срез меряется от верха ПОЛЯ СОДЕРЖИМОГО (`ClampCut` — абсолютный
    // ребёнок без вставок), а `max_h` раскладки — высота РАМОЧНОЙ коробки
    // (`apply.rs`: движок всегда трактует размер как border-box). Без
    // вертикальных паддингов и рамок самого контейнера коробка выходила ниже
    // на их сумму и резала последнюю строку (`line-clamp-auto-002`: 128
    // вместо 1+4+128+4+1 = 138).
    let side = |l: Option<Len>| match l {
        Some(Len::Px(v)) => v,
        _ => 0.0,
    };
    let bw = c.borders();
    let mbp_y = side(bw.top) + side(bw.bottom) + side(c.padding.top) + side(c.padding.bottom);
    if !multicol && e.style.clamp_auto == Some(true) && auto_clamp_limit(c).is_some() {
        if let Some(cut) =
            crate::text::clamp::clamp_cut(e.node_id).filter(|c| !sized && c.is_finite())
        {
            d = d.max_h(px(cut + mbp_y));
        }
        // Прячется только содержимое ЗА точкой среза — по блочной оси;
        // оставленные строки по строчной оси переполняют коробку как
        // обычно (css-overflow-4 §5.3: `overflow` клампом не меняется;
        // `block-ellipsis-037`: непереносимое слово шире коробки).
        d.style().overflow.y = Some(gpui::Overflow::Hidden);
    }
    if let Some(n) = e.style.clamp_lines().filter(|_| !multicol) {
        // Без `Styled::line_clamp`: тот попутно включает `overflow_hidden`,
        // а что прятать, решает срез ниже.
        d.text_style().line_clamp = Some(n as usize);
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
        let cut = crate::text::clamp::clamp_cut(e.node_id).unwrap_or(n as f32 * line);
        if !sized && cut.is_finite() {
            d = d.max_h(px(cut + mbp_y));
        }
        // Счётный кламп прячет только то, что ЗА точкой среза; сами
        // оставленные строки переполняют коробку как обычно (css-overflow-4
        // §5.3 — `overflow` клампом не меняется). «Резать нечего»
        // (бесконечная точка: за N-й строкой своего абзаца ничего нет, хвост
        // абзаца уже снял бюджет строк) — обрезки нет, и «Line 4…» видна под
        // коробкой с `height: 3lh` (`line-clamp-011/035`).
        if cut.is_finite() {
            d.style().overflow.y = Some(gpui::Overflow::Hidden);
            // Содержимое ЗА точкой среза не видно и в нижнем паддинге
            // контейнера (css-overflow-4 §5.3: оно «visually hidden», а не
            // обрезано краем паддинга; Blink — `is_hidden_for_paint`):
            // нижний край обрезки — край поля содержимого
            // (`webkit-line-clamp-050`: «Line4» в паддинге 10px).
            let pad_bottom = side(c.padding.bottom);
            if pad_bottom > 0.0 && d.style().overflow_clip_offset.is_none() {
                d.style().overflow_clip_offset = Some([0.0, 0.0, -pad_bottom, 0.0]);
            }
        }
    }
    let empty = !e.children.iter().any(|n| !is_blank(n));
    // Корень документа — сам корень подложки (filter-effects-2
    // §BackdropRoot): под ним фильтровать нечего, а холст с фоном корня лежит
    // СОСЕДОМ коробки и попал бы в копию кадра (`backdrop-filter-root-element`).
    let rootless;
    let c = if e.tag == "html" && c.backdrop_color.is_some() {
        rootless = Computed {
            backdrop_color: None,
            ..c.clone()
        };
        &rootless
    } else {
        c
    };
    for extra in decorations(c, empty) {
        d = d.child(extra);
    }
    d
}
