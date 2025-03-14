use alloy::signers::local::LocalSignerError;
use sea_orm::DbErr;
use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Account not found")]
    NotFound,

    #[error(transparent)]
    Request(#[from] rquest::Error),

    #[error(transparent)]
    Common(#[from] common::Error),

    // external
    #[error(transparent)]
    LocalSigner(#[from] LocalSignerError),

    #[error(transparent)]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Db(#[from] DbErr),

    #[error(transparent)]
    Toml(#[from] toml::de::Error),
}
