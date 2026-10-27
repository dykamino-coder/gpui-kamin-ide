//! Подписи обёртки таблицы: сторона из computed `caption-side` и размеры только из
//! записанной раскладки (подпись, коробка таблицы, обёртка).

use super::*;

/// Число верхних и нижних подписей первой таблицы документа после
/// материализации computed стилей (наследование идёт при ней, как в render).
fn sides(html: &str) -> (usize, usize) {
    let doc = crate::dom::parse(html, "");
    crate::style::cascade::materialize::materialize(
        &doc,
        &std::rc::Rc::new(crate::style::computed::Computed::default()),
        doc.values(),
    );
    let (b, _) = collected(&doc);
    let t = b.table.as_ref().expect("table box");
    (t.caps_top.len(), t.caps_bot.len())
}

#[test]
fn caption_side_is_the_inherited_computed_value() {
    // `caption-side` наследуется (CSS 2.2 §17.4.1): значение предка таблицы
    // доходит до подписи, собственное значение подписи его перекрывает, а
    // значение ряда до подписи не доходит (подпись — не его ребёнок).
    let cap = |style: &str| format!("<div style=\"display:table-caption;{style}\">c</div>");
    let html = format!(
        "<div style=\"caption-side:bottom\"><div style=\"display:table\">{}{}</div></div>",
        cap(""),
        cap("caption-side:top")
    );
    assert_eq!(sides(&html), (1, 1));
    let html = format!(
        "<div style=\"display:table;caption-side:bottom\">{}</div>",
        cap("")
    );
    assert_eq!(sides(&html), (0, 1));
    let html = format!(
        "<div style=\"display:table\"><div style=\"display:table-row;caption-side:bottom\"></div>{}</div>",
        cap("")
    );
    assert_eq!(sides(&html), (1, 0));
}

#[test]
fn caption_and_table_box_sizes_come_from_the_laid_wrapper() {
    // Подпись 10 над коробкой таблицы 20 без рядов: обёртка 30 — стопка двух
    // соседей с точкой класса A между ними (css-tables-3 §terminology), а не
    // оценка с нулевой подписью.
    let (b, ps) = table(
        "<div style=\"display:table;height:20px\"><div style=\"display:table-caption\">\
         <p style=\"margin:0\">c</p></div></div>",
    );
    let t = b.table.as_ref().expect("table box");
    let cap = t.caps_top[0].id;
    let mut laid = Laid::of_boxes(&[(cap, 0.0, 10.0), (ps[0], 0.0, 10.0)]);
    laid.tables.insert(
        t.key,
        TableFrame {
            table: (10.0, 30.0),
            grid: (10.0, 30.0),
            rows: Vec::new(),
        },
    );
    let m = b
        .breaks(30.0, &laid)
        .finish(FlowGeometry::lines([10.0]), 30.0);
    assert_eq!(m.cuts, vec![(10.0, 10.0)]);
    assert!(m.forced.is_empty());
}
