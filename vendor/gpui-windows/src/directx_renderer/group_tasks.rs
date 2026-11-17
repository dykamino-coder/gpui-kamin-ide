//! Finished Gaussian tasks, reused only after an exact GPU comparison of the input.
//!
//! A predicate sees the complete raw rectangle, including the kernel halo. It
//! discards equal RGBA8 texels; any surviving sample selects the original blur.
//! No CPU readback, normalized-coordinate equality, or hash proves a cache hit.

use super::*;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct Key([u32; 7]);

fn key(g: &PaintGroup, width: u32, height: u32) -> Option<Key> {
    let radius = gaussian::direct_radius(g.blur_radius)?;
    if radius > gaussian::MAX_SINGLE_PASS_RADIUS || g.projection.is_some() {
        return None;
    }
    let [raw, out] = Region::for_group(g, width, height);
    // Four entries are at most 2 MiB, independently of document length.
    (u64::from(raw.width) * u64::from(raw.height) <= 128 * 1024).then_some(Key([
        g.blur_radius.to_bits(), raw.width, raw.height, out.width, out.height,
        out.x - raw.x, out.y - raw.y,
    ]))
}

struct Entry {
    key: Key,
    input: Option<BlurTexture>,
    output: Option<group_atlas::Place>,
}

#[derive(Default)]
pub(super) struct Tasks {
    entries: Vec<Entry>,
    predicate: Option<ID3D11Predicate>,
    no_write: Option<ID3D11BlendState>,
    #[cfg(test)]
    pub(super) disabled: bool,
    #[cfg(test)]
    pub(super) probes: usize,
}

impl Tasks {
    pub(super) fn inputs(&self) -> impl Iterator<Item = &ID3D11Texture2D> {
        self.entries.iter().filter_map(|e| e.input.as_ref().map(|t| &t.texture))
    }

    #[cfg(test)]
    pub(super) fn last_difference(&self, dc: &ID3D11DeviceContext) -> bool {
        // The caller has mapped the final target, so this predicate is complete.
        let mut different = 0u32;
        let predicate = self.predicate.as_ref().unwrap();
        let hr = unsafe {
            (Interface::vtable(dc).GetData)(Interface::as_raw(dc), Interface::as_raw(predicate),
                &mut different as *mut u32 as *mut _, 4, D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32)
        };
        assert_eq!(hr, windows::Win32::Foundation::S_OK);
        different != 0
    }

    fn resources(&mut self, device: &ID3D11Device) -> Result<()> {
        if self.predicate.is_none() {
            // A hint may execute a skipped copy and would be incorrect here.
            let desc = D3D11_QUERY_DESC { Query: D3D11_QUERY_OCCLUSION_PREDICATE, MiscFlags: 0 };
            unsafe { device.CreatePredicate(&desc, Some(&mut self.predicate))? };
        }
        if self.no_write.is_none() {
            let desc = D3D11_BLEND_DESC::default();
            unsafe { device.CreateBlendState(&desc, Some(&mut self.no_write))? };
        }
        Ok(())
    }
}

