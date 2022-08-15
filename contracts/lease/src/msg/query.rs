use finance::{coin::Coin, percent::Percent, currency::Currency};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub struct StateQuery {}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum StateResponse<C, Lpn>
where
    C: Currency,
    Lpn: Currency,
{
    Opened {
        amount: Coin<C>,
        interest_rate: Percent,
        interest_rate_margin: Percent,
        principal_due: Coin<Lpn>,
        previous_margin_due: Coin<Lpn>,
        previous_interest_due: Coin<Lpn>,
        current_margin_due: Coin<Lpn>,
        current_interest_due: Coin<Lpn>,
    },
    Paid(Coin<C>),
    Closed(),
}
