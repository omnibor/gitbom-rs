#[cfg(doc)]
use crate::hashes::Sha256;
use crate::hashes::SupportedHash;
use crate::Error;
use crate::InputManifest;
use crate::Result;
use gitoid::Blob;
use gitoid::GitOid;
#[cfg(feature = "serde")]
use serde::de::Deserializer;
#[cfg(feature = "serde")]
use serde::Deserialize;
#[cfg(feature = "serde")]
use serde::Serialize;
#[cfg(feature = "serde")]
use serde::Serializer;
use std::cmp::Ordering;
use std::fmt::Debug;
use std::fmt::Display;
use std::fmt::Formatter;
use std::fmt::Result as FmtResult;
use std::hash::Hash;
use std::hash::Hasher;
use std::io::Read;
use std::io::Seek;
use std::path::PathBuf;
#[cfg(feature = "serde")]
use std::result::Result as StdResult;
use std::str::FromStr;
use tokio::io::AsyncRead;
use tokio::io::AsyncSeek;
use url::Url;

/// An OmniBOR Artifact Identifier.
///
/// This is a content-based unique identifier for any software artifact.
///
/// It is built around, per the specification, any supported hash algorithm.
/// Currently, only SHA-256 is supported, but others may be added in the future.
pub struct ArtifactId<H: SupportedHash> {
    #[doc(hidden)]
    gitoid: GitOid<H::HashAlgorithm, Blob>,
}

impl<H: SupportedHash> ArtifactId<H> {
    /// Construct an [`ArtifactId`] from an existing [`GitOid`].
    ///
    /// This produces an identifier using the provided [`GitOid`] directly,
    /// without additional validation. The type system ensures the [`GitOid`]
    /// hash algorithm is one supported for an [`ArtifactId`], and that the
    /// object type is [`gitoid::Blob`].
    ///
    /// # Note
    ///
    /// This function is not exported because we don't re-export the `GitOid`
    /// type we use, which would mean users of the crate would themselves
    /// need to import a binary-compatible version of the `GitOid` crate as
    /// well. This is extra complexity for minimal gain, so we don't support it.
    ///
    /// If it were ever absolutely needed in the future, we might expose this
    /// constructor with a `#[doc(hidden)]` attribute, or with documentation
    /// which clearly outlines the extra complexity.
    fn from_gitoid(gitoid: GitOid<H::HashAlgorithm, Blob>) -> ArtifactId<H> {
        ArtifactId { gitoid }
    }

    /// Construct an [`ArtifactId`] from raw bytes.
    ///
    /// This hashes the bytes to produce an identifier.
    ///
    /// # Note
    ///
    /// Generally, `ArtifactId`s are produced so independent parties
    /// can compare ID's in the future. It's generally not useful to identify
    /// artifacts which are never persisted or shared in some way.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_bytes(&[0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
    /// println!("Artifact ID: {}", id);
    /// ```
    pub fn id_bytes<B: AsRef<[u8]>>(content: B) -> ArtifactId<H> {
        ArtifactId::from_gitoid(GitOid::id_bytes(content))
    }

    /// Construct an [`ArtifactId`] from a string.
    ///
    /// This hashes the contents of the string to produce an identifier.
    ///
    /// # Note
    ///
    /// Generally, `ArtifactId`s are produced so independent parties
    /// can compare ID's in the future. It's generally not useful to identify
    /// artifacts which are never persisted or shared in some way.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID: {}", id);
    /// ```
    pub fn id_str<S: AsRef<str>>(s: S) -> ArtifactId<H> {
        ArtifactId::from_gitoid(GitOid::id_str(s))
    }

