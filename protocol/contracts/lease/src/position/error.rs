use std::result::Result as StdResult;

use thiserror::Error;

use finance::{error::Error as FinanceError, percent::Percent};

use crate::finance::LpnCoinDTO;

use super::CloseStrategy;

#[derive(Error, Debug, PartialEq)]
pub enum Error {
    #[error("[Position] {0}")]
    Finance(#[from] FinanceError),

    #[error("[Position] The asset amount should worth at least {0}")]
    InsufficientAssetAmount(LpnCoinDTO),

    #[error("[Position] The transaction amount should worth at least {0}")]
    InsufficientTransactionAmount(LpnCoinDTO),

    #[error("[Position] The position close amount should worth at least {0}")]
    PositionCloseAmountTooSmall(LpnCoinDTO),

    #[error("[Position] The position past this close should worth at least {0}")]
    PositionCloseAmountTooBig(LpnCoinDTO),

    #[error("[Position] Invalid close policy! The current lease LTV '{lease_ltv}' would trigger '{strategy}'!")]
    TriggerClose {
        lease_ltv: Percent,
        strategy: CloseStrategy,
    },

    #[error("[Position] The close policy '{0}' should not be zero!")]
    ZeroClosePolicy(&'static str),

    #[error("[Position] Invalid close policy! The new strategy '{strategy}' is not less than the max lease liability LTV '{top_bound}'!")]
    LiquidationConflict {
        strategy: CloseStrategy,
        top_bound: Percent,
    },
}

impl Error {
    pub fn trigger_close(lease_ltv: Percent, strategy: CloseStrategy) -> Self {
        Self::TriggerClose {
            lease_ltv,
            strategy,
        }
    }

    pub fn zero_take_profit() -> Self {
        Self::ZeroClosePolicy("take profit")
    }

    pub fn zero_stop_loss() -> Self {
        Self::ZeroClosePolicy("stop loss")
    }

    pub fn liquidation_conflict(liquidation_ltv: Percent, strategy: CloseStrategy) -> Self {
        Self::LiquidationConflict {
            top_bound: liquidation_ltv,
            strategy,
        }
    }
}

pub type Result<T> = StdResult<T, Error>;
