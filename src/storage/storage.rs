use uuid::Uuid;
use std::fmt;
use std::path::{PathBuf};
use serde_json::{Value, json};
use serde::{Deserialize, Serialize};
use std::fs;

use crate::routing::model::{Bundle};


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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StorageLayer {
    pub storage_dir: PathBuf,
    pub bundles: Vec<Bundle>,
    pub capacity: usize, //maximum number of bundles this node can store
}

impl StorageLayer {

    pub fn new(directory: String, capacity: usize) -> Self {  // initialisation au demarrage 
        let storage_dir = PathBuf::from(&directory);// constructeur prend chemin repertoir et retourne une instance jsonfilestorage 
        // convertit le string en PathBuf qui est un type rust optimisé pour les chemins 

        // SI LE REPERTOIRE N'EXISTE PAS IN LE CREE
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).expect("Failed to create storage directory");
            println!("created storzge directory at {}", storage_dir.display());
            }

            // on va construirte tout le chemain complet 
            // bundels/bundles.json
            let file_path = storage_dir.join("bundles.json");

            // si le fichier existe on va charger les bundles existants deja 
            let bundles = if file_path.exists() {

               match fs::read_to_string(&file_path) { // si le fichier eciste on charge les bundles
                // essaie de lire le fichier en entier comme texte !!!!!!!  et retourne le content 
                Ok(content) => {
                    match serde_json::from_str::<Value>(&content) {// si ok on parse le json en structure value qui est un arbre generique 
                        Ok(json_value) => { // nouveau match car la serialisation peut echouer 
                            if let Some(bundles_array) = json_value.get("bundles").and_then(|v| v.as_array()) {
                                bundles_array
                                    .iter()
                                    .filter_map(|bundle_json| {
                                        serde_json::from_value::<Bundle>(bundle_json.clone()).ok()
                                    })
                                    .collect()
                            } else {
                                Vec::new()
                            }
                        }
                        Err(e) => {
                            eprintln!("Error parsing bundles.json: {}", e);
                            Vec::new()
                        }
                    }
                }
                Err(e) => {
                    // affiche l'erreur et retiurne un vecteur vide 
                    eprintln!("Error the reading of bundles.json didn't work : {}", e);
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };
        
        StorageLayer { storage_dir, bundles, capacity }
    }



    pub fn save_to_file(&self) -> Result<(), StorageError> {
        let json_bundles: Vec<Value> = self.bundles
            .iter()
            .filter_map(|bundle| serde_json::to_value(bundle).ok())
            .collect();
        
        let json_content = json!({ "bundles": json_bundles });
        
        let file_path = self.storage_dir.join("bundles.json");
        match fs::write(&file_path, serde_json::to_string_pretty(&json_content).unwrap()) {
            Ok(_) => {
                println!(" Bundles saved to {}", file_path.display());
                Ok(())
            }
            Err(e) => {
                let error = StorageError::SerializationError(format!("Failed to write bundles.json: {}", e));
                eprintln!("{}", error);
                Err(error)
            }
        }
    }



    // Issue #11
    // Duplicate detection logic 
    pub fn save_bundle(&mut self, bundle: &Bundle) -> bool {
        // Check if we have reached storage capacity
        if self.bundles.len() >= self.capacity {
            let err= StorageError::StorageFull(bundle.id.to_string());
            eprintln!("{}", err);
            return false; // Storage is full, reject new bundle and abort saving
        }

        //check if the bundle ID already exists in the shared file bundle.json
        if self.bundles.iter().any(|b| b.id == bundle.id) {
            let err = StorageError::AlreadyExists(bundle.id.to_string());
            eprintln!("{}", err);
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
    pub fn get_bundle(&self, bundle_id: Uuid) -> Option<Bundle> {
        self.bundles.iter().find(|b| b.id == bundle_id).cloned()
    }

    //Retrieve all bundles
    pub fn get_all_bundles(&self) -> Vec<Bundle> {
        self.bundles.clone()
    }

    //retrieve bundles originating from a specific node
    pub fn get_bundles_by_node(&self, node_id: Uuid) -> Vec<Uuid> {
        self.bundles
            .iter()
            .filter(|b| b.source.id == node_id)
            .map(|b| b.id)
            .collect()
    }

    // Issue #13
    //Implement bundle deletion after delivery
    pub fn delete_bundle(&mut self, bundle_id: Uuid) -> bool {
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
    pub fn cleanup_expired_bundles(&mut self) -> usize {
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
