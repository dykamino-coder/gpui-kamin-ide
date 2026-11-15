//! KaminIDE patch: the shelves of a packed group atlas, reused between frames.
//!
//! `pack` lays every group out again from the list of sizes, so one inserted
//! or removed group moves the rectangles of the groups after it, and their
//! kept images (`group_cache`) are lost. The shelves of the last full layout
//! stay instead: kept images keep their rectangles, and the groups without an
//! image take free space on a shelf at least as tall as they are (best fit
//! by shelf height, first fit along the shelf), the way WebRender's texture
//! cache (etagere bucketed shelves) and Skia's glyph atlas plots allocate
//! into a persistent page. Pages are never grown here: when nothing fits, the
//! caller packs the whole atlas again, which also undoes fragmentation.

use super::Place;

#[derive(Clone, Debug)]
struct Shelf {
    page: usize,
    y: u32,
    height: u32,
    /// Occupied spans `(x, width)`, sorted by `x`.
    used: Vec<(u32, u32)>,
}

/// The empty shelves of one atlas layout, or those shelves with the
/// rectangles of one frame placed on them.
#[derive(Clone, Debug, Default)]
pub(crate) struct Shelves {
    widths: Vec<u32>,
    shelves: Vec<Shelf>,
}

impl Shelves {
    /// The shelves of a `pack` layout (`places` of `sizes`) on pages whose
    /// textures are `pages` (width, height) — every shelf empty. A shelf
    /// reaches the next shelf of its page; the last one reaches the page end.
    pub(crate) fn of_layout(places: &[Place], pages: &[(u32, u32)]) -> Self {
        let mut tops: Vec<(usize, u32)> = places.iter().map(|p| (p.page, p.y)).collect();
        tops.sort_unstable();
        tops.dedup();
        let shelves = tops
            .iter()
            .enumerate()
            .filter_map(|(k, &(page, y))| {
                let end = match tops.get(k + 1) {
                    Some(&(next, ny)) if next == page => ny,
                    _ => pages.get(page)?.1,
                };
                Some(Shelf {
                    page,
                    y,
                    height: end.checked_sub(y)?,
                    used: Vec::new(),
                })
            })
            .collect();
        Self {
            widths: pages.iter().map(|p| p.0).collect(),
            shelves,
        }
    }

    /// Mark the rectangle `size` at `place` as taken. False when it is not on
    /// a shelf of this layout or overlaps a taken one.
    pub(crate) fn occupy(&mut self, place: Place, size: (u32, u32)) -> bool {
        let Some(shelf) = self.shelves.iter_mut().find(|s| {
            s.page == place.page && s.y == place.y && size.1 <= s.height
        }) else {
            return false;
        };
        let at = shelf.used.partition_point(|&(x, _)| x < place.x);
        let left_free = at == 0 || {
            let (x, w) = shelf.used[at - 1];
            x + w <= place.x
        };
        let right_free = shelf.used.get(at).is_none_or(|&(x, _)| place.x + size.0 <= x);
        let inside = self
            .widths
            .get(place.page)
            .is_some_and(|&w| place.x + size.0 <= w);
        if !(left_free && right_free && inside) {
            return false;
        }
        shelf.used.insert(at, (place.x, size.0));
        true
    }

    /// A free rectangle of `size`: on the lowest shelf that is tall enough and
    /// has a gap that wide, at the leftmost such gap.
    pub(crate) fn alloc(&mut self, size: (u32, u32)) -> Option<Place> {
        let (w, h) = (size.0.max(1), size.1.max(1));
        let mut best: Option<(u32, usize, u32)> = None;
        for (k, s) in self.shelves.iter().enumerate() {
            if s.height < h || best.is_some_and(|b| b.0 <= s.height) {
                continue;
            }
            let width = self.widths.get(s.page).copied().unwrap_or(0);
            let mut cursor = 0;
            let mut gap = None;
            for &(x, uw) in s.used.iter().chain([(width, 0)].iter()) {
                if x >= cursor && x - cursor >= w {
                    gap = Some(cursor);
                    break;
                }
                cursor = cursor.max(x + uw);
            }
            if let Some(x) = gap {
                best = Some((s.height, k, x));
            }
        }
        let (_, k, x) = best?;
        let shelf = &mut self.shelves[k];
        let at = shelf.used.partition_point(|&(ux, _)| ux < x);
        shelf.used.insert(at, (x, w));
        Some(Place {
            page: shelf.page,
            x,
            y: shelf.y,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::super::pack;
    use super::Shelves;

    #[test]
    fn a_freed_rectangle_is_given_to_a_group_of_its_size() {
        let sizes: Vec<(u32, u32)> = (0..40).map(|i| (100 + i % 3, 80)).collect();
        let (places, pages) = pack(&sizes);
        let mut s = Shelves::of_layout(&places, &pages);
        for (i, (&p, &z)) in places.iter().zip(&sizes).enumerate() {
            if i != 17 {
                assert!(s.occupy(p, z), "{i}");
            }
        }
        // Taken rectangles cannot be taken twice.
        assert!(!s.occupy(places[3], sizes[3]));
        assert_eq!(s.alloc(sizes[17]), Some(places[17]));
    }

    #[test]
    fn allocation_prefers_the_lowest_tall_enough_shelf_and_fails_when_full() {
        let (places, pages) = pack(&[(300, 200), (100, 50)]);
        let mut s = Shelves::of_layout(&places, &pages);
        assert!(s.occupy(places[0], (300, 200)));
        assert!(s.occupy(places[1], (100, 50)));
        let p = s.alloc((10, 40)).unwrap();
        assert_eq!(p.y, places[1].y, "the short shelf fits a short rectangle");
        assert!(s.alloc((pages[0].0, pages[0].1 + 1)).is_none());
        assert_eq!(
            s.alloc((1, 1)).map(|p| p.page),
            Some(0),
            "space is left on the page"
        );
    }
}
