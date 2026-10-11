//! Value helpers for the print-pair page box: lengths, sides and shorthands of `@page`.

use kamin_html::value::{Color, Len};

/// Поля листа по умолчанию — ЗАМЕР отдельным прогоном: WPT их не
/// оговаривает, но `monolithic-overflow-027/-028/-029` сходятся с эталоном
/// только при page area 2in (5in x 3in при полях 0.5in = 48px, wpt#40788;
/// `media-queries-001`: «WPT tests that assume that there's a half-inch
/// margin on each side»). `WPT_PAGE_MARGIN=48` против нуля.
pub(super) fn default_margin() -> f32 {
    std::env::var("WPT_PAGE_MARGIN")
        .ok()
        .and_then(|v| v.parse().ok())
        // ★ Умолчание — 0.5in: так печатает раннер WPT (wpt#40788; Blink
        // `StyleForPage` берёт поля из параметров печати), и эталоны это
        // ЗАШИВАЮТ: `monolithic-overflow-027-ref` 400vh = 8in = четыре листа
        // по 2in, `monolithic-overflow-030-ref` — `height: 1.5in` + квадрат
        // 0.5in на лист. `WPT_PAGE_MARGIN=0` — прежнее поведение.
        .unwrap_or(48.0)
}

/// Логическая сторона листа → физический индекс [верх, право, низ, лево]
/// по письму КОРНЯ (css-writing-modes-4 §6.2): `page-box-008`
/// (`html { writing-mode: vertical-rl }`) — inline-start сверху,
/// block-start справа; `page-box-009` — как у горизонтального корня.
pub(super) fn logical_side(wm: (bool, bool, bool), block: bool, start: bool) -> usize {
    let (vertical, vertical_rl, rtl) = wm;
    match (vertical, block) {
        (false, true) if start => 0,
        (false, true) => 2,
        (false, false) if start != rtl => 3,
        (false, false) => 1,
        (true, true) if start == vertical_rl => 1,
        (true, true) => 3,
        (true, false) if start != rtl => 0,
        (true, false) => 2,
    }
}

/// `em` — от кегля контекста страницы (css-page-3 §page-properties: «Values
/// in units of em … relative to the font associated with their context»);
/// кегль листа по умолчанию — 16 (`content-004`: `margin: 4em` = 64).
pub(super) fn page_font_size(decls: &[(String, String)]) -> f32 {
    decls
        .iter()
        .rev()
        .find(|(k, _)| k == "font-size")
        .and_then(|(_, v)| match Len::parse(v.trim()) {
            Some(Len::Px(p)) => Some(p),
            Some(Len::Em(k)) => Some(k * 16.0),
            Some(Len::Pct(k)) => Some(k * 16.0),
            _ => None,
        })
        .unwrap_or(16.0)
}

/// Длина: точки как есть; vw/vh — от ДЕФОЛТНОЙ страницы (по тестам
/// page-size-016/017).
pub(super) fn abs_len(t: &str, fs: f32) -> Option<f32> {
    match Len::parse(t)? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * fs),
        Len::Vw(k) => Some(k * 480.0),
        Len::Vh(k) => Some(k * 288.0),
        _ => None,
    }
}

/// Как `abs_len`, плюс проценты — от ОБЪЯВЛЕННОГО размера страницы по своей
/// оси (page-margin-005: 10% от 300px = 30px).
pub(super) fn rel_len(t: &str, axis_h: bool, fs: f32, (pw, ph): (f32, f32)) -> Option<f32> {
    match Len::parse(t)? {
        Len::Px(v) => Some(v),
        Len::Em(k) => Some(k * fs),
        Len::Vw(k) => Some(k * 480.0),
        Len::Vh(k) => Some(k * 288.0),
        Len::Pct(k) => Some(k * if axis_h { ph } else { pw }),
        _ => None,
    }
}

/// Стороны сокращения `margin`/`padding`: верх, право, низ, лево.
fn sides(v: &str) -> (&str, &str, &str, &str) {
    let vals: Vec<&str> = v.split_whitespace().collect();
    let side = |i: usize| vals.get(i).copied().unwrap_or("0");
    match vals.len() {
        1 => (side(0), side(0), side(0), side(0)),
        2 => (side(0), side(1), side(0), side(1)),
        3 => (side(0), side(1), side(2), side(1)),
        _ => (side(0), side(1), side(2), side(3)),
    }
}

