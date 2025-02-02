use vortex::compute::slice::{slice, SliceFn};
use vortex::compute::take::{take, TakeFn};
use vortex::compute::ArrayCompute;
use vortex::{Array, ArrayDType, IntoArray};
use vortex_error::VortexResult;

use crate::DateTimePartsArray;

impl ArrayCompute for DateTimePartsArray {
    fn slice(&self) -> Option<&dyn SliceFn> {
        Some(self)
    }

    fn take(&self) -> Option<&dyn TakeFn> {
        Some(self)
    }
}

impl TakeFn for DateTimePartsArray {
    fn take(&self, indices: &Array) -> VortexResult<Array> {
        Ok(Self::try_new(
            self.dtype().clone(),
            take(&self.days(), indices)?,
            take(&self.seconds(), indices)?,
            take(&self.subsecond(), indices)?,
        )?
        .into_array())
    }
}

impl SliceFn for DateTimePartsArray {
    fn slice(&self, start: usize, stop: usize) -> VortexResult<Array> {
        Ok(Self::try_new(
            self.dtype().clone(),
            slice(&self.days(), start, stop)?,
            slice(&self.seconds(), start, stop)?,
            slice(&self.subsecond(), start, stop)?,
        )?
        .into_array())
    }
}
