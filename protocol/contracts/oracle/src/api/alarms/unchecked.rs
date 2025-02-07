use serde::{Deserialize, Serialize};

use currency::{CurrencyDef, Group, MemberOf};
use finance::price::base::BasePrice;

use super::{Alarm as ValidatedAlarm, Error};

#[derive(Deserialize, Serialize)]
#[serde(bound(serialize = "", deserialize = ""))]
pub(super) struct Alarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    below: BasePrice<G, Lpn, Lpns>,
    above: Option<BasePrice<G, Lpn, Lpns>>,
}

impl<G, Lpn, Lpns> TryFrom<Alarm<G, Lpn, Lpns>> for ValidatedAlarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    type Error = Error;

    fn try_from(unchecked: Alarm<G, Lpn, Lpns>) -> Result<Self, Self::Error> {
        let validated = Self {
            below: unchecked.below,
            above: unchecked.above,
        };
        validated.invariant_held().map(|()| validated)
    }
}

impl<G, Lpn, Lpns> From<ValidatedAlarm<G, Lpn, Lpns>> for Alarm<G, Lpn, Lpns>
where
    G: Group,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<Lpns> + MemberOf<G::TopG>,
    Lpns: Group,
{
    fn from(validated: ValidatedAlarm<G, Lpn, Lpns>) -> Self {
        Self {
            below: validated.below,
            above: validated.above,
        }
    }
}
