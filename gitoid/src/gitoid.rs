//! A gitoid representing a single artifact.

use crate::Error;
use crate::HashAlgorithm;
use crate::ObjectType;
use crate::Result;
use core::cmp::Ordering;
use core::fmt::Debug;
#[cfg(feature = "hex")]
use core::fmt::Display;
use core::fmt::Formatter;
use core::fmt::Result as FmtResult;
use core::hash::Hash;
use core::hash::Hasher;
use core::marker::PhantomData;
use core::ops::Not as _;
#[cfg(feature = "serde")]
use core::result::Result as StdResult;
#[cfg(feature = "url")]
use core::str::FromStr;
#[cfg(feature = "url")]
use core::str::Split;
#[cfg(feature = "std")]
use digest::block_buffer::generic_array::sequence::GenericSequence;
use digest::block_buffer::generic_array::GenericArray;
use digest::Digest;
use digest::OutputSizeUser;
#[cfg(feature = "std")]
use format_bytes::format_bytes;
#[cfg(feature = "serde")]
use serde::{
    de::{Deserializer, Error as DeserializeError, Visitor},
    Deserialize, Serialize, Serializer,
};
#[cfg(feature = "std")]
use std::io::{BufRead, BufReader, Read, Seek, SeekFrom};
#[cfg(feature = "async")]
use tokio::io::{
    AsyncBufReadExt as _, AsyncRead, AsyncSeek, AsyncSeekExt as _, BufReader as AsyncBufReader,
};
#[cfg(feature = "url")]
use url::Url;

/// A struct that computes [gitoids][g] based on the selected algorithm
///
/// [g]: https://git-scm.com/book/en/v2/Git-Internals-Git-Objects
#[repr(C)]
pub struct GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    #[doc(hidden)]
    _phantom: PhantomData<O>,

    #[doc(hidden)]
    value: H::Array,
}

#[cfg(feature = "url")]
const GITOID_URL_SCHEME: &str = "gitoid";

impl<H, O> GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    /// Create a new `GitOid` based on a slice of bytes.
    pub fn id_bytes<B: AsRef<[u8]>>(content: B) -> GitOid<H, O> {
        fn inner<H, O>(content: &[u8]) -> GitOid<H, O>
        where
            H: HashAlgorithm,
            O: ObjectType,
        {
            // PANIC SAFETY: We're reading from an in-memory buffer, so no IO errors can arise.
            gitoid_from_buffer(H::new(), content, content.len()).unwrap()
        }

        inner(content.as_ref())
    }

    /// Create a `GitOid` from a UTF-8 string slice.
    pub fn id_str<S: AsRef<str>>(s: S) -> GitOid<H, O> {
        fn inner<H, O>(s: &str) -> GitOid<H, O>
        where
            H: HashAlgorithm,
            O: ObjectType,
        {
            GitOid::id_bytes(s.as_bytes())
        }

        inner(s.as_ref())
    }

    #[cfg(feature = "std")]
    /// Create a `GitOid` from a reader.
    pub fn id_reader<R: Read + Seek>(mut reader: R) -> Result<GitOid<H, O>> {
        let expected_length = stream_len(&mut reader)? as usize;
        GitOid::id_reader_with_length(reader, expected_length)
    }

    #[cfg(feature = "std")]
    /// Generate a `GitOid` from a reader, providing an expected length in bytes.
    pub fn id_reader_with_length<R>(reader: R, expected_length: usize) -> Result<GitOid<H, O>>
    where
        R: Read + Seek,
    {
        gitoid_from_reader(H::new(), reader, expected_length)
    }

    #[cfg(feature = "async")]
    /// Generate a `GitOid` from an asynchronous reader.
    pub async fn id_async_reader<R: AsyncRead + AsyncSeek + Unpin>(
        mut reader: R,
    ) -> Result<GitOid<H, O>> {
        let expected_length = async_stream_len(&mut reader).await? as usize;
        GitOid::id_async_reader_with_length(reader, expected_length).await
    }

    #[cfg(feature = "async")]
    /// Generate a `GitOid` from an asynchronous reader, providing an expected length in bytes.
    pub async fn id_async_reader_with_length<R: AsyncRead + Unpin>(
        reader: R,
        expected_length: usize,
    ) -> Result<GitOid<H, O>> {
        gitoid_from_async_buffer(H::new(), reader, expected_length).await
    }

    #[cfg(feature = "url")]
    /// Construct a new `GitOid` from a `Url`.
    pub fn try_from_url(url: Url) -> Result<GitOid<H, O>> {
        GitOid::try_from(url)
    }

    #[cfg(feature = "url")]
    /// Get a URL for the current `GitOid`.
    pub fn url(&self) -> Url {
        // PANIC SAFETY: We know that this is a valid URL;
        //               our `Display` impl is the URL representation.
        Url::parse(&self.to_string()).unwrap()
    }

    /// Get the underlying bytes of the hash.
    pub fn as_bytes(&self) -> &[u8] {
        &self.value[..]
    }

    #[cfg(feature = "hex")]
    /// Convert the hash to a hexadecimal string.
    pub fn as_hex(&self) -> String {
        hex::encode(self.as_bytes())
    }

    /// Get the hash algorithm used for the `GitOid`.
    pub const fn hash_algorithm(&self) -> &'static str {
        H::NAME
    }

    /// Get the object type of the `GitOid`.
    pub const fn object_type(&self) -> &'static str {
        O::NAME
    }

    /// Get the length of the hash in bytes.
    pub fn hash_len(&self) -> usize {
        <H::Alg as OutputSizeUser>::output_size()
    }
}