    /// Construct an [`ArtifactId`] from a synchronous reader.
    ///
    /// This reads the content of the reader and hashes it to produce an identifier.
    ///
    /// Note that this will figure out the expected size in bytes of the content
    /// being read by seeking to the end of the content and then back to wherever the
    /// reading initially started. This is to enable a correctness check where the total
    /// number of bytes hashed is checked against the expected length. If they do not
    /// match, we return an [`Error`] rather than proceeding with a potentially-invalid
    /// identifier.
    ///
    /// If you don't want this seeking to occur, you can use
    /// [`ArtifactId::id_reader_with_length`] instead, which takes an explicit expected
    /// length and checks against _that_ value, rather than inferring an expected length.
    ///
    /// Also note that this doesn't reset the reader to the beginning of its region; if
    /// you provide a reader which has already read some portion of an underlying file or
    /// has seeked to a point that's not the beginning, this function will continue reading
    /// from that point, and the resulting hash will _not_ encompass the contents of the
    /// entire file. You can use [`ArtifactId::id_reader_with_length`] and provide the
    /// expected length of the full file in bytes to defend against this "partial hash"
    /// error.
    ///
    /// Reads are buffered internally to reduce the number of syscalls and context switches
    /// between the kernel and user code.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// # use std::fs::File;
    /// let file = File::open("test/data/hello_world.txt").unwrap();
    /// let id: ArtifactId<Sha256> = ArtifactId::id_reader(&file).unwrap();
    /// println!("Artifact ID: {}", id);
    /// ```
    pub fn id_reader<R: Read + Seek>(reader: R) -> Result<ArtifactId<H>> {
        let gitoid = GitOid::id_reader(reader)?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }

    /// Construct an [`ArtifactId`] from a synchronous reader with an expected length.
    ///
    /// This reads the content of the reader and hashes it to produce an identifier.
    ///
    /// This uses the `expected_len` to enable a correctness check where the total
    /// number of bytes hashed is checked against the expected length. If they do not
    /// match, we return an [`Error`] rather than proceeding with a potentially-invalid
    /// identifier.
    ///
    /// Also note that this doesn't reset the reader to the beginning of its region; if
    /// you provide a reader which has already read some portion of an underlying file or
    /// has seeked to a point that's not the beginning, this function will continue reading
    /// from that point, and the resulting hash will _not_ encompass the contents of the
    /// entire file. Make sure to provide the expected number of bytes for the full file
    /// to protect against this error.
    ///
    /// Reads are buffered internally to reduce the number of syscalls and context switches
    /// between the kernel and user code.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// # use std::fs::File;
    /// let file = File::open("test/data/hello_world.txt").unwrap();
    /// let id: ArtifactId<Sha256> = ArtifactId::id_reader_with_length(&file, 11).unwrap();
    /// println!("Artifact ID: {}", id);
    /// ```
    pub fn id_reader_with_length<R>(reader: R, expected_length: usize) -> Result<ArtifactId<H>>
    where
        R: Read + Seek,
    {
        let gitoid = GitOid::id_reader_with_length(reader, expected_length)?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }

    /// Construct an [`ArtifactId`] from an asynchronous reader.
    ///
    /// This reads the content of the reader and hashes it to produce an identifier.
    ///
    /// Reading is done asynchronously by the Tokio runtime. The specifics of how this
    /// is done are based on the configuration of the runtime.
    ///
    /// Note that this will figure out the expected size in bytes of the content
    /// being read by seeking to the end of the content and then back to wherever the
    /// reading initially started. This is to enable a correctness check where the total
    /// number of bytes hashed is checked against the expected length. If they do not
    /// match, we return an [`Error`] rather than proceeding with a potentially-invalid
    /// identifier.
    ///
    /// If you don't want this seeking to occur, you can use
    /// [`ArtifactId::id_reader_with_length`] instead, which takes an explicit expected
    /// length and checks against _that_ value, rather than inferring an expected length.
    ///
    /// Also note that this doesn't reset the reader to the beginning of its region; if
    /// you provide a reader which has already read some portion of an underlying file or
    /// has seeked to a point that's not the beginning, this function will continue reading
    /// from that point, and the resulting hash will _not_ encompass the contents of the
    /// entire file. You can use [`ArtifactId::id_reader_with_length`] and provide the
    /// expected length of the full file in bytes to defend against this "partial hash"
    /// error.
    ///
    /// Reads are buffered internally to reduce the number of syscalls and context switches
    /// between the kernel and user code.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// # use tokio::fs::File;
    /// # tokio_test::block_on(async {
    /// let mut file = File::open("test/data/hello_world.txt").await.unwrap();
    /// let id: ArtifactId<Sha256> = ArtifactId::id_async_reader(&mut file).await.unwrap();
    /// println!("Artifact ID: {}", id);
    /// # })
    /// ```
    pub async fn id_async_reader<R: AsyncRead + AsyncSeek + Unpin>(
        reader: R,
    ) -> Result<ArtifactId<H>> {
        let gitoid = GitOid::id_async_reader(reader).await?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }

