#[cfg(doc)]
use crate::ArtifactId;
#[cfg(doc)]
use crate::InputManifest;
use gitoid::Error as GitOidError;
use std::io::Error as IoError;
use std::path::PathBuf;
use std::result::Result as StdResult;
use url::ParseError as UrlError;

pub type Result<T> = StdResult<T, Error>;

/// Errors arising from [`ArtifactId`] use or [`InputManifest`] interaction.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no storage root found; provide one or set the 'OMNIBOR_DIR' environment variable")]
    NoStorageRoot,

    #[error("unable to access file system storage root '{0}'; please check permissions")]
    CantAccessRoot(String, #[source] IoError),

    #[error("object store '{0}' is not a directory")]
    ObjectStoreNotDir(String),

    #[error("'{0}' is not a valid object store path")]
    InvalidObjectStorePath(String),

    #[error("object store '{0}' is not empty")]
    ObjectStoreDirNotEmpty(String),

    #[error("can't create object store '{0}'")]
    CantCreateObjectStoreDir(String, #[source] IoError),

    #[error("can't write manifest directory '{0}'")]
    CantWriteManifestDir(String, #[source] IoError),

    #[error("can't create target index '{0}'")]
    CantCreateTargetIndex(PathBuf, #[source] IoError),

    #[error("can't open target index file '{0}'")]
    CantOpenTargetIndex(String, #[source] IoError),

    #[error("can't open target index temp file for upsert '{0}'")]
    CantOpenTargetIndexTemp(String, #[source] IoError),

    #[error("can't delete target index temp file for upsert '{0}'")]
    CantDeleteTargetIndexTemp(String, #[source] IoError),

    #[error("can't write manifest file '{0}'")]
    CantWriteManifest(String, #[source] IoError),

    #[error("the target index file has been corrupted and can't be parsed")]
    CorruptedTargetIndex,

    #[error("the target index file has been corrupted and can't be parsed")]
    CorruptedTargetIndexIoReason(#[source] IoError),

    #[error("the target index file has been corrupted and can't be parsed")]
    CorruptedTargetIndexOmniBorReason(#[source] Box<Error>),

    #[error("missing manifest_aid or target_aid from target index upsert operation")]
    InvalidTargetIndexUpsert,

    #[error("can't upsert the target index")]
    CantUpsertTargetIndex(#[source] IoError),

    #[error("invalid relation kind in input manifest: '{0}'")]
    InvalidRelationKind(String),

    #[error("input manifest missing header line")]
    ManifestMissingHeader,

    #[error("missing 'gitoid' in manifest header")]
    MissingGitOidInHeader,

    #[error("missing object type 'blob' in manifest header")]
    MissingObjectTypeInHeader,

    #[error("missing object type 'blob' in manifest relation")]
    MissingObjectTypeInRelation,

    #[error("missing one or more header parts")]
    MissingHeaderParts,

    #[error("missing bom indicator in relation")]
    MissingBomIndicatorInRelation,

    #[error("missing one or more relation parts")]
    MissingRelationParts,

    #[error("wrong hash algorithm; expected '{expected}', got '{got}'")]
    WrongHashAlgorithm { expected: &'static str, got: String },

    #[error("missing manifest-for entry in manifest")]
    MissingManifestForRelation,

    #[error("the transaction to make an input manifest was already closed")]
    TransactionClosed,

    #[error("unknown file type for manifest ID embedding")]
    UnknownEmbeddingTarget,

    #[error("unable to detect a language for '{0}'")]
    CantDetectLanguage(PathBuf),

    #[error("detected a binary type not supported for embedding '{0}'")]
    UnsupportedBinaryType(String),

    #[error("failed to read input manifest file")]
    FailedManifestRead(#[source] IoError),

    #[error("failed to write manifest file")]
    FailedManifestWrite(#[source] IoError),

    #[error("failed to open target artifact for editing")]
    CantOpenTargetForEditing {
        path: PathBuf,
        #[source]
        err: IoError,
    },

    #[error("failed to open target for reading")]
    CantReadTarget {
        path: PathBuf,
        #[source]
        err: IoError,
    },

    #[error("can't clean the object store root")]
    CantCleanRoot {
        path: PathBuf,
        #[source]
        err: IoError,
    },

    #[error(transparent)]
    GitOid(#[from] GitOidError),

    #[error(transparent)]
    Url(#[from] UrlError),
}
