//! Обратная сериализация поддерева `<svg>` в разметку для растеризатора.

use super::clip::{reference_box, synth_clip};
use super::size::{size_of, view_box_ratio};
use super::{IN_CLIP, VIEW_BOX};
use crate::dom::{Element, Node};

/// Обратная сериализация поддерева в разметку SVG.
///
/// Дерево у нас уже разобрано, исходного текста нет — а растеризатору нужен
/// именно текст. Пишем только то, что имеет смысл внутри SVG: имя тега,
/// атрибуты и текстовые узлы.
pub fn serialize(e: &Element) -> String {
    let mut out = String::new();
    if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        write_element(&copy, &mut out);
    } else {
        write_element(e, &mut out);
    }
    out
}

/// ★ ЗАМЕРЕНО И ОТКАЧЕНО (04.09): дописывать в `<defs>` недостающие
/// `<filter>` из соседних `<svg>` документа (реестр `render::mask_def`,
/// ключ `filter:<id>`) перед `</svg>` — срез filter-effects 184 -> 184,
/// `svg-filter-primitive-units-user-space` и родня не сдвинулись:
/// одной подстановки определения мало, единицы фильтра считаются от
/// чужого вьюпорта. Разбор: target/scout-masking-filters-2026-09.md, F5.
fn subtree_has(e: &Element, tag: &str) -> bool {
    e.tag.eq_ignore_ascii_case(tag)
        || e.children.iter().any(|n| match n {
            Node::Element(c) => subtree_has(c, tag),
            _ => false,
        })
}

/// filter-effects-1 «Restrictions on filter primitives»: примитив, чей цвет
/// зависит от currentColor (`flood-color`/`lighting-color` в любой обёртке),
/// «загрязнён»; `feDisplacementMap` с загрязнённой картой (in2) обязан
/// работать сквозным проходом. resvg о загрязнении не знает — такой примитив
/// заменяется на `feOffset dx=0 dy=0` до сериализации (WPT tainting-*).
fn untaint_filters(e: &mut Element) {
    if e.tag == "filter" {
        let mut tainted: std::collections::HashSet<String> = Default::default();
        let mut prev = false;
        for child in e.children.iter_mut() {
            let Node::Element(p) = child else { continue };
            if !p.tag.starts_with("fe") {
                continue;
            }
            let val =
                |p: &Element, k: &str| p.attrs.iter().find(|(a, _)| a == k).map(|(_, v)| v.clone());
            let dirty_ref = |name: &Option<String>, prev: bool| match name.as_deref() {
                None => prev,
                Some(n) => tainted.contains(n),
            };
            // Собственное загрязнение: цвет примитива от currentColor —
            // в атрибуте или в style, включая обёртки color-mix()/color(from).
            let self_dirty = p.attrs.iter().any(|(k, v)| {
                matches!(k.as_str(), "flood-color" | "lighting-color" | "style")
                    && v.to_ascii_lowercase().contains("currentcolor")
            });
            let in1 = val(p, "in");
            let in2 = val(p, "in2");
            let mut dirty = self_dirty || dirty_ref(&in1, prev);
            if p.tag == "feMerge" {
                for n in &p.children {
                    if let Node::Element(m) = n
                        && m.tag == "feMergeNode"
                        && dirty_ref(&val(m, "in"), prev)
                    {
                        dirty = true;
                    }
                }
            }
            if p.tag == "feDisplacementMap" && dirty_ref(&in2, prev) {
                // Сквозной проход: сохранить in/result, снять карту.
                p.tag = "feOffset".to_string();
                p.attrs.retain(|(k, _)| {
                    !matches!(
                        k.as_str(),
                        "in2" | "scale" | "xChannelSelector" | "yChannelSelector" | "dx" | "dy"
                    )
                });
                p.attrs.push(("dx".to_string(), "0".to_string()));
                p.attrs.push(("dy".to_string(), "0".to_string()));
                // Выход сквозного прохода загрязнён лишь настолько,
                // насколько его основной вход.
                dirty = dirty_ref(&in1, prev);
            } else if p.tag == "feDisplacementMap" {
                dirty = dirty || dirty_ref(&in2, prev);
            }
            if dirty && let Some(r) = val(p, "result") {
                tainted.insert(r);
            }
            prev = dirty;
        }
        return;
    }
    for child in e.children.iter_mut() {
        if let Node::Element(c) = child {
            untaint_filters(c);
        }
    }
}

