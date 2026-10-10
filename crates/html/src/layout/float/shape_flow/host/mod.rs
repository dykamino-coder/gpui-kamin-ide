//! Хост обтекания формой: div хоста, полосы и прогон атомов.

use crate::dom::{Element, Node};
use crate::layout::float::band_host::{BandPiece, band_piece, px_margin, px_margin_box};
use crate::render::{RenderOpts, blocks, styled_div_with};
use crate::style::cascade::inherit::inherit;
use crate::style::computed::Computed;
use gpui::{AnyElement, IntoElement, ParentElement, Styled, div, px};
mod atoms;
pub(super) use atoms::atoms_flow;

pub(super) fn shape_host_div(
    e: &Element,
    inherited: &Computed,
    vert_lr: bool,
    bands: &super::super::bands::FloatBands,
) -> gpui::Div {
    if inherited.vertical_rl == Some(true) || vert_lr {
        div().relative()
    } else if e.attr("inflow-height") != Some("1") && bands.bottom(None) > 0.0 {
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО: гейтить охват флоатов признаком «коробка
        // образует свой контекст форматирования», как это делают Blink
        // (`block_layout_algorithm.cc`: `IsNewFormattingContext()`) и Servo
        // (`BlockFormattingContext::layout`). Предикат был полный: флоат,
        // внепоточность, `inline-*`, таблица, ячейка, гибкий, сетка,
        // `flow-root`, `contain`, обрезка по `overflow`, корень и тело.
        // Срез флоатов (333 пары, 233 зелёных): 230. Приобретений ноль,
        // потери — `clear-003` 0.00 → 3.84, `floats-005` 0.00 → 0.72,
        // `floats-wrap-top-below-bfc-001l` 0.01 → 0.61.
        // Причина: у нас флоаты в этом хосте АБСОЛЮТНЫЕ, и `min_h` держит не
        // только §10.6.7, но и высоту, которую по спеке дают ОЧИСТИВШИЕ их
        // братья в потоке. Возвращать вместе с клиренсом как величиной в
        // потоке (шаг F6 из `target/scout-float-bands-design.md`).
        // §10.6.7: хост обязан охватить флоаты высотой — здесь они
        // абсолютные и сами её не растят.
        // ★ ЗАМЕРЕНО И ОТКАЧЕНО (10.09, `scout-floatover-2026-09.md`, 9
        // хунков): отрицательное `margin-top` у блочного ребёнка обнуляет
        // авто-высоту родителя (шесть проб `k1`-`k6`: верные 100/120/150/
        // 130/100/140, у нас 0.8 во всех шести — места детей при этом
        // верны, ломается только §10.6.7). Скаут закрывал это ОБЁРТКОЙ
        // заданной высоты и замерил ей восемь своих проб в 0.00.
        // Срез 3018 пар (флоаты + clamp + 700 текстовых): 2264 -> 2265,
        // и вся прибавка — от ДРУГОГО патча в пачке. Своё: `floats-135`
        // 0.00 взята, `margin-collapse-158` 0.06 -> 1.13 потеряна.
        // Бисект однозначен: тот же corpus без этого патча оставляет
        // `margin-collapse-158` = 0.06. Обёртка заданной высоты рвёт
        // §8.3.1 — сквозь неё перестают схлопываться поля. Находку
        // (обнуление высоты) держать, лечение искать без обёртки:
        // §10.6.7 считает низ содержимого от КРАЁВ детей с учётом
        // отрицательных полей, а не от суммы высот.
        div().relative().w_full().min_h(px(bands.bottom(None)))
    } else {
        div().relative().w_full()
    }
}

