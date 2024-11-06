use thiserror::Error;

use lease::api::MigrateMsg;
use platform::{batch::Batch, contract::Code};
use sdk::cosmwasm_std::Addr;

use crate::{error::ContractError, msg::MaxLeases, result::ContractResult};

pub struct Customer<LeaseIter> {
    customer: Addr,
    leases: LeaseIter,
}

#[derive(Default, Debug)]
#[cfg_attr(test, derive(Eq, PartialEq))]
pub struct MigrationResult {
    pub msgs: Batch,
    pub next_customer: Option<Addr>,
}

pub type MaybeCustomer<LI> = ContractResult<Customer<LI>>;

/// Builds a batch of messages for the migration of up to `max_leases`
///
/// The customer connected leases are migrated up to the `max_leases` boundary and atomically for a customer.
/// If there are still pending customers, then the next customer is returned as a key to start from the next chunk of leases.
///
/// Consumes the customers iterator to the next customer or error.
pub fn migrate_leases<I, LI, MsgFactory>(
    mut customers: I,
    lease_code: Code,
    max_leases: MaxLeases,
    migrate_msg: MsgFactory,
) -> ContractResult<MigrationResult>
where
    I: Iterator<Item = MaybeCustomer<LI>>,
    LI: ExactSizeIterator<Item = Addr>,
    MsgFactory: Fn() -> MigrateMsg,
{
    let migration_batch = MigrateBatch::new(lease_code, max_leases);

    let result: Result<MigrateBatch, MigrateError> =
        customers.try_fold(migration_batch, |migration_set, maybe_customer| {
            maybe_customer.map_err(Into::into).and_then(|customer| {
                migration_set
                    .migrate_or_be_next(customer, &migrate_msg)
                    .map_err(Into::into)
                    .and_then(|outcome| match outcome {
                        MigrationStatus::Success(migrated) => Ok(migrated),
                        MigrationStatus::CapacityReached(not_migrated) => {
                            let incomplete_result = MigrationResult {
                                msgs: not_migrated.result.msgs,
                                next_customer: not_migrated.result.next_customer,
                            };
                            Err(MigrateError::MigrationIncomplete(incomplete_result))
                        }
                    })
            })
        });

    match result {
        Ok(migrate_summary) => Ok(migrate_summary.result),
        Err(migrate_err) => match migrate_err {
            MigrateError::MigrationIncomplete(outcome) => Ok(outcome),
            MigrateError::ContractError(err) => Err(err),
        },
    }
}

impl<LeaseIter> Customer<LeaseIter>
where
    LeaseIter: Iterator<Item = Addr>,
{
    pub fn from(customer: Addr, leases: LeaseIter) -> Self {
        Self { customer, leases }
    }
}

impl MigrationResult {
    pub fn try_add_msgs<F>(self, add_fn: F) -> ContractResult<Self>
    where
        F: FnOnce(Batch) -> ContractResult<Batch>,
    {
        add_fn(self.msgs).map(|msgs| Self {
            msgs,
            next_customer: self.next_customer,
        })
    }

    #[cfg(test)]
    fn new(msgs: Batch, next_customer: Option<Addr>) -> Self {
        Self {
            msgs,
            next_customer,
        }
    }
}

struct MigrateBatch {
    new_code: Code,
    leases_left: MaxLeases,
    result: MigrationResult,
}

enum MigrationStatus {
    CapacityReached(MigrateBatch),
    Success(MigrateBatch),
}

impl MigrateBatch {
    fn new(new_code: Code, max_leases: MaxLeases) -> Self {
        Self::new_with_result(new_code, max_leases, MigrationResult::default())
    }

    fn new_with_result(new_code: Code, leases_left: MaxLeases, result: MigrationResult) -> Self {
        Self {
            new_code,
            leases_left,
            result,
        }
    }

    /// Returns the result of the migration process presented as Ok(MigrationStatus) and ContractError if an error occurs during the migration.
    /// The MigrationStatus can be:
    /// - `Success` - All leases were migrated successfully.
    /// - `CapacityReached` - Indicates that the migration could not be completed due to capacity constraints.
    fn migrate_leases<Leases, MsgFactory>(
        mut self,
        mut leases: Leases,
        migrate_msg: &MsgFactory,
    ) -> ContractResult<MigrationStatus>
    where
        Leases: ExactSizeIterator<Item = Addr>,
        MsgFactory: Fn() -> MigrateMsg,
    {
        let maybe_leases_nb: Result<MaxLeases, _> = leases.len().try_into();

        match maybe_leases_nb {
            Err(err) => Err(err.into()),
            Ok(leases_nb) => {
                if let Some(left) = self.leases_left.checked_sub(leases_nb) {
                    self.leases_left = left;

                    leases
                        .try_fold(self, |mut state, lease| {
                            let updated_msgs = state.result.msgs.schedule_migrate_wasm_no_reply(
                                lease,
                                &migrate_msg(),
                                state.new_code,
                            )?;

                            state.result.msgs = updated_msgs;

                            Ok(state)
                        })
                        .map(MigrationStatus::Success)
                } else {
                    Ok(MigrationStatus::CapacityReached(self))
                }
            }
        }
    }