#[cfg(feature = "url")]
impl<H, O> FromStr for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    type Err = Error;

    fn from_str(s: &str) -> Result<GitOid<H, O>> {
        let url = Url::parse(s)?;
        GitOid::try_from_url(url)
    }
}

impl<H, O> Clone for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn clone(&self) -> Self {
        *self
    }
}

impl<H, O> Copy for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
}

impl<H, O> PartialEq<GitOid<H, O>> for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn eq(&self, other: &GitOid<H, O>) -> bool {
        self.value == other.value
    }
}

impl<H, O> Eq for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
}

impl<H, O> PartialOrd<GitOid<H, O>> for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl<H, O> Ord for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn cmp(&self, other: &Self) -> Ordering {
        self.value.cmp(&other.value)
    }
}

impl<H, O> Hash for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn hash<H2>(&self, state: &mut H2)
    where
        H2: Hasher,
    {
        self.value.hash(state);
    }
}

impl<H, O> Debug for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        f.debug_struct("GitOid")
            .field("value", &self.value)
            .finish()
    }
}

#[cfg(feature = "hex")]
impl<H, O> Display for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> FmtResult {
        write!(
            f,
            "{}:{}:{}:{}",
            GITOID_URL_SCHEME,
            O::NAME,
            H::NAME,
            self.as_hex()
        )
    }
}

#[cfg(feature = "serde")]
impl<H, O> Serialize for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn serialize<S>(&self, serializer: S) -> StdResult<S::Ok, S::Error>
    where
        S: Serializer,
    {
        // Serialize self as the URL string.
        let self_as_url_str = self.url().to_string();
        serializer.serialize_str(&self_as_url_str)
    }
}

#[cfg(feature = "serde")]
impl<'de, H, O> Deserialize<'de> for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn deserialize<D>(deserializer: D) -> StdResult<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        // Deserialize self from the URL string.
        struct GitOidVisitor<H: HashAlgorithm, O: ObjectType>(PhantomData<H>, PhantomData<O>);

        impl<H: HashAlgorithm, O: ObjectType> Visitor<'_> for GitOidVisitor<H, O> {
            type Value = GitOid<H, O>;

            fn expecting(&self, formatter: &mut Formatter<'_>) -> FmtResult {
                formatter.write_str("a gitoid-scheme URL")
            }

            fn visit_str<E>(self, value: &str) -> StdResult<Self::Value, E>
            where
                E: DeserializeError,
            {
                let url = Url::parse(value).map_err(E::custom)?;
                let id = GitOid::try_from(url).map_err(E::custom)?;
                Ok(id)
            }
        }

        deserializer.deserialize_str(GitOidVisitor(PhantomData, PhantomData))
    }
}

