use thiserror::Error as ThisError;

use platform::contract::CodeId;
use sdk::cosmwasm_std::{Addr, StdError};

#[derive(Debug, ThisError)]
pub enum Error {
    #[error("[Admin] [Std] {0}")]
    StdError(#[from] StdError),
    #[error("[Admin] [Std] [Instantiate2] {0}")]
    StdInstantiate2Addr(#[from] sdk::cosmwasm_std::Instantiate2AddressError),
    #[error("[Admin] {0}")]
    AccessControl(#[from] access_control::error::Error),
    #[error("[Admin] {0}")]
    Platform(#[from] platform::error::Error),
    #[error("No data in migration response!")]
    NoMigrationResponseData {},
    #[error("Contract returned wrong release string! \"{reported}\" was returned, but \"{expected}\" was expected!")]
    WrongRelease { reported: String, expected: String },
    #[error("Contract returned wrong address! Expected \"{expected}\", but got \"{reported}\"!")]
    DifferentInstantiatedAddress { reported: Addr, expected: Addr },
    #[error("Contract returned wrong code id! Expected \"{expected}\", but got \"{reported}\"!")]
    DifferentInstantiatedCodeId { reported: CodeId, expected: CodeId },
    #[error("Protocol not mentioned under either migration messages, or post-migration execution messages! Protocol's friendly name: {0}")]
    MissingProtocol(String),
    #[error(
        "Protocol set of contracts already exists for this protocol name! Protocol's friendly name: {0}"
    )]
    ProtocolSetAlreadyExists(String),
    #[error(
        "Protocol set doesn't contain a protocol with this protocol name! Protocol's friendly name: {0}"
    )]
    ProtocolDoesNotExists(String),
    #[error(
        "No protocol set of contracts exists for this protocol name! Protocol's friendly name: {0}"
    )]
    UnknownProtocol(String),
}
