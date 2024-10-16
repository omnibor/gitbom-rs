//! Defines how manifests are stored and accessed.

use crate::hashes::SupportedHash;
use crate::supported_hash::Sha256;
use crate::ArtifactId;
use crate::Error;
use crate::InputManifest;
use crate::Result;
use pathbuf::pathbuf;
use std::collections::HashMap;
use std::env::var_os;
use std::fmt::Debug;
use std::fs;
use std::fs::create_dir_all;
use std::fs::write;
use std::fs::File;
use std::io::BufRead as _;
use std::io::BufReader;
use std::io::BufWriter;
use std::io::Write as _;
use std::ops::Not as _;
use std::path::Path;
use std::path::PathBuf;
use std::str::FromStr;
use walkdir::DirEntry;
use walkdir::WalkDir;

/// Represents the interface for storing and querying manifests.
pub trait Storage<H: SupportedHash> {
    /// Check if we have the manifest for a specific artifact.
    fn has_manifest_for_artifact(&self, target_aid: ArtifactId<H>) -> bool;

    /// Get the manifest for a specific artifact.
    fn get_manifest_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<InputManifest<H>>>;

    /// Get the ID of the manifest for the artifact.
    fn get_manifest_id_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<ArtifactId<H>>>;

    /// Write a manifest to the storage.
    fn write_manifest(&mut self, manifest: &InputManifest<H>) -> Result<ArtifactId<H>>;

    /// Update the manifest file to reflect the target ID.
    fn update_target_for_manifest(
        &mut self,
        manifest_aid: ArtifactId<H>,
        target_aid: ArtifactId<H>,
    ) -> Result<()>;

    /// Get all manifests from the storage.
    fn get_manifests(&self) -> Result<Vec<InputManifest<H>>>;
}

impl<H: SupportedHash, S: Storage<H>> Storage<H> for &mut S {
    fn has_manifest_for_artifact(&self, target_aid: ArtifactId<H>) -> bool {
        (**self).has_manifest_for_artifact(target_aid)
    }

    fn get_manifest_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<InputManifest<H>>> {
        (**self).get_manifest_for_artifact(target_aid)
    }

    fn get_manifest_id_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<ArtifactId<H>>> {
        (**self).get_manifest_id_for_artifact(target_aid)
    }

    fn write_manifest(&mut self, manifest: &InputManifest<H>) -> Result<ArtifactId<H>> {
        (**self).write_manifest(manifest)
    }

    fn update_target_for_manifest(
        &mut self,
        manifest_aid: ArtifactId<H>,
        target_aid: ArtifactId<H>,
    ) -> Result<()> {
        (**self).update_target_for_manifest(manifest_aid, target_aid)
    }

    fn get_manifests(&self) -> Result<Vec<InputManifest<H>>> {
        (**self).get_manifests()
    }
}

/// File system storage for [`InputManifest`]s.
#[derive(Debug)]
pub struct FileSystemStorage {
    root: PathBuf,
}

impl FileSystemStorage {
    /// Start building a new [`FileSystemStorage`].
    pub fn new(root: impl AsRef<Path>) -> Result<FileSystemStorage> {
        let root = root.as_ref().to_owned();

        if root.exists() {
            let meta = fs::metadata(&root)
                .map_err(|e| Error::CantAccessRoot(root.display().to_string(), e))?;

            if meta.is_dir().not() {
                return Err(Error::ObjectStoreNotDir(root.display().to_string()));
            }
        } else {
            create_dir_all(&root)
                .map_err(|e| Error::CantCreateObjectStoreDir(root.display().to_string(), e))?;
        }

        Ok(FileSystemStorage { root })
    }

    /// Build a [`FileSystemStorage`] with a root set from
    /// the `OMNIBOR_DIR` environment variable.
    pub fn from_env() -> Result<FileSystemStorage> {
        var_os("OMNIBOR_DIR")
            .ok_or(Error::NoStorageRoot)
            .map(|root| FileSystemStorage {
                root: PathBuf::from(root),
            })
    }

