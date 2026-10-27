//! Тесты таблицы блочной стопки: факты стиля рядов и точки разрыва по заданному кадру.

use crate::dom::{Element, Node};
use crate::layout::fragment::flow::FlowGeometry;
use crate::layout::fragment::flow::blocks::{BlockBox, Laid, Pushed, TableFrame};
use crate::style::computed::Display;

mod captions;

/// Стопка первой таблицы документа (`display: table`) и номера узлов блоков
/// `<p>` в её ячейках в порядке документа.
fn table(html: &str) -> (BlockBox, Vec<u64>) {
    let doc = crate::dom::parse(html, "");
    fn find(n: &Node) -> Option<&Element> {
        let Node::Element(e) = n else { return None };
        if e.style.used().display == Some(Display::Table) {
            return Some(e);
        }
        e.children.iter().find_map(find)
    }
    let root = doc.iter().find_map(find).expect("table");
    let b = BlockBox::collect(
        root,
        crate::layout::fragment::ShapeCx::COLUMNS,
        doc.values(),
    );
    fn paragraphs(e: &Element, out: &mut Vec<u64>) {
        for k in &e.children {
            if let Node::Element(k) = k {
                if k.tag == "p" {
                    out.push(k.node_id);
                }
                paragraphs(k, out);
            }
        }
    }
    let mut ids = Vec::new();
    paragraphs(root, &mut ids);
    (b, ids)
}

/// Таблица из рядов со стилями `styles`, по ячейке с содержимым `cell` в ряду.
fn rows(styles: &[&str], cell: &str) -> String {
    let rows: String = styles
        .iter()
        .map(|s| {
            format!(
                "<div style=\"display:table-row;{s}\"><div style=\"display:table-cell\">{cell}</div></div>"
            )
        })
        .collect();
    format!("<div style=\"display:table\">{rows}</div>")
}

/// Шапка с запретом разрыва внутри и тело из двух рядов; во втором ряду —
/// блок `<p>`.
fn sectioned() -> String {
    let row = |cell: &str| {
        format!(
            "<div style=\"display:table-row\"><div style=\"display:table-cell\">{cell}</div></div>"
        )
    };
    format!(
        "<div style=\"display:table\"><div style=\"display:table-header-group;break-inside:avoid\">{}</div><div style=\"display:table-row-group\">{}{}</div></div>",
        row("a"),
        row("<p style=\"margin:0\">b</p>"),
        row("c")
    )
}

/// Кадр таблицы: коробка 0..100, сетка 10..90, ряды через зазор 2.
fn frame(key: u64, boxes: &[(u64, f32, f32)]) -> Laid {
    let mut laid = Laid::of_boxes(boxes);
    laid.tables.insert(
        key,
        TableFrame {
            table: (0.0, 100.0),
            grid: (10.0, 90.0),
            rows: vec![(12.0, 30.0), (32.0, 60.0), (62.0, 88.0)],
        },
    );
    laid
}

type Out = (Vec<(f32, f32)>, Vec<f32>, Vec<(f32, f32)>);

fn finish(b: &BlockBox, laid: &Laid) -> Out {
    let m = b.breaks(100.0, laid).finish(FlowGeometry::default(), 100.0);
    (m.cuts, m.forced, m.solid)
}

fn key(b: &BlockBox) -> u64 {
    b.table.as_ref().expect("table box").key
}

#[test]
fn rows_are_collected_in_grid_order_with_their_sections() {
    let (b, _) = table(&sectioned());
    let t = b.table.as_ref().expect("table box");
    assert_eq!(t.rows.len(), 3);
    assert_eq!(t.head, Some((0, 0, true)));
    assert_eq!(t.foot, None);
    assert!(t.rows.iter().all(|r| r.cells.len() == 1));
    assert_ne!(t.rows[0].group, t.rows[1].group);
    assert_eq!(t.rows[1].group, t.rows[2].group);
}

#[test]
fn row_boundaries_truncate_the_spacing_and_keep_the_edges_whole() {
    let (b, _) = table(&rows(&["", "", ""], "x"));
    let (cuts, forced, solid) = finish(&b, &frame(key(&b), &[]));
    assert_eq!(cuts, vec![(30.0, 32.0), (60.0, 62.0)]);
    assert!(forced.is_empty());
    // Верх коробки до сетки и низ сетки до низа коробки — сплошные.
    assert!(solid.contains(&(0.0, 10.0)));
    assert!(solid.contains(&(90.0, 100.0)));
}

