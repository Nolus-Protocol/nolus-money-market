use crate::finance::LpnCoin;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct Receipt {
    overdue_margin_paid: LpnCoin,
    overdue_interest_paid: LpnCoin,
    due_margin_paid: LpnCoin,
    due_interest_paid: LpnCoin,
    principal_paid: LpnCoin,
    change: LpnCoin,
    close: bool,
}

impl Receipt {
    pub fn new(
        overdue_interest: LpnCoin,
        overdue_margin: LpnCoin,
        due_interest: LpnCoin,
        due_margin: LpnCoin,
        principal_due: LpnCoin,
        principal_paid: LpnCoin,
        change: LpnCoin,
    ) -> Self {
        debug_assert!(
            principal_paid <= principal_due,
            "Payment exceeds principal!"
        );

        Self {
            overdue_interest_paid: overdue_interest,
            overdue_margin_paid: overdue_margin,
            due_interest_paid: due_interest,
            due_margin_paid: due_margin,
            principal_paid,
            change,
            close: principal_due == principal_paid,
        }
    }

    pub fn overdue_margin_paid(&self) -> LpnCoin {
        self.overdue_margin_paid
    }

    pub fn overdue_interest_paid(&self) -> LpnCoin {
        self.overdue_interest_paid
    }

    pub fn due_margin_paid(&self) -> LpnCoin {
        self.due_margin_paid
    }

    pub fn due_interest_paid(&self) -> LpnCoin {
        self.due_interest_paid
    }

    pub fn principal_paid(&self) -> LpnCoin {
        self.principal_paid
    }

    pub fn change(&self) -> LpnCoin {
        self.change
    }

    pub fn close(&self) -> bool {
        self.close
    }

    pub fn total(&self) -> LpnCoin {
        self.overdue_margin_paid
            + self.overdue_interest_paid
            + self.due_margin_paid
            + self.due_interest_paid
            + self.principal_paid
            + self.change
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod tests {
    use finance::{coin::Coin, zero::Zero};

    use crate::loan::RepayReceipt;

    #[test]
    fn pay_principal_full() {
        let principal = Coin::new(10);

        let receipt = RepayReceipt::new(
            Coin::ZERO,
            Coin::ZERO,
            Coin::ZERO,
            Coin::ZERO,
            principal,
            principal,
            Coin::ZERO,
        );

        assert_eq!(principal, receipt.principal_paid());
        assert!(receipt.close());
        assert_eq!(principal, receipt.total());
    }

    #[cfg(debug_assertions)]
    #[test]
    #[should_panic = "Payment exceeds principal!"]
    fn pay_principal_overpaid() {
        let principal = Coin::new(10);

        RepayReceipt::new(
            Coin::ZERO,
            Coin::ZERO,
            Coin::ZERO,
            Coin::ZERO,
            principal,
            principal + Coin::new(1),
            Coin::ZERO,
        );
    }
}
