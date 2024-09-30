use currency::{CurrencyDef, Group, MemberOf};
use finance::coin::Coin;

use crate::{error::Error, stub::Oracle};

use self::impl_::PriceConvert;

pub fn from_quote<QuoteC, QuoteG, OracleS, OutC, OutG>(
    oracle: &OracleS,
    in_amount: Coin<QuoteC>,
) -> Result<Coin<OutC>, Error>
where
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG>,
    QuoteG: Group,
    OracleS: Oracle<OutG, QuoteC = QuoteC, QuoteG = QuoteG>,
    OutC: CurrencyDef,
    OutC::Group: MemberOf<OutG>,
    OutG: Group,
{
    PriceConvert::<QuoteC, QuoteG, OutC, OutG>::new(in_amount).with_quote_in(oracle)
}

pub fn to_quote<InC, InG, QuoteC, QuoteG, OracleS>(
    oracle: &OracleS,
    in_amount: Coin<InC>,
) -> Result<Coin<QuoteC>, Error>
where
    InC: CurrencyDef,
    InC::Group: MemberOf<InG>,
    InG: Group,
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG>,
    QuoteG: Group,
    OracleS: Oracle<InG, QuoteC = QuoteC, QuoteG = QuoteG>,
{
    PriceConvert::<InC, InG, QuoteC, QuoteG>::new(in_amount).with_quote_out(oracle)
}

mod impl_ {
    use std::marker::PhantomData;

    use currency::{Currency, CurrencyDef, Group, MemberOf};
    use finance::{coin::Coin, error::Error as FinanceErr, price};

    use crate::{error::Error, Oracle};

    pub(super) struct PriceConvert<InC, InG, OutC, OutG>
    where
        InC: Currency + MemberOf<InG>,
        InG: Group,
        OutC: Currency + MemberOf<OutG>,
        OutG: Group,
    {
        in_amount: Coin<InC>,
        _in_group: PhantomData<InG>,
        _out: PhantomData<OutC>,
        _out_group: PhantomData<OutG>,
    }

    impl<InC, InG, OutC, OutG> PriceConvert<InC, InG, OutC, OutG>
    where
        InC: CurrencyDef,
        InC::Group: MemberOf<InG>,
        InG: Group,
        OutC: CurrencyDef,
        OutC::Group: MemberOf<OutG>,
        OutG: Group,
    {
        pub(super) fn new(in_amount: Coin<InC>) -> Self {
            Self {
                in_amount,
                _in_group: PhantomData,
                _out: PhantomData,
                _out_group: PhantomData,
            }
        }

        pub(super) fn with_quote_in<OracleImpl>(
            &self,
            oracle: &OracleImpl,
        ) -> Result<Coin<OutC>, Error>
        where
            OracleImpl: Oracle<OutG, QuoteC = InC, QuoteG = InG>,
        {
            oracle.price_of::<OutC>().and_then(|price| {
                price::total(self.in_amount, price.inv()).ok_or(Error::Finance(
                    FinanceErr::overflow_err(
                        "while calculating the total",
                        self.in_amount,
                        price.inv(),
                    ),
                ))
            })
        }

        pub(super) fn with_quote_out<OracleImpl>(
            &self,
            oracle: &OracleImpl,
        ) -> Result<Coin<OutC>, Error>
        where
            OracleImpl: Oracle<InG, QuoteC = OutC, QuoteG = OutG>,
        {
            oracle.price_of::<InC>().and_then(|price| {
                price::total(self.in_amount, price).ok_or(Error::Finance(FinanceErr::overflow_err(
                    "while calculating the total",
                    self.in_amount,
                    price,
                )))
            })
        }
    }
}
