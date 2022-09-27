use finance::currency::{AnyVisitor, Currency, Group, Member, Symbol};

#[cfg(feature = "testing")]
use crate::test::{TestCurrencyA, TestCurrencyB, TestCurrencyC, TestCurrencyD};
use crate::{
    lease::{Atom, Osmo},
    lpn::Usdc,
    native::Nls,
};

impl Member<PaymentGroup> for Usdc {}
impl Member<PaymentGroup> for Osmo {}
impl Member<PaymentGroup> for Atom {}
impl Member<PaymentGroup> for Nls {}

pub struct PaymentGroup {}
impl Group for PaymentGroup {
    fn resolve<V>(symbol: Symbol, visitor: V) -> Result<V::Output, V::Error>
    where
        V: AnyVisitor<Self>,
    {
        #[cfg(not(feature = "testing"))]
        {
            match symbol {
                Usdc::SYMBOL => visitor.on::<Usdc>(),
                Osmo::SYMBOL => visitor.on::<Osmo>(),
                Atom::SYMBOL => visitor.on::<Atom>(),
                Nls::SYMBOL => visitor.on::<Nls>(),
                _ => visitor.on_unknown(),
            }
        }
        #[cfg(feature = "testing")]
        {
            match symbol {
                Usdc::SYMBOL => visitor.on::<Usdc>(),
                Osmo::SYMBOL => visitor.on::<Osmo>(),
                Atom::SYMBOL => visitor.on::<Atom>(),
                Nls::SYMBOL => visitor.on::<Nls>(),
                TestCurrencyA::SYMBOL => visitor.on::<TestCurrencyA>(),
                TestCurrencyB::SYMBOL => visitor.on::<TestCurrencyB>(),
                TestCurrencyC::SYMBOL => visitor.on::<TestCurrencyC>(),
                TestCurrencyD::SYMBOL => visitor.on::<TestCurrencyD>(),
                _ => visitor.on_unknown(),
            }
        }
    }
}
