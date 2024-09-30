use sdk::schemars;

use crate::{define_currency, define_symbol, Lpns};

define_symbol! {
    USDC_AXELAR {
        // full ibc route: transfer/channel-1/transfer/channel-8/uausdc
        bank: "ibc/88012ABE034CE754022417BFEDF29F8B16C5B3338386EA20298ADCECA8329019",
        // full ibc route: transfer/channel-8/uausdc
        dex: "ibc/F91EA2C0A23697A1048E08C2F787E3A58AC6F706A1CD2257A504925158CFC0F3",
    }
}
define_currency!(UsdcAxelar, USDC_AXELAR, Lpns, 6);

pub use UsdcAxelar as Lpn;
