//! Общие прогоны и состояние выделения текста мышью.

use gpui::{
    GlobalElementId, Hsla, MouseButton, MouseDownEvent, MouseMoveEvent, MouseUpEvent, Pixels,
    Point, TextRun, Window,
};

/// Границы выделения в байтах и состояние перетаскивания между кадрами.
#[derive(Default, Clone, Copy)]
pub(crate) struct Selection {
    pub(crate) anchor: usize,
    pub(crate) head: usize,
    pub(crate) dragging: bool,
}
impl Selection {
    pub(crate) fn range(&self) -> (usize, usize) {
        (self.anchor.min(self.head), self.anchor.max(self.head))
    }
}

pub(crate) fn selected_runs(
    runs: &[TextRun],
    highlight: Hsla,
    from: usize,
    to: usize,
) -> Vec<TextRun> {
    if from >= to {
        return runs.to_vec();
    }
    let mut out = Vec::with_capacity(runs.len() + 2);
    let mut at = 0usize;
    for run in runs {
        let end = at + run.len;
        let mut cut = |start: usize, stop: usize, selected: bool| {
            if stop <= start {
                return;
            }
            let mut piece = run.clone();
            piece.len = stop - start;
            piece.background_color = selected.then_some(highlight);
            out.push(piece);
        };
        cut(at, end.min(from), false);
        cut(at.max(from), end.min(to), true);
        cut(at.max(to), end, false);
        at = end;
    }
    out
}

pub(crate) fn track_selection(
    global: &GlobalElementId,
    inside: bool,
    index_at: impl Fn(Point<Pixels>) -> Option<usize> + Clone + 'static,
    window: &mut Window,
) {
    window.with_element_state::<Selection, _>(global, |state, window| {
        let st = std::rc::Rc::new(std::cell::Cell::new(state.unwrap_or_default()));
        let down = st.clone();
        let at_down = index_at.clone();
        window.on_mouse_event(move |e: &MouseDownEvent, phase, window, _cx| {
            if !phase.bubble() || e.button != MouseButton::Left || !inside {
                return;
            }
            let Some(i) = at_down(e.position) else { return };
            down.set(Selection {
                anchor: i,
                head: i,
                dragging: true,
            });
            window.refresh();
        });

        let mv = st.clone();
        let at_move = index_at.clone();
        window.on_mouse_event(move |e: &MouseMoveEvent, phase, window, _cx| {
            if !phase.bubble() {
                return;
            }
            let mut s = mv.get();
            if !s.dragging {
                return;
            }
            if let Some(i) = at_move(e.position)
                && s.head != i
            {
                s.head = i;
                mv.set(s);
                window.refresh();
            }
        });

        let up = st.clone();
        window.on_mouse_event(move |_e: &MouseUpEvent, phase, _window, _cx| {
            if !phase.bubble() {
                return;
            }
            let mut s = up.get();
            if s.dragging {
                s.dragging = false;
                up.set(s);
            }
        });

        ((), st.get())
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_splits_runs_and_preserves_their_style() {
        let old = gpui::hsla(0.2, 0.3, 0.4, 1.0);
        let highlight = gpui::hsla(0.6, 0.7, 0.8, 1.0);
        let runs = vec![
            TextRun {
                len: 4,
                background_color: Some(old),
                font_size: Some(gpui::px(18.0)),
                ..Default::default()
            },
            TextRun {
                len: 6,
                color: old,
                ..Default::default()
            },
        ];
        let pieces = selected_runs(&runs, highlight, 2, 7);
        assert_eq!(
            pieces.iter().map(|r| r.len).collect::<Vec<_>>(),
            [2, 2, 3, 3]
        );
        for (i, piece) in pieces.iter().enumerate() {
            let mut expected = runs[usize::from(i >= 2)].clone();
            expected.len = [2, 2, 3, 3][i];
            expected.background_color = (i == 1 || i == 2).then_some(highlight);
            assert_eq!(*piece, expected);
        }
        assert_eq!(selected_runs(&runs, highlight, 4, 4), runs);
        assert_eq!(selected_runs(&runs, highlight, 8, 2), runs);
        assert_eq!(
            selected_runs(&runs, highlight, 0, 20)
                .iter()
                .map(|r| r.len)
                .sum::<usize>(),
            10
        );
    }

    #[test]
    fn dragging_backwards_normalizes_byte_range() {
        let state = Selection {
            anchor: 7,
            head: 2,
            dragging: true,
        };
        assert_eq!(state.range(), (2, 7));
    }
}