/// `size` листа: числа, имена носителей и ориентация.
pub(super) fn apply_size(v: &str, fs: f32, w: &mut f32, h: &mut f32) {
    let toks: Vec<&str> = v.split_whitespace().collect();
    let nums: Vec<f32> = toks.iter().filter_map(|t| abs_len(t, fs)).collect();
    // Нулевой лист — начальное значение (csswg#8335;
    // `printing/zero-size-001-print`: «The used page size is the
    // initial value instead of the authored width and height of zero»).
    let nums: Vec<f32> = if nums.iter().any(|v| *v <= 0.0) {
        Vec::new()
    } else {
        nums
    };
    match nums.len() {
        2 => {
            *w = nums[0];
            *h = nums[1];
        }
        1 => {
            *w = nums[0];
            *h = nums[0];
        }
        _ => {}
    }
    // Имена носителей (css-page-3 §page-size-prop, `<page-size>`:
    // «A5 — 148mm wide and 210 mm high» и т.д.) и ориентация:
    // `landscape` кладёт длинную сторону горизонтально, `portrait`
    // — вертикально (`page-size-011-print`: эталон пишет те же
    // листы в мм). Без имени ориентация поворачивает лист WPT.
    let mm = 96.0 / 25.4;
    for t in &toks {
        let named = match t.to_ascii_lowercase().as_str() {
            "a5" => Some((148.0 * mm, 210.0 * mm)),
            "a4" => Some((210.0 * mm, 297.0 * mm)),
            "a3" => Some((297.0 * mm, 420.0 * mm)),
            "b5" => Some((176.0 * mm, 250.0 * mm)),
            "b4" => Some((250.0 * mm, 353.0 * mm)),
            "jis-b5" => Some((182.0 * mm, 257.0 * mm)),
            "jis-b4" => Some((257.0 * mm, 364.0 * mm)),
            "letter" => Some((8.5 * 96.0, 11.0 * 96.0)),
            "legal" => Some((8.5 * 96.0, 14.0 * 96.0)),
            "ledger" => Some((11.0 * 96.0, 17.0 * 96.0)),
            _ => None,
        };
        if let Some((a, b)) = named {
            *w = a;
            *h = b;
        }
    }
    for t in &toks {
        match t.to_ascii_lowercase().as_str() {
            "landscape" if *w < *h => std::mem::swap(w, h),
            "portrait" if *w > *h => std::mem::swap(w, h),
            _ => {}
        }
    }
}

/// Сокращение `margin`: `auto`-стороны помечаются, их размер решает остаток.
pub(super) fn apply_margin(
    v: &str,
    px_of: impl Fn(&str, bool) -> Option<f32>,
    margin: &mut [f32; 4],
    auto_m: &mut [bool; 4],
) {
    let (a, b, c, d) = sides(v);
    for (i, (slot, (t, vert))) in margin
        .iter_mut()
        .zip([(a, true), (b, false), (c, true), (d, false)])
        .enumerate()
    {
        auto_m[i] = t == "auto";
        if auto_m[i] {
            *slot = 0.0;
        } else if let Some(px) = px_of(t, vert) {
            *slot = px;
        }
    }
}

/// Сокращение `padding`.
pub(super) fn apply_padding(
    v: &str,
    px_of: impl Fn(&str, bool) -> Option<f32>,
    padding: &mut [f32; 4],
) {
    let (a, b, c, d) = sides(v);
    for (slot, (t, vert)) in padding
        .iter_mut()
        .zip([(a, true), (b, false), (c, true), (d, false)])
    {
        if let Some(px) = px_of(t, vert) {
            *slot = px;
        }
    }
}

/// Сокращение `border` листа.
pub(super) fn apply_border(
    v: &str,
    px_of: impl Fn(&str, bool) -> Option<f32>,
    border: &mut (f32, Color),
) {
    // Сокращение без толщины — `medium`, 3px (css-backgrounds-3
    // §border-width); `none`/`hidden` — без рамки
    // (`margin-boxes/auto-margins-001`: `@page { border: solid }`).
    let mut width: Option<f32> = None;
    let mut styled = false;
    for t in v.split_whitespace() {
        match t.to_ascii_lowercase().as_str() {
            "none" | "hidden" => {
                width = Some(0.0);
                styled = false;
            }
            "solid" | "dashed" | "dotted" | "double" | "groove" | "ridge" | "inset" | "outset" => {
                styled = true
            }
            "thin" => width = Some(1.0),
            "medium" => width = Some(3.0),
            "thick" => width = Some(5.0),
            _ => {
                if let Some(px) = px_of(t, false) {
                    width = Some(px);
                } else if let Some(c) = Color::parse(t) {
                    border.1 = c;
                }
            }
        }
    }
    border.0 = match width {
        Some(w) if styled => w,
        None if styled => 3.0,
        _ => 0.0,
    };
}

/// Размер листа по оси при явном `width`/`height` (см. `page_box`).
pub(super) fn fit(size: f32, used: f32, a: bool, b: bool, ma: &mut f32, mb: &mut f32) -> f32 {
    match (a, b) {
        (true, true) => {
            *ma = (size - used) / 2.0;
            *mb = *ma;
            size
        }
        (true, false) => {
            *ma = size - used - *mb;
            size
        }
        (false, true) => {
            *mb = size - used - *ma;
            size
        }
        (false, false) => used + *ma + *mb,
    }
}