#[cfg(feature = "url")]
struct GitOidUrlParser<'u, H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    url: &'u Url,

    segments: Split<'u, char>,

    #[doc(hidden)]
    _hash_algorithm: PhantomData<H>,

    #[doc(hidden)]
    _object_type: PhantomData<O>,
}

#[allow(dead_code)]
fn some_if_not_empty(s: &str) -> Option<&str> {
    s.is_empty().not().then_some(s)
}

#[cfg(feature = "url")]
impl<'u, H, O> GitOidUrlParser<'u, H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    fn new(url: &'u Url) -> GitOidUrlParser<'u, H, O> {
        GitOidUrlParser {
            url,
            segments: url.path().split(':'),
            _hash_algorithm: PhantomData,
            _object_type: PhantomData,
        }
    }

    fn parse(&mut self) -> Result<GitOid<H, O>> {
        self.validate_url_scheme()
            .and_then(|_| self.validate_object_type())
            .and_then(|_| self.validate_hash_algorithm())
            .and_then(|_| self.parse_hash())
            .map(|hash| GitOid {
                _phantom: PhantomData,
                value: H::array_from_generic(hash),
            })
    }

    fn validate_url_scheme(&self) -> Result<()> {
        if self.url.scheme() != GITOID_URL_SCHEME {
            return Err(Error::InvalidScheme(self.url.clone()));
        }

        Ok(())
    }

    fn validate_object_type(&mut self) -> Result<()> {
        let object_type = self
            .segments
            .next()
            .and_then(some_if_not_empty)
            .ok_or_else(|| Error::MissingObjectType(self.url.clone()))?;

        if object_type != O::NAME {
            return Err(Error::MismatchedObjectType { expected: O::NAME });
        }

        Ok(())
    }

    fn validate_hash_algorithm(&mut self) -> Result<()> {
        let hash_algorithm = self
            .segments
            .next()
            .and_then(some_if_not_empty)
            .ok_or_else(|| Error::MissingHashAlgorithm(self.url.clone()))?;

        if hash_algorithm != H::NAME {
            return Err(Error::MismatchedHashAlgorithm { expected: H::NAME });
        }

        Ok(())
    }

    fn parse_hash(&mut self) -> Result<GenericArray<u8, <H::Alg as OutputSizeUser>::OutputSize>> {
        let hex_str = self
            .segments
            .next()
            .and_then(some_if_not_empty)
            .ok_or_else(|| Error::MissingHash(self.url.clone()))?;

        // TODO(alilleybrinker): When `sha1` et al. move to generic-array 1.0,
        //                       update this to use the `arr!` macro.
        let mut value = GenericArray::generate(|_| 0);
        hex::decode_to_slice(hex_str, &mut value)?;

        let expected_size = <H::Alg as OutputSizeUser>::output_size();
        if value.len() != expected_size {
            return Err(Error::UnexpectedHashLength {
                expected: expected_size,
                observed: value.len(),
            });
        }

        Ok(value)
    }
}

#[cfg(feature = "url")]
impl<H, O> TryFrom<Url> for GitOid<H, O>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    type Error = Error;

    fn try_from(url: Url) -> Result<GitOid<H, O>> {
        GitOidUrlParser::new(&url).parse()
    }
}

#[cfg(feature = "std")]
/// Generate a GitOid by reading from an arbitrary reader.
fn gitoid_from_reader<H, O, R>(
    digester: H::Alg,
    reader: R,
    expected_read_length: usize,
) -> Result<GitOid<H, O>>
where
    H: HashAlgorithm,
    O: ObjectType,
    R: Read + Seek,
{
    let expected_hash_length = <H::Alg as OutputSizeUser>::output_size();
    let (hash, amount_read) =
        hash_from_reader::<H::Alg, O, R>(digester, reader, expected_read_length)?;

    if amount_read != expected_read_length {
        return Err(Error::UnexpectedReadLength {
            expected: expected_read_length,
            observed: amount_read,
        });
    }

    if hash.len() != expected_hash_length {
        return Err(Error::UnexpectedHashLength {
            expected: expected_hash_length,
            observed: hash.len(),
        });
    }

    Ok(GitOid {
        _phantom: PhantomData,
        value: H::array_from_generic(hash),
    })
}

