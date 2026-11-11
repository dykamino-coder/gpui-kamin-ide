//! Compact break opportunities of a far paragraph's kept probe.

/// Break opportunities (`Paragraph::kept_stops`, ascending packed values) as
/// LEB128 deltas: one or two bytes per opportunity instead of four.
pub(super) struct PackedStops(Box<[u8]>);

impl PackedStops {
    pub(super) fn pack(stops: &[u32]) -> Self {
        let deltas = || {
            stops.iter().scan(0u32, |prev, &s| {
                Some(s.wrapping_sub(std::mem::replace(prev, s)))
            })
        };
        // Exact size first: the kept slice is allocated once.
        let len = deltas()
            .map(|d| (32 - (d | 1).leading_zeros()).div_ceil(7) as usize)
            .sum();
        let mut out = Vec::with_capacity(len);
        for mut d in deltas() {
            loop {
                let byte = (d & 0x7f) as u8;
                d >>= 7;
                if d == 0 {
                    out.push(byte);
                    break;
                }
                out.push(byte | 0x80);
            }
        }
        Self(out.into_boxed_slice())
    }

    pub(super) fn unpack(&self) -> Box<[u32]> {
        let count = self.0.iter().filter(|b| **b & 0x80 == 0).count();
        let mut out = Vec::with_capacity(count);
        let (mut prev, mut d, mut shift) = (0u32, 0u32, 0u32);
        for &byte in self.0.iter() {
            d |= u32::from(byte & 0x7f) << shift;
            if byte & 0x80 == 0 {
                prev = prev.wrapping_add(d);
                out.push(prev);
                (d, shift) = (0, 0);
            } else {
                shift += 7;
            }
        }
        out.into_boxed_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::PackedStops;

    #[test]
    fn packed_stops_round_trip() {
        for stops in [
            vec![],
            vec![0],
            vec![2, 3, 14, 15, 300, 301, 70_000, u32::MAX >> 1 << 1 | 1],
        ] {
            assert_eq!(&*PackedStops::pack(&stops).unpack(), &stops[..]);
        }
        assert_eq!(PackedStops::pack(&[10, 20, 30]).0.len(), 3);
    }
}
