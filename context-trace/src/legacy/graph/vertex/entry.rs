//! Concurrent vertex entry wrapper with per-vertex locking.
//!
//! This module provides `VertexEntry`, a wrapper around `VertexData` that uses
//! `RwLock` to allow concurrent reads while serializing writes to individual vertices.
//!
//! Uses `Arc<RwLock<VertexData>>` so the lock can be held independently of
//! the DashMap shard lock, preventing deadlocks.

use std::sync::{
    Arc,
    RwLock,
    RwLockReadGuard,
    RwLockWriteGuard,
};

use serde::{
    Deserialize,
    Deserializer,
    Serialize,
    Serializer,
};

use super::data::VertexData;

/// A concurrent wrapper around `VertexData` with per-vertex read-write locking.
///
/// Multiple readers can access the vertex data simultaneously.
/// Writers get exclusive access to just this vertex (not the whole graph).
///
/// Uses `Arc<RwLock<...>>` so the lock can be cloned and held independently
/// of the DashMap, preventing deadlocks when accessing multiple vertices.
#[derive(Debug, Clone)]
pub struct VertexEntry {
    data: Arc<RwLock<VertexData>>,
}

impl VertexEntry {
    /// Create a new vertex entry wrapping the given data.
    pub fn new(data: VertexData) -> Self {
        Self {
            data: Arc::new(RwLock::new(data)),
        }
    }

    /// Acquire a read lock on the vertex data.
    ///
    /// This will block if another thread holds a write lock.
    /// Multiple readers can hold read locks simultaneously.
    pub fn read(&self) -> RwLockReadGuard<'_, VertexData> {
        self.data.read().expect("VertexEntry lock poisoned")
    }

    /// Try to acquire a read lock without blocking.
    ///
    /// Returns `None` if a write lock is held.
    pub fn try_read(&self) -> Option<RwLockReadGuard<'_, VertexData>> {
        self.data.try_read().ok()
    }

    /// Acquire a write lock on the vertex data.
    ///
    /// This will block if another thread holds any lock (read or write).
    pub fn write(&self) -> RwLockWriteGuard<'_, VertexData> {
        self.data.write().expect("VertexEntry lock poisoned")
    }

    /// Try to acquire a write lock without blocking.
    ///
    /// Returns `None` if any lock is held.
    pub fn try_write(&self) -> Option<RwLockWriteGuard<'_, VertexData>> {
        self.data.try_write().ok()
    }

    /// Get a clone of the inner data.
    ///
    /// This acquires a read lock, clones the data, then releases the lock.
    pub fn clone_data(&self) -> VertexData {
        self.read().clone()
    }

    /// Try to get a clone of the inner data without blocking.
    ///
    /// Returns `None` if a write lock is held (useful to avoid deadlocks
    /// when called from within a write lock callback on the same vertex).
    pub fn try_clone_data(&self) -> Option<VertexData> {
        self.try_read().map(|guard| guard.clone())
    }
}

impl Serialize for VertexEntry {
    fn serialize<S: Serializer>(
        &self,
        serializer: S,
    ) -> Result<S::Ok, S::Error> {
        // Acquire read lock and serialize the inner data
        self.read().serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for VertexEntry {
    fn deserialize<D: Deserializer<'de>>(
        deserializer: D
    ) -> Result<Self, D::Error> {
        // Deserialize the data and wrap in Arc<RwLock<...>>
        let data = VertexData::deserialize(deserializer)?;
        Ok(VertexEntry::new(data))
    }
}