pub(crate) fn write_element(e: &Element, out: &mut String) {
    // Синтезированный `<clipPath>` пишется ПЕРЕД элементом (см. `synth_clip`).
    let clip_id = synth_clip(e, out);
    out.push('<');
    out.push_str(&e.tag);
    // `transform-origin` растеризатор не знает — точка отсчёта
    // вкатывается в сам transform парой translate. Одно значение — x,
    // второй осью служит середина fill-box (css-transforms §4);
    // доли — от fill-box (атрибуты width/height фигуры).
    let attr_of = |name: &str| {
        e.attrs
            .iter()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.as_str())
    };
    let num_attr = |name: &str| attr_of(name).and_then(|v| v.trim().parse::<f32>().ok());
    let (fx, fy, fw, fh) = (
        num_attr("x").unwrap_or(0.0),
        num_attr("y").unwrap_or(0.0),
        num_attr("width").unwrap_or(0.0),
        num_attr("height").unwrap_or(0.0),
    );
    // Опорная коробка (css-transforms-1 §transform-box) — только при ЯВНОЙ
    // `fill-box`/`stroke-box` (и их CSS-двойниках `content-box`/`border-box`).
    // Тогда и доли, и ДЛИНЫ `transform-origin` отсчитываются от её левого
    // верха, а проценты `translate()` — от её размера (`fill-box-001`
    // target4: `75px 75px` — это (100,100), а не (75,75)). Без явного
    // значения путь прежний: доли от рамки по атрибутам, длины как есть.
    let ref_box = match e.style.transform_box {
        Some(k @ (1 | 2)) => Some(reference_box(e, k == 2).unwrap_or((fx, fy, fw, fh))),
        _ => None,
    };
    let (bx, by, bw, bh) = ref_box.unwrap_or((fx, fy, fw, fh));
    let from_box = |v: f32, o: f32| if ref_box.is_some() { o + v } else { v };
    // CSS `transform-origin` сильнее презентационного атрибута
    // (css-transforms-1 §specificity): доли — от той же опорной коробки, что
    // и у атрибута, точки — от её начала при явной коробке, иначе как есть
    // (`transform-box/fill-box-*`, `svg-origin-relative-length-*`).
    let style_origin = match (e.style.transform_origin, e.style.transform_origin_px) {
        (_, (Some(px), Some(py))) => Some((from_box(px, bx), from_box(py, by))),
        (Some((kx, ky)), (px, py)) => Some((
            px.map_or(bx + bw * kx, |v| from_box(v, bx)),
            py.map_or(by + bh * ky, |v| from_box(v, by)),
        )),
        _ => None,
    };
    let origin = style_origin.or_else(|| {
        attr_of("transform-origin").and_then(|raw| {
            let side = |t: &str, base: f32, off: f32| -> Option<f32> {
                let t = t.trim();
                Some(match t {
                    "left" | "top" => off,
                    "center" => off + base * 0.5,
                    "right" | "bottom" => off + base,
                    _ if t.ends_with('%') => {
                        off + t.trim_end_matches('%').parse::<f32>().ok()? / 100.0 * base
                    }
                    _ => {
                        // Абсолютные единицы (css-values §6.2): 1in = 96px.
                        let (num, k) = if let Some(n) = t.strip_suffix("px") {
                            (n, 1.0)
                        } else if let Some(n) = t.strip_suffix("cm") {
                            (n, 96.0 / 2.54)
                        } else if let Some(n) = t.strip_suffix("mm") {
                            (n, 96.0 / 25.4)
                        } else if let Some(n) = t.strip_suffix("in") {
                            (n, 96.0)
                        } else if let Some(n) = t.strip_suffix("pt") {
                            (n, 96.0 / 72.0)
                        } else if let Some(n) = t.strip_suffix("pc") {
                            (n, 16.0)
                        } else if let Some(n) = t.strip_suffix('q').or_else(|| t.strip_suffix('Q'))
                        {
                            (n, 96.0 / 101.6)
                        } else {
                            (t, 1.0)
                        };
                        // Длина от рамки фигуры — только при
                        // `transform-box: fill-box`; по умолчанию (`view-box`)
                        // отсчёт от вьюпорта, то есть без сдвига на `off`
                        // (`svg-origin-length-*` зелены именно так).
                        let v = num.trim().parse::<f32>().ok()? * k;
                        return Some(if e.style.transform_box_fill == Some(true) {
                            off + v
                        } else {
                            v
                        });
                    }
                })
            };
            let toks: Vec<&str> = raw.split_whitespace().collect();
            // Осевые ключевые слова (css-transforms §4): одиночный `top`/`bottom`
            // — это ось Y с центром по X; пара слов может идти в любом порядке,
            // но `top 100%` невалидна — тогда точка отсчёта остаётся `0 0`
            // (None = без origin-обёртки).
            let vert_only = |t: &str| matches!(t.trim(), "top" | "bottom");
            let horiz_only = |t: &str| matches!(t.trim(), "left" | "right");
            let keyword =
                |t: &str| matches!(t.trim(), "top" | "bottom" | "left" | "right" | "center");
            let (ox, oy) = match toks.as_slice() {
                [a] if vert_only(a) => (fx + fw * 0.5, side(a, fh, fy)?),
                [a] => (side(a, fw, fx)?, fy + fh * 0.5),
                // Пара слов ОДНОЙ оси невалидна (css-transforms-1 §transform-origin):
                // `top bottom`, `left right` — объявление отбрасывается целиком.
                [a, b] if (vert_only(a) && vert_only(b)) || (horiz_only(a) && horiz_only(b)) => {
                    return None;
                }
                [a, b] if vert_only(a) || horiz_only(b) => {
                    // Обратный порядок допустим только у ПАРЫ ключевых слов.
                    if keyword(a) && keyword(b) {
                        (side(b, fw, fx)?, side(a, fh, fy)?)
                    } else {
                        return None;
                    }
                }
                [a, b] => (side(a, fw, fx)?, side(b, fh, fy)?),
                _ => return None,
            };
            Some((ox, oy))
        })
    });
    // Начальный `transform-origin` SVG-элемента — `0 0` (UA-лист:
    // `*:not(svg), *:not(foreignObject) > svg { transform-origin: 0 0 }`), и
    // отсчитывается он от опорной коробки: при явной `fill-box`/`stroke-box`
    // это её левый верх, а не начало координат (`fill-box-001` target1:
    // `rotate(90deg)` вокруг (0,0) уводил фигуру за кадр).
    let origin = origin.or(ref_box.map(|(x, y, _, _)| (x, y)));
    // Стилевой transform на SVG-ребёнке СИЛЬНЕЕ презентационного атрибута
    // (css-transforms §specificity) — сериализуется атрибутом для
    // растеризатора.
    // Матрица функций В ПОРЯДКЕ ЗАПИСИ (см. `computed::Transform::lin`);
    // проценты сдвига — от опорной коробки фигуры (css-transforms-1
    // §transform-box: доля — от reference box; здесь fill-box по атрибутам).
    let style_t = e.style.transform.as_ref().map(|t| {
        let unit = t.lin == [[1.0, 0.0], [0.0, 1.0]] && t.tr == [[0.0; 3]; 2];
        if unit {
            return String::new();
        }
        let tx = t.tr[0][0] + bw * t.tr[0][1] + bh * t.tr[0][2];
        let ty = t.tr[1][0] + bw * t.tr[1][1] + bh * t.tr[1][2];
        format!(
            "matrix({} {} {} {} {} {})",
            t.lin[0][0], t.lin[1][0], t.lin[0][1], t.lin[1][1], tx, ty
        )
    });
    // Отдельное свойство `translate` (css-transforms-2 §individual-transforms)
    // действует и на SVG-фигуре — она transformable element
    // (css-transforms-1 §transformable-element), — но до растеризатора не
    // доезжало: матрица выше собирается из `lin`/`tr` свойства `transform`, а
    // `translate:` живёт отдельным полем `Computed::translate`, и его
    // единственный потребитель `apply.rs` двигает CSS-КОРОБКУ, внутрь `<svg>`
    // не заходя. Оттого `<rect style="translate: 100px 100px">` стоял на
    // месте: наш зелёный — (10,67)-(134,191) точек устройства, эталонный —
    // (135,192)-(259,316), ровно 100 css-точек по обеим осям, и накрываемый
    // красный оставался виден (`translate/translate-in-svg`).
    //
    // Порядок сборки — css-transforms-2 §ctm: «translate, then rotate, then
    // scale, then transform», то есть сдвиг стоит СЛЕВА и от матрицы свойства
    // `transform`, и от презентационного атрибута `transform=` (тот
    // отображается в то же свойство — css-transforms-1 §svg-transform).
    // Доли — от опорной коробки (§transform-box; здесь fill-box по атрибутам
    // фигуры, как и у процентов сдвига выше).
    //
    // Корневой `<svg>` исключён: он обычная CSS-коробка, и `translate:` ему
    // уже сдвигает `apply.rs` — иначе сдвиг лёг бы дважды.
    let ind_t = e
        .style
        .translate
        .filter(|_| e.tag != "svg")
        .and_then(|(x, y)| {
            let axis = |l: crate::style::values::value::Len, base: f32| match l {
                crate::style::values::value::Len::Px(v) => v,
                crate::style::values::value::Len::Pct(k) => k * base,
                _ => 0.0,
            };
            let (dx, dy) = (axis(x, bw), axis(y, bh));
            (dx != 0.0 || dy != 0.0).then_some((dx, dy))
        });
    // Невалидный список преобразований В АТРИБУТЕ (`rotate(90,)`: запятая без
    // аргумента — грамматика `transform-list`, SVG 1.1 §7.6). Атрибут —
    // презентационная форма свойства `transform` (css-transforms-1
    // §svg-transform), и ошибка разбора отбрасывает его целиком, как
    // невалидное объявление. usvg висячую запятую прощает и поворачивал
    // фигуру (`svg-rotate-3args-invalid-002`, `svg-external-styles-014`).
    let attr_bad = attr_of("transform").is_some_and(|t| {
        let s: String = t.chars().filter(|c| !c.is_whitespace()).collect();
        s.contains(",)") || s.contains("(,") || s.contains(",,")
    });
    let attr_t = attr_of("transform")
        .filter(|_| !attr_bad)
        .map(str::to_string);
    let base_t = style_t.filter(|t| !t.is_empty()).or(attr_t);
    let transform = match (ind_t, base_t) {
        (Some((dx, dy)), Some(t)) => Some(format!("translate({dx} {dy}) {t}")),
        (Some((dx, dy)), None) => Some(format!("translate({dx} {dy})")),
        (None, t) => t,
    };
    let combined = match (origin, transform) {
        (Some((ox, oy)), Some(t)) => Some(format!(
            "translate({ox} {oy}) {t} translate({} {})",
            -ox, -oy
        )),
        // Сдвиг от `translate:` — такой же повод перебить презентационный
        // атрибут, как и своё свойство `transform`: без этой ветки собранная
        // строка терялась, а в разметку уходил нетронутый атрибут.
        (None, Some(t)) if e.style.transform.is_some() || ind_t.is_some() => Some(t),
        _ => None,
    };
    for (k, v) in &e.attrs {
        if combined.is_some() && (k == "transform" || k == "transform-origin") {
            continue;
        }
        // Синтезированная обрезка заменяет CSS-запись фигуры: usvg её не
        // разбирает, а оставленная рядом с нашей `url()` спорила бы с ней.
        if clip_id.is_some() && k == "clip-path" {
            continue;
        }
        if k == "transform-origin" {
            continue;
        }
        if k == "transform" && attr_bad {
            continue;
        }
        // `divisor="0"` у feConvolveMatrix: по спеке берётся умолчание (сумма
        // ядра), а usvg на нуле возвращает ошибку и элемент исчезает.
        if k == "divisor" && v.trim().parse::<f32>().ok() == Some(0.0) {
            continue;
        }
        // Объявления трансформа из `style=` уже учтены в `e.style` и
        // уходят нашим `transform="…"`; в usvg `transform` —
        // презентационный атрибут, и объявление из `style` его ПЕРЕБИВАЕТ,
        // а CSS-запись `translate(100px, 0)` парсер SVG не понимает —
        // получалась единичная матрица (`svg-inline-styles-001..013`).
        let v = if k == "style" {
            let kept: Vec<&str> = v
                .split(';')
                .filter(|d| {
                    let name = d.split(':').next().unwrap_or("").trim();
                    // `clip-path` из `style=` уже ушёл синтезированным
                    // `<clipPath>`: в usvg объявление `style` перебивает
                    // презентационный атрибут и погасило бы нашу ссылку.
                    !(matches!(
                        name,
                        "transform"
                            | "transform-origin"
                            | "transform-box"
                            | "translate"
                            | "rotate"
                            | "scale"
                    ) || (clip_id.is_some() && name == "clip-path"))
                })
                .collect();
            std::borrow::Cow::Owned(kept.join(";"))
        } else {
            std::borrow::Cow::Borrowed(v.as_str())
        };
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        escape_attr(&v, out);
        out.push('"');
    }
    if let Some(t) = &combined {
        out.push_str(" transform=\"");
        escape_attr(t, out);
        out.push('"');
    }
    if let Some(id) = &clip_id {
        out.push_str(&format!(" clip-path=\"url(#{id})\""));
    }
    // CSS-геометрия и заливка SVG-фигур (SVG 2): стилевые ширина/высота
    // и `fill` доезжают до растеризатора презентационными атрибутами,
    // если разметка своих не задала.
    let has = |name: &str| e.attrs.iter().any(|(k, _)| k == name);
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(w)) = e.style.width
            && !has("width")
        {
            out.push_str(&format!(" width=\"{w}\""));
        }
        if let Some(crate::style::values::value::Len::Px(h)) = e.style.height
            && !has("height")
        {
            out.push_str(&format!(" height=\"{h}\""));
        }
    }
    if let Some(fill) = &e.style.svg_fill
        && !has("fill")
    {
        out.push_str(" fill=\"");
        escape_attr(fill, out);
        out.push('"');
    }
    // Обводка и геометрия из КАСКАДА: правило `rect.frame { x: -0.5px;
    // stroke: black }` живёт в `<style>` с селектором, в разметке фигуры его
    // нет, а растеризатор видит только разметку. Разметка сильнее: свой
    // атрибут не перебиваем — как у `fill` выше.
    if let Some(stroke) = &e.style.svg_stroke
        && !has("stroke")
    {
        out.push_str(" stroke=\"");
        escape_attr(stroke, out);
        out.push('"');
    }
    if let Some(w) = &e.style.svg_stroke_width
        && !has("stroke-width")
    {
        // `vector-effect: non-scaling-stroke` из КАСКАДА (SVG 2
        // §vector-effect): толщина — в точках экрана. Само свойство в разметку
        // не уходит, поэтому собственный масштаб фигуры снимается с толщины
        // здесь: равномерный множитель стилевой матрицы √|det| (масштаб
        // предков не учитывается). Без этого `svgbox-stroke-box-003/004` при
        // верной геометрии рисовали обводку вдвое тоньше эталонной.
        let k = match (e.style.svg_non_scaling, e.style.transform) {
            (Some(true), Some(t)) if !has("vector-effect") => (t.lin[0][0] * t.lin[1][1]
                - t.lin[0][1] * t.lin[1][0])
                .abs()
                .sqrt(),
            _ => 1.0,
        };
        match w.trim().trim_end_matches("px").parse::<f32>() {
            Ok(v) if k > 1e-6 && (k - 1.0).abs() > 1e-6 => {
                out.push_str(&format!(" stroke-width=\"{}\"", v / k));
            }
            _ => {
                out.push_str(" stroke-width=\"");
                escape_attr(w, out);
                out.push('"');
            }
        }
    }
    if e.tag != "svg" {
        if let Some(crate::style::values::value::Len::Px(x)) = e.style.svg_x
            && !has("x")
        {
            out.push_str(&format!(" x=\"{x}\""));
        }
        if let Some(crate::style::values::value::Len::Px(y)) = e.style.svg_y
            && !has("y")
        {
            out.push_str(&format!(" y=\"{y}\""));
        }
    }
    if e.children.is_empty() {
        out.push_str("/>");
        return;
    }
    out.push('>');
    // Вложенный `<svg>` — свой вьюпорт для `view-box` детей; дети
    // `<clipPath>` — без синтеза своей обрезки (см. `IN_CLIP`).
    let prev_vb = VIEW_BOX.with(|v| v.get());
    if e.tag.eq_ignore_ascii_case("svg") {
        let vb = view_box_ratio(e).unwrap_or_else(|| size_of(e));
        VIEW_BOX.with(|v| v.set(vb));
    }
    let prev_clip = IN_CLIP.with(|c| c.get());
    if e.tag.eq_ignore_ascii_case("clippath") {
        IN_CLIP.with(|c| c.set(true));
    }
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, out),
            Node::Element(el) => write_element(el, out),
        }
    }
    IN_CLIP.with(|c| c.set(prev_clip));
    VIEW_BOX.with(|v| v.set(prev_vb));
    out.push_str("</");
    out.push_str(&e.tag);
    out.push('>');
}