    /// Returns the result of the migration process presented as Ok(MigrationStatus) and ContractError if an error occurs during the migration.
    /// The MigrationStatus can be:
    /// - `Success` - Indicates that there is enough room for all of the customer's leases,
    ///   and in this case the customer's address is not passed.
    /// - `CapacityReached` - Indicates that there was not enough room to migrate all leases,
    ///   and therefore the address of the current client is passed as the next client.
    fn migrate_or_be_next<LI, MsgFactory>(
        self,
        customer: Customer<LI>,
        migrate_msg: &MsgFactory,
    ) -> ContractResult<MigrationStatus>
    where
        LI: ExactSizeIterator<Item = Addr>,
        MsgFactory: Fn() -> MigrateMsg,
    {
        match self.migrate_leases(customer.leases, migrate_msg) {
            Ok(status) => match status {
                MigrationStatus::Success(mut migrated_self) => {
                    migrated_self.result.next_customer = None;
                    Ok(MigrationStatus::Success(migrated_self))
                }
                MigrationStatus::CapacityReached(mut not_migrated_self) => {
                    not_migrated_self.result.next_customer = Some(customer.customer);
                    Ok(MigrationStatus::CapacityReached(not_migrated_self))
                }
            },
            Err(err) => Err(err),
        }
    }
}

impl From<MigrateBatch> for Batch {
    fn from(this: MigrateBatch) -> Self {
        this.result.msgs
    }
}

#[derive(Error, Debug)]
enum MigrateError {
    #[error("[Leaser] Migration incomplete with next customer")]
    MigrationIncomplete(MigrationResult),

    #[error("{0}")]
    ContractError(#[from] ContractError),
}

#[cfg(test)]
mod test {
    use std::vec::IntoIter;

    use lease::api::MigrateMsg;
    use platform::contract::Code;
    use sdk::cosmwasm_std::Addr;

    use crate::{
        migrate::{Customer, MigrationResult},
        result::ContractResult,
        ContractError,
    };

    const LEASE1: &str = "lease1";
    const LEASE21: &str = "lease21";
    const LEASE22: &str = "lease22";
    const LEASE3: &str = "lease3";
    const LEASE41: &str = "lease41";
    const LEASE42: &str = "lease42";
    const LEASE43: &str = "lease43";
    const CUSTOMER_ADDR1: &str = "customer1";
    const CUSTOMER_ADDR2: &str = "customer2";
    const CUSTOMER_ADDR3: &str = "customer3";
    const CUSTOMER_ADDR4: &str = "customer4";

    #[test]
    fn no_leases() {
        use std::array::IntoIter;
        let new_code = Code::unchecked(242);
        let no_leases: Vec<Customer<IntoIter<Addr, 0>>> = vec![];
        assert_eq!(
            Ok(MigrationResult::default()),
            super::migrate_leases(no_leases.into_iter().map(Ok), new_code, 2, migrate_msg,)
        );
    }

    #[test]
    fn more_than_max_leases() {
        let new_code = Code::unchecked(242);
        let lease1 = Addr::unchecked(LEASE1);
        let lease2 = Addr::unchecked(LEASE21);
        let lease3 = Addr::unchecked(LEASE22);
        let customer_addr1 = Addr::unchecked(CUSTOMER_ADDR1);
        let cust1 = Customer::from(customer_addr1.clone(), [lease1, lease2, lease3].into_iter());

        let customers = [Ok(cust1)];
        {
            let exp = MigrationResult {
                next_customer: Some(customer_addr1),
                ..Default::default()
            };
            assert_eq!(
                Ok(exp),
                super::migrate_leases(customers.into_iter(), new_code, 2, migrate_msg)
            );
        }
    }

