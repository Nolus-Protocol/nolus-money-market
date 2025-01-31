pub(super) mod definitions {
    use serde::{Deserialize, Serialize};

    use currency::{CurrencyDTO, CurrencyDef, Definition, InPoolWith};

    use crate::{
        lease::{LeaseC2, LeaseC7},
        native::Nls,
    };

    use super::super::Group as LpnGroup;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
    #[serde(deny_unknown_fields, rename_all = "snake_case")]
    pub struct Lpn(CurrencyDTO<LpnGroup>);

    impl CurrencyDef for Lpn {
        type Group = LpnGroup;

        #[inline]
        fn definition() -> &'static Self {
            const {
                &Lpn(CurrencyDTO::new(
                    const { &Definition::new("LPN", "ibc/bank_LPN", "ibc/dex_LPN", 6) },
                ))
            }
        }

        #[inline]
        fn dto(&self) -> &CurrencyDTO<Self::Group> {
            &self.0
        }
    }

    impl InPoolWith<LeaseC2> for Lpn {}

    impl InPoolWith<LeaseC7> for Lpn {}

    impl InPoolWith<Nls> for Lpn {}
}
