//! OmniBOR Artifact Identifiers and Artifact Input Manifests in Rust.
//!
//! ## What is OmniBOR?
//!
//! [OmniBOR][omnibor] is a draft specification which defines two key concepts:
//!
//! - __Artifact Identifiers__: independently-reproducible identifiers for
//!   software artifacts. Use [`ArtifactId`] for these.
//! - __Artifact Input Manifests__: record the IDs of every input used in the
//!   build process for an artifact. Use [`InputManifest`] for these.
//!
//! Artifact IDs enable _anyone_ to identify and cross-reference information for
//! software artifacts without a central authority. Unlike [pURL][purl] or [CPE][cpe],
//! OmniBOR Artifact IDs don't rely on a third-party, they are _inherent
//! identifiers_ determined only by an artifact itself. They're based on
//! [Git Object Identifiers (GitOIDs)][gitoid] in both construction and choice of
//! cryptographic hash functions.
//!
//! Artifact Input Manifests allow consumers to reconstruct Artifact Dependency
//! Graphs that give _fine-grained_ visibility into how artifacts in their
//! software supply chain were made. With these graphs, consumers could
//! in the future identify the presence of exact files associated with known
//! vulnerabilities, side-stepping the complexities of matching version numbers
//! across platforms and patching practices.
//!
//! [__You can view the OmniBOR specification here.__][omnibor_spec]
//!
//! The United States Cybersecurity & Infrastructure Security Agency (CISA)
//! identified OmniBOR as a major candidate for software identities
//! in its 2023 report ["Software Identification Ecosystem Option
//! Analysis."][cisa_report]
//!
//! [contributing]: CONTRIBUTING.md
//! [cbindgen]: https://github.com/eqrion/cbindgen
//! [cisa_report]: https://www.cisa.gov/sites/default/files/2023-10/Software-Identification-Ecosystem-Option-Analysis-508c.pdf
//! [cpe]: https://nvd.nist.gov/products/cpe
//! [gitoid]: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
//! [gitoid_crate]: https://crates.io/crates/gitoid
//! [omnibor]: https://omnibor.io
//! [omnibor_crate]: https://crates.io/crates/omnibor
//! [omnibor_spec]: https://github.com/omnibor/spec
//! [purl]: https://github.com/package-url/purl-spec

// Make this public within the crate to aid with writing sealed
// traits, a pattern we use repeatedly.
pub(crate) mod sealed;

// This is hidden for now, as we are not yet ready to commit to any
// stability guarantees for FFI.
#[doc(hidden)]
pub mod ffi;

// Keep modules private and just re-export the symbols we care about.
mod artifact_id;
mod embedding_mode;
mod error;
mod input_manifest;
mod input_manifest_builder;
mod into_artifact_id;
pub mod storage;
mod supported_hash;
mod target_type;

#[cfg(test)]
mod test;

// Only make this public within the crate, for convenience
// elsewhere since we always expect to be using our own `Error`
// type anyway.
pub(crate) use crate::error::Result;

/// Defines whether data for an [`InputManifest`] is embedded in the artifact itself.
pub mod embedding {
    pub use crate::embedding_mode::Embed;
    pub use crate::embedding_mode::EmbeddingMode;
    pub use crate::embedding_mode::NoEmbed;
}

/// Defines the hash algorithms supported for [`ArtifactId`]s.
pub mod hashes {
    pub use crate::supported_hash::Sha256;
    pub use crate::supported_hash::SupportedHash;
}

pub use crate::artifact_id::ArtifactId;
pub use crate::error::Error;
pub use crate::input_manifest::InputManifest;
pub use crate::input_manifest::Relation;
pub use crate::input_manifest::RelationKind;
pub use crate::input_manifest_builder::CustomEmbedding;
pub use crate::input_manifest_builder::InputManifestBuilder;
pub use crate::into_artifact_id::IntoArtifactId;
