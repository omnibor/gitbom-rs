use crate::embedding::EmbeddingMode;
use crate::embedding_mode::Mode;
use crate::hashes::SupportedHash;
use crate::storage::Storage;
use crate::target_type::BinaryType;
use crate::target_type::TargetType;
use crate::target_type::TextType;
use crate::ArtifactId;
use crate::Error;
use crate::InputManifest;
use crate::IntoArtifactId;
use crate::Relation;
use crate::RelationKind;
use crate::Result;
use std::collections::BTreeSet;
use std::fmt::Debug;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::fs::File;
use std::fs::OpenOptions;
use std::marker::PhantomData;
use std::path::Path;

/// An [`InputManifest`] builder.
pub struct InputManifestBuilder<H: SupportedHash, M: EmbeddingMode, S: Storage<H>> {
    /// The relations to be written to a new manifest by this transaction.
    relations: BTreeSet<Relation<H>>,

    /// Indicates whether manifests should be embedded in the artifact or not.
    mode: PhantomData<M>,

    /// The storage system used to store manifests.
    storage: S,
}

impl<H: SupportedHash, M: EmbeddingMode, S: Storage<H>> Debug for InputManifestBuilder<H, M, S> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("InputManifestBuilder")
            .field("mode", &M::mode())
            .field("relations", &self.relations)
            .finish_non_exhaustive()
    }
}

impl<H: SupportedHash, M: EmbeddingMode, S: Storage<H>> InputManifestBuilder<H, M, S> {
    /// Construct a new [`InputManifestBuilder`] with a specific type of storage.
    pub fn with_storage(storage: S) -> Self {
        Self {
            relations: BTreeSet::new(),
            mode: PhantomData,
            storage,
        }
    }

    /// Add a relation to an artifact to the transaction.
    pub fn add_relation(
        &mut self,
        kind: RelationKind,
        artifact: impl IntoArtifactId<H>,
    ) -> Result<&mut Self> {
        let artifact = artifact.into_artifact_id()?;

        let manifest = self.storage.get_manifest_id_for_artifact(artifact)?;

        self.relations
            .insert(Relation::new(kind, artifact, manifest));

        Ok(self)
    }

    #[cfg(feature = "builtin_embedding")]
    /// Build the input manifest with the built-in embedding.
    ///
    /// This method will try to detect the type of target being used.
    pub fn finish(&mut self, target: &Path) -> Result<TransactionIds<H>> {
        self.finish_with_optional_embedding(target, M::mode(), None)
    }

    /// Complete creation of a new [`InputManifest`], possibly embedding in the target.
    ///
    /// This is provided as a helper method which the two public methods call into
    /// because the logic is pretty specific in terms of order-of-operations, and
    /// _nearly_ the same except for the embedding choice.
    fn finish_with_optional_embedding(
        &mut self,
        target: &Path,
        embed_mode: Mode,
        embed_fn: Option<CustomEmbedding<H>>,
    ) -> Result<TransactionIds<H>> {
        // Construct a new input manifest.
        let mut manifest = InputManifest::with_relations(self.relations.iter().cloned());

        // Write the manifest to storage.
        let manifest_aid = self.storage.write_manifest(&manifest)?;

        // Get the ArtifactID of the target, possibly embedding the
        // manifest ArtifactID into the target first.
        let target_aid = match embed_mode {
            Mode::Embed => {
                let mut file = OpenOptions::new()
                    .read(true)
                    .write(true)
                    .open(target)
                    .map_err(|err| Error::CantOpenTargetForEditing {
                        path: target.to_owned(),
                        err,
                    })?;

                let embed_fn = embed_fn.unwrap_or(embed_manifest_in_target);
                embed_fn(target, &mut file, manifest_aid)?;

                ArtifactId::id_reader(file)?
            }
            Mode::NoEmbed => {
                let file = File::open(target).map_err(|err| Error::CantReadTarget {
                    path: target.to_owned(),
                    err,
                })?;
                ArtifactId::id_reader(file)?
            }
        };

        // Update the manifest in storage with the target ArtifactID.
        self.storage
            .update_target_for_manifest(manifest_aid, target_aid)?;

        // Update the manifest in memory with the target ArtifactID.
        manifest.set_target(Some(target_aid));

        // Clear out the set of relations so you can reuse the builder.
        self.relations.clear();

        Ok(TransactionIds {
            target_aid,
            manifest_aid,
            manifest,
        })
    }

    pub fn finish_with_custom_embedding(
        &mut self,
        target: &Path,
        embed_fn: CustomEmbedding<H>,
    ) -> Result<TransactionIds<H>> {
        self.finish_with_optional_embedding(target, M::mode(), Some(embed_fn))
    }
}

/// A custom embedding function.
pub type CustomEmbedding<H> = fn(&Path, &mut File, ArtifactId<H>) -> Result<ArtifactId<H>>;

pub struct TransactionIds<H: SupportedHash> {
    /// The ArtifactId of the target.
    target_aid: ArtifactId<H>,

    /// The ArtifactId of the manifest.
    manifest_aid: ArtifactId<H>,

    /// The manifest.
    manifest: InputManifest<H>,
}

