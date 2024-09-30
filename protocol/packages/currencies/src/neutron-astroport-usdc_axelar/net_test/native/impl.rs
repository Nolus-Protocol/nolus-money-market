use sdk::schemars;

use crate::{define_currency, define_symbol, Native};

define_symbol! {
    NLS {
        bank: "unls",
        // full ibc route: transfer/channel-1061/unls
        dex: "ibc/E808FAAE7ADDA37453A8F0F67D74669F6580CBA5EF0F7889D46FB02D282098E3"
    }
}
define_currency!(Nls, NLS, Native, 6);
