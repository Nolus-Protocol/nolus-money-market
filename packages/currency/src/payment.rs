use serde::{Deserialize, Serialize};

use sdk::schemars::{self, JsonSchema};

use crate::{
    currency::{AnyVisitor, Group, MaybeAnyVisitResult, Symbol, SymbolStatic},
    lease::LeaseGroup,
    lpn::Lpns,
    native::Native,
};

#[derive(PartialEq, Eq, Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub struct PaymentGroup {}

impl Group for PaymentGroup {
    const DESCR: SymbolStatic = "payment";

    fn maybe_visit_on_ticker<V>(ticker: Symbol<'_>, visitor: V) -> MaybeAnyVisitResult<V>
    where
        V: AnyVisitor,
    {
        LeaseGroup::maybe_visit_on_ticker(ticker, visitor)
            .or_else(|v| Lpns::maybe_visit_on_ticker(ticker, v))
            .or_else(|v| Native::maybe_visit_on_ticker(ticker, v))
    }

    fn maybe_visit_on_bank_symbol<V>(bank_symbol: Symbol<'_>, visitor: V) -> MaybeAnyVisitResult<V>
    where
        Self: Sized,
        V: AnyVisitor,
    {
        LeaseGroup::maybe_visit_on_bank_symbol(bank_symbol, visitor)
            .or_else(|v| Lpns::maybe_visit_on_bank_symbol(bank_symbol, v))
            .or_else(|v| Native::maybe_visit_on_bank_symbol(bank_symbol, v))
    }
}

#[cfg(test)]
mod test {
    use crate::{
        lease::{Atom, Osmo, StAtom, StOsmo, Wbtc, Weth},
        lpn::Usdc,
        native::Nls,
        test::group::{
            maybe_visit_on_bank_symbol_err, maybe_visit_on_bank_symbol_impl,
            maybe_visit_on_ticker_err, maybe_visit_on_ticker_impl,
        },
        Currency,
    };

    use super::PaymentGroup;

    #[test]
    fn maybe_visit_on_ticker() {
        maybe_visit_on_ticker_impl::<Atom, PaymentGroup>();
        maybe_visit_on_ticker_impl::<StAtom, PaymentGroup>();
        maybe_visit_on_ticker_impl::<Osmo, PaymentGroup>();
        maybe_visit_on_ticker_impl::<StOsmo, PaymentGroup>();
        maybe_visit_on_ticker_impl::<Weth, PaymentGroup>();
        maybe_visit_on_ticker_impl::<Wbtc, PaymentGroup>();
        maybe_visit_on_ticker_impl::<Usdc, PaymentGroup>();
        maybe_visit_on_ticker_impl::<Nls, PaymentGroup>();
        maybe_visit_on_ticker_err::<Nls, PaymentGroup>(Nls::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Atom, PaymentGroup>(Atom::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Usdc, PaymentGroup>(Nls::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Usdc, PaymentGroup>(Usdc::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Osmo, PaymentGroup>(Atom::BANK_SYMBOL);
        maybe_visit_on_ticker_err::<Osmo, PaymentGroup>(Osmo::BANK_SYMBOL);
    }

    #[test]
    fn maybe_visit_on_bank_symbol() {
        maybe_visit_on_bank_symbol_impl::<Atom, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<StAtom, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<Osmo, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<StOsmo, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<Weth, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<Wbtc, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<Usdc, PaymentGroup>();
        maybe_visit_on_bank_symbol_impl::<Nls, PaymentGroup>();
        maybe_visit_on_bank_symbol_err::<Nls, PaymentGroup>(Nls::TICKER);
        maybe_visit_on_bank_symbol_err::<Atom, PaymentGroup>(Atom::TICKER);
        maybe_visit_on_bank_symbol_err::<Usdc, PaymentGroup>(Nls::TICKER);
        maybe_visit_on_bank_symbol_err::<Usdc, PaymentGroup>(Usdc::TICKER);
    }
}