impl<H: SupportedHash> Debug for TransactionIds<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("TransactionIds")
            .field("target_aid", &self.target_aid)
            .field("manifest_aid", &self.manifest_aid)
            .field("manifest", &self.manifest)
            .finish()
    }
}

impl<H: SupportedHash> TransactionIds<H> {
    /// Get the ArtifactId of the file targeted by the transaction.
    pub fn target_aid(&self) -> ArtifactId<H> {
        self.target_aid
    }

    /// Get the ArtifactId of the input manifest produced by the transaction.
    pub fn manifest_aid(&self) -> ArtifactId<H> {
        self.manifest_aid
    }

    /// Get the input manifest produced by the transaction.
    pub fn manifest(&self) -> &InputManifest<H> {
        &self.manifest
    }
}

/// Embed the manifest's [`ArtifactId`] into the target file.
fn embed_manifest_in_target<H: SupportedHash>(
    path: &Path,
    file: &mut File,
    manifest_aid: ArtifactId<H>,
) -> Result<ArtifactId<H>> {
    match TargetType::infer(path, file)? {
        TargetType::KnownBinaryType(BinaryType::ElfFile) => {
            embed_in_elf_file(path, file, manifest_aid)
        }
        TargetType::KnownTextType(TextType::PrefixComments { prefix }) => {
            embed_in_text_file_with_prefix_comment(path, file, manifest_aid, &prefix)
        }
        TargetType::KnownTextType(TextType::WrappedComments { prefix, suffix }) => {
            embed_in_text_file_with_wrapped_comment(path, file, manifest_aid, &prefix, &suffix)
        }
        TargetType::Unknown => Err(Error::UnknownEmbeddingTarget),
    }
}

fn embed_in_elf_file<H: SupportedHash>(
    _path: &Path,
    _file: &mut File,
    _manifest_aid: ArtifactId<H>,
) -> Result<ArtifactId<H>> {
    todo!("embedding mode for ELF files is not yet implemented")
}

fn embed_in_text_file_with_prefix_comment<H: SupportedHash>(
    _path: &Path,
    _file: &mut File,
    _manifest_aid: ArtifactId<H>,
    _prefix: &str,
) -> Result<ArtifactId<H>> {
    todo!("embedding mode for text files is not yet implemented")
}

fn embed_in_text_file_with_wrapped_comment<H: SupportedHash>(
    _path: &Path,
    _file: &mut File,
    _manifest_aid: ArtifactId<H>,
    _prefix: &str,
    _suffix: &str,
) -> Result<ArtifactId<H>> {
    todo!("embedding mode for text files is not yet implemented")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::embedding_mode::NoEmbed;
    use crate::hashes::Sha256;
    use crate::storage::{FileSystemStorage, InMemoryStorage};
    use pathbuf::pathbuf;
    use std::str::FromStr;

    /// A basic builder test that creates a single manifest and validates it.
    fn basic_builder_test(storage: impl Storage<Sha256>) {
        let target = pathbuf![
            env!("CARGO_MANIFEST_DIR"),
            "test",
            "data",
            "hello_world.txt"
        ];

        let first_input_aid = ArtifactId::id_str("test_1");
        let second_input_aid = ArtifactId::id_str("test_2");

        let expected_target_aid = ArtifactId::from_str(
            "gitoid:blob:sha256:fee53a18d32820613c0527aa79be5cb30173c823a9b448fa4817767cc84c6f03",
        )
        .unwrap();

        let expected_manifest_aid = ArtifactId::from_str(
            "gitoid:blob:sha256:2915dcbb3be71eb375246f716c4a851f0229e356747fe0e853158169c1f58ac7",
        )
        .unwrap();

        let ids = InputManifestBuilder::<Sha256, NoEmbed, _>::with_storage(storage)
            .add_relation(RelationKind::Input, first_input_aid)
            .unwrap()
            .add_relation(RelationKind::Input, second_input_aid)
            .unwrap()
            .finish(&target)
            .unwrap();

        // Check the ArtifactIDs of the target and the manifest.
        assert_eq!(ids.target_aid, expected_target_aid);
        assert_eq!(ids.manifest_aid, expected_manifest_aid);

        // Check the first relation in the manifest.
        let first_relation = &ids.manifest.relations()[0];
        assert_eq!(first_relation.artifact(), second_input_aid);
        assert_eq!(first_relation.kind(), RelationKind::Input);

        // Check the second relation in the manifest.
        let second_relation = &ids.manifest.relations()[1];
        assert_eq!(second_relation.artifact(), first_input_aid);
        assert_eq!(second_relation.kind(), RelationKind::Input);

        // Make sure we update the target in the manifest.
        assert_eq!(ids.manifest.target(), Some(ids.target_aid));
    }

    #[test]
    fn in_memory_builder_works() {
        let storage = InMemoryStorage::new();
        basic_builder_test(storage);
    }

    #[test]
    fn file_system_builder_works() {
        let storage_root = pathbuf![env!("CARGO_MANIFEST_DIR"), "test", "fs_storage"];
        let mut storage = FileSystemStorage::new(&storage_root).unwrap();
        basic_builder_test(&mut storage);
        storage.cleanup().unwrap();
    }
}