/// Generate a GitOid from data in a buffer of bytes.
fn gitoid_from_buffer<H, O>(
    digester: H::Alg,
    reader: &[u8],
    expected_read_length: usize,
) -> Result<GitOid<H, O>>
where
    H: HashAlgorithm,
    O: ObjectType,
{
    let expected_hash_length = <H::Alg as OutputSizeUser>::output_size();
    let (hash, amount_read) =
        hash_from_buffer::<H::Alg, O>(digester, reader, expected_read_length)?;

    if amount_read != expected_read_length {
        return Err(Error::UnexpectedReadLength {
            expected: expected_read_length,
            observed: amount_read,
        });
    }

    if hash.len() != expected_hash_length {
        return Err(Error::UnexpectedHashLength {
            expected: expected_hash_length,
            observed: hash.len(),
        });
    }

    Ok(GitOid {
        _phantom: PhantomData,
        value: H::array_from_generic(hash),
    })
}

#[cfg(feature = "std")]
// Helper extension trait to give a convenient way to iterate over
// chunks sized to the size of the internal buffer of the reader.
trait ForEachChunk: BufRead {
    // Takes a function to apply to each chunk, and returns if any
    // errors arose along with the number of bytes read in total.
    fn for_each_chunk(&mut self, f: impl FnMut(&[u8])) -> Result<usize>;
}

#[cfg(feature = "std")]
impl<R: BufRead> ForEachChunk for R {
    fn for_each_chunk(&mut self, mut f: impl FnMut(&[u8])) -> Result<usize> {
        let mut total_read = 0;

        loop {
            let buffer = self.fill_buf()?;
            let amount_read = buffer.len();

            if amount_read == 0 {
                break;
            }

            f(buffer);

            self.consume(amount_read);
            total_read += amount_read;
        }

        Ok(total_read)
    }
}

fn dos_newline_boundary(char1: &u8, char2: &u8) -> bool {
    ((*char1, *char2) == (b'\r', b'\n')).not()
}

fn digest_with_normalized_newlines<D>(buf: &[u8], digester: &mut D)
where
    D: Digest,
{
    for chunk in buf.chunk_by(dos_newline_boundary) {
        if chunk.last() == Some(&b'\r') {
            // Chunk without the carriage return at the end.
            let chunk = &chunk[0..(chunk.len() - 1)];
            digester.update(chunk);
        } else {
            digester.update(chunk);
        }
    }
}

fn num_dos_newlines_in_reader<R>(reader: R) -> Result<(usize, R)>
where
    R: Read + Seek,
{
    let mut buf_reader = BufReader::new(reader);

    let mut total_dos_newlines = 0;

    buf_reader.for_each_chunk(|buf| {
        // The number of separators is the number of chunks minus one.
        total_dos_newlines += buf.chunk_by(dos_newline_boundary).count() - 1
    })?;

    let mut reader = buf_reader.into_inner();

    // Return to the start.
    reader.seek(SeekFrom::Start(0))?;

    Ok((total_dos_newlines, reader))
}

fn num_dos_newlines_in_buffer(buffer: &[u8]) -> usize {
    buffer.windows(2).filter(|buf| buf == b"\r\n").count()
}

#[cfg(feature = "std")]
/// Helper function which actually applies the [`GitOid`] construction rules.
///
/// This function handles actually constructing the hash with the GitOID prefix,
/// and delegates to a buffered reader for performance of the chunked reading.
fn hash_from_reader<D, O, R>(
    mut digester: D,
    reader: R,
    expected_read_length: usize,
) -> Result<(GenericArray<u8, D::OutputSize>, usize)>
where
    D: Digest,
    O: ObjectType,
    R: Read + Seek,
{
    let (dos_newlines, reader) = num_dos_newlines_in_reader(reader)?;
    let adjusted_expected_read_length = expected_read_length - dos_newlines;

    println!("header bytes: {}", adjusted_expected_read_length);

    digester.update(format_bytes!(
        b"{} {}\0",
        O::NAME.as_bytes(),
        adjusted_expected_read_length
    ));

    let amount_read = BufReader::new(reader)
        .for_each_chunk(|b| digest_with_normalized_newlines(b, &mut digester))?;

    let hash = digester.finalize();

    Ok((hash, amount_read))
}

