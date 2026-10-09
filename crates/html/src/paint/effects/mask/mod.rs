//! Маски.
// owner: A

use crate::render::*;

pub mod element;

thread_local! {
    /// Определения `<mask id>` / `<clipPath id>` документа: id — разметка
    /// содержимого. Ссылки `url(#id)` из `mask-image`/`clip-path` резолвятся
    /// при отрисовке (см. `interact::Grouped`).
    pub(crate) static MASK_DEFS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    /// Чей документ собран: адрес среза узлов. Виртуализация рисует ПО
    /// БЛОКАМ (`render_block`) — сбор на каждый блок каждого кадра был бы
    /// расточительным, а документ между кадрами один и тот же.
    pub(crate) static MASK_DEFS_FOR: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    /// Определения `<mask>` с `mask-type: alpha`: их снимок помечается, и
    /// `match-source` маскирует альфой, а не светимостью.
    pub(crate) static MASK_ALPHA_IDS: std::cell::RefCell<std::collections::HashSet<String>> =
        std::cell::RefCell::new(std::collections::HashSet::new());
}

thread_local! {
    /// Размер окна документа в css-точках — для единиц `vw`/`vh` там, куда
    /// `RenderOpts` не доходит (`grouped`: вершины `polygon()`). Ставится в
    /// `element()` рядом с `resolve_viewport` — тем же значением, каким
    /// разрешаются `width: 50vw` эталонов.
    pub(crate) static PAINT_VIEWPORT: std::cell::Cell<(f32, f32)> =
        const { std::cell::Cell::new((0.0, 0.0)) };
}

/// Содержимое определения маски по имени (`#id` без решётки).
pub(crate) fn mask_def(id: &str) -> Option<String> {
    MASK_DEFS.with(|m| m.borrow().get(id).cloned())
}

/// SVG `<filter>` для `backdrop-filter: url(#id)` → матрица 4×5 над
/// НЕумноженным RGBA (строки R, G, B, A: четыре множителя и сдвиг — как
/// `Filter::color_matrix`). Только ОДИН примитив с аффинной формулой:
/// `feColorMatrix type="matrix"` (20 чисел; filter-effects-1 Overview.bs:950)
/// или `feComponentTransfer` с `identity`/`linear`/`table` из двух значений
/// (Overview.bs:1159-1168: C' = v0 + C·(v1 − v0); пустой список — тождество,
/// Overview.bs:1197). И только при `color-interpolation-filters="sRGB"`:
/// начальное `linearRGB` (Overview.bs:614) делает формулу нелинейной в sRGB
/// кадра. Остальное — None: подложка не рисуется, как прежде.
pub(crate) fn svg_filter_matrix(def: &str) -> Option<[f32; 20]> {
    fn attr(tag: &str, name: &str) -> Option<String> {
        let head = &tag[..tag.find('>')?];
        let key = format!(" {name}=\"");
        let at = head.find(&key)? + key.len();
        let rest = &head[at..];
        Some(rest[..rest.find('"')?].trim().to_string())
    }
    fn nums(s: &str) -> Option<Vec<f32>> {
        s.split(|ch: char| ch.is_whitespace() || ch == ',')
            .filter(|t| !t.is_empty())
            .map(|t| t.parse::<f32>().ok())
            .collect()
    }
    // Разметка `svg::write_element`: атрибуты ` имя="значение"`; регистр
    // имён тегов и атрибутов сводится к нижнему.
    let d = def.to_ascii_lowercase();
    if !d.contains("color-interpolation-filters=\"srgb\"") {
        return None;
    }
    let prims: Vec<&str> = d
        .match_indices("<fe")
        .map(|(i, _)| &d[i..])
        .filter(|s| !s.starts_with("<fefunc"))
        .collect();
    let [p] = prims.as_slice() else {
        return None;
    };
    let p: &str = p;
    let mut m = [
        1.0f32, 0.0, 0.0, 0.0, 0.0, //
        0.0, 1.0, 0.0, 0.0, 0.0, //
        0.0, 0.0, 1.0, 0.0, 0.0, //
        0.0, 0.0, 0.0, 1.0, 0.0,
    ];
    if p.starts_with("<fecolormatrix") {
        if attr(p, "type").is_some_and(|t| t != "matrix") {
            return None;
        }
        let v = nums(&attr(p, "values")?)?;
        if v.len() != 20 {
            return None;
        }
        m.copy_from_slice(&v);
    } else if p.starts_with("<fecomponenttransfer") {
        let body = &p[..p.find("</fecomponenttransfer").unwrap_or(p.len())];
        for (row, ch) in ['r', 'g', 'b', 'a'].into_iter().enumerate() {
            let Some(at) = body.find(&format!("<fefunc{ch}")) else {
                continue;
            };
            let f = &body[at..];
            let (slope, intercept) = match attr(f, "type").as_deref() {
                Some("identity") => continue,
                Some("table") => {
                    let v = nums(attr(f, "tablevalues").as_deref().unwrap_or(""))?;
                    match v.as_slice() {
                        [] => continue,
                        [v0, v1] => (v1 - v0, *v0),
                        _ => return None,
                    }
                }
                Some("linear") => (
                    attr(f, "slope")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(1.0),
                    attr(f, "intercept")
                        .and_then(|s| s.parse::<f32>().ok())
                        .unwrap_or(0.0),
                ),
                _ => return None,
            };
            m[row * 5 + row] = slope;
            m[row * 5 + 4] = intercept;
        }
    } else {
        return None;
    }
    Some(m)
}

