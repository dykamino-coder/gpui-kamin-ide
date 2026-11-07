//! KaminIDE patch: one shared atlas for the retained images of a frame's paint groups.
//!
//! Every group used to own a texture of its exact region size, so a page with
//! hundreds of filtered or blended groups created hundreds of textures on its
//! first frame and switched render targets between all of them. Like
//! WebRender's texture cache and render-task atlases, the retained images of
//! one frame are shelf-packed into a few large pages instead: writes and
//! composites then address an integer rectangle of a shared surface, and
//! independent groups can be drawn as instances of one draw call.

/// Largest page side the packer opens on its own (a single larger region
/// still gets a page of its own size).
pub(super) const PAGE_LIMIT: u32 = 4096;

/// Where a group's retained image lives: page index and integer top-left corner.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Place {
    pub page: usize,
    pub x: u32,
    pub y: u32,
}

/// Shelf-pack `sizes` (width, height). Returns the place of every size, in
/// input order, and the used size of every page. Taller rectangles go first,
/// so each shelf is as tall as its first rectangle; equal heights keep their
/// input order, which keeps the layout stable from frame to frame.
pub(super) fn pack(sizes: &[(u32, u32)]) -> (Vec<Place>, Vec<(u32, u32)>) {
    if sizes.is_empty() {
        return (Vec::new(), Vec::new());
    }
    let max_w = sizes.iter().map(|s| s.0).max().unwrap_or(1).max(1);
    let area: u64 = sizes.iter().map(|&(w, h)| u64::from(w) * u64::from(h)).sum();
    // Square-ish pages; the width is rounded so small changes of the content
    // do not change the layout of every shelf.
    let side = ((area as f64).sqrt().ceil() as u32).next_multiple_of(256);
    let width = side.clamp(256, PAGE_LIMIT).max(max_w);
    let mut order: Vec<usize> = (0..sizes.len()).collect();
    order.sort_by_key(|&i| (std::cmp::Reverse(sizes[i].1), i));
    let mut places = vec![Place::default(); sizes.len()];
    let mut pages = vec![(width, 0u32)];
    let (mut x, mut y, mut shelf) = (0u32, 0u32, 0u32);
    for i in order {
        let (w, h) = (sizes[i].0.max(1), sizes[i].1.max(1));
        if x + w > width {
            y += shelf;
            (x, shelf) = (0, 0);
        }
        if y > 0 && y + h > PAGE_LIMIT.max(h) {
            pages.push((width, 0));
            (x, y, shelf) = (0, 0, 0);
        }
        places[i] = Place {
            page: pages.len() - 1,
            x,
            y,
        };
        x += w;
        shelf = shelf.max(h);
        let page = pages.last_mut().unwrap();
        page.1 = page.1.max(y + shelf);
    }
    (places, pages)
}

/// Whether an existing page of `have` size can hold `need` without wasting
/// more than three quarters of its memory.
pub(super) fn page_fits(have: (u32, u32), need: (u32, u32)) -> bool {
    have.0 >= need.0
        && have.1 >= need.1
        && u64::from(have.0) * u64::from(have.1) <= 4 * u64::from(need.0) * u64::from(need.1.max(64))
}

/// Size of a new page for `need`: the height is rounded up so that a slowly
/// growing set of groups does not recreate the page every frame.
pub(super) fn page_size(need: (u32, u32)) -> (u32, u32) {
    (need.0, need.1.next_multiple_of(256).min(PAGE_LIMIT.max(need.1)))
}

#[cfg(test)]
mod tests {
    use super::{PAGE_LIMIT, Place, pack, page_fits, page_size};

    fn overlaps(a: (Place, (u32, u32)), b: (Place, (u32, u32))) -> bool {
        a.0.page == b.0.page
            && a.0.x < b.0.x + b.1.0
            && b.0.x < a.0.x + a.1.0
            && a.0.y < b.0.y + b.1.1
            && b.0.y < a.0.y + a.1.1
    }

    #[test]
    fn rectangles_are_disjoint_and_inside_their_pages() {
        let sizes: Vec<(u32, u32)> = (0..480)
            .map(|i| (200 + (i % 7) * 3, 160 + (i % 5) * 2))
            .chain([(1, 1), (800, 600), (37, 4000)])
            .collect();
        let (places, pages) = pack(&sizes);
        assert_eq!(places.len(), sizes.len());
        for (i, (&p, &s)) in places.iter().zip(&sizes).enumerate() {
            let page = pages[p.page];
            assert!(p.x + s.0 <= page.0 && p.y + s.1 <= page.1, "{i}: {p:?} {s:?} {page:?}");
            for (&q, &t) in places.iter().zip(&sizes).skip(i + 1) {
                assert!(!overlaps((p, s), (q, t)), "{p:?}{s:?} / {q:?}{t:?}");
            }
        }
        assert!(pages.iter().all(|p| p.0 <= PAGE_LIMIT && p.1 <= PAGE_LIMIT));
        // Near-uniform rectangles pack densely: the pages hold little more than
        // the rectangles themselves.
        let (_, pages) = pack(&sizes[..480]);
        let used: u64 = pages.iter().map(|p| u64::from(p.0) * u64::from(p.1)).sum();
        let need: u64 = sizes[..480].iter().map(|s| u64::from(s.0) * u64::from(s.1)).sum();
        assert!(used < need * 12 / 10, "{used} vs {need}");
    }

    #[test]
    fn oversized_rectangles_get_their_own_page_and_layout_is_stable() {
        let (places, pages) = pack(&[(5000, 10), (10, 5000), (3, 3)]);
        assert_eq!(pages.len(), 2);
        assert_eq!(places[0], Place { page: 1, x: 0, y: 0 });
        assert_eq!(places[1], Place { page: 0, x: 0, y: 0 });
        assert_eq!(pack(&[(5000, 10), (10, 5000), (3, 3)]).0, places);
        assert_eq!(pack(&[]), (Vec::new(), Vec::new()));
    }

    #[test]
    fn pages_are_reused_only_without_gross_waste() {
        assert!(page_fits((1024, 1024), (1000, 700)));
        assert!(!page_fits((1024, 512), (1000, 700)));
        assert!(!page_fits((4096, 4096), (512, 256)));
        assert_eq!(page_size((768, 700)), (768, 768));
        assert_eq!(page_size((768, 4090)), (768, 4096));
        assert_eq!(page_size((768, 5000)), (768, 5000));
    }
}
