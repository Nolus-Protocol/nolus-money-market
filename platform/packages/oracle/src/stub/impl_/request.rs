use serde::Serialize;

use currency::{CurrencyDef, Group};

use crate::msg::{BaseCurrencyQueryMsg, StableCurrencyQueryMsg};

pub trait RequestBuilder {
    fn currency<G>() -> impl Serialize
    where
        G: Group;

    fn price<C>() -> impl Serialize
    where
        C: CurrencyDef;
}

pub struct BasePriceRequest {}
impl RequestBuilder for BasePriceRequest {
    fn currency<G>() -> impl Serialize
    where
        G: Group,
    {
        BaseCurrencyQueryMsg::<G>::BaseCurrency {}
    }

    fn price<C>() -> impl Serialize
    where
        C: CurrencyDef,
    {
        BaseCurrencyQueryMsg::BasePrice {
            currency: *C::dto(),
        }
    }
}

pub struct StablePriceRequest {}
impl RequestBuilder for StablePriceRequest {
    fn currency<G>() -> impl Serialize
    where
        G: Group,
    {
        StableCurrencyQueryMsg::<G>::StableCurrency {}
    }

    fn price<C>() -> impl Serialize
    where
        C: CurrencyDef,
    {
        StableCurrencyQueryMsg::StablePrice {
            currency: *C::dto(),
        }
    }
}