    /// Construct an [`ArtifactId`] from an asynchronous reader with an expected length.
    ///
    /// This reads the content of the reader and hashes it to produce an identifier.
    ///
    /// Reading is done asynchronously by the Tokio runtime. The specifics of how this
    /// is done are based on the configuration of the runtime.
    ///
    /// This uses the `expected_len` to enable a correctness check where the total
    /// number of bytes hashed is checked against the expected length. If they do not
    /// match, we return an [`Error`] rather than proceeding with a potentially-invalid
    /// identifier.
    ///
    /// Also note that this doesn't reset the reader to the beginning of its region; if
    /// you provide a reader which has already read some portion of an underlying file or
    /// has seeked to a point that's not the beginning, this function will continue reading
    /// from that point, and the resulting hash will _not_ encompass the contents of the
    /// entire file. Make sure to provide the expected number of bytes for the full file
    /// to protect against this error.
    ///
    /// Reads are buffered internally to reduce the number of syscalls and context switches
    /// between the kernel and user code.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// # use tokio::fs::File;
    /// # tokio_test::block_on(async {
    /// let mut file = File::open("test/data/hello_world.txt").await.unwrap();
    /// let id: ArtifactId<Sha256> = ArtifactId::id_async_reader_with_length(&mut file, 11).await.unwrap();
    /// println!("Artifact ID: {}", id);
    /// # })
    /// ```
    pub async fn id_async_reader_with_length<R: AsyncRead + Unpin>(
        reader: R,
        expected_length: usize,
    ) -> Result<ArtifactId<H>> {
        let gitoid = GitOid::id_async_reader_with_length(reader, expected_length).await?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }

    /// Construct an [`ArtifactId`] for an [`InputManifest`].
    pub fn id_manifest(manifest: &InputManifest<H>) -> Result<Self> {
        Ok(ArtifactId::id_bytes(manifest.as_bytes()?))
    }

    /// Construct an [`ArtifactId`] from a `gitoid`-scheme [`Url`].
    ///
    /// This validates that the provided URL has a hashing scheme which matches the one
    /// selected for your [`ArtifactId`] (today, only `sha256` is supported), and has the
    /// `blob` object type. It also validates that the provided hash is a valid hash for
    /// the specified hashing scheme. If any of these checks fail, the function returns
    /// an [`Error`].
    ///
    /// Note that this expects a `gitoid`-scheme URL, as defined by IANA. This method
    /// _does not_ expect an HTTP or HTTPS URL to access, retrieve contents, and hash
    /// those contents to produce an identifier. You _can_ implement that yourself with
    /// a Rust HTTP(S) crate and [`ArtifactId::id_bytes`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// # use url::Url;
    /// let url = Url::parse("gitoid:blob:sha256:fee53a18d32820613c0527aa79be5cb30173c823a9b448fa4817767cc84c6f03").unwrap();
    /// let id: ArtifactId<Sha256> = ArtifactId::try_from_url(url).unwrap();
    /// println!("Artifact ID: {}", id);
    /// ```
    pub fn try_from_url(url: Url) -> Result<ArtifactId<H>> {
        ArtifactId::try_from(url)
    }

    /// Try to construct an [`ArtifactId`] from a filesystem-safe representation.
    pub fn try_from_safe_name(s: &str) -> Result<ArtifactId<H>> {
        ArtifactId::from_str(&s.replace('_', ":"))
    }

    /// Get the [`Url`] representation of the [`ArtifactId`].
    ///
    /// This returns a `gitoid`-scheme URL for the [`ArtifactId`].
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID URL: {}", id.url());
    /// ```
    pub fn url(&self) -> Url {
        self.gitoid.url()
    }

    /// Get a filesystem-safe representation of the [`ArtifactId`].
    ///
    /// This is a conservative method that tries to use _only_ characters
    /// which can be expected to work broadly cross-platform.
    ///
    /// What that means for us is that the `:` separator character is
    /// replaced with `_`.
    pub fn as_file_name(&self) -> PathBuf {
        let name = self.gitoid.url().to_string().replace(':', "_");
        let mut path = PathBuf::from(name);
        path.set_extension("manifest");
        path
    }

    /// Get the underlying bytes of the [`ArtifactId`] hash.
    ///
    /// This slice is the raw underlying buffer of the [`ArtifactId`], exactly
    /// as produced by the hasher.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID bytes: {:?}", id.as_bytes());
    /// ```
    pub fn as_bytes(&self) -> &[u8] {
        self.gitoid.as_bytes()
    }

