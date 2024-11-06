use std::marker::PhantomData;

use currency::{Currency, CurrencyDef, MemberOf};
use platform::{bank::BankAccount, batch::Batch};
use sdk::cosmwasm_std::Addr;

use crate::{
    api::LeaseAssetCurrencies, error::ContractResult, finance::LpnCurrencies, position::Position,
};

use super::LeaseDTO;

pub struct Lease<Asset, Lpn> {
    customer: Addr,
    position: Position<Asset>,
    lpn: PhantomData<Lpn>,
}

impl<Asset, Lpn> Lease<Asset, Lpn>
where
    Asset: Currency + MemberOf<LeaseAssetCurrencies>,
{
    pub(crate) fn from_dto(dto: LeaseDTO, position: Position<Asset>) -> Self {
        Self {
            customer: dto.customer,
            position,
            lpn: PhantomData,
        }
    }
}

impl<Asset, Lpn> Lease<Asset, Lpn>
where
    Asset: CurrencyDef,
    Lpn: CurrencyDef,
    Lpn::Group: MemberOf<LpnCurrencies>,
{
    pub(crate) fn close<B>(self, lease_account: B) -> ContractResult<Batch>
    where
        B: BankAccount,
    {
        let surplus = lease_account.balance::<Lpn, LpnCurrencies>()?;

        let updated_account = if !surplus.is_zero() {
            lease_account.send(surplus, self.customer.clone())
        } else {
            lease_account
        };

        Ok(updated_account
            .send(self.position.amount(), self.customer)
            .into())
    }
}

#[cfg(test)]
mod tests {
    use std::marker::PhantomData;

    use currencies::{Lpn, PaymentC3};
    use currency::{Currency, CurrencyDef, Group};
    use finance::{
        coin::{Coin, WithCoin},
        duration::Duration,
        liability::Liability,
        percent::Percent,
        zero::Zero,
    };
    use platform::{
        bank::{
            self, Aggregate, BalancesResult, BankAccountView, BankStub, FixedAddressSender,
            LazySenderStub,
        },
        batch::Batch,
        result::Result as PlatformResult,
    };
    use sdk::cosmwasm_std::Addr;

    use crate::position::{Position, Spec};

    use super::Lease;

    const CUSTOMER: &str = "customer";
    type TestLpn = Lpn;
    type TestAsset = PaymentC3;

    pub struct MockBankView {
        balance: Coin<TestAsset>,
        balance_surplus: Coin<TestLpn>,
    }

    impl MockBankView {
        fn new(amount: Coin<TestAsset>, amount_surplus: Coin<TestLpn>) -> Self {
            Self {
                balance: amount,
                balance_surplus: amount_surplus,
            }
        }
        fn only_balance(amount: Coin<TestAsset>) -> Self {
            Self {
                balance: amount,
                balance_surplus: Coin::ZERO,
            }
        }
    }

    impl BankAccountView for MockBankView {
        fn balance<C, G>(&self) -> PlatformResult<Coin<C>>
        where
            C: CurrencyDef,
        {
            if currency::equal::<C, TestAsset>() {
                Ok(Coin::<C>::new(self.balance.into()))
            } else if currency::equal::<C, TestLpn>() {
                Ok(Coin::<C>::new(self.balance_surplus.into()))
            } else {
                unreachable!(
                    "Expected {}, found {}",
                    currency::to_string(TestAsset::definition()),
                    currency::to_string(C::definition())
                );
            }
        }

        fn balances<G, Cmd>(&self, _: Cmd) -> BalancesResult<G, Cmd>
        where
            G: Group,
            Cmd: WithCoin<G>,
            Cmd::Output: Aggregate,
        {
            unimplemented!()
        }
    }

    fn create_lease<Asset, Lpn>(amount: Coin<Asset>) -> Lease<Asset, Lpn>
    where
        Asset: Currency,
    {
        let liability = Liability::new(
            Percent::from_percent(65),
            Percent::from_percent(70),
            Percent::from_percent(72),
            Percent::from_percent(74),
            Percent::from_percent(77),
            Percent::from_percent(80),
            Duration::from_days(3),
        );
        let spec = Spec::new(
            liability,
            Coin::<TestLpn>::new(15_000_000),
            Coin::<TestLpn>::new(10_000),
        );

        Lease {
            customer: Addr::unchecked(CUSTOMER),
            position: Position::new(amount, spec),
            lpn: PhantomData,
        }
    }

    #[test]
    fn close_no_surplus() {
        let lease_amount = 10.into();
        let lease: Lease<TestAsset, TestLpn> = create_lease(lease_amount);
        let lease_account = BankStub::with_view(MockBankView::only_balance(lease_amount));
        let res = lease.close(lease_account).unwrap();
        assert_eq!(
            res,
            bank::bank_send(Addr::unchecked(CUSTOMER), lease_amount)
        );
    }

    #[test]
    fn close_with_surplus() {
        let customer = Addr::unchecked(CUSTOMER);
        let lease_amount = 10.into();
        let surplus_amount = 2.into();
        let lease: Lease<TestAsset, TestLpn> = create_lease(lease_amount);
        let lease_account = BankStub::with_view(MockBankView::new(lease_amount, surplus_amount));
        let res = lease.close(lease_account).unwrap();
        assert_eq!(res, {
            {
                let mut sender = LazySenderStub::new(customer.clone());
                sender.send(surplus_amount);
                Batch::from(sender)
            }
            .merge({
                let mut sender = LazySenderStub::new(customer);
                sender.send(lease_amount);
                sender.into()
            })
        });
    }
}
