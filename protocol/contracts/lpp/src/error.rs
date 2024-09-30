use thiserror::Error;

use sdk::cosmwasm_std::StdError;

#[derive(Error, Debug, PartialEq)]
pub enum ContractError {
    #[error("[Lpp] [Std] {0}")]
    Std(#[from] StdError),

    #[error("[Lpp] Failed to convert query response to binary! Cause: {0}")]
    ConvertToBinary(StdError),

    #[error("[Lpp] {0}")]
    Currency(#[from] currency::error::Error),

    #[error("[Lpp] Invalid Oracle Base currency! Cause: {0}")]
    InvalidOracleBaseCurrency(oracle_platform::error::Error),

    #[error("[Lpp] Failure converting from the quote currency! Cause: {0}")]
    ConvertFromQuote(oracle_platform::error::Error),

    #[error("[Lpp] {0}")]
    Finance(#[from] finance::error::Error),

    #[error("[Lpp] {0}")]
    Platform(#[from] platform::error::Error),

    #[error("[Lpp] {0}")]
    Unauthorized(#[from] access_control::error::Error),

    #[error("[Lpp] Unknown currency, details '{0}'")]
    UnknownCurrency(currency::error::Error),

    #[error("[Lpp] No liquidity")]
    NoLiquidity {},

    #[error("[Lpp] The loan exists")]
    LoanExists {},

    #[error("[Lpp] The loan does not exist")]
    NoLoan {},

    #[error("[Lpp] The deposit does not exist")]
    NoDeposit {},

    #[error("[Lpp] Zero loan amount")]
    ZeroLoanAmount,

    #[error("[Lpp] Zero deposit")]
    ZeroDepositFunds,

    #[error("[Lpp] Zero withdraw amount")]
    ZeroWithdrawFunds,

    #[error("[Lpp] No pending rewards")]
    NoRewards {},

    #[error("[Lpp] Zero rewards")]
    ZeroRewardsFunds {},

    #[error("[Lpp] Distribute rewards with zero balance nlpn")]
    ZeroBalanceRewards {},

    #[error("[Lpp] Lpp requires single currency")]
    FundsLen {},

    #[error("[Lpp] Insufficient balance")]
    InsufficientBalance,

    #[error("[Lpp] Utilization is below the set minimal rate")]
    UtilizationBelowMinimalRates,

    #[error("[Lpp Stub] No response sent back from LPP contract")]
    NoResponseStubError,
}

pub type Result<T> = std::result::Result<T, ContractError>;
