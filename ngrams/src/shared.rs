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

#[cfg(any(test, feature = "test-hashing"))]
pub(crate) type HashSet<T> =
    std::collections::HashSet<T, BuildHasherDefault<DefaultHasher>>;

#[cfg(any(test, feature = "test-hashing"))]
pub(crate) type HashMap<K, V> =
    std::collections::HashMap<K, V, BuildHasherDefault<DefaultHasher>>;
