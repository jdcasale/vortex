use arrow_buffer::{BooleanBuffer, NullBuffer};
use serde::{Deserialize, Serialize};
use vortex_dtype::{DType, Nullability};
use vortex_error::{vortex_bail, VortexResult};

use crate::array::bool::BoolArray;
use crate::compute::as_contiguous::as_contiguous;
use crate::compute::scalar_at::scalar_at;
use crate::compute::slice::slice;
use crate::compute::take::take;
use crate::stats::ArrayStatistics;
use crate::{Array, ArrayData, IntoArray, IntoArrayData, ToArray, ToArrayData};

pub trait ArrayValidity {
    fn is_valid(&self, index: usize) -> bool;
    fn logical_validity(&self) -> LogicalValidity;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ValidityMetadata {
    NonNullable,
    AllValid,
    AllInvalid,
    Array,
}

impl ValidityMetadata {
    pub fn to_validity(&self, array: Option<Array>) -> Validity {
        match self {
            Self::NonNullable => Validity::NonNullable,
            Self::AllValid => Validity::AllValid,
            Self::AllInvalid => Validity::AllInvalid,
            Self::Array => match array {
                None => panic!("Missing validity array"),
                Some(a) => Validity::Array(a),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub enum Validity {
    NonNullable,
    AllValid,
    AllInvalid,
    Array(Array),
}

impl Validity {
    pub const DTYPE: DType = DType::Bool(Nullability::NonNullable);

    pub fn into_array_data(self) -> Option<ArrayData> {
        match self {
            Self::Array(a) => Some(a.into_array_data()),
            _ => None,
        }
    }

    pub fn to_metadata(&self, length: usize) -> VortexResult<ValidityMetadata> {
        match self {
            Self::NonNullable => Ok(ValidityMetadata::NonNullable),
            Self::AllValid => Ok(ValidityMetadata::AllValid),
            Self::AllInvalid => Ok(ValidityMetadata::AllInvalid),
            Self::Array(a) => {
                // We force the caller to validate the length here.
                let validity_len = a.with_dyn(|a| a.len());
                if validity_len != length {
                    vortex_bail!(
                        "Validity array length {} doesn't match array length {}",
                        validity_len,
                        length
                    )
                }
                Ok(ValidityMetadata::Array)
            }
        }
    }

    pub fn array(&self) -> Option<&Array> {
        match self {
            Self::Array(a) => Some(a),
            _ => None,
        }
    }

    pub fn nullability(&self) -> Nullability {
        match self {
            Self::NonNullable => Nullability::NonNullable,
            _ => Nullability::Nullable,
        }
    }

    pub fn is_valid(&self, index: usize) -> bool {
        match self {
            Self::NonNullable | Self::AllValid => true,
            Self::AllInvalid => false,
            Self::Array(a) => bool::try_from(&scalar_at(a, index).unwrap()).unwrap(),
        }
    }

    pub fn slice(&self, start: usize, stop: usize) -> VortexResult<Self> {
        match self {
            Self::Array(a) => Ok(Self::Array(slice(a, start, stop)?)),
            _ => Ok(self.clone()),
        }
    }

    pub fn take(&self, indices: &Array) -> VortexResult<Self> {
        match self {
            Self::NonNullable => Ok(Self::NonNullable),
            Self::AllValid => Ok(Self::AllValid),
            Self::AllInvalid => Ok(Self::AllInvalid),
            Self::Array(a) => Ok(Self::Array(take(a, indices)?)),
        }
    }

    pub fn to_logical(&self, length: usize) -> LogicalValidity {
        match self {
            Self::NonNullable => LogicalValidity::AllValid(length),
            Self::AllValid => LogicalValidity::AllValid(length),
            Self::AllInvalid => LogicalValidity::AllInvalid(length),
            Self::Array(a) => {
                // Logical validity should map into AllValid/AllInvalid where possible.
                if a.statistics().compute_min::<bool>().unwrap_or(false) {
                    LogicalValidity::AllValid(length)
                } else if a
                    .statistics()
                    .compute_max::<bool>()
                    .map(|m| !m)
                    .unwrap_or(false)
                {
                    LogicalValidity::AllInvalid(length)
                } else {
                    LogicalValidity::Array(a.to_array_data())
                }
            }
        }
    }
}

impl PartialEq for Validity {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NonNullable, Self::NonNullable) => true,
            (Self::AllValid, Self::AllValid) => true,
            (Self::AllInvalid, Self::AllInvalid) => true,
            (Self::Array(a), Self::Array(b)) => {
                a.clone().flatten_bool().unwrap().boolean_buffer()
                    == b.clone().flatten_bool().unwrap().boolean_buffer()
            }
            _ => false,
        }
    }
}

impl From<Vec<bool>> for Validity {
    fn from(bools: Vec<bool>) -> Self {
        if bools.iter().all(|b| *b) {
            Self::AllValid
        } else if !bools.iter().any(|b| *b) {
            Self::AllInvalid
        } else {
            Self::Array(BoolArray::from_vec(bools, Self::NonNullable).into_array())
        }
    }
}

impl From<BooleanBuffer> for Validity {
    fn from(value: BooleanBuffer) -> Self {
        if value.count_set_bits() == value.len() {
            Self::AllValid
        } else if value.count_set_bits() == 0 {
            Self::AllInvalid
        } else {
            Self::Array(BoolArray::from(value).into_array())
        }
    }
}

impl From<NullBuffer> for Validity {
    fn from(value: NullBuffer) -> Self {
        value.into_inner().into()
    }
}

impl FromIterator<LogicalValidity> for Validity {
    fn from_iter<T: IntoIterator<Item = LogicalValidity>>(iter: T) -> Self {
        let validities: Vec<LogicalValidity> = iter.into_iter().collect();

        // If they're all valid, then return a single validity.
        if validities.iter().all(|v| v.all_valid()) {
            return Self::AllValid;
        }
        // If they're all invalid, then return a single invalidity.
        if validities.iter().all(|v| v.all_invalid()) {
            return Self::AllInvalid;
        }

        // Otherwise, map each to a bool array and concatenate them.
        let arrays = validities
            .iter()
            .map(|v| {
                v.to_present_null_buffer()
                    .unwrap()
                    .into_array_data()
                    .into_array()
            })
            .collect::<Vec<_>>();
        Self::Array(as_contiguous(&arrays).unwrap())
    }
}

impl<'a, E> FromIterator<&'a Option<E>> for Validity {
    fn from_iter<T: IntoIterator<Item = &'a Option<E>>>(iter: T) -> Self {
        let bools: Vec<bool> = iter.into_iter().map(|option| option.is_some()).collect();
        Self::from(bools)
    }
}

#[derive(Clone, Debug)]
pub enum LogicalValidity {
    AllValid(usize),
    AllInvalid(usize),
    Array(ArrayData),
}

impl LogicalValidity {
    pub fn to_null_buffer(&self) -> VortexResult<Option<NullBuffer>> {
        match self {
            Self::AllValid(_) => Ok(None),
            Self::AllInvalid(l) => Ok(Some(NullBuffer::new_null(*l))),
            Self::Array(a) => Ok(Some(NullBuffer::new(
                a.to_array().flatten_bool()?.boolean_buffer(),
            ))),
        }
    }

