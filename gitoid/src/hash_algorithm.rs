//! Trait specifying valid [`GitOid`] hash algorithms.

use crate::sealed::Sealed;
#[cfg(doc)]
use crate::GitOid;
use core::fmt::Debug;
use core::hash::Hash;
use core::ops::Deref;
use digest::Digest;
use digest::OutputSizeUser;
use generic_array::GenericArray;

/// Hash algorithms that can be used to make a [`GitOid`].
///
/// This is a sealed trait to ensure it's only used for hash
/// algorithms which are actually supported by Git. No other
/// types, even if they implement [`Digest`] can implement
/// this trait.
///
/// For more information on sealed traits, read Predrag
/// Gruevski's ["A Definitive Guide to Sealed Traits in Rust"][1].
///
/// [1]: https://predr.ag/blog/definitive-guide-to-sealed-traits-in-rust/
pub trait HashAlgorithm: Sealed {
    /// The name of the hash algorithm in lowercase ASCII.
    const NAME: &'static str;

    /// The actual digest type used by the algorithm.
    type Alg: Digest;

    /// The array type generated by the hash.
    type Array: Copy + PartialEq + Ord + Hash + Debug + Deref<Target = [u8]>;

    /// Helper function to convert the GenericArray type to Self::Array
    fn array_from_generic(
        arr: GenericArray<u8, <Self::Alg as OutputSizeUser>::OutputSize>,
    ) -> Self::Array;

    /// Get an instance of the digester.
    fn new() -> Self::Alg;
}

macro_rules! impl_hash_algorithm {
    ( $type:ident, $alg_ty:ty, $name:literal ) => {
        impl Sealed for $type {}

        impl HashAlgorithm for $type {
            const NAME: &'static str = $name;

            type Alg = $alg_ty;

            type Array = GenericArray<u8, <Self::Alg as OutputSizeUser>::OutputSize>;

            fn array_from_generic(
                arr: GenericArray<u8, <Self::Alg as OutputSizeUser>::OutputSize>,
            ) -> Self::Array {
                arr
            }

            fn new() -> Self::Alg {
                Self::Alg::new()
            }
        }
    };
}

/// SHA-1 algorithm,
pub struct Sha1 {
    #[doc(hidden)]
    _private: (),
}

impl_hash_algorithm!(Sha1, sha1::Sha1, "sha1");

/// SHA-256 algorithm.
pub struct Sha256 {
    #[doc(hidden)]
    _private: (),
}

impl_hash_algorithm!(Sha256, sha2::Sha256, "sha256");

/// SHA-1Cd (collision detection) algorithm.
pub struct Sha1Cd {
    #[doc(hidden)]
    _private: (),
}

impl_hash_algorithm!(Sha1Cd, sha1collisiondetection::Sha1CD, "sha1cd");
