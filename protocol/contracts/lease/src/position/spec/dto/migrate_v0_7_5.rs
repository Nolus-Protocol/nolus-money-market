use serde::Deserialize;

use crate::{api::open::PositionSpecDTO, position::close::Policy as ClosePolicy};

use super::SpecDTO as LastVersionSpecDTO;

/// Migrate v0.7.5 Position spec to the next v0.7.6 format
// TODO clean-up the v0.7.5 support once all leases have gone through this migration
#[derive(Copy, Clone, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "snake_case", untagged)]
#[cfg_attr(feature = "contract_testing", derive(Debug, PartialEq, Eq))]
pub(super) enum SpecDTO {
    V0_7_6 {
        // this is the first variant since the untagged enum representation tries to deserialize to variants as per their order in the definition
        r#const: PositionSpecDTO,
        close: ClosePolicy,
    },
    V0_7_5 {
        #[serde(flatten)]
        spec: PositionSpecDTO,
    },
}

impl From<SpecDTO> for LastVersionSpecDTO {
    fn from(value: SpecDTO) -> Self {
        match value {
            SpecDTO::V0_7_5 { spec } => LastVersionSpecDTO::initial(spec),

            SpecDTO::V0_7_6 { r#const, close } => LastVersionSpecDTO::new(r#const, close),
        }
    }
}

#[cfg(all(feature = "internal.test.contract", test))]
mod test_two_versions {

    use currencies::Lpn;
    use finance::{
        coin::{Coin, CoinDTO},
        duration::Duration,
        liability::Liability,
        percent::Percent,
    };
    use sdk::cosmwasm_std;

    use super::{LastVersionSpecDTO, SpecDTO};
    use crate::api::open::PositionSpecDTO;

    #[test]
    fn read_7_5_into_7_6() {
        const RAW_7_5: &str = r#"{
            "liability":{"initial":600,"healthy":830,"first_liq_warn":850,"second_liq_warn":865,"third_liq_warn":880,"max":900,"recalc_time":432000000000000},
            "min_asset":{"amount":"15000000","ticker":"LPN"},
            "min_transaction":{"amount":"10000","ticker":"LPN"}}"#;

        let position_spec = PositionSpecDTO::new(
            Liability::new(
                Percent::from_permille(600),
                Percent::from_permille(830),
                Percent::from_permille(850),
                Percent::from_permille(865),
                Percent::from_permille(880),
                Percent::from_permille(900),
                Duration::from_nanos(432000000000000),
            ),
            CoinDTO::from(Coin::<Lpn>::from(15000000)),
            CoinDTO::from(Coin::<Lpn>::from(10000)),
        );
        let spec_v7_6 = SpecDTO::V0_7_5 {
            spec: position_spec,
        };

        assert_eq!(position_spec, cosmwasm_std::from_json(RAW_7_5).unwrap());
        assert_eq!(spec_v7_6, cosmwasm_std::from_json(RAW_7_5).unwrap());
        assert_eq!(
            LastVersionSpecDTO::from(spec_v7_6),
            cosmwasm_std::from_json(RAW_7_5).unwrap()
        );

        assert_eq!(
            LastVersionSpecDTO::initial(position_spec),
            cosmwasm_std::from_json(
                cosmwasm_std::to_json_vec(&LastVersionSpecDTO::from(spec_v7_6))
                    .expect("serialization passed"),
            )
            .expect("deserialization passed")
        );
    }
}
