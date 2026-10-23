//! Диспетчер специальных тегов; сохраняет порядок ветвей element.

use super::element;
use crate::defaults::{DEFAULT_REPLACED_HEIGHT_PX, DEFAULT_REPLACED_WIDTH_PX};
use crate::dom::{Element, Node};
use crate::layout::box_kind::inline_level;
use crate::layout::float::band_flow_host::band_flow_host;
use crate::layout::list::{self, list_item};
use crate::layout::options::RenderOpts;
use crate::layout::replaced::iframe::object_is_document;
use crate::layout::replaced::image::pct_height_to_px;
use crate::layout::replaced::limits::{atom_base_font, with_inherited_font};
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::layout::replaced::{replaced_used_style, svg_size};
use crate::layout::table::anon::anon_element;
use crate::layout::table::table;
use crate::paint::box_tree::styled::styled_div;
use crate::paint::box_tree::styled::styled_div_with;
use crate::paint::replaced::image::{image, image_with};
use crate::render::generic_box::generic_box;
use crate::style::computed::{BoxStyle, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled};

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_element(
    e: &Element,
    merged: BoxStyle,
    inherited: &BoxStyle,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
    built_iframe: Option<AnyElement>,
) -> AnyElement {
    let st = e.style.computed();
    let vcx = opts.cx.vcx();
    if let Some(math) = crate::layout::mathml::element(e, inherited, opts) {
        return math;
    }
    // Вложенный документ имеет приоритет перед картинкой и запасным содержимым.
    if matches!(e.tag.as_str(), "iframe" | "object")
        && let Some(frame) = built_iframe
    {
        return frame;
    }
    match e.tag.as_str() {
        // CSS Lists 3 §2: a block list item generates its own marker even
        // when its parent is an ordinary block rather than a list container.
        _ if (st.display == Some(Display::ListItem) || (e.tag == "li" && st.display.is_none()))
            && !matches!(
                e.tag.as_str(),
                "img" | "svg" | "embed" | "object" | "video" | "canvas" | "iframe"
            ) =>
        {
            list_item::render_with_style(e, inherited, &merged, opts)
        }
        // `image-orientation` НАСЛЕДУЕТСЯ (css-images-3 §5.4): слитый стиль
        // его уже несёт, а копия для замещаемой коробки — нет. Без переноса
        // блочная картинка под `body { image-orientation: none }` всё равно
        // разворачивалась по метке EXIF.
        "canvas" if merged.parent_grid != 0 => {
            crate::layout::replaced::canvas_grid::render(e, &merged, inherited, &opts.cx)
                .unwrap_or_else(|| generic_box(e, merged, inherited, opts, outer_row))
        }
        "img" => {
            // Единицы шрифта в размерах БЛОЧНОЙ картинки разрешаются так же,
            // как у строчной (рукав `"img"` в `atom`): `image()` = `image_with(e,
            // None)` в собственном стиле держал `Len::Em`, и `height: 1em` при
            // `font-size: 3.75em` пропадал — картинка оставалась 15×15
            // (`c43-rpl-bbx-002`). CSS 2.1 §4.3.2: `em` — вычисленный кегль
            // САМОГО элемента, его и даёт `resolve_em` от кегля родителя.
            let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
            let used = copy.style.used_mut();
            if inline_level(e) {
                replaced_used_style::inline_percentage_width(used, opts.cx.constraints.avail_w);
            }
            used.image_orient_none = merged.image_orient_none;
            crate::layout::replaced::image::inherit_item_metadata(used, &merged, inherited);
            // Признак определённого блока (§10.5) — от слитого стиля: держатель
            // строится из сырого, и `apply` иначе выбрасывал `height: %`
            // (см. строчный рукав `"img"` в `atom_element`; ячейка и лунки —
            // свои пути).
            if !matches!(
                inherited.display,
                Some(Display::TableCell) | Some(Display::GridLanes)
            ) {
                used.cb_height_def_hint = merged.cb_height_def;
            }
            // Единицы окна — в точки, как у строчной картинки.
            used.resolve_viewport(opts.viewport, vcx);
            crate::layout::replaced::svg_document_image::render(&copy, opts).unwrap_or_else(|| {
                image_with(&copy, Some(atom_base_font(inherited, opts)), &opts.cx)
            })
        }
        // Замещаемые с картинкой-источником рисуются как <img>: embed через
        // src, object через data, video через poster (css-images §5:
        // object-fit/-position действуют на всех замещаемых).
        "embed" if e.attr("src").is_some() => image(e, &opts.cx),
        // Плавающий кадр без адреса — замещаемая коробка в своём BFC: пустой
        // контекст просмотра держит размер по умолчанию (CSS 2.1 §§10.3.2,
        // 10.6.2).
        "iframe"
            if st.floated_box && crate::layout::replaced::replaced_content::default_iframe(e) =>
        {
            crate::layout::replaced::replaced_content::empty_iframe(e, inherited, opts)
        }
        // Кадр в обычном блочном потоке — тоже замещаемая коробка: по умолчанию
        // 300×150 (CSS 2.1 §10.3.2, §10.6.2; HTML §15.4.4 — атрибуты
        // `width/height`). Общим рукавом он шёл пустым блоком: ширина
        // растягивалась на содержащий блок, высота была нулевой
        // (`block-replaced-height-004/005/007`). Строчный кадр сюда не идёт
        // (рукав `"iframe"` в `atom_element`), у обёрток флоатов свой путь
        // размеров с процентной базой (`float-replaced-height-005/007`).
        "iframe"
            if st.float.is_none_or(|f| f == 0)
                && matches!(inherited.display, None | Some(Display::Block))
                && st.flow_root != Some(true)
                && !matches!(
                    st.width,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                ) =>
        {
            let mut copy = pct_height_to_px(e, inherited);
            if !matches!(
                copy.style.computed().width,
                Some(Len::Px(_)) | Some(Len::Pct(_))
            ) {
                copy.style.used_mut().width = Some(Len::Px(DEFAULT_REPLACED_WIDTH_PX));
            }
            let pct_ok =
                matches!(copy.style.computed().height, Some(Len::Pct(_))) && merged.cb_height_def;
            if !matches!(copy.style.computed().height, Some(Len::Px(_))) && !pct_ok {
                copy.style.used_mut().height = Some(Len::Px(DEFAULT_REPLACED_HEIGHT_PX));
            }
            copy.style.used_mut().cb_height_def_hint = merged.cb_height_def;
            styled_div(&copy, &opts.cx)
                .flex_shrink_0()
                .into_any_element()
        }
        "object" if e.attr("data").is_some() && !object_is_document(e) => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy, &opts.cx)
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy, &opts.cx)
        }
        _ if crate::layout::replaced::media::empty(e) => {
            crate::layout::replaced::media::render(e, opts)
        }
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => {
            // Тот же stretch-fit, что у строчного атома (`atom_element`).
            // Содержащий блок — `inherited` (родитель), а не `merged`: это
            // уже собственный стиль рисунка.
            let cb_w = match inherited.width {
                Some(Len::Px(v)) if v > 0.0 => Some(v),
                _ => opts.cx.constraints.cb_width.filter(|v| *v > 0.0),
            };
            // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`.
            svg_replaced(e, &svg_size::stretch_fit(e, cb_w), &merged, opts).unwrap_or_else(|| {
                styled_div_with(e, &merged, &opts.cx)
                    .child(SharedString::from("[рисунок]"))
                    .into_any_element()
            })
        }
        // `w_full` — растяжка по СТРОЧНОЙ оси горизонтального родителя. В
        // вертикальном родителе ширина — блочный размер (css-writing-modes-4
        // §7.1): полная ширина раздувала линию `width: 10px` на всё тело
        // (`horizontal-rule-vlr-003`), а строчную ось (высоту) растягивает сам
        // ряд блочного потока.
        "hr" => {
            let d = styled_div_with(e, &merged, &opts.cx);
            // A `width` (CSS or the §15.3.11 hint) keeps its own size.
            if inherited.wm().vertical() || merged.width.is_some() {
                d.into_any_element()
            } else {
                d.w_full().into_any_element()
            }
        }
        // Синтетический хост полос обтекания (см. wrap_floats).
        _ if e.marks.shape_flow.is_some() => band_flow_host(e, &merged, opts),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        // Лунки идут путём сетки taffy (`dom::lanes_as_grid`); сюда доходит
        // только узел, собранный мимо того прохода, — он переводится тем же
        // правилом. Рукописная раскладка `lanes` удалена: все оси (вертикальное
        // письмо, `rtl`) ведёт `taffy::compute::grid::lanes`.
        _ if merged.display == Some(Display::GridLanes) => {
            let mut copy = e.clone();
            crate::dom::lanes_to_grid(copy.style.used_mut());
            element(&copy, inherited, opts)
        }
        // Таблица — независимый контекст форматирования: её строки в бюджет
        // `line-clamp` не входят (css-overflow-4 §5.3, `webkit-line-clamp-013`).
        // Табличный путь минует общий, где стоит этот сторож (`makes_bfc`),
        // поэтому он ставится здесь.
        _ if matches!(
            st.display,
            Some(Display::Table) | Some(Display::InlineTable)
        ) =>
        {
            let opts = &*opts.inner(|cx| cx.clamp.build = cx.clamp.build.bfc());
            table(e, &merged, opts)
        }
        // Ряд, группа рядов или ячейка ВНЕ таблицы получают анонимную
        // таблицу-обёртку (css-tables-3 §3.1): иначе ячейки складывались
        // столбиком обычных блоков.
        _ if matches!(
            st.display,
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
        ) =>
        {
            let opts = &*opts.inner(|cx| cx.clamp.build = cx.clamp.build.bfc());
            let wrapper = anon_element("table", vec![Node::Element(e.clone())]);
            table(&wrapper, &merged, opts)
        }
        "table" => {
            let opts = &*opts.inner(|cx| cx.clamp.build = cx.clamp.build.bfc());
            table(e, &merged, opts)
        }
        // Список с заданной раскладкой — это уже не список, а контейнер:
        // на `ul` верстают навигацию и наборы чипов.
        "ul" | "ol" if st.display.is_none() => list::render(e, &merged, opts),
        // `white-space: pre*` значим не меньше тега: переводы строк сохраняет
        // именно он, и на `<div style="white-space: pre">` разметка обязана
        // вести себя так же, как на `<pre>`.
        // Преформат отдельным рисователем — ТОЛЬКО для непереносящегося
        // `white-space: pre`. Переводы строк хранят четыре режима, и три из
        // них переносят строки: `pre-wrap`, `pre-line`, `break-spaces`. Пока
        // сюда уходили все четыре, эти три шли мимо нашей строчной раскладки,
        // где и живут висящие пробелы, разрыв после сохранённого пробела и
        // правила куска. Отсюда же `break-spaces` был неотличим от `pre-wrap`.
        // Внутри преформата может стоять кусок со СВОИМ `white-space`, и он
        // переносится, хотя абзац — нет. Отдельный рисователь преформата
        // правил куска не знает, поэтому такой случай уходит в обычную
        // строчную раскладку (`white-space-pre-031`).
        _ => generic_box(e, merged, inherited, opts, outer_row),
    }
}
