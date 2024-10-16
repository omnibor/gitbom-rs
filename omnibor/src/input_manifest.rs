//! [`InputManifest`] type that represents build inputs for an artifact.

use crate::hashes::SupportedHash;
use crate::ArtifactId;
use crate::Error;
use crate::Result;
use gitoid::Blob;
use gitoid::HashAlgorithm;
use gitoid::ObjectType;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::fs::File;
use std::io::BufRead;
use std::io::BufReader;
use std::io::Write;
use std::path::Path;
use std::str::FromStr;

/// A manifest describing the inputs used to build an artifact.
///
/// The manifest is constructed with a specific target artifact in mind.
/// The rest of the manifest then describes relations to other artifacts.
/// Each relation can be thought of as describing edges between nodes
/// in an Artifact Dependency Graph.
///
/// Each relation encodes a kind which describes how the other artifact
/// relates to the target artifact.
///
/// Relations may additionally refer to the [`InputManifest`] of the
/// related artifact.
#[derive(PartialEq, Eq)]
pub struct InputManifest<H: SupportedHash> {
    /// The artifact the manifest is describing.
    ///
    /// A manifest without this is "detached" because we don't know
    /// what artifact it's describing.
    target: Option<ArtifactId<H>>,

    /// The relations recorded in the manifest.
    relations: Vec<Relation<H>>,
}

impl<H: SupportedHash> InputManifest<H> {
    pub(crate) fn with_relations(relations: impl Iterator<Item = Relation<H>>) -> Self {
        InputManifest {
            target: None,
            relations: relations.collect(),
        }
    }

    /// Get the ID of the artifact this manifest is describing.
    ///
    /// If the manifest does not have a target, it is a "detached" manifest.
    ///
    /// Detached manifests may still be usable if the target artifact was
    /// created in embedding mode, in which case it will carry the [`ArtifactId`]
    /// of its own input manifest in its contents.
    #[inline]
    pub fn target(&self) -> Option<ArtifactId<H>> {
        self.target
    }

    /// Identify if the manifest is a "detached" manifest.
    ///
    /// "Detached" manifests are ones without a target [`ArtifactId`].
    pub fn is_detached(&self) -> bool {
        self.target.is_none()
    }

    /// Set a new target.
    pub(crate) fn set_target(&mut self, target: Option<ArtifactId<H>>) -> &mut Self {
        self.target = target;
        self
    }

    /// Get the relations inside an [`InputManifest`].
    #[inline]
    pub fn relations(&self) -> &[Relation<H>] {
        &self.relations[..]
    }

    /// Construct an [`InputManifest`] from a file at a specified path.
    pub fn from_path(path: &Path) -> Result<Self> {
        let file = BufReader::new(File::open(path).map_err(Error::FailedManifestRead)?);
        let mut lines = file.lines();

        let first_line = lines
            .next()
            .ok_or(Error::ManifestMissingHeader)?
            .map_err(Error::FailedManifestRead)?;

        let parts = first_line.split(':').collect::<Vec<_>>();

        if parts.len() != 3 {
            return Err(Error::MissingHeaderParts);
        }

        // Panic Safety: we've already checked the length.
        let (gitoid, blob, hash_algorithm) = (parts[0], parts[1], parts[2]);

        if gitoid != "gitoid" {
            return Err(Error::MissingGitOidInHeader);
        }

        if blob != "blob" {
            return Err(Error::MissingObjectTypeInHeader);
        }

        if hash_algorithm != H::HashAlgorithm::NAME {
            return Err(Error::WrongHashAlgorithm {
                expected: H::HashAlgorithm::NAME,
                got: hash_algorithm.to_owned(),
            });
        }

        let mut relations = Vec::new();
        for line in lines {
            let line = line.map_err(Error::FailedManifestRead)?;
            let relation = parse_relation::<H>(&line)?;
            relations.push(relation);
        }

        Ok(InputManifest {
            target: None,
            relations,
        })
    }

    /// Write the manifest out at the given path.
    #[allow(clippy::write_with_newline)]
    pub fn as_bytes(&self) -> Result<Vec<u8>> {
        let mut bytes = vec![];

        // Per the spec, this prefix is present to substantially shorten
        // the metadata info that would otherwise be attached to all IDs in
        // a manifest if they were written in full form. Instead, only the
        // hex-encoded hashes are recorded elsewhere, because all the metadata
        // is identical in a manifest and only recorded once at the beginning.
        write!(bytes, "gitoid:{}:{}\n", Blob::NAME, H::HashAlgorithm::NAME)
            .map_err(Error::FailedManifestWrite)?;

        for relation in &self.relations {
            let aid = relation.artifact;

            write!(bytes, "{} {}", relation.kind, aid.as_hex())
                .map_err(Error::FailedManifestWrite)?;

            if let Some(mid) = relation.manifest {
                write!(bytes, " bom {}", mid.as_hex()).map_err(Error::FailedManifestWrite)?;
            }

            write!(bytes, "\n").map_err(Error::FailedManifestWrite)?;
        }

        Ok(bytes)
    }
}

