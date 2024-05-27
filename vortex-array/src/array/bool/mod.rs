use std::ops::{BitAnd, BitOr, BitXor, Not};

use arrow_buffer::BooleanBuffer;
use itertools::Itertools;
use serde::{Deserialize, Serialize};
use vortex_buffer::Buffer;
use vortex_expr::operators::Operator;

use crate::validity::{ArrayValidity, ValidityMetadata};
use crate::validity::{LogicalValidity, Validity};
use crate::visitor::{AcceptArrayVisitor, ArrayVisitor};
use crate::{impl_encoding, ArrayFlatten};

mod compute;
mod stats;

impl_encoding!("vortex.bool", Bool);

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BoolMetadata {
    validity: ValidityMetadata,
    length: usize,
}

impl BoolArray {
    pub fn buffer(&self) -> &Buffer {
        self.array().buffer().expect("missing buffer")
    }

    pub fn boolean_buffer(&self) -> BooleanBuffer {
        BooleanBuffer::new(self.buffer().clone().into(), 0, self.len())
    }

    pub fn validity(&self) -> Validity {
        self.metadata()
            .validity
            .to_validity(self.array().child(0, &Validity::DTYPE))
    }
}

impl BoolArray {
    pub fn try_new(buffer: BooleanBuffer, validity: Validity) -> VortexResult<Self> {
        Ok(Self {
            typed: TypedArray::try_from_parts(
                DType::Bool(validity.nullability()),
                BoolMetadata {
                    validity: validity.to_metadata(buffer.len())?,
                    length: buffer.len(),
                },
                Some(Buffer::from(buffer.into_inner())),
                validity.into_array_data().into_iter().collect_vec().into(),
                StatsSet::new(),
            )?,
        })
    }

    pub fn from_vec(bools: Vec<bool>, validity: Validity) -> Self {
        let buffer = BooleanBuffer::from(bools);
        Self::try_new(buffer, validity).unwrap()
    }
}

pub fn apply_comparison_op(lhs: BooleanBuffer, rhs: BooleanBuffer, op: Operator) -> BooleanBuffer {
    match op {
        Operator::EqualTo => lhs.bitxor(&rhs).not(),
        Operator::NotEqualTo => lhs.bitxor(&rhs),
        Operator::GreaterThan => lhs.bitand(&rhs.not()),
        Operator::GreaterThanOrEqualTo => lhs.bitor(&rhs.not()),
        Operator::LessThan => lhs.not().bitand(&rhs),
        Operator::LessThanOrEqualTo => lhs.not().bitor(&rhs),
    }
}

impl From<BooleanBuffer> for BoolArray {
    fn from(value: BooleanBuffer) -> Self {
        Self::try_new(value, Validity::NonNullable).unwrap()
    }
}

impl From<Vec<bool>> for BoolArray {
    fn from(value: Vec<bool>) -> Self {
        Self::from_vec(value, Validity::NonNullable)
    }
}

impl FromIterator<Option<bool>> for BoolArray {
    fn from_iter<I: IntoIterator<Item = Option<bool>>>(iter: I) -> Self {
        let iter = iter.into_iter();
        let (lower, _) = iter.size_hint();

        let mut validity: Vec<bool> = Vec::with_capacity(lower);
        let values: Vec<bool> = iter
            .map(|i| {
                validity.push(i.is_some());
                i.unwrap_or_default()
            })
            .collect::<Vec<_>>();

        Self::try_new(BooleanBuffer::from(values), Validity::from(validity)).unwrap()
    }
}

impl ArrayTrait for BoolArray {
    fn len(&self) -> usize {
        self.metadata().length
    }
}

impl ArrayFlatten for BoolArray {
    fn flatten(self) -> VortexResult<Flattened> {
        Ok(Flattened::Bool(self))
    }
}

impl ArrayValidity for BoolArray {
    fn is_valid(&self, index: usize) -> bool {
        self.validity().is_valid(index)
    }

    fn logical_validity(&self) -> LogicalValidity {
        self.validity().to_logical(self.len())
    }
}

impl AcceptArrayVisitor for BoolArray {
    fn accept(&self, visitor: &mut dyn ArrayVisitor) -> VortexResult<()> {
        visitor.visit_buffer(self.buffer())?;
        visitor.visit_validity(&self.validity())
    }
}

impl EncodingCompression for BoolEncoding {}

#[cfg(test)]
mod tests {
    use crate::array::bool::BoolArray;
    use crate::compute::scalar_at::scalar_at;
    use crate::IntoArray;

    #[test]
    fn bool_array() {
        let arr = BoolArray::from(vec![true, false, true]).into_array();
        let scalar = bool::try_from(&scalar_at(&arr, 0).unwrap()).unwrap();
        assert!(scalar);
    }
}