pub(super) fn escape_attr(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '"' => out.push_str("&quot;"),
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            c => out.push(c),
        }
    }
}

fn escape_text(v: &str, out: &mut String) {
    for ch in v.chars() {
        match ch {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            c => out.push(c),
        }
    }
}

/// Разметка с корневой канвой ЗАДАННОГО размера: собственные width/height
/// корня заменяются, иначе растеризатор вписал бы рисунок по ним.
pub(super) fn serialize_sized(e: &Element, w: f32, h: f32, nx: f32, ny: f32) -> String {
    let cleaned;
    let e = if subtree_has(e, "fedisplacementmap") {
        let mut copy = e.clone();
        untaint_filters(&mut copy);
        cleaned = copy;
        &cleaned
    } else {
        e
    };
    let mut out = String::new();
    // Канва с запасом влево/вверх (`nx`, `ny` — переполнение в минус).
    let (cw, ch) = (w + nx, h + ny);
    out.push_str(&format!("<svg width=\"{cw}\" height=\"{ch}\""));
    // Пространство имён обязательно: разметка идёт растеризатору отдельным
    // документом. В XHTML рисунок часто пишут с ПРЕФИКСОМ (`<svg:svg
    // xmlns:svg=…>`), собственного `xmlns` у корня нет, а имена мы храним
    // без префикса — без этой строки usvg не разбирал документ вовсе, и
    // рисунок пропадал целиком (вся семья `*-replaced-*` в CSS2).
    if !e.attrs.iter().any(|(k, _)| k == "xmlns") {
        out.push_str(" xmlns=\"http://www.w3.org/2000/svg\"");
    }
    // Под зумом без `viewBox` пользовательские единицы обязаны вырасти вместе
    // с канвой (длины внутри рисунка — тоже used-значения, css-viewport-1
    // §zoom): `viewBox` в незумленных единицах растягивает содержимое на
    // зумленную канву (`zoom/svg-path`, `zoom/svg`). С `viewBox` масштаб уже
    // задаёт он сам.
    let z = e.style.zoom_eff.unwrap_or(1.0);
    let shifted = nx > 0.0 || ny > 0.0;
    if ((z - 1.0).abs() > f32::EPSILON || shifted)
        && !e
            .attrs
            .iter()
            .any(|(k, _)| k.eq_ignore_ascii_case("viewbox"))
    {
        out.push_str(&format!(
            " viewBox=\"{} {} {} {}\"",
            -nx / z,
            -ny / z,
            cw / z,
            ch / z
        ));
    }
    // Канва шире коробки (`overflow: visible`, см. `element`): при `viewBox`
    // пользовательские единицы обязаны остаться прежними, иначе рисунок
    // растянулся бы на выросшую канву (`border-shape-shadow-ref`: рамка
    // 220 → 230). `viewBox` расширяется в той же пропорции, что и канва, а
    // при запасе влево/вверх — сдвигается на него.
    let (w0, h0) = size_of(e);
    let grown = w > w0 + 0.5 || h > h0 + 0.5 || shifted;
    for (k, v) in &e.attrs {
        if k == "width" || k == "height" {
            continue;
        }
        out.push(' ');
        out.push_str(k);
        out.push_str("=\"");
        if grown && k.eq_ignore_ascii_case("viewbox") {
            let p: Vec<f32> = v
                .split([' ', ','])
                .filter(|s| !s.is_empty())
                .filter_map(|s| s.parse().ok())
                .collect();
            if p.len() == 4 && w0 > 0.0 && h0 > 0.0 {
                let (ux, uy) = (p[2] / w0, p[3] / h0);
                out.push_str(&format!(
                    "{} {} {} {}",
                    p[0] - nx * ux,
                    p[1] - ny * uy,
                    cw * ux,
                    ch * uy
                ));
                out.push('"');
                continue;
            }
        }
        escape_attr(v, &mut out);
        out.push('"');
    }
    out.push('>');
    // Опорная коробка `view-box` для CSS-обрезки детей (`synth_clip`): корень
    // пишется здесь, а не в `write_element`.
    VIEW_BOX.with(|v| v.set(view_box_ratio(e).unwrap_or_else(|| size_of(e))));
    IN_CLIP.with(|c| c.set(false));
    for child in &e.children {
        match child {
            Node::Text(t) => escape_text(t, &mut out),
            Node::Element(el) => write_element(el, &mut out),
        }
    }
    out.push_str("</svg>");
    out
}