#[allow(clippy::result_large_err)]
#[allow(clippy::too_many_arguments)]
pub(super) fn band_host_flow(
    e: &Element,
    inherited: &Computed,
    opts: &RenderOpts,
    vert_lr: bool,
    bands: super::super::bands::FloatBands,
    rest: &Vec<Node>,
    mut host: gpui::Div,
    band_host: bool,
) -> Result<AnyElement, gpui::Div> {
    if band_host
        && inherited.vertical_rl != Some(true)
        && !vert_lr
        && rest
            .iter()
            .any(|n| matches!(band_piece(n), Some(BandPiece::Bfc) | Some(BandPiece::Strut)))
    {
        // Потолок потока: низ предыдущего куска (правило 5 §9.5.1). Бежит по
        // кускам и служит стартом поиска окна для следующего.
        let mut y = 0.0f32;
        for n in rest {
            let Node::Element(c) = n else {
                continue;
            };
            let (Some(kind), Some((mw, mh))) = (band_piece(n), px_margin_box(&c.style)) else {
                continue;
            };
            if matches!(kind, BandPiece::Strut) {
                // Распорка своего контекста не заводит: её border-box флоаты
                // перекрывают (обтекают только строки), а показать ей нечего
                // — от неё нужна одна высота.
                y += mh;
                continue;
            }
            let (ml, mr, mt, mb) = (
                px_margin(&c.style.margin.left).unwrap_or(0.0),
                px_margin(&c.style.margin.right).unwrap_or(0.0),
                px_margin(&c.style.margin.top).unwrap_or(0.0),
                px_margin(&c.style.margin.bottom).unwrap_or(0.0),
            );
            // §9.5, последний абзац, дословно: «The border box of a table, a
            // block-level replaced element, or an element in the normal flow
            // that establishes a new block formatting context … must not
            // overlap the margin box of any floats». Требование стоит на
            // BORDER-box коробки; её собственные поля в перечень не входят
            // ВООБЩЕ. Поэтому окно ищется под border-box, а поля работают
            // только по блочной оси: верхнее опускает потолок поиска, нижнее
            // задаёт потолок следующего куска.
            //
            // Так же у Blink (`block_layout_algorithm.cc:2164-2172`):
            // «Margins are applied from the content-box, not the layout
            // opportunity area», и проверка влезания там идёт по
            // `fragment.InlineSize()` / `fragment.BlockSize()` (`:2206`,
            // `:2283`) — то есть по border-box фрагмента.
            //
            // Отпечаток числа (`new-fc-beside-float-with-margin`): коробка
            // 50 точек с `margin-right: 1px` рядом с флоатом 50 в блоке 100.
            // margin-box 51 в окно 50 не влезал, и коробка уезжала на y=100
            // под флоат; border-box 50 влезает ровно, и она встаёт на y=0
            // сбоку — как и требует `meta assert` теста.
            //
            // border-box выводится вычитанием полей из уже посчитанного
            // margin-box: `px_margin_box` складывает width + padding + border
            // + margin теми же `px_margin`, поэтому разность точна.
            let (bw, bh) = (mw - ml - mr, mh - mt - mb);
            let (l, top, _avail) = bands.place_among(bw, bh, y + mt);
            y = top + bh + mb;
            // Коробка прижимается к инлайн-НАЧАЛУ полосы. `margin: auto`
            // прижимом не считается СОЗНАТЕЛЬНО: эталоны `-001r` выравнивают
            // свои коробки `text-align: right`, о котором `FlowRow` не знает
            // вовсе, и учесть одно без другого — значит развести пару
            // (см. scout-bfcdraft §0.3(г)).
            let mut inner = c.clone();
            // Поля кладёт держатель — на самой коробке они сдвинули бы её
            // ещё раз (та же причина, что у флоатов, `:5092-5095`).
            inner.style.margin = crate::style::computed::Sides::default();
            let merged = inherit(inherited, &c.style);
            let built = styled_div_with(&inner, &merged)
                .children(blocks(&inner.children, &merged, opts))
                .into_any_element();
            host = host.child(div().absolute().left(px(l + ml)).top(px(top)).child(built));
        }
        // §10.6.7 плюс собственная высота потока: держатели абсолютные и сами
        // хост не растят. `min_h` переопределяет поставленный на `:5129` —
        // это и нужно, там учтены только флоаты.
        let bottom = if e.attr("inflow-height") == Some("1") {
            y
        } else {
            bands.bottom(None).max(y)
        };
        return Ok(host.min_h(px(bottom)).into_any_element());
    }
    Err(host)
}
