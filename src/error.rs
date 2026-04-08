use std::path::PathBuf;

use thiserror::Error;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("missing genesis secret in environment variable {0}")]
    MissingGenesisSecret(String),
    #[error("invalid builder state: {0}")]
    InvalidBuilderState(&'static str),
    #[error("invalid protocol frame: {0}")]
    InvalidProtocolFrame(&'static str),
    #[error("database not found: {0}")]
    DatabaseNotFound(String),
    #[error("bootstrap metadata is missing but the ouroboros ring already contains data")]
    BootstrapMetadataMissing,
    #[error("invalid membrane record")]
    InvalidMembraneRecord,
    #[error("cell resolution loop detected starting at index {start_index}")]
    ResolutionLoop { start_index: u32 },
    #[error("invalid path for correspondence config: {0}")]
    InvalidPath(PathBuf),
    #[error(transparent)]
    Ouroboros(#[from] ouroboros::Error),
    #[error(transparent)]
    Redb(#[from] redb::Error),
    #[error(transparent)]
    RedbDatabase(#[from] redb::DatabaseError),
    #[error(transparent)]
    RedbTransaction(#[from] redb::TransactionError),
    #[error(transparent)]
    RedbTable(#[from] redb::TableError),
    #[error(transparent)]
    RedbCommit(#[from] redb::CommitError),
    #[error(transparent)]
    RedbStorage(#[from] redb::StorageError),
    #[error(transparent)]
    Io(#[from] std::io::Error),
}
