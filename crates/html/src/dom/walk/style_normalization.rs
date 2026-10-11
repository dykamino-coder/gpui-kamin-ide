//! Style normalization for walk; split out to keep the owning module within 250 lines.

use super::presentational_hints;
use crate::dom::*;
use crate::style::computed::{Computed, Position};
use crate::style::select::Ancestor;

#[allow(clippy::too_many_arguments)]
#[allow(clippy::needless_borrow)]
pub(super) fn normalize_style(
    mut style: &mut Computed,
    tag: &String,
    attrs: &[(String, String)],
    path: &[Ancestor],
) {
    // Корневые метрики для `rem`/`rlh` (css-values-4 §6.1.4).
    // Записываются ЗДЕСЬ, а не в наследовании: `Len::parse` работает
    // на разборе объявлений, а `walk` идёт в порядке документа —
    // корень разбирается раньше любого потомка, и его `25rem` уже
    // читается верно. Собственные объявления корня успевают
    // разобраться по прежней базе; на самом корне `rem` по спеке и
    // так меряется РОДИТЕЛЬСКИМИ (начальными) метриками.
    if tag == "html" {
        let font = match style.font_size {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Em(k))
            | Some(crate::style::values::value::Len::Pct(k)) => k * 16.0,
            _ => 16.0,
        };
        let family = style.font_family.clone().unwrap_or_default();
        let line = match style.line_height {
            Some(crate::style::values::value::Len::Px(v)) => v,
            Some(crate::style::values::value::Len::Em(k))
            | Some(crate::style::values::value::Len::Pct(k)) => k * font,
            _ => {
                let f = crate::text::metrics::normal_line(&family);
                if f > 0.0 { f * font } else { 1.2 * font }
            }
        };
        crate::style::values::value::set_root_metrics(font, line);
        crate::style::values::value::set_root_font_view(match style.font_size {
            Some(crate::style::values::value::Len::Vh(k)) => Some((true, k)),
            Some(crate::style::values::value::Len::Vw(k)) => Some((false, k)),
            _ => None,
        });
    }
    apply_presentational_size(&mut style, &tag, &attrs);
    promote_auto_ratio(&mut style, &tag);
    presentational_hints::colors(&mut style, &tag, &attrs);
    finish_inline_display(&mut style, &tag, &attrs);
    style.plain_block_box = {
        use crate::style::computed::Display;
        let special = matches!(
            tag.as_str(),
            "table"
                | "caption"
                | "colgroup"
                | "col"
                | "thead"
                | "tbody"
                | "tfoot"
                | "tr"
                | "td"
                | "th"
                | "hr"
                | "fieldset"
                | "legend"
                | "details"
                | "summary"
                | "dialog"
                | "option"
                | "optgroup"
                | "html"
                | "body"
                | "input"
                | "textarea"
                | "select"
                | "button"
                | "img"
                | "video"
                | "canvas"
                | "iframe"
                | "embed"
                | "object"
                | "svg"
                | "meter"
                | "progress"
        );
        let block = match style.display {
            Some(Display::Block)
            | Some(Display::GridLanes)
            | Some(Display::ListItem)
            | Some(Display::Flex)
            | Some(Display::Grid) => true,
            None => style.inline_display != Some(true) && BLOCK_TAGS.contains(&tag.as_str()),
            _ => false,
        };
        block && !special && style.float.is_none_or(|f| f == 0)
    };
    // css-will-change-1: обещанный `transform`/`contain` делает коробку
    // содержащим блоком и контекстом наложения лишь там, где само
    // свойство применимо. У строчной НЕатомарной коробки его нет
    // (`will-change-transform-inline`: `fixed` внутри `<span>` стоит от
    // окна); замещаемые и вынесенные из потока — атомарны. Вид коробки
    // известен только здесь, после `finish_inline_display`.
    if style.will_change & crate::style::computed::wc::BOX != 0 {
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            );
        let replaced = matches!(
            tag.as_str(),
            "img"
                | "svg"
                | "input"
                | "select"
                | "textarea"
                | "button"
                | "video"
                | "canvas"
                | "iframe"
                | "object"
                | "embed"
                | "meter"
                | "progress"
        );
        let inline_tag = INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
        let non_atomic = !out_of_flow
            && !replaced
            && (style.inline_display == Some(true) || (style.display.is_none() && inline_tag));
        if !non_atomic {
            use crate::style::computed::wc;
            style.will_change |= wc::CB_ABS | wc::CB_FIXED | wc::STACK;
        }
    }
    // `transform-style` applies only to transformable elements
    // (css-transforms-2): a non-atomic inline with `preserve-3d` is no
    // 3D context, stacking context or containing block
    // (`preserve-3d-flat-grouping-properties-containing-block-inline`).
    if style.preserve_3d == Some(true) {
        let out_of_flow = style.float.is_some_and(|f| f != 0)
            || matches!(
                style.position,
                Some(Position::Absolute) | Some(Position::Fixed)
            );
        let inline_tag = INLINE_TAGS.contains(&tag.as_str()) || !BLOCK_TAGS.contains(&tag.as_str());
        let replaced = matches!(
            tag.as_str(),
            "img"
                | "svg"
                | "input"
                | "select"
                | "textarea"
                | "button"
                | "video"
                | "canvas"
                | "iframe"
                | "object"
                | "embed"
                | "meter"
                | "progress"
        );
        if !out_of_flow
            && !replaced
            && (style.inline_display == Some(true) || (style.display.is_none() && inline_tag))
        {
            style.preserve_3d = None;
            style.frame_3d = None;
        }
    }
    inlinify_in_ruby(&mut style, &tag, path.iter().rev());
    // css-ruby-1 §3.3: «Neither the margin, padding, and border
    // properties … apply to base containers or annotation containers»
    // (`ruby-box-model-001`: `.rbc.pv { padding: 100px }` не должен
    // отодвигать аннотацию от базы). Контейнер — по тегу или по роли.
    if matches!(tag.as_str(), "rbc" | "rtc")
        || matches!(
            style.ruby_role,
            Some(crate::style::computed::RubyRole::BaseContainer)
                | Some(crate::style::computed::RubyRole::TextContainer)
        )
    {
        style.margin = crate::style::computed::Sides::default();
        style.padding = crate::style::computed::Sides::default();
        style.border_width = crate::style::computed::Sides::default();
    }
    // motion-1: offset-трансформ считается НЕ здесь, а вторым проходом
    // по дереву коробок (`motion::settle`, зовётся из `doc.rs`).
    // §offset-path: «In CSS contexts, the boxes being referenced are
    // from the element that establishes the containing block for this
    // element» — опорная коробка `<coord-box>`, длина `ray()` и начало
    // `at <position>` берутся у СОДЕРЖАЩЕГО БЛОКА, а на разборе стиля
    // родителя нет вовсе: сюда доезжает только собственный каскад.
    // Язык — свойство узла, а не CSS: по нему выбираются образцы
    // слогораздела (`hyphens: auto`).
    if let Some((_, v)) = attrs.iter().find(|(k, _)| k == "lang") {
        style.lang = Some(v.clone());
    }
    apply_direction(&mut style, &tag, &attrs);
}
