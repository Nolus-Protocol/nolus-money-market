use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct HigherOrderType<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor>(
    PhantomData<StructuralTypeConstructor>,
    PhantomData<GranularUnitWrapperTypeConstructor>,
)
where
    StructuralTypeConstructor: super::HigherOrderType,
    GranularUnitWrapperTypeConstructor: super::HigherOrderType;

impl<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor> super::HigherOrderType
    for HigherOrderType<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor>
where
    StructuralTypeConstructor: super::HigherOrderType,
    GranularUnitWrapperTypeConstructor: super::HigherOrderType,
{
    type Of<T> = Granularity<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor, T>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "snake_case",
    deny_unknown_fields,
    untagged,
    bound(
        serialize = "StructuralTypeConstructor::Of<GranularUnitWrapperTypeConstructor::Of<Unit>>: Serialize,\
            GranularUnitWrapperTypeConstructor::Of<StructuralTypeConstructor::Of<Unit>>: Serialize",
        deserialize = "StructuralTypeConstructor::Of<GranularUnitWrapperTypeConstructor::Of<Unit>>: Deserialize<'de>,\
            GranularUnitWrapperTypeConstructor::Of<StructuralTypeConstructor::Of<Unit>>: Deserialize<'de>",
    )
)]
pub enum Granularity<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor, Unit>
where
    StructuralTypeConstructor: super::HigherOrderType,
    GranularUnitWrapperTypeConstructor: super::HigherOrderType,
{
    Some {
        some: StructuralTypeConstructor::Of<GranularUnitWrapperTypeConstructor::Of<Unit>>,
    },
    All(GranularUnitWrapperTypeConstructor::Of<StructuralTypeConstructor::Of<Unit>>),
}
