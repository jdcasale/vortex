use vortex_dtype::DType;
use vortex_error::{vortex_err, VortexResult};

use crate::{Array, ArrayDType};

pub trait CastFn {
    fn cast(&self, dtype: &DType) -> VortexResult<Array>;
}

pub fn cast(array: &Array, dtype: &DType) -> VortexResult<Array> {
    if array.dtype() == dtype {
        return Ok(array.clone());
    }

    // TODO(ngates): check for null_count if dtype is non-nullable
    array.with_dyn(|a| {
        a.cast()
            .map(|f| f.cast(dtype))
            .unwrap_or_else(|| Err(vortex_err!(NotImplemented: "cast", array.encoding().id())))
    })
}
