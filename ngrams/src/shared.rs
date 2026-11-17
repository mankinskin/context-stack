//! Internal release-build re-exports and optional deterministic collection aliases.
//!
//! These aliases affect users importing them from this module; graph code that
//! imports collections from `context_trace` uses that crate's hashing policy.

#[cfg(not(any(test, feature = "test-hashing")))]
pub(crate) use std::collections::{
    HashMap,
    HashSet,
};
#[cfg(any(test, feature = "test-hashing"))]
use std::hash::{
    BuildHasherDefault,
    DefaultHasher,
};

pub(crate) use itertools::*;
pub(crate) use ngram::*;
pub(crate) use range_ext::intersect::Intersect;
pub(crate) use std::{
    borrow::Borrow,
    default::Default,
    fmt::Debug,
    hash::Hash,
};
pub(crate) use tap::prelude::*;

/// Hash set with a default, reproducible hasher for tests or `test-hashing`.
#[cfg(any(test, feature = "test-hashing"))]
pub(crate) type HashSet<T> =
    std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;

/// Hash map counterpart to the deterministic set alias.
#[cfg(any(test, feature = "test-hashing"))]
pub(crate) type HashMap<K, V> =
    std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
