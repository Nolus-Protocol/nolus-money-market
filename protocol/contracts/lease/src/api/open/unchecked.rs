use serde::Deserialize;

use finance::liability::Liability;

use crate::{error_de::ErrorDe, finance::LpnCoinDTO};

use super::PositionSpecDTO as ValidatedPositionSpec;

/// Bring invariant checking as a step in deserializing a PositionSpecDTO
#[derive(Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct PositionSpecDTO {
    liability: Liability,
    min_asset: LpnCoinDTO,
    min_transaction: LpnCoinDTO,
}

impl TryFrom<PositionSpecDTO> for ValidatedPositionSpec {
    type Error = ErrorDe;

    fn try_from(value: PositionSpecDTO) -> Result<Self, Self::Error> {
        let res = Self {
            liability: value.liability,
            min_asset: value.min_asset,
            min_transaction: value.min_transaction,
        };
        res.invariant_held().map(|_| res)
    }
}

#[cfg(all(feature = "internal.test.skel", test))]
mod test {
    use finance::{duration::Duration, percent::Percent};
    use sdk::cosmwasm_std::{from_json, to_json_vec, Addr};

    use crate::api::open::LoanForm;

    const LPP_ADDR: &str = "nolus1qg5ega6dykkxc307y25pecuufrjkxkaggkkxh7nad0vhyhtuhw3sqaa3c5";
    const PROFIT_ADDR: &str = "nolus1mf6ptkssddfmxvhdx0ech0k03ktp6kf9yk59renau2gvht3nq2gqkxgywu";
    const DUE_PERIOD: Duration = Duration::from_nanos(604800000000000);
    const MARGIN_INTEREST: Percent = Percent::from_permille(40);

    #[test]
    fn read_5_0() {
        assert_eq!(
            loan_v5_0(),
            from_json(to_json_vec(&loan_v5_0()).expect("serialization passed"))
                .expect("deserialization passed")
        );
    }

    fn loan_v5_0() -> LoanForm {
        LoanForm {
            lpp: Addr::unchecked(LPP_ADDR),
            profit: Addr::unchecked(PROFIT_ADDR),
            annual_margin_interest: MARGIN_INTEREST,
            due_period: DUE_PERIOD,
        }
    }
}
