use sdk::schemars;

use crate::{define_currency, define_symbol, Native};

define_symbol! {
    NLS {
        bank: "unls",
        // full ibc route: transfer/channel-585/unls
        dex: "ibc/0D9EB2C9961610CD2F04003188C0B713E72297DCBC32371602897069DC0E3055"
    }
}
define_currency!(Nls, NLS, Native, 6);
