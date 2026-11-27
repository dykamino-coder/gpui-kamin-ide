//! Native surface selection for comparison of offscreen and presented paths.

/// Offscreen textures are invisible to DWM; window is the legacy parked surface.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Surface {
    Offscreen,
    Window,
}

#[cfg(windows)]
impl Surface {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Offscreen => "offscreen",
            Self::Window => "window",
        }
    }
}
