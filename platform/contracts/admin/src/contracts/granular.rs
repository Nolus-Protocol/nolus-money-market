use std::marker::PhantomData;

use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use super::higher_order_type::FirstOrderType;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
#[schemars(bound = "StructuralTypeConstructor: JsonSchema, \
    StructuralTypeConstructor::Of<GranularUnitWrapperTypeConstructor::Of<Unit>>: JsonSchema, \
    GranularUnitWrapperTypeConstructor: JsonSchema, \
    GranularUnitWrapperTypeConstructor::Of<StructuralTypeConstructor::Of<Unit>>: JsonSchema, \
    Unit: JsonSchema")]
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

impl<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor, Unit>
    FirstOrderType<HigherOrderType<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor>>
    for Granularity<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor, Unit>
where
    StructuralTypeConstructor: super::HigherOrderType,
    GranularUnitWrapperTypeConstructor: super::HigherOrderType,
{
    type Unit = Unit;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
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
    type Of<Unit> =
        Granularity<StructuralTypeConstructor, GranularUnitWrapperTypeConstructor, Unit>;
}