/// Helper function which actually applies the [`GitOid`] construction rules.
///
/// This function handles actually constructing the hash with the GitOID prefix,
/// and delegates to a buffered reader for performance of the chunked reading.
fn hash_from_buffer<D, O>(
    mut digester: D,
    reader: &[u8],
    expected_read_length: usize,
) -> Result<(GenericArray<u8, D::OutputSize>, usize)>
where
    D: Digest,
    O: ObjectType,
{
    let dos_newlines = num_dos_newlines_in_buffer(reader);
    let adjusted_expected_read_length = expected_read_length - dos_newlines;

    digester.update(format_bytes!(
        b"{} {}\0",
        O::NAME.as_bytes(),
        adjusted_expected_read_length
    ));

    // It's in memory, so we know the exact size up front.
    let amount_read = reader.len();
    digest_with_normalized_newlines(reader, &mut digester);
    let hash = digester.finalize();
    Ok((hash, amount_read))
}

#[cfg(feature = "async")]
/// Async version of `gitoid_from_buffer`.
async fn gitoid_from_async_buffer<H, O, R>(
    digester: H::Alg,
    reader: R,
    expected_read_length: usize,
) -> Result<GitOid<H, O>>
where
    H: HashAlgorithm,
    O: ObjectType,
    R: AsyncRead + Unpin,
{
    let expected_hash_length = <H::Alg as OutputSizeUser>::output_size();
    let (hash, amount_read) =
        hash_from_async_buffer::<H::Alg, O, R>(digester, reader, expected_read_length).await?;

    if amount_read != expected_read_length {
        return Err(Error::UnexpectedHashLength {
            expected: expected_read_length,
            observed: amount_read,
        });
    }

    if hash.len() != expected_hash_length {
        return Err(Error::UnexpectedHashLength {
            expected: expected_hash_length,
            observed: hash.len(),
        });
    }

    Ok(GitOid {
        _phantom: PhantomData,
        value: H::array_from_generic(hash),
    })
}

#[cfg(feature = "async")]
/// Async version of `hash_from_buffer`.
async fn hash_from_async_buffer<D, O, R>(
    mut digester: D,
    reader: R,
    expected_read_length: usize,
) -> Result<(GenericArray<u8, D::OutputSize>, usize)>
where
    D: Digest,
    O: ObjectType,
    R: AsyncRead + Unpin,
{
    digester.update(format_bytes!(
        b"{} {}\0",
        O::NAME.as_bytes(),
        expected_read_length
    ));

    let mut reader = AsyncBufReader::new(reader);

    let mut total_read = 0;

    loop {
        let buffer = reader.fill_buf().await?;
        let amount_read = buffer.len();

        if amount_read == 0 {
            break;
        }

        digester.update(buffer);

        reader.consume(amount_read);
        total_read += amount_read;
    }

    let hash = digester.finalize();
    Ok((hash, total_read))
}

// Adapted from the Rust standard library's unstable implementation
// of `Seek::stream_len`.
//
// TODO(abrinker): Remove this when `Seek::stream_len` is stabilized.
//
// License reproduction:
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER
// DEALINGS IN THE SOFTWARE.
#[cfg(feature = "std")]
fn stream_len<R>(mut stream: R) -> Result<u64>
where
    R: Seek,
{
    let old_pos = stream.stream_position()?;
    let len = stream.seek(SeekFrom::End(0))?;

    // Avoid seeking a third time when we were already at the end of the
    // stream. The branch is usually way cheaper than a seek operation.
    if old_pos != len {
        stream.seek(SeekFrom::Start(old_pos))?;
    }

    Ok(len)
}

#[cfg(feature = "async")]
/// An async equivalent of `stream_len`.
async fn async_stream_len<R>(mut stream: R) -> Result<u64>
where
    R: AsyncSeek + Unpin,
{
    let old_pos = stream.stream_position().await?;
    let len = stream.seek(SeekFrom::End(0)).await?;

    // Avoid seeking a third time when we were already at the end of the
    // stream. The branch is usually way cheaper than a seek operation.
    if old_pos != len {
        stream.seek(SeekFrom::Start(old_pos)).await?;
    }

    Ok(len)
}
