use currency::Currency;
use finance::coin::Coin;

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct Receipt<C>
where
    C: Currency,
{
    margin_paid: Coin<C>,
    interest_paid: Coin<C>,
    principal_paid: Coin<C>,
    change: Coin<C>,
    close: bool,
}

impl<C> Receipt<C>
where
    C: Currency,
{
    pub fn margin_paid(&self) -> Coin<C> {
        self.margin_paid
    }

    pub fn interest_paid(&self) -> Coin<C> {
        self.interest_paid
    }

    pub fn principal_paid(&self) -> Coin<C> {
        self.principal_paid
    }

    pub fn change(&self) -> Coin<C> {
        self.change
    }

    pub fn close(&self) -> bool {
        self.close
    }

    pub fn total(&self) -> Coin<C> {
        self.margin_paid + self.interest_paid + self.principal_paid + self.change
    }

    pub(super) fn pay_margin(&mut self, payment: Coin<C>) {
        debug_assert_eq!(self.margin_paid, Coin::default());

        self.margin_paid = payment;
    }

    pub(super) fn pay_interest(&mut self, payment: Coin<C>) {
        debug_assert_eq!(self.interest_paid, Coin::default());

        self.interest_paid = payment;
    }

    pub(super) fn pay_principal(&mut self, principal: Coin<C>, payment: Coin<C>) {
        debug_assert_eq!(self.principal_paid, Coin::default());

        debug_assert!(payment <= principal, "Payment exceeds principal!");

        self.principal_paid = payment;

        self.close = principal == payment;
    }

    pub(super) fn keep_change(&mut self, change: Coin<C>) {
        debug_assert_eq!(self.change, Coin::default());

        self.change = change;
    }
}

#[cfg(test)]
mod tests {
    use currency::test::SuperGroupTestC1;
    use finance::coin::Coin;

    use crate::loan::RepayReceipt;

    type BorrowC = SuperGroupTestC1;

    #[test]
    fn pay_principal_full() {
        let principal = Coin::<BorrowC>::new(10);

        let mut receipt = RepayReceipt::default();

        receipt.pay_principal(principal, principal);

        assert_eq!(
            receipt,
            RepayReceipt {
                principal_paid: principal,
                close: true,
                ..Default::default()
            },
        );
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic = "Payment exceeds principal!"]
    fn pay_principal_overpaid() {
        let principal = Coin::<BorrowC>::new(10);

        let payment = principal + Coin::<BorrowC>::new(1);

        let mut receipt = RepayReceipt::default();

        receipt.pay_principal(principal, payment);
    }
}
