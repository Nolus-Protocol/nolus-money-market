use currency::{AnyVisitor, Matcher, MaybeAnyVisitResult, MemberOf};
use sdk::schemars;

use crate::{define_currency, define_symbol, LeaseGroup};

// Resources:
// 1. Symbol hashes are computed using the SHA256 Hash Generator https://coding.tools/sha256
// 2. Currencies that come from Axelar are documented at https://docs.axelar.dev/resources
// 3. IBC routes from https://github.com/Nolus-Protocol/Wiki/blob/main/testnet-rila/currencies.json

define_symbol! {
    ATOM {
        // full ibc route: transfer/channel-1/transfer/channel-1/uatom
        bank: "ibc/B62610294777CD7D4567F7125B5D88DE95C6B7F7ED25430F3808F863202BC599",
        // full ibc route: transfer/channel-1/uatom
        dex: "ibc/C4CFF46FD6DE35CA4CF4CE031E643C8FDC9BA4B99AE598E9B0ED98FE3A2319F9",
    }
}
define_currency!(Atom, ATOM, LeaseGroup, 6);

define_symbol! {
    NTRN {
        // full ibc route: transfer/channel-1/untrn
        bank: "ibc/0C698C8970DB4C539455E5225665A804F6338753211319E44BAD39758B238695",
        dex: "untrn",
    }
}
define_currency!(Ntrn, NTRN, LeaseGroup, 6);

pub(super) fn maybe_visit<M, V>(matcher: &M, visitor: V) -> MaybeAnyVisitResult<V>
where
    M: Matcher,
    V: AnyVisitor,
    LeaseGroup: MemberOf<V::VisitedG> + MemberOf<M::Group>,
{
    use currency::maybe_visit_any as maybe_visit;
    maybe_visit::<_, Atom, _>(matcher, visitor)
        .or_else(|visitor| maybe_visit::<_, Ntrn, _>(matcher, visitor))
}

#[cfg(test)]
mod test {
    use currency::Currency;

    use crate::{
        test_impl::{
            maybe_visit_on_bank_symbol_err, maybe_visit_on_bank_symbol_impl,
            maybe_visit_on_ticker_err, maybe_visit_on_ticker_impl,
        },
        {
            lease::LeaseGroup,
            lpn::{Lpn, Lpns},
            native::Nls,
        },
    };

    use super::{Atom, Ntrn};

    #[test]
    fn maybe_visit_on_ticker() {
        maybe_visit_on_ticker_impl::<Atom, LeaseGroup>();
        maybe_visit_on_ticker_impl::<Ntrn, LeaseGroup>();
        maybe_visit_on_ticker_err::<Lpn, Lpns>(Lpn::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Atom::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Nls::TICKER);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Atom::DEX_SYMBOL);
        maybe_visit_on_ticker_err::<Ntrn, LeaseGroup>(Ntrn::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Ntrn, LeaseGroup>(Ntrn::DEX_SYMBOL);
    }

    #[test]
    fn maybe_visit_on_bank_symbol() {
        maybe_visit_on_bank_symbol_impl::<Atom, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<Ntrn, LeaseGroup>();
        maybe_visit_on_bank_symbol_err::<Lpn, Lpns>(Lpn::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Atom::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Lpn::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Atom::DEX_SYMBOL);
        maybe_visit_on_bank_symbol_err::<Ntrn, LeaseGroup>(Ntrn::DEX_SYMBOL);
    }
}