    /// Fully delete the contents of the root dir.
    ///
    /// This is just used for tests to ensure idempotency.
    #[cfg(test)]
    pub fn cleanup(self) -> Result<()> {
        fs::remove_dir_all(&self.root).map_err(|err| Error::CantCleanRoot {
            path: self.root.clone(),
            err,
        })?;
        fs::create_dir_all(&self.root)
            .map_err(|err| Error::CantCreateObjectStoreDir(self.root.display().to_string(), err))?;
        Ok(())
    }

    /// Get the path to the manifest store.
    fn manifests_path(&self) -> PathBuf {
        pathbuf![&self.root, "manifests"]
    }

    /// Get the path to the target index file.
    fn target_file_path(&self) -> PathBuf {
        pathbuf![&self.root, "targets"]
    }

    /// Open the target index file
    fn target_index(&self) -> Result<TargetIndex> {
        TargetIndex::new(self.target_file_path())
    }

    /// Get the path for storing a manifest with this [`ArtifactId`].
    fn manifest_path<H: SupportedHash>(&self, aid: ArtifactId<H>) -> PathBuf {
        let kind = format!("gitoid_{}_{}", aid.object_type(), aid.hash_algorithm());
        let hash = aid.as_hex();
        let (prefix, remainder) = hash.split_at(2);
        pathbuf![&self.manifests_path(), &kind, prefix, remainder]
    }

    /// Iterate over the targets of manifests currently in the object store.
    fn manifests<H: SupportedHash>(&self) -> impl Iterator<Item = ManifestsEntry<H>> + '_ {
        WalkDir::new(self.manifests_path())
            .into_iter()
            .filter_map(|result| result.ok())
            .filter(|entry| entry.file_type().is_file())
            .filter(|entry| entry.file_name().to_str().is_some())
            .filter_map(|entry| {
                let manifest_aid = artifact_id_from_dir_entry(&entry)?;
                let target_aid = self.target_index().ok()?.find(manifest_aid).ok().flatten();
                let manifest_path = entry.path().to_owned();

                Some(ManifestsEntry {
                    target_aid,
                    manifest_path,
                })
            })
    }
}

impl<H: SupportedHash> Storage<H> for FileSystemStorage {
    fn has_manifest_for_artifact(&self, target_aid: ArtifactId<H>) -> bool {
        self.manifests()
            .any(|entry| entry.target_aid == Some(target_aid))
    }

    fn get_manifest_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<InputManifest<H>>> {
        match self
            .manifests()
            .find(|entry| entry.target_aid == Some(target_aid))
        {
            Some(entry) => entry.manifest().map(Some),
            None => Ok(None),
        }
    }

