use std::fmt::{Display, Formatter};

use sdk::schemars::{self, JsonSchema};
use serde::{Deserialize, Serialize};

use crate::{group::MemberOf, Group, SymbolOwned, Tickers};

use crate::error::Error;

use super::CurrencyDTO as ValidatedDTO;

/// Brings invariant checking as a step in deserializing a CurrencyDTO
#[derive(Serialize, Deserialize, JsonSchema)]
#[serde(transparent, deny_unknown_fields, rename_all = "snake_case")]
pub(super) struct TickerDTO(SymbolOwned);

impl Display for TickerDTO {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl<G> TryFrom<TickerDTO> for ValidatedDTO<G>
where
    G: Group + MemberOf<G>,
{
    type Error = Error;

    fn try_from(dto: TickerDTO) -> Result<Self, Self::Error> {
        Self::from_symbol::<Tickers<G>>(&dto.0)
    }
}

impl<G> From<ValidatedDTO<G>> for TickerDTO
where
    G: Group,
{
    fn from(value: ValidatedDTO<G>) -> Self {
        Self(value.into_symbol::<Tickers<G>>().to_owned())
    }
}

#[cfg(test)]
mod test {
    use sdk::cosmwasm_std;

    use crate::{
        test::{
            SubGroupCurrency, SubGroupTestC10, SuperGroup, SuperGroupCurrency, SuperGroupTestC1,
        },
        CurrencyDef,
    };

    #[test]
    fn deser_same_group() {
        let coin = SuperGroupTestC1::definition().dto();
        let coin_deser: SuperGroupCurrency = cosmwasm_std::to_json_vec(coin)
            .and_then(cosmwasm_std::from_json)
            .expect("correct raw bytes");
        assert_eq!(coin, &coin_deser);
    }

    #[test]
    fn deser_parent_group() {
        type ParentGroup = SuperGroupCurrency;

        let coin = SubGroupTestC10::definition().dto();
        let coin_deser: ParentGroup = cosmwasm_std::to_json_vec(coin)
            .and_then(cosmwasm_std::from_json)
            .expect("correct raw bytes");
        let coin_exp = coin.into_super_group::<SuperGroup>();
        assert_eq!(coin_exp, coin_deser);
    }

    #[test]
    fn deser_wrong_group() {
        let coin = *SuperGroupTestC1::definition().dto();
        let coin_raw = cosmwasm_std::to_json_vec(&coin).unwrap();

        assert!(cosmwasm_std::from_json::<SubGroupCurrency>(&coin_raw).is_err());
    }
}
