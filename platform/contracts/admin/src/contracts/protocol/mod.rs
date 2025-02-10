use serde::{Deserialize, Serialize};

pub(super) mod higher_order_type;
#[cfg(feature = "contract")]
mod impl_mod;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct ProtocolContracts<T> {
    pub leaser: T,
    pub lpp: T,
    pub oracle: T,
    pub profit: T,
    pub reserve: T,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Protocol<T> {
    pub network: Network,
    pub dex: Dex,
    pub contracts: ProtocolContracts<T>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "PascalCase",
    rename_all_fields = "snake_case",
    deny_unknown_fields
)]
pub enum Network {
    Neutron,
    Osmosis,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(
    rename_all = "PascalCase",
    rename_all_fields = "snake_case",
    deny_unknown_fields
)]
pub enum Dex {
    Astroport { router_address: String },
    Osmosis,
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_dex_serde() {
        const ASTROPORT_ROUTER_ADDRESS: &str = "neutron0123456789ABCDEF";

        assert_eq!(
            sdk::cosmwasm_std::from_json(format!(
                r#"{{
                    "Astroport": {{
                        "router_address": {ASTROPORT_ROUTER_ADDRESS:?}
                    }}
                }}"#
            )),
            Ok(super::Dex::Astroport {
                router_address: ASTROPORT_ROUTER_ADDRESS.to_string()
            })
        );

        assert_eq!(
            sdk::cosmwasm_std::from_json(r#""Osmosis""#),
            Ok(super::Dex::Osmosis {})
        );
    }
}