#[test]
fn forced_and_avoided_row_boundaries() {
    let (b, _) = table(&rows(
        &["", "break-after:avoid", "break-before:column"],
        "x",
    ));
    let (cuts, forced, _) = finish(&b, &frame(key(&b), &[]));
    // Принудительный разрыв открывает границу, закрытую запретом
    // (css-break-4 §forced-breaks).
    assert_eq!(cuts, vec![(30.0, 32.0), (60.0, 62.0)]);
    assert_eq!(forced, vec![60.0]);
    let (b, _) = table(&rows(&["", "break-after:avoid", ""], "x"));
    let (cuts, _, solid) = finish(&b, &frame(key(&b), &[]));
    // Сцепка рядов 2–3 — сплошная от границы перед рядом 2; низ коробки
    // приклеен к ней.
    assert_eq!(cuts, vec![(30.0, 32.0)]);
    assert!(solid.contains(&(30.0, 88.0)));
    assert!(solid.contains(&(30.0, 100.0)));
}

#[test]
fn sections_and_pushed_cell_content_come_from_the_laid_out_rows() {
    let (b, ps) = table(&sectioned());
    let laid = frame(key(&b), &[(ps[0], 40.0, 50.0)]);
    let s = b.breaks(100.0, &laid).sections.expect("sections");
    assert_eq!(s.head, Some((12.0, 18.0, true)));
    assert_eq!(s.spacing, 2.0);
    assert_eq!((s.box_top, s.box_end), (0.0, 100.0));
    // Коробка в ячейке второго ряда — перед ней встаёт распорка роста; ряд,
    // перенесённый с границы рядов, не растёт.
    assert_eq!(b.pushed_at(40.0, &laid), Some(Pushed::Box(ps[0])));
    assert_eq!(b.pushed_at(32.0, &laid), None);
    assert_eq!(b.pushed_at(12.0, &laid), None);
}

#[test]
fn block_start_edge_without_a_gap_holds_the_first_row_monolith() {
    // Рамка 0..10, первый ряд сразу под ней — монолит 10..60: разрыва между
    // рамкой и рядом нет (класс C — только при зазоре), монолит тянет её.
    let (b, _) = table(&rows(
        &[""],
        "<div style=\"contain:size;height:50px\"></div>",
    ));
    let k = key(&b);
    let mut laid = Laid::default();
    laid.tables.insert(
        k,
        TableFrame {
            table: (0.0, 70.0),
            grid: (10.0, 60.0),
            rows: vec![(10.0, 60.0)],
        },
    );
    let t = b.table.as_ref().unwrap();
    let mono = t.rows[0].cells[0].1.kids[0].id;
    laid.boxes.insert(mono, (10.0, 60.0));
    let m = b.breaks(70.0, &laid).finish(FlowGeometry::default(), 70.0);
    assert!(m.solid.contains(&(0.0, 60.0)));
}

#[test]
fn caption_line_geometry_stays_inside_the_table_wrapper_flow() {
    let (b, ps) = table(
        "<table style=\"display:table\"><caption style=\"display:table-caption\"><p style=\"margin:0\">caption</p></caption></table>",
    );
    let t = b.table.as_ref().unwrap();
    let cap = t.caps_top[0].id;
    let laid = Laid::of_boxes(&[(b.id, 0.0, 32.0), (cap, 0.0, 32.0), (ps[0], 0.0, 32.0)]);
    // Строка подписи — неделимый диапазон внутри обёртки; 16px остатка
    // колонки не разрешают разрезать её. Измерение строки должно дать
    // именно собственный поток подписи, а не выдуманную высоту таблицы.
    let lines = FlowGeometry::lines([32.0]);
    let m = b.breaks(32.0, &laid).finish(lines, 32.0);
    assert!(m.cuts.is_empty());
    assert_eq!(m.solid, vec![(0.0, 32.0)]);
    assert_eq!(b.pushed_at(0.0, &laid), Some(Pushed::Box(cap)));
}

#[test]
fn trailing_border_spacing_is_truncated_at_a_break() {
    // Без нижней рамки и отбивки коробка таблицы кончается сразу после
    // последнего ряда, если разрыв приходится на зазор под ним (Blink
    // «Truncate trailing border-spacing to fit within the fragmentainer»).
    let (b, _) = table(&rows(&["", ""], "x"));
    let mut laid = Laid::default();
    laid.tables.insert(
        key(&b),
        TableFrame {
            table: (0.0, 70.0),
            grid: (0.0, 70.0),
            rows: vec![(10.0, 30.0), (40.0, 60.0)],
        },
    );
    let (cuts, _, _) = finish(&b, &laid);
    assert_eq!(cuts, vec![(30.0, 40.0), (60.0, 70.0)]);
    // Нижняя рамка с отбивкой остаётся с последним рядом: усекать нечего.
    let (cuts, _, solid) = finish(&b, &frame(key(&b), &[]));
    assert_eq!(cuts, vec![(30.0, 32.0)]);
    assert!(solid.contains(&(90.0, 100.0)));
}
