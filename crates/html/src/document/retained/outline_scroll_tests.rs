//! Outline ink must not determine scrollport bars (CSS Overflow 3, section 2.1).

use super::tests::{frame, open};
use gpui::TestAppContext;

fn page(display: &str, outline: &str, width: &str) -> String {
    format!(
        "<style>*{{box-sizing:border-box}}body{{margin:0}}\
         .scroller{{width:200px;overflow:auto;border:1px solid;padding:24px;{display}}}\
         .child{{padding:24px 5%;outline:{outline};{width}}}p{{margin:0}}</style>\
         <div class=scroller><div class=child><p>short line</p></div></div>"
    )
}

#[gpui::test]
fn outline_ink_does_not_create_scrollbars(cx: &mut TestAppContext) {
    let (_one, _sub) = crate::why::test_reader();
    for display in ["", "display:flex;flex-direction:column", "display:grid"] {
        for outline in ["2px dotted blue", "4px solid blue", "6px double blue"] {
            let html = page(display, outline, "");
            let mut app = cx.clone();
            crate::why::take();
            let (_view, vcx) = open(&mut app, &html);
            frame(vcx);
            let journal = crate::why::take().journal;
            assert!(
                !journal.causes.iter().any(|cause| matches!(
                    cause.code,
                    "scroll_extent.layout_bars" | "scroll_extent.bars_disagree"
                )),
                "{display} / {outline}: {journal:?}"
            );
            assert_eq!(vcx.update(|window, _| window.take_frame_requests()), 0);
            let first = vcx.update(|window, _| format!("{:?}", window.painted_quads()));
            frame(vcx);
            assert_eq!(
                first,
                vcx.update(|window, _| format!("{:?}", window.painted_quads()))
            );
            assert_eq!(vcx.update(|window, _| window.take_frame_requests()), 0);
        }
    }
}

#[gpui::test]
fn excluding_outline_ink_keeps_real_child_overflow(cx: &mut TestAppContext) {
    let (_one, _sub) = crate::why::test_reader();
    let mut baseline = None;
    for outline in ["none", "2px dotted blue"] {
        let mut app = cx.clone();
        crate::why::take();
        let (_view, vcx) = open(&mut app, &page("", outline, "width:300px"));
        frame(vcx);
        let journal = crate::why::take().journal;
        let bars: Vec<_> = journal
            .causes
            .iter()
            .filter(|cause| cause.code == "scroll_extent.layout_bars")
            .map(|cause| cause.detail.split(';').next().unwrap().to_owned())
            .collect();
        assert!(
            bars.iter().any(|detail| detail.contains("-> [true,")),
            "{journal:?}"
        );
        if let Some(baseline) = &baseline {
            assert_eq!(&bars, baseline, "outline changed real overflow");
        } else {
            baseline = Some(bars);
        }
    }
}
