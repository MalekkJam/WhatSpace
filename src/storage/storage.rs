use uuid::Uuid;
use std::fmt;
use std::path::{Path, PathBuf};
 
use crate::routing::model::{Bundle};
use super::storage_interface::{StorageLayer, StorageError};
use crate:: JsonFileStorage;

// implementation of StorageLayer
impl StorageLayer for JsonFileStorage{
    // Issue #11
    // Duplicate detection logic 
    fn save_bundle(&mut self, bundle: &Bundle) -> bool {
        //check if the bundle ID already exists in the shared file bundle.json
        if self.bundles.iter().any(|b| b.id == bundle.id) {
            eprintln!("{}", StorageError::AlreadyExists(bundle.id.to_string()));
            return false;
        }

        // Add the new bundle to the array
        self.bundles.push(bundle.clone());

        //Save the updates to the JSON file
        match self.save_to_file() {
            Ok(_) => true, //saving succeeded
            Err(e) => {
                eprintln!("Error saving bundle to file: {}", e);
                false
            }
        }
    }

    //Retrieve a specific bundle
    fn get_bundle(&self, bundle_id: Uuid) -> Option<Bundle> {
        self.bundles.iter().find(|b| b.id == bundle_id).cloned()
    }

    //Retrieve all bundles
    fn get_all_bundles(&self) -> Vec<Bundle> {
        self.bundles.clone()
    }

    //retrieve bundles originating from a specific node
    fn get_bundles_by_node(&self, node_id: Uuid) -> Vec<Uuid> {
        self.bundles
            .iter()
            .filter(|b| b.source.id == node_id)
            .map(|b| b.id)
            .collect()
    }

    // Issue #13
    //Implement bundle deletion after delivery
    fn delete_bundle(&mut self, bundle_id: Uuid) -> bool {
        let initial_len = self.bundles.len();

        // keep all bundles except the one matching the id we want to delete
        self.bundles.retain(|b| b.id != bundle_id);

        //if the length decreased, it means we successfully removed a bundle, so we save the updates to the JSON file
        if self.bundles.len() < initial_len {
            match self.save_to_file() {
                Ok(_) => true, //saving succeeded
                Err(e) => {
                    eprintln!("Error saving bundle to file after deletion: {}", e);
                    false
                }
            }
        } else {
            eprintln!("{}", StorageError::NotFound(bundle_id.to_string()));
            false //no bundle with the given id was found
            
        }
    }

    //Issue #12
    //iterates through the shared file, checks expiration and removes expired bundles
    fn cleanup_expired_bundles(&mut self) -> usize {
        let initial_len = self.bundles.len();

        //keep only bundles that are not expired
        self.bundles.retain(|b| !b.is_expired());

        // calculation of how many bundles were deleted
        let removed_count = initial_len - self.bundles.len();

        if removed_count > 0 {
        //save_to_file() returns a Result (Success or Error). 
        // Rust's safety rules require us to acknowledge this Result.
        // By assigning it to the underscore wildcard (let _ =), we tell the 
        // compiler: "I am intentionally ignoring the return value because 
        // save_to_file() already prints its own error messages."
            let _ = self.save_to_file();
            println!("Storage: Cleaned up {} expired bundle(s) in a single disk write.", removed_count);
        }

        removed_count
    }

}