    fn get_manifest_id_for_artifact(
        &self,
        target_aid: ArtifactId<H>,
    ) -> Result<Option<ArtifactId<H>>> {
        match self.get_manifest_for_artifact(target_aid) {
            Ok(Some(manifest)) => ArtifactId::id_manifest(&manifest).map(Some),
            Ok(None) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn write_manifest(&mut self, manifest: &InputManifest<H>) -> Result<ArtifactId<H>> {
        let manifest_aid = ArtifactId::<H>::id_manifest(manifest)?;
        let path = self.manifest_path(manifest_aid);
        let parent_dirs = path
            .parent()
            .ok_or_else(|| Error::InvalidObjectStorePath(path.display().to_string()))?;

        create_dir_all(parent_dirs)
            .map_err(|e| Error::CantWriteManifestDir(parent_dirs.display().to_string(), e))?;

        write(&path, manifest.as_bytes()?)
            .map_err(|e| Error::CantWriteManifest(path.display().to_string(), e))?;

        Ok(manifest_aid)
    }

    fn update_target_for_manifest(
        &mut self,
        manifest_aid: ArtifactId<H>,
        target_aid: ArtifactId<H>,
    ) -> Result<()> {
        self.target_index()?
            .upsert()
            .manifest_aid(manifest_aid)
            .target_aid(target_aid)
            .run()
    }

    fn get_manifests(&self) -> Result<Vec<InputManifest<H>>> {
        self.manifests()
            .map(|entry: ManifestsEntry<H>| InputManifest::from_path(&entry.manifest_path))
            .collect()
    }
}

fn artifact_id_from_dir_entry<H: SupportedHash>(entry: &DirEntry) -> Option<ArtifactId<H>> {
    let gitoid_url = {
        let path_components = entry
            .path()
            .components()
            .map(|comp| comp.as_os_str().to_str())
            .collect::<Option<Vec<_>>>()?;

        let n_components = path_components.len();
        let remainder = path_components.get(n_components - 1)?;
        let prefix = path_components.get(n_components - 2)?;
        let meta = path_components.get(n_components - 3)?;
        let hash = format!("{}{}", prefix, remainder);
        let front = meta.replace('_', ":");
        format!("{}:{}", front, hash)
    };

    ArtifactId::<H>::from_str(&gitoid_url).ok()
}

/// An entry when iterating over manifests in the manifest store.
struct ManifestsEntry<H: SupportedHash> {
    /// The [`ArtifactId`] of the target artifact.
    target_aid: Option<ArtifactId<H>>,

    /// The path to the manifest in the manifest store.
    manifest_path: PathBuf,
}

impl<H: SupportedHash> ManifestsEntry<H> {
    /// Load the [`InputManifest`] represented by this entry.
    fn manifest(&self) -> Result<InputManifest<H>> {
        let mut manifest = InputManifest::from_path(&self.manifest_path)?;
        manifest.set_target(self.target_aid);
        Ok(manifest)
    }
}

/// Represents the target index file on disk.
struct TargetIndex {
    path: PathBuf,
}

impl TargetIndex {
    /// Create a new [`TargetIndex`]
    fn new(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();

        if path.exists().not() {
            File::create_new(path)
                .map_err(|err| Error::CantCreateTargetIndex(path.to_owned(), err))?;
        }

        Ok(TargetIndex {
            path: path.to_owned(),
        })
    }

    /// Find an entry for a specific manifest [`ArtifactId`].
    fn find<H: SupportedHash>(&self, manifest_aid: ArtifactId<H>) -> Result<Option<ArtifactId<H>>> {
        let file = File::open(&self.path)
            .map_err(|e| Error::CantOpenTargetIndex(self.path.display().to_string(), e))?;

        let reader = BufReader::new(&file);

        for line in reader.lines() {
            let line = line.map_err(Error::CorruptedTargetIndexIoReason)?;

            let (line_manifest_aid, line_target_aid) = match line.split_once(' ') {
                Some(pair) => pair,
                None => return Err(Error::CorruptedTargetIndex),
            };

            let line_manifest_aid = ArtifactId::from_str(line_manifest_aid)?;

            if line_manifest_aid == manifest_aid {
                return Ok(ArtifactId::from_str(line_target_aid).ok());
            }
        }

        Ok(None)
    }

    // Begin an "upsert" operation in the [`TargetIndex`].
    //
    // This either updates or inserts, as appropriate, into the index.
    fn upsert<H: SupportedHash>(&self) -> TargetIndexUpsert<H> {
        let root = self.path.parent().unwrap();
        TargetIndexUpsert::new(root)
    }
}

struct TargetIndexUpsert<H: SupportedHash> {
    root: PathBuf,
    manifest_aid: Option<ArtifactId<H>>,
    target_aid: Option<ArtifactId<H>>,
}

impl<H: SupportedHash> TargetIndexUpsert<H> {
    /// Start a new upsert operation.
    fn new(root: impl AsRef<Path>) -> Self {
        TargetIndexUpsert {
            root: root.as_ref().to_owned(),
            manifest_aid: None,
            target_aid: None,
        }
    }

    /// Set the manifest [`ArtifactId`] for the upsert.
    fn manifest_aid(mut self, manifest_aid: ArtifactId<H>) -> Self {
        self.manifest_aid = Some(manifest_aid);
        self
    }

    /// Set the target [`ArtifactId`] for the upsert.
    fn target_aid(mut self, target_aid: ArtifactId<H>) -> Self {
        self.target_aid = Some(target_aid);
        self
    }

    /// Get the path to a temporary file used during upserting.
    fn tempfile(&self) -> PathBuf {
        pathbuf![&self.root, "targets.temp"]
    }

    fn target_file(&self) -> PathBuf {
        pathbuf![&self.root, "targets"]
    }

    /// Run the upsert operation.
    fn run(self) -> Result<()> {
        let manifest_aid = self.manifest_aid.ok_or(Error::InvalidTargetIndexUpsert)?;
        let target_aid = self.target_aid.ok_or(Error::InvalidTargetIndexUpsert)?;

        let file = File::open(self.target_file())
            .map_err(|e| Error::CantOpenTargetIndex(self.target_file().display().to_string(), e))?;

        // Read the current target index from disk.
        let mut target_index = HashMap::new();

        for line in BufReader::new(file).lines() {
            let line = line.map_err(Error::CorruptedTargetIndexIoReason)?;

            let (line_manifest_aid, line_target_aid) =
                line.split_once(' ').ok_or(Error::CorruptedTargetIndex)?;

            let line_manifest_aid = ArtifactId::from_str(line_manifest_aid)
                .map_err(|e| Error::CorruptedTargetIndexOmniBorReason(Box::new(e)))?;

            let line_target_aid = ArtifactId::from_str(line_target_aid)
                .map_err(|e| Error::CorruptedTargetIndexOmniBorReason(Box::new(e)))?;

            target_index.insert(line_manifest_aid, line_target_aid);
        }

        // Update the index in memory.
        target_index
            .entry(manifest_aid)
            .and_modify(|old_target_aid| *old_target_aid = target_aid)
            .or_insert(target_aid);

        // Write out updated index to a tempfile.
        let mut tempfile = File::create(self.tempfile()).map_err(|e| {
            Error::CantOpenTargetIndexTemp(self.tempfile().display().to_string(), e)
        })?;

        let mut writer = BufWriter::new(&mut tempfile);
        for (manifest_aid, target_aid) in target_index {
            if let Err(e) = writeln!(writer, "{} {}", manifest_aid, target_aid) {
                fs::remove_file(self.tempfile()).map_err(|e| {
                    Error::CantDeleteTargetIndexTemp(self.tempfile().display().to_string(), e)
                })?;
                return Err(Error::CantUpsertTargetIndex(e));
            }
        }

        // Replace the prior index with the new one.
        if let Err(e) = fs::rename(self.tempfile(), self.target_file()) {
            fs::remove_dir(self.tempfile()).map_err(|e| {
                Error::CantDeleteTargetIndexTemp(self.tempfile().display().to_string(), e)
            })?;
            return Err(Error::CantUpsertTargetIndex(e));
        }

        Ok(())
    }
}

/// In-memory storage for [`InputManifest`]s.
///
/// Note that this "storage" doesn't persist anything. We use it for testing, and it
/// may be useful in other applications where you only care about producing and using
/// manifests in the short-term, and not in persisting them to a disk or some other
/// durable location.
#[derive(Debug, Default)]
pub struct InMemoryStorage {
    /// Stored SHA-256 [`InputManifest`]s.
    sha256_manifests: Vec<ManifestEntry<Sha256>>,
}

impl InMemoryStorage {
    /// Construct a new `InMemoryStorage` instance.
    pub fn new() -> Self {
        InMemoryStorage::default()
    }

    /// Find the manifest entry that matches the target [`ArtifactId`]
    fn match_by_target_aid(
        &self,
        target_aid: ArtifactId<Sha256>,
    ) -> Option<&ManifestEntry<Sha256>> {
        self.sha256_manifests
            .iter()
            .find(|entry| entry.manifest.target() == Some(target_aid))
    }
}

impl Storage<Sha256> for InMemoryStorage {
    fn has_manifest_for_artifact(&self, target_aid: ArtifactId<Sha256>) -> bool {
        self.match_by_target_aid(target_aid).is_some()
    }

    fn get_manifest_for_artifact(
        &self,
        target_aid: ArtifactId<Sha256>,
    ) -> Result<Option<InputManifest<Sha256>>> {
        Ok(self
            .match_by_target_aid(target_aid)
            .map(|entry| entry.manifest.clone()))
    }

    fn get_manifest_id_for_artifact(
        &self,
        target_aid: ArtifactId<Sha256>,
    ) -> Result<Option<ArtifactId<Sha256>>> {
        Ok(self
            .match_by_target_aid(target_aid)
            .and_then(|entry| entry.manifest.target()))
    }

    fn write_manifest(&mut self, manifest: &InputManifest<Sha256>) -> Result<ArtifactId<Sha256>> {
        let manifest_aid = ArtifactId::<Sha256>::id_manifest(manifest)?;

        self.sha256_manifests.push(ManifestEntry {
            manifest_aid,
            manifest: manifest.clone(),
        });

        Ok(manifest_aid)
    }

    fn update_target_for_manifest(
        &mut self,
        manifest_aid: ArtifactId<Sha256>,
        target_aid: ArtifactId<Sha256>,
    ) -> Result<()> {
        self.sha256_manifests
            .iter_mut()
            .find(|entry| entry.manifest_aid == manifest_aid)
            .map(|entry| entry.manifest.set_target(Some(target_aid)));

        Ok(())
    }

    fn get_manifests(&self) -> Result<Vec<InputManifest<Sha256>>> {
        Ok(self
            .sha256_manifests
            .iter()
            .map(|entry| entry.manifest.clone())
            .collect())
    }
}

/// An entry in the in-memory manifest storage.
struct ManifestEntry<H: SupportedHash> {
    /// The [`ArtifactId`] of the manifest.
    manifest_aid: ArtifactId<H>,

    /// The manifest itself.
    manifest: InputManifest<H>,
}

impl<H: SupportedHash> Debug for ManifestEntry<H> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ManifestEntry")
            .field("manifest_aid", &self.manifest_aid)
            .field("manifest", &self.manifest)
            .finish()
    }
}

impl<H: SupportedHash> Clone for ManifestEntry<H> {
    fn clone(&self) -> Self {
        ManifestEntry {
            manifest_aid: self.manifest_aid,
            manifest: self.manifest.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::FileSystemStorage;
    use crate::hashes::Sha256;
    use crate::ArtifactId;
    use pathbuf::pathbuf;
    use std::str::FromStr;

    #[test]
    fn correct_aid_storage_path() {
        let root = pathbuf![env!("CARGO_MANIFEST_DIR"), "test", "fs_storage"];
        let storage = FileSystemStorage::new(&root).unwrap();

        let aid = ArtifactId::<Sha256>::from_str(
            "gitoid:blob:sha256:9d09789f20162dca6d80d2d884f46af22c824f6409d4f447332d079a2d1e364f",
        )
        .unwrap();

        let path = storage.manifest_path(aid);
        let path = path.strip_prefix(&root).unwrap();
        let expected = pathbuf![
            "manifests",
            "gitoid_blob_sha256",
            "9d",
            "09789f20162dca6d80d2d884f46af22c824f6409d4f447332d079a2d1e364f"
        ];

        assert_eq!(path, expected);
    }
}