thread_local! {
    /// Снимки определений на момент СБОРКИ дерева: отрисовка идёт позже, а
    /// документов в кадре может быть два (тест и эталон стенда) — реестр
    /// определений к моменту отрисовки уже перезаписан другим документом.
    /// Снимки копятся под уникальными ключами и не чистятся.
    pub(crate) static MASK_SNAPS: std::cell::RefCell<std::collections::HashMap<String, String>> =
        std::cell::RefCell::new(std::collections::HashMap::new());
    pub(crate) static MASK_SNAP_N: std::cell::Cell<u64> = const { std::cell::Cell::new(0) };
}

/// Снять снимок определения; ключ живёт до конца кадра и дольше.
pub(crate) fn snapshot_mask_def(id: &str) -> Option<String> {
    let markup = mask_def(id)?;
    let alpha = MASK_ALPHA_IDS.with(|s| s.borrow().contains(id));
    let key = MASK_SNAP_N.with(|c| {
        let n = c.get() + 1;
        c.set(n);
        // Хвост `A` — `mask-type: alpha` определения (см. `interact`).
        if alpha { format!("k{n}A") } else { format!("k{n}") }
    });
    MASK_SNAPS.with(|m| {
        let mut map = m.borrow_mut();
        // Кадры идут бесконечно — тысяча снимков означает утечку, чистим.
        if map.len() > 1000 {
            map.clear();
        }
        map.insert(key.clone(), markup);
    });
    Some(key)
}

/// Разметка по ключу снимка (для отрисовки).
pub(crate) fn mask_snapshot(key: &str) -> Option<String> {
    MASK_SNAPS.with(|m| m.borrow().get(key).cloned())
}

/// Заменить ссылки `url(#id)` / `clipref:id` в строке маски снимками
/// определений: к отрисовке реестр может смениться другим документом.
pub(crate) fn resolve_mask_refs(raw: &str) -> String {
    if let Some(id) = raw.strip_prefix("clipref:") {
        return match snapshot_mask_def(id) {
            Some(key) => format!("clipsnap:{key}"),
            None => raw.to_string(),
        };
    }
    // Ссылка на определение в документе — `url(#id)` в любом виде записи:
    // и в кавычках (`url("#id")`, `url('#id')`). Прежде узнавалась только
    // голая форма, и маска в кавычках не применялась вовсе
    // (`mask-mode-to-mask-type`: все шесть квадратов сплошные).
    let mut out = String::with_capacity(raw.len());
    let mut rest = raw;
    while let Some(at) = rest.find("url(") {
        let tail = &rest[at + 4..];
        let Some(end) = tail.find(')') else {
            break;
        };
        let inner = tail[..end].trim().trim_matches(|c| c == '"' || c == '\'');
        out.push_str(&rest[..at]);
        match inner.strip_prefix('#').and_then(snapshot_mask_def) {
            Some(key) => out.push_str(&format!("url(svgsnap:{key})")),
            None => out.push_str(&rest[at..at + 4 + end + 1]),
        }
        rest = &tail[end + 1..];
    }
    out.push_str(rest);
    out
}

/// Собрать определения масок ДО отрисовки: ссылка может стоять раньше
/// определения по тексту.
pub(crate) fn collect_mask_defs(nodes: &[Node]) {
    let key = nodes.as_ptr() as usize;
    if MASK_DEFS_FOR.with(|c| c.get()) == key {
        return;
    }
    MASK_DEFS_FOR.with(|c| c.set(key));
    fn walk(nodes: &[Node], out: &mut std::collections::HashMap<String, String>) {
        for n in nodes {
            let Node::Element(e) = n else { continue };
            let tag = e.tag.to_ascii_lowercase();
            if tag == "mask"
                && e.style.mask_type_alpha == Some(true)
                && let Some(id) = e.attr("id")
            {
                MASK_ALPHA_IDS.with(|s| s.borrow_mut().insert(id.to_string()));
            }
            if (tag == "mask" || tag == "clippath")
                && let Some(id) = e.attr("id")
            {
                let mut markup = String::new();
                for c in &e.children {
                    if let Node::Element(el) = c {
                        crate::svg::write_element(el, &mut markup);
                    }
                }
                out.insert(id.to_string(), markup);
            }
            // `<filter id>` — целиком, с атрибутами области (x/y/width/height,
            // filterUnits): ключ с префиксом, чтобы не спутать с маской.
            if tag == "filter" && let Some(id) = e.attr("id") {
                let mut markup = String::new();
                crate::svg::write_element(e, &mut markup);
                out.insert(format!("filter:{id}"), markup);
            }
            walk(&e.children, out);
        }
    }
    MASK_ALPHA_IDS.with(|s| s.borrow_mut().clear());
    MASK_DEFS.with(|m| {
        let mut map = m.borrow_mut();
        map.clear();
        walk(nodes, &mut map);
    });
}
