//! Строки с краской, не зависящей от окна (градиент, рамка), при смене ширины
//! потока: `Computed::has_viewport_lengths` отвечает за них «нет», и они
//! переживают её так же, как строки простого текста.

use super::super::tests::{frame, open};
use super::{resize, reused};
use gpui::TestAppContext;

fn rows(count: usize, style: &str) -> String {
    let rows: String = (0..count)
        .map(|i| format!("<div class=row>row {i} alpha beta</div>"))
        .collect();
    format!(
        "<style>body{{margin:0}}.row{{height:20px;border-bottom:1px solid #abc;{style}}}\
         .row:nth-child(2n){{padding-left:7px}}</style><body>{rows}</body>"
    )
}

/// Blocks recorded, skipped, refused and laid out from a far record by one
/// more ±1px build of a page that has already been built at both widths.
fn one_resize(cx: &mut TestAppContext, page: &str) -> (u64, u64, u64, u64) {
    let (view, cx) = open(cx, page);
    frame(cx);
    for width in [401., 400., 401.] {
        resize(&view, cx, width);
    }
    let far = |cx: &mut gpui::VisualTestContext| cx.update(|_, cx| view.read(cx).doc.far_blocks());
    let (before, far_before) = (reused(&view, cx), far(cx));
    resize(&view, cx, 400.);
    let after = reused(&view, cx);
    (
        after.0 - before.0,
        after.1 - before.1,
        after.2 - before.2,
        far(cx) - far_before,
    )
}

/// A row painted with a gradient is skipped at another flow width like a row
/// of plain text. What is still built is the band near the screen (a
/// stretched block cannot repeat the paint of another width): its size does
/// not grow with the document. No block takes the far layout path, because a
/// skip placeholder already replaced every far one.
#[gpui::test]
fn gradient_rows_are_skipped_across_a_width_change(cx: &mut TestAppContext) {
    const GRADIENT: &str = "background:linear-gradient(90deg,#def,#fff)";
    let plain = one_resize(cx, &rows(400, ""));
    let short = one_resize(cx, &rows(400, GRADIENT));
    let long = one_resize(cx, &rows(800, GRADIENT));
    assert_eq!(short, plain, "gradient rows differ from plain rows");
    let (recorded, skipped, rebuilt, far) = short;
    assert!(skipped > 200, "gradient rows were rebuilt: {skipped}");
    assert_eq!((rebuilt, far), (0, 0));
    assert_eq!(
        (long.0, long.1, long.2, long.3),
        (recorded, skipped + 400, 0, 0),
        "the built band grew with the document"
    );
    // Every built row but an edge of the flow is recorded again.
    assert!(400 - skipped - recorded <= 2, "{recorded} {skipped}");
}

/// The same row with a viewport unit in its gradient follows the viewport.
#[gpui::test]
fn rows_with_viewport_units_in_a_gradient_are_rebuilt(cx: &mut TestAppContext) {
    let page = rows(400, "background:linear-gradient(90deg,#def 1vw,#fff)");
    let (recorded, skipped, _, _) = one_resize(cx, &page);
    assert_eq!((recorded, skipped), (0, 0));
}
