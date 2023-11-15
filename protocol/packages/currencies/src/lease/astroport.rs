use sdk::schemars;

use currency::{AnyVisitor, Matcher, MaybeAnyVisitResult, SymbolSlice};

use crate::{define_currency, define_symbol};

// Resources:
// 1. Symbol hashes are computed using the SHA256 Hash Generator https://coding.tools/sha256
// 2. Currencies that come from Axelar are documented at https://docs.axelar.dev/resources
// 3. IBC routes from https://github.com/Nolus-Protocol/Wiki/blob/main/testnet-rila/currencies.json

// ibc/C4CFF46FD6DE35CA4CF4CE031E643C8FDC9BA4B99AE598E9B0ED98FE3A2319F9 (ATOM)
// ibc/B7864B03E1B9FD4F049243E92ABD691586F682137037A9F3FCA5222815620B3C (stATOM)
define_symbol! {
    ATOM {
        ["dev"]: {
            /// full ibc route: transfer/channel-116/transfer/channel-1/uatom
            bank: "ibc/59BA0C7FDC7C3CDA4C777EDEC5572C762B68DDCC9FD253BC12B6F5676395157E",
            /// full ibc route: transfer/channel-1/uatom
            dex: "ibc/C4CFF46FD6DE35CA4CF4CE031E643C8FDC9BA4B99AE598E9B0ED98FE3A2319F9",
        },
        ["test"]: {
            /// full ibc route: transfer/channel-1990/transfer/channel-1/uatom
            bank: "ibc/FB644CC503C21C67F89C111B25362AA71CEE60E803211AC939DEBF820109660C",
            /// full ibc route: transfer/channel-1/uatom
            dex: "ibc/C4CFF46FD6DE35CA4CF4CE031E643C8FDC9BA4B99AE598E9B0ED98FE3A2319F9",
        },
        ["main"]: {
            /// full ibc route: transfer/channel-?/transfer/channel-?/uatom
            bank: "ibc/NA_ATOM",
            /// full ibc route: transfer/channel-?/uatom
            dex: "ibc/NA_ATOM_DEX",
        },
    }
}
define_currency!(Atom, ATOM);

define_symbol! {
    ST_ATOM {
        ["dev", "test"]: {
            /// full ibc route: transfer/channel-?/transfer/channel-?/stuatom
            bank: "ibc/NA_ST_ATOM",
            /// full ibc route: transfer/channel-?/stuatom
            dex: "ibc/NA_ST_ATOM_DEX",
        },
        ["main"]: {
            /// full ibc route: transfer/channel-?/transfer/channel-?/stuatom
            bank: "ibc/NA_ST_ATOM",
            /// full ibc route: transfer/channel-?/stuatom
            dex: "ibc/NA_ST_ATOM_DEX",
        },
    }
}
define_currency!(StAtom, ST_ATOM);

define_symbol! {
    NTRN {
        ["dev"]: {
            /// full ibc route: transfer/channel-116/untrn
            bank: "ibc/AED28B54A45FBE585FD81FF8571F729F87EF3CFD5DBBD1CAFED9E1AA20BFD983",
            dex: "untrn",
        },
        ["test"]: {
            /// full ibc route: transfer/channel-1990/untrn
            bank: "ibc/712F900E327780AAB33B9204DB5257FB1D6FACCF9CD7B70A0EFB31ED4C1255C4",
            dex: "untrn",
        },
        ["main"]: {
            /// full ibc route: transfer/channel-?/untrn
            bank: "ibc/NA_NTRN",
            dex: "ibc/NO_NRTN_DEX",
        },
    }
}
define_currency!(Ntrn, NTRN);

pub(super) fn maybe_visit<M, V>(
    matcher: &M,
    symbol: &SymbolSlice,
    visitor: V,
) -> MaybeAnyVisitResult<V>
where
    M: Matcher + ?Sized,
    V: AnyVisitor,
{
    use currency::maybe_visit_any as maybe_visit;
    maybe_visit::<_, Atom, _>(matcher, symbol, visitor)
        .or_else(|visitor| maybe_visit::<_, StAtom, _>(matcher, symbol, visitor))
        .or_else(|visitor| maybe_visit::<_, Ntrn, _>(matcher, symbol, visitor))
}

#[cfg(test)]
mod test {
    use currency::Currency;

    use crate::{
        test_impl::{
            maybe_visit_on_bank_symbol_err, maybe_visit_on_bank_symbol_impl,
            maybe_visit_on_ticker_err, maybe_visit_on_ticker_impl,
        },
        {lease::LeaseGroup, lpn::osmosis::Usdc, native::osmosis::Nls},
    };

    use super::{Atom, Osmo, StAtom, StOsmo, Tia, Wbtc, Weth};

    #[test]
    fn maybe_visit_on_ticker() {
        maybe_visit_on_ticker_impl::<Atom, LeaseGroup>();
        maybe_visit_on_ticker_impl::<StAtom, LeaseGroup>();
        maybe_visit_on_ticker_impl::<Ntrn, LeaseGroup>();
        maybe_visit_on_ticker_err::<Usdc, LeaseGroup>(Usdc::TICKER);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Atom::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Nls::TICKER);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Nls::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Atom, LeaseGroup>(Usdc::BANK_SYMBOL);
    }

    #[test]
    fn maybe_visit_on_bank_symbol() {
        maybe_visit_on_bank_symbol_impl::<Atom, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<StAtom, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<Osmo, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<StOsmo, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<Weth, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<Wbtc, LeaseGroup>();
        maybe_visit_on_bank_symbol_impl::<Tia, LeaseGroup>();
        maybe_visit_on_bank_symbol_err::<Usdc, LeaseGroup>(Usdc::BANK_SYMBOL);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Atom::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Usdc::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Nls::BANK_SYMBOL);
        maybe_visit_on_bank_symbol_err::<Atom, LeaseGroup>(Nls::TICKER);
    }
}