impl DirectXRenderer {
    pub(super) fn begin_blur_tasks(&mut self, scene: &Scene, reuse: &[bool], needed: Option<&[bool]>) {
        let mut counts = HashMap::<Key, usize>::new();
        let enabled = retained::enabled();
        #[cfg(test)]
        let enabled = enabled && !self.blur.tasks.disabled;
        if enabled {
            for (i, g) in scene.groups.iter().enumerate() {
                if !reuse[i] && needed.is_none_or(|n| n.get(i).copied().unwrap_or(true)) {
                    if let Some(k) = key(g, self.width, self.height) {
                        *counts.entry(k).or_default() += 1;
                    }
                }
            }
        }
        let mut common: Vec<_> = counts.into_iter().filter(|(_, n)| *n >= 8).collect();
        common.sort_unstable_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.0.cmp(&b.0.0)));
        common.truncate(4);
        let entries = &mut self.blur.tasks.entries;
        entries.retain(|e| common.iter().any(|(k, _)| *k == e.key));
        for e in entries.iter_mut() {
            e.output = None;
        }
        for (k, _) in common {
            if !entries.iter().any(|e| e.key == k) {
                entries.push(Entry { key: k, input: None, output: None });
            }
        }
    }

    /// Return true once this group has either copied a proven equal result or
    /// executed its original kernel. The atlas owns the finished output.
    pub(super) fn cached_blur_task(&mut self, i: usize, g: &PaintGroup, raw: Region) -> Result<bool> {
        let Some(k) = key(g, self.width, self.height) else { return Ok(false) };
        let Some(slot) = self.blur.tasks.entries.iter().position(|e| e.key == k) else { return Ok(false) };
        let device = self.dev().device.clone();
        let dc = self.dev().device_context.clone();
        self.blur.tasks.resources(&device)?;
        let r = gaussian::direct_radius(g.blur_radius).unwrap();
        let place = self.blur.group_place[i];
        let region = self.blur.group_regions[i];
        let Some(source) = self.blur.tasks.entries[slot].output else {
            self.gaussian_group(i, g.blur_radius, r, raw)?;
            let entry = &mut self.blur.tasks.entries[slot];
            if entry.input.is_none() {
                entry.input = Some(create_blur_texture(&device, raw.width, raw.height, false)?);
            }
            let area = D3D11_BOX { left: raw.x, top: raw.y, front: 0,
                right: raw.x + raw.width, bottom: raw.y + raw.height, back: 1 };
            unsafe {
                dc.PSSetShaderResources(0, Some(&[None]));
                dc.CopySubresourceRegion(&entry.input.as_ref().unwrap().texture, 0, 0, 0, 0,
                    &self.blur.stage.as_ref().unwrap().texture, 0, Some(&area));
            }
            entry.output = Some(place);
            return Ok(true);
        };
        // Every raw texel must be rasterized, including the halo. A page
        // smaller than the comparison viewport cannot prove equality.
        if self.blur.groups[place.page].width < raw.width
            || self.blur.groups[place.page].height < raw.height { return Ok(false); }
        let input = self.blur.tasks.entries[slot].input.as_ref().unwrap().srv.clone();
        let stage = self.blur.stage.as_ref().unwrap().srv.clone();
        let predicate = self.blur.tasks.predicate.as_ref().unwrap().clone();
        let compare = gaussian::kernel_quad(
            [0., 0., raw.width as f32, raw.height as f32], [raw.x as f32, raw.y as f32],
            [0.; 4], 7., [0.; 2], 0, [[0.; 4]; 4],
        );
        let target = self.blur.groups[place.page].rtv.clone();
        unsafe {
            dc.PSSetShaderResources(3, Some(&input));
            dc.Begin(&predicate);
        }
        let compared = self.kernel_instances(&stage, &target, (raw.width, raw.height), &[compare]);
        unsafe {
            dc.End(&predicate);
            dc.PSSetShaderResources(3, Some(&[None]));
        }
        compared?;
        #[cfg(test)]
        { self.blur.tasks.probes += 1; }
        let area = D3D11_BOX { left: source.x, top: source.y, front: 0,
            right: source.x + region.width, bottom: source.y + region.height, back: 1 };
        unsafe {
            dc.OMSetRenderTargets(None, None);
            dc.PSSetShaderResources(0, Some(&[None]));
            // SetPredication skips the operation when result == PredicateValue.
            dc.SetPredication(&predicate, true);
            dc.CopySubresourceRegion(&self.blur.groups[place.page].texture, 0, place.x, place.y, 0,
                &self.blur.groups[source.page].texture, 0, Some(&area));
            dc.SetPredication(&predicate, false);
        }
        let blurred = self.gaussian_group(i, g.blur_radius, r, raw);
        // Restore even on an upload/draw error; the next pass is unconditional.
        unsafe { dc.SetPredication(None, false) };
        blurred?;
        Ok(true)
    }

    pub(super) fn blur_task_blend(&mut self, quads: &[BlurQuad]) -> Result<ID3D11BlendState> {
        if quads.first().is_some_and(|q| q.blur_pass == 7.0) {
            Ok(self.blur.tasks.no_write.as_ref().unwrap().clone())
        } else {
            self.replace_blend()
        }
    }
}