    #[test]
    fn paging() {
        let new_code = Code::unchecked(242);
        let lease1 = Addr::unchecked(LEASE1);
        let lease21 = Addr::unchecked(LEASE21);
        let lease22 = Addr::unchecked(LEASE22);
        let lease3 = Addr::unchecked(LEASE3);
        let lease41 = Addr::unchecked(LEASE41);
        let lease42 = Addr::unchecked(LEASE42);
        let lease43 = Addr::unchecked(LEASE43);
        fn customer1() -> Addr {
            Addr::unchecked(CUSTOMER_ADDR1)
        }
        fn customer2() -> Addr {
            Addr::unchecked(CUSTOMER_ADDR2)
        }
        fn customer3() -> Addr {
            Addr::unchecked(CUSTOMER_ADDR3)
        }
        fn customer4() -> Addr {
            Addr::unchecked(CUSTOMER_ADDR4)
        }

        {
            let exp = MigrationResult {
                next_customer: Some(customer1()),
                ..Default::default()
            };
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 0, migrate_msg)
            );
        }
        {
            let mut exp = add_expected(MigrationResult::default(), lease1.clone(), new_code);
            exp.next_customer = Some(customer2());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 1, migrate_msg)
            );
        }
        {
            let mut exp = add_expected(MigrationResult::default(), lease1.clone(), new_code);
            exp.next_customer = Some(customer2());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 2, migrate_msg)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), lease1.clone(), new_code);
            let exp = add_expected(exp, lease21.clone(), new_code);
            let mut exp = add_expected(exp, lease22.clone(), new_code);
            exp.next_customer = Some(customer3());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 3, migrate_msg)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), lease1.clone(), new_code);
            let exp = add_expected(exp, lease21.clone(), new_code);
            let exp = add_expected(exp, lease22.clone(), new_code);
            let mut exp = add_expected(exp, lease3.clone(), new_code);
            exp.next_customer = Some(customer4());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 4, migrate_msg)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), lease1.clone(), new_code);
            let exp = add_expected(exp, lease21.clone(), new_code);
            let exp = add_expected(exp, lease22.clone(), new_code);
            let mut exp = add_expected(exp, lease3.clone(), new_code);
            exp.next_customer = Some(customer4());
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 5, migrate_msg)
            );
        }
        {
            let exp = add_expected(MigrationResult::default(), lease1, new_code);
            let exp = add_expected(exp, lease21, new_code);
            let exp = add_expected(exp, lease22, new_code);
            let exp = add_expected(exp, lease3, new_code);
            let exp = add_expected(exp, lease41, new_code);
            let exp = add_expected(exp, lease42, new_code);
            let mut exp = add_expected(exp, lease43, new_code);
            exp.next_customer = None;
            assert_eq!(
                Ok(exp),
                super::migrate_leases(test_customers(), new_code, 7, migrate_msg)
            );
        }
    }

    #[test]
    fn err_leases() {
        let new_code = Code::unchecked(242);
        let lease1 = Addr::unchecked("lease11");
        let lease2 = Addr::unchecked("lease12");
        let lease3 = Addr::unchecked("lease13");
        let cust1 = Customer::from(
            Addr::unchecked("customer1"),
            [lease1, lease2, lease3].into_iter(),
        );
        let err = "testing error";

        let customers = [
            Ok(cust1),
            Err(ContractError::ParseError { err: err.into() }),
        ];
        assert_eq!(
            Err(ContractError::ParseError { err: err.into() }),
            super::migrate_leases(customers.into_iter(), new_code, 3, migrate_msg)
        );
    }

    fn add_expected(exp: MigrationResult, lease_addr: Addr, new_code: Code) -> MigrationResult {
        MigrationResult::new(
            exp.msgs
                .schedule_migrate_wasm_no_reply(lease_addr, &migrate_msg(), new_code)
                .unwrap(),
            exp.next_customer,
        )
    }

    fn test_customers() -> impl Iterator<Item = ContractResult<Customer<IntoIter<Addr>>>> {
        let lease1 = Addr::unchecked(LEASE1);
        let customer_addr1 = Addr::unchecked(CUSTOMER_ADDR1);
        let cust1 = Customer::from(customer_addr1, vec![lease1].into_iter());

        let lease21 = Addr::unchecked(LEASE21);
        let lease22 = Addr::unchecked(LEASE22);
        let customer_addr2 = Addr::unchecked(CUSTOMER_ADDR2);
        let cust2 = Customer::from(customer_addr2, vec![lease21, lease22].into_iter());

        let lease3 = Addr::unchecked(LEASE3);
        let customer_addr3 = Addr::unchecked(CUSTOMER_ADDR3);
        let cust3 = Customer::from(customer_addr3, vec![lease3].into_iter());

        let lease41 = Addr::unchecked(LEASE41);
        let lease42 = Addr::unchecked(LEASE42);
        let lease43 = Addr::unchecked(LEASE43);
        let customer_addr4 = Addr::unchecked(CUSTOMER_ADDR4);
        let cust4 = Customer::from(customer_addr4, vec![lease41, lease42, lease43].into_iter());

        vec![Ok(cust1), Ok(cust2), Ok(cust3), Ok(cust4)].into_iter()
    }

    fn migrate_msg() -> MigrateMsg {
        MigrateMsg {}
    }
}