    pub fn to_present_null_buffer(&self) -> VortexResult<NullBuffer> {
        match self {
            Self::AllValid(l) => Ok(NullBuffer::new_valid(*l)),
            Self::AllInvalid(l) => Ok(NullBuffer::new_null(*l)),
            Self::Array(a) => Ok(NullBuffer::new(
                a.to_array().flatten_bool()?.boolean_buffer(),
            )),
        }
    }

    pub fn all_valid(&self) -> bool {
        matches!(self, Self::AllValid(_))
    }

    pub fn all_invalid(&self) -> bool {
        matches!(self, Self::AllInvalid(_))
    }

    pub fn len(&self) -> usize {
        match self {
            Self::AllValid(n) => *n,
            Self::AllInvalid(n) => *n,
            Self::Array(a) => a.to_array().len(),
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            Self::AllValid(n) => *n == 0,
            Self::AllInvalid(n) => *n == 0,
            Self::Array(a) => a.to_array().is_empty(),
        }
    }

    pub fn into_validity(self) -> Validity {
        match self {
            Self::AllValid(_) => Validity::AllValid,
            Self::AllInvalid(_) => Validity::AllInvalid,
            Self::Array(a) => Validity::Array(a.into_array()),
        }
    }
}

impl IntoArray for LogicalValidity {
    fn into_array(self) -> Array {
        match self {
            Self::AllValid(len) => BoolArray::from(vec![true; len]).into_array(),
            Self::AllInvalid(len) => BoolArray::from(vec![false; len]).into_array(),
            Self::Array(a) => a.into_array(),
        }
    }
}
