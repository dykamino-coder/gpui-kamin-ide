//! Внешний маркер не создаёт точки разрыва в основном потоке пункта.

use crate::layout::options::RenderOpts;

pub(super) fn options(opts: &RenderOpts) -> RenderOpts {
    let mut marker = opts.clone();
    // css-lists-3 §3.5: outside находится вне principal block box.
    // Метрики его строки задают позицию маркера, но не удерживают основное
    // содержимое во фрагментаинере и не потребляют его inline-продолжение.
    marker.frag.watch = None;
    marker
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::layout::fragment::paginate::watch::Watched;
    use std::rc::Rc;

    #[test]
    fn outside_line_cannot_publish_into_the_item_recorder() {
        let watch = Watched::new(&[1]);
        let opts = RenderOpts {
            text: Default::default(),
            viewport: (800., 600.),
            normal_line_height: 1.2,
            cx: Default::default(),
            frag: crate::layout::fragment::types::FragCx {
                watch: Some(watch.clone()),
                in_stack_copy: true,
                ..Default::default()
            },
        };
        let marker = options(&opts);
        assert!(marker.frag.watch.is_none());
        assert!(Rc::ptr_eq(opts.frag.watch.as_ref().unwrap(), &watch));
        assert!(marker.frag.in_stack_copy);
        assert_eq!(marker.viewport, opts.viewport);
    }
}
