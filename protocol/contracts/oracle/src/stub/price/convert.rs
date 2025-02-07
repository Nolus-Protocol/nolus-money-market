use std::marker::PhantomData;

use currency::{Currency, CurrencyDef, Group, MemberOf};
use finance::coin::Coin;
use oracle_platform::{
    error::{Error, Result},
    Oracle, OracleRef, WithOracle,
};
use sdk::cosmwasm_std::QuerierWrapper;

pub fn from_quote<QuoteC, QuoteG, OutC, OutG>(
    oracle_ref: OracleRef<QuoteC, QuoteG>,
    in_amount: Coin<QuoteC>,
    querier: QuerierWrapper<'_>,
) -> Result<Coin<OutC>>
where
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<OutG::TopG>,
    QuoteG: Group,
    OutC: CurrencyDef,
    OutC::Group: MemberOf<OutG>,
    OutG: Group,
{
    struct PriceConvert<QuoteC, QuoteG, OutC, OutG>
    where
        QuoteC: Currency,
        QuoteG: Group,
        OutC: Currency,
        OutG: Group,
    {
        in_amount: Coin<QuoteC>,
        in_group: PhantomData<QuoteG>,
        _out: PhantomData<OutC>,
        _out_group: PhantomData<OutG>,
    }

    impl<QuoteC, QuoteG, OutC, OutG> WithOracle<QuoteC, QuoteG>
        for PriceConvert<QuoteC, QuoteG, OutC, OutG>
    where
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG>,
        QuoteG: Group,
        OutC: CurrencyDef,
        OutC::Group: MemberOf<OutG>,
        OutG: Group,
    {
        type G = OutG;

        type Output = Coin<OutC>;
        type Error = Error;

        fn exec<OracleImpl>(self, oracle: OracleImpl) -> Result<Self::Output>
        where
            OracleImpl: Oracle<OutG, QuoteC = QuoteC, QuoteG = QuoteG>,
        {
            //oracle_platform::convert::from_quote::<_, _, _, OutC, _>(
            oracle_platform::convert::from_quote::<QuoteC, QuoteG, OracleImpl, OutC, OutG>(
                &oracle,
                self.in_amount,
            )
        }
    }

    oracle_ref.execute_as_oracle(
        PriceConvert {
            in_amount,
            in_group: PhantomData::<QuoteG>,
            _out: PhantomData::<OutC>,
            _out_group: PhantomData::<OutG>,
        },
        querier,
    )
}

pub fn to_quote<InC, InG, QuoteC, QuoteG>(
    oracle_ref: OracleRef<QuoteC, QuoteG>,
    in_amount: Coin<InC>,
    querier: QuerierWrapper<'_>,
) -> Result<Coin<QuoteC>>
where
    QuoteC: CurrencyDef,
    QuoteC::Group: MemberOf<QuoteG> + MemberOf<InG::TopG>,
    QuoteG: Group + MemberOf<InG>,
    InC: CurrencyDef,
    InC::Group: MemberOf<InG>,
    InG: Group,
{
    struct PriceConvert<InC, InG, QuoteC, QuoteG>
    where
        InC: Currency + MemberOf<InG>,
        InG: Group,
        QuoteC: Currency,
        QuoteG: Group,
    {
        in_amount: Coin<InC>,
        _in_group: PhantomData<InG>,
        _out: PhantomData<QuoteC>,
        _out_group: PhantomData<QuoteG>,
    }

    impl<InC, InG, QuoteC, QuoteG> WithOracle<QuoteC, QuoteG> for PriceConvert<InC, InG, QuoteC, QuoteG>
    where
        InC: CurrencyDef,
        InC::Group: MemberOf<InG>,
        InG: Group,
        QuoteC: CurrencyDef,
        QuoteC::Group: MemberOf<QuoteG>,
        QuoteG: Group,
    {
        type G = InG;

        type Output = Coin<QuoteC>;
        type Error = Error;

        fn exec<OracleImpl>(self, oracle: OracleImpl) -> Result<Self::Output>
        where
            OracleImpl: Oracle<Self::G, QuoteC = QuoteC, QuoteG = QuoteG>,
        {
            oracle_platform::convert::to_quote::<_, InG, _, _, _>(&oracle, self.in_amount)
        }
    }

    oracle_ref.execute_as_oracle(
        PriceConvert {
            in_amount,
            _in_group: PhantomData::<InG>,
            _out: PhantomData::<QuoteC>,
            _out_group: PhantomData::<QuoteG>,
        },
        querier,
    )
}
