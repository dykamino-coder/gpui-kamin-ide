//! Draw already transformed content without applying the active transform twice.

use super::*;

impl Window {
    /// Paint content expressed in the final coordinate system. Existing
    /// content masks remain in that system; new masks are not remapped.
    pub fn with_absolute_transformation<R>(
        &mut self,
        transformation: TransformationMatrix,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        self.with_transform_frame(transformation, None, f)
    }

    pub(super) fn with_transform_frame<R>(
        &mut self,
        transformation: TransformationMatrix,
        mask_map: Option<TransformationMatrix>,
        f: impl FnOnce(&mut Self) -> R,
    ) -> R {
        let depth = self.transformation_stack.len();
        self.transformation_stack.push(transformation);
        let previous_map = mem::replace(&mut self.mask_map, mask_map);
        self.with_restore((depth, previous_map), |window, previous| {
            window.transformation_stack.truncate(previous.0);
            window.mask_map = previous.1;
        }, f).0
    }
}
