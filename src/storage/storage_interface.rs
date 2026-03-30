use uuid::Uuid;
use std::fmt;
 
use crate::routing::model::{Bundle};


// Issue #17
// Error handling strategy
// The bundle manager will match on this enum to deide wether to retry or log

#[derive(Debug)]
pub enum StorageError {
    //No bundle with the requested id exists in the storage
    NotFound(String),

    //A bundle with this id already exists in the storage
    AlreadyExists(String),

    //The storage is full and cannot accept new bundles
    StorageFull(String),

    //A record could not be serialized or deserialized correctly
    SerializationError(String),
}

// Eroor display
impl fmt::Display for StorageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StorageError::NotFound(id) => write!(f, "Bundle with id {} not found", id),
            StorageError::AlreadyExists(id) => write!(f, "Bundle with id {} already exists", id),
            StorageError::StorageFull(id) => write!(f, "Storage full for bundle with id {}", id),
            StorageError::SerializationError(id) => write!(f, "Serialization error for bundle with id {} (error in serialization or deserialization)", id),
        }
    }
}


// Storage layer interface (public API)
pub trait StorageLayer {
    // returns true on succes and false on failure (implementation should log the 
    //underlying StorageError before converting the bool to false so that the failure is observable)
    fn save_bundle(&mut self, bundle: &Bundle) -> bool;

    fn get_bundle(&self, bundle_id: Uuid) -> Option<Bundle>;

    // return every bundle that is currently stored
    fn get_all_bundles(&self) -> Vec<Bundle>;

    // Return the ids of all bundles that originated from `node_id`.
    fn get_bundles_by_node(&self, node_id: uuid::Uuid) -> Vec<Uuid>;

    fn delete_bundle(&mut self, bundle_id: Uuid) -> bool;

    fn cleanup_expired_bundles(&mut self) -> usize;
}
