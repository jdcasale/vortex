use std::fmt::{Debug, Display, Formatter};
use std::hash::Hash;
use std::sync::Arc;

use itertools::Itertools;
use DType::*;

use crate::nullability::Nullability;
use crate::{ExtDType, PType};

pub type FieldName = Arc<str>;
pub type FieldNames = Arc<[FieldName]>;

pub type Metadata = Vec<u8>;

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub enum DType {
    Null,
    Bool(Nullability),
    Primitive(PType, Nullability),
    Utf8(Nullability),
    Binary(Nullability),
    Struct(StructDType, Nullability),
    List(Arc<DType>, Nullability),
    Extension(ExtDType, Nullability),
}

impl DType {
    pub const BYTES: Self = Primitive(PType::U8, Nullability::NonNullable);

    /// The default DType for indices
    pub const IDX: Self = Primitive(PType::U64, Nullability::NonNullable);

    pub fn nullability(&self) -> Nullability {
        self.is_nullable().into()
    }

    pub fn is_nullable(&self) -> bool {
        use crate::nullability::Nullability::*;

        match self {
            Null => true,
            Bool(n) => matches!(n, Nullable),
            Primitive(_, n) => matches!(n, Nullable),
            Utf8(n) => matches!(n, Nullable),
            Binary(n) => matches!(n, Nullable),
            Struct(st, _) => st.dtypes().iter().all(|f| f.is_nullable()),
            List(_, n) => matches!(n, Nullable),
            Extension(_, n) => matches!(n, Nullable),
        }
    }

    pub fn as_nonnullable(&self) -> Self {
        self.with_nullability(Nullability::NonNullable)
    }

    pub fn as_nullable(&self) -> Self {
        self.with_nullability(Nullability::Nullable)
    }

    pub fn with_nullability(&self, nullability: Nullability) -> Self {
        match self {
            Null => Null,
            Bool(_) => Bool(nullability),
            Primitive(p, _) => Primitive(*p, nullability),
            Utf8(_) => Utf8(nullability),
            Binary(_) => Binary(nullability),
            Struct(st, _) => Struct(st.clone(), nullability),
            List(c, _) => List(c.clone(), nullability),
            Extension(ext, _) => Extension(ext.clone(), nullability),
        }
    }

    pub fn eq_ignore_nullability(&self, other: &Self) -> bool {
        self.as_nullable().eq(&other.as_nullable())
    }
}

impl Display for DType {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Null => write!(f, "null"),
            Bool(n) => write!(f, "bool{}", n),
            Primitive(p, n) => write!(f, "{}{}", p, n),
            Utf8(n) => write!(f, "utf8{}", n),
            Binary(n) => write!(f, "binary{}", n),
            Struct(st, n) => write!(
                f,
                "{{{}}}{}",
                st.names()
                    .iter()
                    .zip(st.dtypes().iter())
                    .map(|(n, dt)| format!("{}={}", n, dt))
                    .join(", "),
                n
            ),
            List(c, n) => write!(f, "list({}){}", c, n),
            Extension(ext, n) => write!(
                f,
                "ext({}{}){}",
                ext.id(),
                ext.metadata()
                    .map(|m| format!(", {:?}", m))
                    .unwrap_or_else(|| "".to_string()),
                n
            ),
        }
    }
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
pub struct StructDType {
    names: FieldNames,
    dtypes: Arc<[DType]>,
}

impl StructDType {
    pub fn new(names: FieldNames, dtypes: Vec<DType>) -> Self {
        Self {
            names,
            dtypes: dtypes.into(),
        }
    }

    pub fn names(&self) -> &FieldNames {
        &self.names
    }

    pub fn find_name(&self, name: &str) -> Option<usize> {
        self.names.iter().position(|n| n.as_ref() == name)
    }

    pub fn dtypes(&self) -> &Arc<[DType]> {
        &self.dtypes
    }
}

#[cfg(test)]
mod test {
    use std::mem;

    use crate::dtype::DType;

    #[test]
    fn size_of() {
        assert_eq!(mem::size_of::<DType>(), 40);
    }
}
