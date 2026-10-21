//! Константы OpenType MATH, проверяемые один раз при декодировании веб-шрифта.

#[derive(Clone, Debug)]
pub(crate) struct Constants {
    pub script_scale: f32,
    pub script_script_scale: f32,
    pub values: [f32; 51],
    pub degree_raise: f32,
}

fn u16_at(bytes: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(
        bytes.get(at..at.checked_add(2)?)?.try_into().ok()?,
    ))
}
fn u32_at(bytes: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(
        bytes.get(at..at.checked_add(4)?)?.try_into().ok()?,
    ))
}
fn table<'a>(bytes: &'a [u8], tag: &[u8; 4]) -> Option<&'a [u8]> {
    for i in 0..usize::from(u16_at(bytes, 4)?) {
        let at = 12 + 16 * i;
        if bytes.get(at..at + 4)? == tag {
            let offset = usize::try_from(u32_at(bytes, at + 8)?).ok()?;
            let length = usize::try_from(u32_at(bytes, at + 12)?).ok()?;
            return bytes.get(offset..offset.checked_add(length)?);
        }
    }
    None
}
impl Constants {
    /// MathValueRecords carry a signed design-unit value and a device offset.
    /// Layout uses the design value; device tables affect hinted raster sizes.
    pub(crate) fn parse(bytes: &[u8]) -> Option<Self> {
        let upem = f32::from(u16_at(table(bytes, b"head")?, 18)?);
        if upem == 0.0 {
            return None;
        }
        let math = table(bytes, b"MATH")?;
        if u16_at(math, 0)? != 1 {
            return None;
        }
        let at = usize::from(u16_at(math, 4)?);
        if at == 0 {
            return None;
        }
        let constants = math.get(at..)?;
        let mut values = [0.0; 51];
        for (i, value) in values.iter_mut().enumerate() {
            *value = f32::from(u16_at(constants, 8 + 4 * i)? as i16) / upem;
        }
        Some(Self {
            script_scale: f32::from(u16_at(constants, 0)? as i16) / 100.0,
            script_script_scale: f32::from(u16_at(constants, 2)? as i16) / 100.0,
            values,
            degree_raise: f32::from(u16_at(constants, 212)?) / 100.0,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn font() -> Vec<u8> {
        let mut bytes = vec![0; 300];
        bytes[4..6].copy_from_slice(&2u16.to_be_bytes());
        for (at, tag, offset, length) in [(12, b"head", 44u32, 20u32), (28, b"MATH", 64, 224)] {
            bytes[at..at + 4].copy_from_slice(tag);
            bytes[at + 8..at + 12].copy_from_slice(&offset.to_be_bytes());
            bytes[at + 12..at + 16].copy_from_slice(&length.to_be_bytes());
        }
        bytes[62..64].copy_from_slice(&1000u16.to_be_bytes());
        bytes[64..66].copy_from_slice(&1u16.to_be_bytes());
        bytes[68..70].copy_from_slice(&10u16.to_be_bytes());
        bytes[74..76].copy_from_slice(&70u16.to_be_bytes());
        bytes[76..78].copy_from_slice(&50u16.to_be_bytes());
        bytes[82..84].copy_from_slice(&(-125i16).to_be_bytes());
        bytes[286..288].copy_from_slice(&60u16.to_be_bytes());
        bytes
    }
    #[test]
    fn design_values_and_percentages() {
        let c = Constants::parse(&font()).unwrap();
        assert_eq!((c.script_scale, c.script_script_scale), (0.7, 0.5));
        assert_eq!(c.values[0], -0.125);
        assert_eq!(c.degree_raise, 0.6);
    }
    #[test]
    fn malformed_offsets_never_escape_table() {
        let mut bytes = font();
        for end in 0..288 {
            assert!(Constants::parse(&bytes[..end]).is_none());
        }
        bytes[68..70].copy_from_slice(&u16::MAX.to_be_bytes());
        assert!(Constants::parse(&bytes).is_none());
    }
}