    /// Get the bytes of the [`ArtifactId`] hash as a hexadecimal string.
    ///
    /// This returns a [`String`] rather than [`str`] because the string must be
    /// constructed on the fly, as we do not store a hexadecimal representation
    /// of the hash data.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID bytes as hex: {}", id.as_hex());
    /// ```
    pub fn as_hex(&self) -> String {
        self.gitoid.as_hex()
    }

    /// Get the name of the hash algorithm used in the [`ArtifactId`] as a string.
    ///
    /// For [`Sha256`], this is the string `"sha256"`.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID hash algorithm: {}", id.hash_algorithm());
    /// ```
    pub const fn hash_algorithm(&self) -> &'static str {
        self.gitoid.hash_algorithm()
    }

    /// Get the object type used in the [`ArtifactId`] as a string.
    ///
    /// For all [`ArtifactId`]s this is `"blob"`, but the method is provided
    /// for completeness nonetheless.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID object type: {}", id.object_type());
    /// ```
    pub const fn object_type(&self) -> &'static str {
        self.gitoid.object_type()
    }

    /// Get the length in bytes of the hash used in the [`ArtifactId`].
    ///
    /// In the future this method will be `const`, but is not able to be
    /// today due to limitations in the Rust cryptography crates we use
    /// internally.
    ///
    /// # Example
    ///
    /// ```rust
    /// # use omnibor::ArtifactId;
    /// # use omnibor::hashes::Sha256;
    /// let id: ArtifactId<Sha256> = ArtifactId::id_str("hello, world");
    /// println!("Artifact ID hash length in bytes: {}", id.hash_len());
    /// ```
    pub fn hash_len(&self) -> usize {
        self.gitoid.hash_len()
    }
}

impl<H: SupportedHash> FromStr for ArtifactId<H> {
    type Err = Error;

    fn from_str(s: &str) -> Result<ArtifactId<H>> {
        let url = Url::parse(s)?;
        ArtifactId::try_from_url(url)
    }
}

impl<H: SupportedHash> Clone for ArtifactId<H> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<H: SupportedHash> Copy for ArtifactId<H> {}

impl<H: SupportedHash> PartialEq<ArtifactId<H>> for ArtifactId<H> {
    fn eq(&self, other: &Self) -> bool {
        self.gitoid == other.gitoid
    }
}

impl<H: SupportedHash> Eq for ArtifactId<H> {}

impl<H: SupportedHash> PartialOrd<ArtifactId<H>> for ArtifactId<H> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<H: SupportedHash> Ord for ArtifactId<H> {
    fn cmp(&self, other: &Self) -> Ordering {
        self.gitoid.cmp(&other.gitoid)
    }
}

impl<H: SupportedHash> Hash for ArtifactId<H> {
    fn hash<H2>(&self, state: &mut H2)
    where
        H2: Hasher,
    {
        self.gitoid.hash(state);
    }
}

impl<H: SupportedHash> Debug for ArtifactId<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("ArtifactId")
            .field("gitoid", &self.gitoid)
            .finish()
    }
}

impl<H: SupportedHash> Display for ArtifactId<H> {
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(f, "{}", self.gitoid)
    }
}

impl<H: SupportedHash> From<GitOid<H::HashAlgorithm, Blob>> for ArtifactId<H> {
    fn from(gitoid: GitOid<H::HashAlgorithm, Blob>) -> Self {
        ArtifactId::from_gitoid(gitoid)
    }
}

impl<'r, H: SupportedHash> TryFrom<&'r str> for ArtifactId<H> {
    type Error = Error;

    fn try_from(s: &'r str) -> Result<Self> {
        ArtifactId::from_str(s)
    }
}

impl<H: SupportedHash> TryFrom<Url> for ArtifactId<H> {
    type Error = Error;

    fn try_from(url: Url) -> Result<ArtifactId<H>> {
        let gitoid = GitOid::try_from_url(url)?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }
}

#[cfg(feature = "serde")]
impl<H: SupportedHash> Serialize for ArtifactId<H> {
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        self.gitoid.serialize(serializer)
    }
}

#[cfg(feature = "serde")]
impl<'de, H: SupportedHash> Deserialize<'de> for ArtifactId<H> {
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let gitoid = GitOid::<H::HashAlgorithm, Blob>::deserialize(deserializer)?;
        Ok(ArtifactId::from_gitoid(gitoid))
    }
}
