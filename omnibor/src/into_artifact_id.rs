use crate::supported_hash::SupportedHash;
#[cfg(doc)]
use crate::InputManifestBuilder;
use crate::Result;
use crate::{ArtifactId, Error};
use std::fs::File;
use std::io::BufReader;
use std::path::Path;

/// Types that can produce an [`ArtifactId`].
///
/// This is a convenience trait used by [`InputManifestBuilder`] to provide a more
/// ergonomic API for constructing input manifests.
pub trait IntoArtifactId<H: SupportedHash> {
    /// Produce an [`ArtifactId`] from the current type.
    fn into_artifact_id(self) -> Result<ArtifactId<H>>;
}

impl<H: SupportedHash> IntoArtifactId<H> for ArtifactId<H> {
    fn into_artifact_id(self) -> Result<ArtifactId<H>> {
        Ok(self)
    }
}

impl<H: SupportedHash> IntoArtifactId<H> for &Path {
    fn into_artifact_id(self) -> Result<ArtifactId<H>> {
        File::open(self)
            .map_err(|err| Error::CantReadTarget {
                path: self.to_owned(),
                err,
            })?
            .into_artifact_id()
    }
}

impl<H: SupportedHash> IntoArtifactId<H> for File {
    fn into_artifact_id(self) -> Result<ArtifactId<H>> {
        let file = BufReader::new(self);
        ArtifactId::id_reader(file)
    }
}

impl<H: SupportedHash> IntoArtifactId<H> for &[u8] {
    fn into_artifact_id(self) -> Result<ArtifactId<H>> {
        Ok(ArtifactId::id_bytes(self))
    }
}
