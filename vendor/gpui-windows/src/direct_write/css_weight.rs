//! KaminIDE patch: CSS font-weight matching for HTML document text.
//!
//! `IDWriteFontSet::GetMatchingFonts` orders faces by DirectWrite's own
//! distance, which differs from css-fonts-4 §5.2 step 4 when the requested
//! weight is missing from the family: for `font-weight: 501` over 400/700
//! DirectWrite picks 400, CSS requires 700 ("if the desired weight is greater
//! than 500, weights greater than or equal to the desired weight are checked
//! in ascending order"). The HTML engine opts in with the private feature tag
//! `cssw`; IDE fonts never carry it and keep DirectWrite's choice.
//!
//! Bold synthesis follows Blink (`css_segmented_font_face.cc`): synthesize
//! when the request is >= 600 and the picked face is lighter than 600.

use windows::Win32::Graphics::DirectWrite::*;

/// The face to create first and the simulations to create it with.
pub(super) struct CssPick {
    pub face: IDWriteFontFaceReference,
    pub simulations: DWRITE_FONT_SIMULATIONS,
    /// A collection holding only the picked face: the text layout matches
    /// fonts by itself (family + weight), and over the whole family it would
    /// redo DirectWrite's choice (600 over 100/500: simulated-bold 100).
    pub collection: IDWriteFontCollection1,
    /// Weight to ask the layout for: the face's own, or the heaviest one when
    /// bold is synthesized (the layout then simulates bold on its only face).
    pub layout_weight: DWRITE_FONT_WEIGHT,
}

const BOLD_THRESHOLD: f32 = 600.0;

/// Re-pick the face by the CSS weight rule among ALL real faces of `family`
/// in `collection` (the matching set alone is already narrowed by
/// DirectWrite: for 600 over 100/500 it holds only a simulated-bold 100).
/// Stretch and slope (§5.2 steps 2-3) are those of `best`, DirectWrite's first
/// match. `None` keeps DirectWrite's order.
pub(super) fn css_weight_pick(
    factory: &IDWriteFactory5,
    collection: &IDWriteFontCollection1,
    family: &windows::core::HSTRING,
    best: &IDWriteFontSet,
    want: f32,
    allow_bold: bool,
) -> Option<CssPick> {
    let first = unsafe { best.GetFontFaceReference(0).ok()?.CreateFontFace().ok()? };
    // A simulated oblique stands on an upright real face.
    let oblique = unsafe { first.GetSimulations() }.0 & DWRITE_FONT_SIMULATIONS_OBLIQUE.0;
    let style = if oblique != 0 {
        DWRITE_FONT_STYLE_NORMAL
    } else {
        unsafe { first.GetStyle() }
    };
    let stretch = unsafe { first.GetStretch() };
    let mut index = 0u32;
    let mut exists = windows::core::BOOL(0);
    unsafe {
        collection
            .FindFamilyName(family, &mut index, &mut exists)
            .ok()?
    };
    if !exists.as_bool() {
        return None;
    }
    let fam = unsafe { collection.GetFontFamily(index).ok()? };
    let mut real = Vec::new();
    for i in 0..unsafe { fam.GetFontCount() } {
        let Ok(face_ref) = (unsafe { fam.GetFontFaceReference(i) }) else {
            continue;
        };
        let Ok(face) = (unsafe { face_ref.CreateFontFace() }) else {
            continue;
        };
        let real_face = unsafe { face.GetSimulations() } == DWRITE_FONT_SIMULATIONS_NONE;
        if real_face
            && unsafe { face.GetStyle() } == style
            && unsafe { face.GetStretch() } == stretch
        {
            real.push((unsafe { face.GetWeight() }.0 as f32, face_ref));
        }
    }
    let weights: Vec<f32> = real.iter().map(|f| f.0).collect();
    let weight = pick_weight(&weights, want)?;
    let (_, face) = real.into_iter().find(|f| f.0 == weight)?;
    let mut sims = oblique;
    if allow_bold && want >= BOLD_THRESHOLD && weight < BOLD_THRESHOLD {
        sims |= DWRITE_FONT_SIMULATIONS_BOLD.0;
    }
    // The same face and simulations DirectWrite already picked: keep its path
    // (and its collection) untouched — runs of one face still fold into one
    // shaping range across style boundaries.
    let same = unsafe {
        let r0 = best.GetFontFaceReference(0).ok()?;
        let real0 = r0
            .CreateFontFaceWithSimulations(DWRITE_FONT_SIMULATIONS_NONE)
            .ok()?;
        r0.GetSimulations().0 == sims
            && real0.GetWeight().0 as f32 == weight
            && real0.GetStyle() == style
            && real0.GetStretch() == stretch
    };
    if same {
        return None;
    }
    let only = unsafe {
        let builder = factory.CreateFontSetBuilder().ok()?;
        builder.AddFontFaceReference2(&face).ok()?;
        let set = builder.CreateFontSet().ok()?;
        factory.CreateFontCollectionFromFontSet(&set).ok()?
    };
    let layout_weight = if sims & DWRITE_FONT_SIMULATIONS_BOLD.0 != 0 {
        DWRITE_FONT_WEIGHT_ULTRA_BLACK
    } else {
        DWRITE_FONT_WEIGHT(weight as i32)
    };
    Some(CssPick {
        face,
        simulations: DWRITE_FONT_SIMULATIONS(sims),
        collection: only,
        layout_weight,
    })
}

/// css-fonts-4 §5.2 step 4 over the available weights.
fn pick_weight(weights: &[f32], want: f32) -> Option<f32> {
    if weights.contains(&want) {
        return Some(want);
    }
    let below = || {
        weights
            .iter()
            .copied()
            .filter(|w| *w < want)
            .max_by(f32::total_cmp)
    };
    let above = |limit: f32| {
        weights
            .iter()
            .copied()
            .filter(|w| *w > want && *w <= limit)
            .min_by(f32::total_cmp)
    };
    if (400.0..=500.0).contains(&want) {
        above(500.0).or_else(below).or_else(|| above(f32::MAX))
    } else if want < 400.0 {
        below().or_else(|| above(f32::MAX))
    } else {
        above(f32::MAX).or_else(below)
    }
}

#[cfg(test)]
mod tests {
    use super::pick_weight;

    #[test]
    fn css_weight_order() {
        assert_eq!(pick_weight(&[400.0, 700.0], 501.0), Some(700.0));
        assert_eq!(pick_weight(&[400.0, 700.0], 500.0), Some(400.0));
        assert_eq!(pick_weight(&[100.0, 500.0], 400.0), Some(500.0));
        assert_eq!(pick_weight(&[100.0, 500.0], 900.0), Some(500.0));
        assert_eq!(pick_weight(&[200.0, 400.0], 300.0), Some(200.0));
        assert_eq!(pick_weight(&[200.0, 600.0, 900.0], 700.0), Some(900.0));
        assert_eq!(pick_weight(&[], 400.0), None);
    }
}
