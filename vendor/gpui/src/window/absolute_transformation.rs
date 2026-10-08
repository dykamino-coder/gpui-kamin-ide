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
        self.transformation_stack.push(transformation);
        let previous_map = self.mask_map.take();
        let result = f(self);
        self.mask_map = previous_map;
        self.transformation_stack.pop();
        result
    }
}
