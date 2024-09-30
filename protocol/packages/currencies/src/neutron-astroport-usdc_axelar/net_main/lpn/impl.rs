use sdk::schemars;

use crate::{define_currency, define_symbol, Lpns};

define_symbol! {
    USDC_AXELAR {
        // full ibc route: transfer/channel-3839/transfer/channel-2/uusdc
        bank: "ibc/076CF690A9912E0B7A2CCA75B719D68AF7C20E4B0B6460569B333DDEB19BBBA1",
        // full ibc route: transfer/channel-2/uusdc
        dex: "ibc/F082B65C88E4B6D5EF1DB243CDA1D331D002759E938A0F5CD3FFDC5D53B3E349",
    }
}
define_currency!(UsdcAxelar, USDC_AXELAR, Lpns, 6);

pub use UsdcAxelar as Lpn;
