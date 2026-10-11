//! Диспетчер специальных тегов; сохраняет порядок ветвей element.

use super::element;
use crate::dom::{Element, Node};
use crate::layout::block::containing::{AVAIL_W, CB_WIDTH};
use crate::layout::float::shape_flow::shape_flow;
use crate::layout::list::{list_container, list_item};
use crate::layout::replaced::iframe::object_is_document;
use crate::layout::replaced::image::{image, image_with, pct_height_to_px};
use crate::layout::replaced::limits::{atom_base_font, with_inherited_font};
use crate::layout::replaced::replaced_content::svg_replaced;
use crate::layout::replaced::replaced_used_style;
use crate::layout::table::anon::anon_element;
use crate::layout::table::table;
use crate::render::*;
use crate::style::computed::{Computed, Display};
use crate::style::values::value::Len;
use gpui::{AnyElement, IntoElement, ParentElement, SharedString, Styled};

#[allow(clippy::too_many_arguments)]
pub(crate) fn dispatch_element(
    e: &Element,
    merged: Computed,
    inherited: &Computed,
    opts: &RenderOpts,
    outer_row: Option<(f32, f32)>,
    mut built_iframe: Option<AnyElement>,
) -> AnyElement {
    match e.tag.as_str() {
        // CSS Lists 3 §2: a block list item generates its own marker even
        // when its parent is an ordinary block rather than a list container.
        _ if (e.style.display == Some(Display::ListItem)
            || (e.tag == "li" && e.style.display.is_none()))
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
        "img" => {
            // Единицы шрифта в размерах БЛОЧНОЙ картинки разрешаются так же,
            // как у строчной (рукав `"img"` в `atom`): `image()` = `image_with(e,
            // None)` в собственном стиле держал `Len::Em`, и `height: 1em` при
            // `font-size: 3.75em` пропадал — картинка оставалась 15×15
            // (`c43-rpl-bbx-002`). CSS 2.1 §4.3.2: `em` — вычисленный кегль
            // САМОГО элемента, его и даёт `resolve_em` от кегля родителя.
            let mut copy = with_inherited_font(&pct_height_to_px(e, inherited), inherited);
            if inline_level(e) {
                replaced_used_style::inline_percentage_width(&mut copy.style, AVAIL_W.get());
            }
            copy.style.image_orient_none = merged.image_orient_none;
            // Признак определённого блока (§10.5) — от слитого стиля: держатель
            // строится из сырого, и `apply` иначе выбрасывал `height: %`
            // (см. строчный рукав `"img"` в `atom_element`; ячейка и лунки —
            // свои пути).
            if !matches!(
                inherited.display,
                Some(Display::TableCell) | Some(Display::GridLanes)
            ) {
                copy.style.cb_height_def = merged.cb_height_def;
            }
            // Единицы окна — в точки, как у строчной картинки.
            copy.style.resolve_viewport(opts.viewport);
            image_with(&copy, Some(atom_base_font(inherited, opts)))
        }
        // Замещаемые с картинкой-источником рисуются как <img>: embed через
        // src, object через data, video через poster (css-images §5:
        // object-fit/-position действуют на всех замещаемых).
        "embed" if e.attr("src").is_some() => image(e),
        // Вложенный документ (`<iframe>` и `<object>` с документом в `data`)
        // построен выше; рукав стоит ПЕРЕД картиночным `object`, иначе
        // документ уходил в `image()` пустой коробкой. Объект с документом,
        // который не прочитался, в картинку не превращается — падает в общий
        // рукав и показывает запасное содержимое (HTML §4.8.7).
        "iframe" | "object" if built_iframe.is_some() => built_iframe.take().unwrap(),
        // БЛОЧНЫЙ кадр без пригодного документа — всё равно замещаемая
        // коробка: по умолчанию 300×150 (CSS 2.1 §10.3.2 «…the used value of
        // 'width' becomes 300px», §10.6.2 — 150px; HTML §15.4.4 у `<iframe>`
        // атрибуты `width/height` — размеры). Прежде он шёл общим рукавом
        // пустым блоком: ширина растягивалась на содержащий блок, высота
        // была нулевой, и красная рамка вылезала из-под зелёной
        // (`block-replaced-height-004/005/007`). Строчный кадр не трогаем —
        // там коробка без вклада в строку замерена и откачена (рукав
        // `"iframe"` в `atom_element`).
        // Плавающий кадр сюда не идёт: замерено — `float-replaced-height-005/
        // 007` (0.00 → «красное видно»), у флоата свой путь размеров.
        // Только кадр в обычном блочном потоке: флоат снимает `float` с
        // копии и кладёт её в синтетический гибкий ряд (`wrap_floats`) или в
        // бандовый хост (`flow-root`) — там размер считает свой путь
        // (замерено: `float-replaced-height-005/007` 0.00 → «красное видно»).
        "iframe"
            if !e.style.float.is_some_and(|f| f != 0)
                && matches!(inherited.display, None | Some(Display::Block))
                && e.style.flow_root != Some(true)
                && !matches!(
                    e.style.width,
                    Some(Len::MinContent) | Some(Len::MaxContent) | Some(Len::FitContent)
                ) =>
        {
            let mut copy = pct_height_to_px(e, inherited);
            if !matches!(copy.style.width, Some(Len::Px(_)) | Some(Len::Pct(_))) {
                copy.style.width = Some(Len::Px(300.0));
            }
            let pct_ok = matches!(copy.style.height, Some(Len::Pct(_))) && merged.cb_height_def;
            if !matches!(copy.style.height, Some(Len::Px(_))) && !pct_ok {
                copy.style.height = Some(Len::Px(150.0));
            }
            copy.style.cb_height_def = merged.cb_height_def;
            styled_div(&copy).flex_shrink_0().into_any_element()
        }
        "object" if e.attr("data").is_some() && !object_is_document(e) => {
            let mut copy = e.clone();
            let url = e.attr("data").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        "video" if e.attr("poster").is_some() => {
            let mut copy = e.clone();
            let url = e.attr("poster").unwrap_or_default().to_string();
            copy.attrs.push(("src".to_string(), url));
            image(&copy)
        }
        // ЗАМЕРЕНО И ОТКАЧЕНО: давать рамке БЕЗ адреса резервную ширину 300
        // точек при `display: block` (§10.3.4 -> §10.3.2). Замерено по срезу
        // из 416 пар семей *image*/*replaced*: приобретено 0, потеряно 1 —
        // `float-replaced-height-004` 0.00 -> «красное видно». Это второй
        // заход на резервный размер бесадресной рамки; первый (коробка
        // 300×150 целиком) стоил 13 пар, запись выше.
        // Рисунок не разобрался — показываем запасной текст, а не пустоту.
        "svg" => {
            // Тот же stretch-fit, что у строчного атома (`atom_element`).
            // Содержащий блок — `inherited` (родитель), а не `merged`: это
            // уже собственный стиль рисунка.
            let cb_w = match inherited.width {
                Some(Len::Px(v)) if v > 0.0 => Some(v),
                _ => CB_WIDTH.get().filter(|v| *v > 0.0),
            };
            // CSS-коробка `<svg>` (рамка, отбивка) — `svg_replaced`.
            svg_replaced(e, &crate::svg::size::stretch_fit(e, cb_w), &merged).unwrap_or_else(|| {
                styled_div_with(e, &merged)
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
            let d = styled_div_with(e, &merged);
            if inherited.vertical == Some(true) {
                d.into_any_element()
            } else {
                d.w_full().into_any_element()
            }
        }
        // Синтетический узел обтекания формой (см. wrap_floats).
        "shape-flow" => shape_flow(e, &merged, opts),
        // Табличная раскладка включается и стилем: `display: table` на
        // контейнере значит ровно то же, что тег.
        // Лунки идут путём сетки taffy (`dom::lanes_as_grid`); сюда доходит
        // только узел, собранный мимо того прохода, — он переводится тем же
        // правилом. Рукописная раскладка `lanes` удалена: все оси (вертикальное
        // письмо, `rtl`) ведёт `taffy::compute::grid::lanes`.
        _ if merged.display == Some(Display::GridLanes) => {
            let mut copy = e.clone();
            crate::dom::lanes_to_grid(&mut copy.style);
            element(&copy, inherited, opts)
        }
        // Таблица — независимый контекст форматирования: её строки в бюджет
        // `line-clamp` не входят (css-overflow-4 §5.3, `webkit-line-clamp-013`).
        // Табличный путь минует общий, где стоит этот сторож (`makes_bfc`),
        // поэтому он ставится здесь.
        _ if matches!(
            e.style.display,
            Some(Display::Table) | Some(Display::InlineTable)
        ) =>
        {
            let _clamp_bfc = crate::text::clamp::clamp_context()
                .is_some()
                .then(crate::text::clamp::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Ряд, группа рядов или ячейка ВНЕ таблицы получают анонимную
        // таблицу-обёртку (css-tables-3 §3.1): иначе ячейки складывались
        // столбиком обычных блоков.
        _ if matches!(
            e.style.display,
            Some(Display::TableRowGroup) | Some(Display::TableRow) | Some(Display::TableCell)
        ) =>
        {
            let _clamp_bfc = crate::text::clamp::clamp_context()
                .is_some()
                .then(crate::text::clamp::ClampGuard::enter_bfc);
            let wrapper = anon_element("table", vec![Node::Element(e.clone())]);
            table(&wrapper, &merged, opts)
        }
        "table" => {
            let _clamp_bfc = crate::text::clamp::clamp_context()
                .is_some()
                .then(crate::text::clamp::ClampGuard::enter_bfc);
            table(e, &merged, opts)
        }
        // Список с заданной раскладкой — это уже не список, а контейнер:
        // на `ul` верстают навигацию и наборы чипов.
        "ul" | "ol" if e.style.display.is_none() => list_container::render(e, &merged, opts),
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
