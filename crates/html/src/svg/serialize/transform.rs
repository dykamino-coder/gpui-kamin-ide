//! SVG transform и transform-origin из CSS и презентационных атрибутов.

use crate::dom::Element;
use crate::svg::clip::reference_box;

#[allow(clippy::too_many_arguments)]
pub(crate) fn element_transform(e: &Element) -> (Option<String>, bool) {
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
    let ref_box = match super::svg_box_style(&e.style).0 {
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
                        return Some(if super::svg_box_style(&e.style).1 == Some(true) {
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
    (combined, attr_bad)
}
