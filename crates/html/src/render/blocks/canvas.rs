//! Слой фона холста у корня (группа корня, css-compositing-1 §pagebackdrop).
// owner: A

use crate::dom::Element;
use crate::paint::effects::grouped::grouped;
use crate::render::*;
use crate::style::computed::Computed;
use crate::style::values::value::Len;
use gpui::{ParentElement, Styled, div, px};

pub(crate) fn canvas_layer(e: &Element, opts: &RenderOpts, out: &mut Vec<gpui::AnyElement>) {
    // Фон холста — часть ГРУППЫ КОРНЯ (css-compositing-1
    // §pagebackdrop): фильтр корня красит и его. Слой лежит
    // СОСЕДОМ коробки корня, поэтому единственная точка окраски
    // фильтром (`inline::inherit`) до него не доходит — красим
    // здесь, от СОБСТВЕННОГО фильтра корня.
    let root_filter = e.style.filter;
    let mut layer = div().absolute().top_0().left_0().right_0().bottom_0();
    // Слоёв несколько — их рисуют плитки (`bg_layers` ниже), а
    // заливка всего холста верхним градиентом их закрыла бы
    // (`background-position-right-in-body`: 97.92).
    let canvas_layers = e.style.bg_layers();
    if let Some(g) = e
        .style
        .gradient
        .as_ref()
        .filter(|_| canvas_layers.is_none())
    {
        let mut g = g.clone();
        if let Some(f) = root_filter {
            g.from = f.apply(g.from);
            g.to = f.apply(g.to);
            for stop in g.stops.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
            for stop in g.stops_px.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
            for stop in g.stops_raw.iter_mut() {
                stop.0 = f.apply(stop.0);
            }
        }
        layer = layer.bg(crate::style::apply::fill(&g));
    } else if let Some(bg) = e.style.background {
        let bg = root_filter.map_or(bg, |f| f.apply(bg));
        layer = layer.bg(bg.to_hsla());
    }
    // Фон-КАРТИНКА канваса красит всю область просмотра тем же
    // слоем (CSS 2.2 §14.2: painting area корневого фона —
    // канвас): на коробке корня она начиналась с его сдвинутого
    // схлопкой верха, и над краской проступала полоса
    // (background-size-document-root-vrl-*).
    let mut layer = layer.into_any_element();
    // Донор — САМ корень: область ОТСЧЁТА плитки это его коробка
    // (§14.2 «sized and positioned relative to the root element's
    // box»), а красит она весь холст. Донор-тело сюда не входит:
    // его слой лежит в детях корня, и отсчёт от padding-box корня
    // получается сам (см. записи о двух откатах ниже).
    if e.tag == "html"
        && (e.style.bg_image.is_some() || canvas_layers.is_some())
        && let Some(tiles) = {
            // Единицы шрифта тоже длина: `html { margin-top: 1em }`
            // роняло отсчёт в ноль, и плитка начиналась с края
            // холста (`margin-collapse-020`).
            let em = match e.style.font_size {
                Some(Len::Px(v)) => v,
                _ => opts.base_size(),
            };
            let fam = e.style.font_family.clone().unwrap_or_default();
            let side = |l: Option<Len>| match l {
                Some(Len::Px(v)) => v,
                Some(l @ (Len::Em(_) | Len::Ex(_) | Len::Ch(_))) => {
                    crate::text::metrics::spacing_px(Some(l), &fam, em)
                }
                _ => 0.0,
            };
            let b = e.style.borders();
            let area = crate::paint::background::RootArea {
                left: side(e.style.margin.left) + side(b.left),
                top: side(e.style.margin.top) + side(b.top),
                right: side(e.style.margin.right) + side(b.right),
                bottom: side(e.style.margin.bottom) + side(b.bottom),
                width: match e.style.width {
                    Some(Len::Px(w)) => {
                        Some(w + side(e.style.padding.left) + side(e.style.padding.right))
                    }
                    _ => None,
                },
                height: match e.style.height {
                    Some(Len::Px(h)) => {
                        Some(h + side(e.style.padding.top) + side(e.style.padding.bottom))
                    }
                    _ => None,
                },
                from_right: e.style.vertical_rl == Some(true),
                // Корень `vertical-rl` без заданной ширины — по
                // содержимому у правого края (css-writing-modes-4
                // §7, auto block-size): левый край его коробки
                // пишет обёртка тела ниже при подготовке.
                left_key: (e.style.vertical_rl == Some(true)).then_some(opts.doc_salt),
            };
            match &canvas_layers {
                // Снизу вверх, каждый слой — своей плиткой от
                // коробки корня (§14.2).
                Some(layers) => {
                    let mut stack = div().absolute().top_0().left_0().right_0().bottom_0();
                    for l in layers.iter().rev() {
                        if let Some(t) = crate::paint::background::canvas_layer(l, area) {
                            stack = stack.child(t);
                        }
                    }
                    Some(stack.into_any_element())
                }
                None => crate::paint::background::canvas_layer(&e.style, area),
            }
        }
    {
        layer = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(layer)
            .child(tiles)
            .into_any_element();
    } else if e.style.bg_image.is_some()
        && let Some(tiles) = crate::paint::background::layer(&e.style)
    {
        // Область ПОЗИЦИОНИРОВАНИЯ краски — PADDING-BOX корня:
        // ширина + горизонтальные отступы; полоса прижата по
        // письму с учётом поля и рамки с той стороны.
        //
        // ЗАМЕРЕНО ВТОРОЙ РАЗ (перенос фона тела на корень по §14.2
        // ВМЕСТЕ с `RootArea` + `canvas_layer`): приобретено 1,
        // потеряно 2 — `background-position-001` 0.36 -> 2.39 и
        // `background-root-024` 0.17 -> 5.74. Перенос сам по себе
        // даёт 0 и −2. Значит дело не в кегле тела: расходится
        // геометрия коробки корня, и её надо чинить первой.
        //
        // ЗАМЕРЕНО: считать область от коробки корня целиком
        // (`RootArea` + `canvas_layer`, плитка красит весь холст)
        // — CSS2 +2 в `background-root-001/002`, но -3 в
        // `margin-collapse-020/021` и `block-formatting-contexts-003`:
        // слой на весь холст перекрывает то, что рисуется выше по
        // потоку. Возвращаться вместе с переносом фона тела на
        // корень, когда кегль тела будет разрешаться до переноса.
        let side = |l: Option<Len>| match l {
            Some(Len::Px(v)) => v,
            _ => 0.0,
        };
        let b = e.style.borders();
        let mut band = div().absolute().top_0().bottom_0();
        band = match e.style.width {
            Some(Len::Px(w)) => {
                let pad_w = w + side(e.style.padding.left) + side(e.style.padding.right);
                let band = band.w(px(pad_w));
                if e.style.vertical_rl == Some(true) {
                    band.right(px(side(e.style.margin.right) + side(b.right)))
                } else {
                    band.left(px(side(e.style.margin.left) + side(b.left)))
                }
            }
            _ => band.left_0().right_0(),
        };
        layer = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .child(layer)
            .child(band.child(tiles))
            .into_any_element();
    }
    // Прозрачность корня — на ГОТОВЫЙ слой холста целиком, вместе
    // с плиткой фона-картинки: погаси их порознь, и цвет с плиткой
    // сложились бы с двойной альфой. Коробка корня свою
    // прозрачность получает отдельно (`apply::style`), но краска с
    // неё уже снята, так что перекрытия групп нет.
    if let Some(o) = e.style.opacity.filter(|o| *o < 1.0) {
        layer = div()
            .absolute()
            .top_0()
            .left_0()
            .right_0()
            .bottom_0()
            .opacity(o)
            .child(layer)
            .into_any_element();
    }
    // `clip-path` корня режет и холст (css-masking-1 §the-clip-path +
    // compositing-1 §rootgroup: фон корня — часть корневой группы):
    // слой холста получает ту же обрезку, что и коробка корня.
    // Начало координат у них общее — левый верхний угол окна.
    if e.style.clip_polygon.is_some()
        || e.style.clip_shape.is_some()
        || e.style.clip_inset.is_some()
        || e.style.clip_xywh.is_some()
    {
        let mut clip = Computed::default();
        clip.clip_polygon = e.style.clip_polygon.clone();
        clip.clip_shape = e.style.clip_shape.clone();
        clip.clip_inset = e.style.clip_inset;
        clip.clip_xywh = e.style.clip_xywh;
        layer = grouped(
            div()
                .absolute()
                .top_0()
                .left_0()
                .right_0()
                .bottom_0()
                .child(layer)
                .into_any_element(),
            &clip,
        );
    }
    out.push(layer);
}