impl<H: SupportedHash> Debug for InputManifest<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("InputManifest")
            .field("target", &self.target)
            .field("relations", &self.relations)
            .finish()
    }
}

impl<H: SupportedHash> Clone for InputManifest<H> {
    fn clone(&self) -> Self {
        InputManifest {
            target: self.target,
            relations: self.relations.clone(),
        }
    }
}

/// Parse a single relation line.
fn parse_relation<H: SupportedHash>(input: &str) -> Result<Relation<H>> {
    let parts = input.split(' ').collect::<Vec<_>>();

    if parts.len() < 3 {
        return Err(Error::MissingRelationParts);
    }

    // Panic Safety: we've already checked the length.
    let (kind, aid_hex, bom_indicator, manifest_aid_hex) =
        (parts[0], parts[1], parts.get(2), parts.get(3));

    let kind = RelationKind::from_str(kind)?;

    let artifact = ArtifactId::<H>::from_str(&format!(
        "gitoid:{}:{}:{}",
        Blob::NAME,
        H::HashAlgorithm::NAME,
        aid_hex
    ))?;

    let manifest = match (bom_indicator, manifest_aid_hex) {
        (None, None) | (Some(_), None) | (None, Some(_)) => None,
        (Some(bom_indicator), Some(manifest_aid_hex)) => {
            if *bom_indicator != "bom" {
                return Err(Error::MissingBomIndicatorInRelation);
            }

            let gitoid_url = &format!(
                "gitoid:{}:{}:{}",
                Blob::NAME,
                H::HashAlgorithm::NAME,
                manifest_aid_hex
            );

            ArtifactId::<H>::from_str(gitoid_url).ok()
        }
    };

    Ok(Relation {
        kind,
        artifact,
        manifest,
    })
}

/// A single input artifact represented in a [`InputManifest`].
#[derive(Copy)]
pub struct Relation<H: SupportedHash> {
    /// The kind of relation being represented.
    kind: RelationKind,

    /// The ID of the artifact itself.
    artifact: ArtifactId<H>,

    /// The ID of the manifest, if a manifest is present.
    manifest: Option<ArtifactId<H>>,
}

// We implement this ourselves instead of deriving it because
// the auto-derive logic will only conditionally derive it based
// on whether the `H` type parameter implements `Debug`, which
// isn't actually relevant in this case because we don't _really_
// store a value of type-`H`, we just use it for type-level
// programming.
impl<H: SupportedHash> Debug for Relation<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("Relation")
            .field("kind", &self.kind)
            .field("artifact", &self.artifact)
            .field("manifest", &self.manifest)
            .finish()
    }
}

impl<H: SupportedHash> Clone for Relation<H> {
    fn clone(&self) -> Self {
        Relation {
            kind: self.kind,
            artifact: self.artifact,
            manifest: self.manifest,
        }
    }
}

impl<H: SupportedHash> PartialEq for Relation<H> {
    fn eq(&self, other: &Self) -> bool {
        self.kind.eq(&other.kind)
            && self.artifact.eq(&other.artifact)
            && self.manifest.eq(&other.manifest)
    }
}

impl<H: SupportedHash> Eq for Relation<H> {}

impl<H: SupportedHash> PartialOrd for Relation<H> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<H: SupportedHash> Ord for Relation<H> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.kind
            .cmp(&other.kind)
            .then_with(|| self.artifact.cmp(&other.artifact))
    }
}

impl<H: SupportedHash> Relation<H> {
    pub(crate) fn new(
        kind: RelationKind,
        artifact: ArtifactId<H>,
        manifest: Option<ArtifactId<H>>,
    ) -> Relation<H> {
        Relation {
            kind,
            artifact,
            manifest,
        }
    }

    /// Get the kind of relation being described.
    #[inline]
    pub fn kind(&self) -> RelationKind {
        self.kind
    }

    /// Get the ID of the artifact.
    #[inline]
    pub fn artifact(&self) -> ArtifactId<H> {
        self.artifact
    }

    /// Get the manifest ID, if present.
    #[inline]
    pub fn manifest(&self) -> Option<ArtifactId<H>> {
        self.manifest
    }
}

/// Describes the relationship between an [`InputManifest`]'s target artifact and other artifacts.
#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum RelationKind {
    /// Is a build input for the target artifact.
    Input,

    /// Is a tool used to build the target artifact.
    BuiltBy,
}

impl Display for RelationKind {
    #[inline]
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        match self {
            RelationKind::Input => write!(f, "input"),
            RelationKind::BuiltBy => write!(f, "built-by"),
        }
    }
}

impl FromStr for RelationKind {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "input" => Ok(RelationKind::Input),
            "built-by" => Ok(RelationKind::BuiltBy),
            _ => Err(Error::InvalidRelationKind(s.to_owned())),
        }
    }
}
