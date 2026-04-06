use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::fmt;
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::sync::{LazyLock, Mutex};
use uuid::Uuid;

use crate::routing::model::{Bundle, BundleKind};

static STORAGE_IO_LOCK: LazyLock<Mutex<()>> = LazyLock::new(|| Mutex::new(()));

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
    fn has_ack_for_data_id(&self, data_id: Uuid) -> bool {
        self.bundles.iter().any(|b| match &b.kind {
            BundleKind::Ack { ack_bundle_id } => *ack_bundle_id == data_id,
            _ => false,
        })
    }

    fn load_bundles_from_file_unlocked(storage_dir: &PathBuf) -> Vec<Bundle> {
        let file_path = storage_dir.join("bundles.json");

        if !file_path.exists() {
            return Vec::new();
        }

        match fs::read_to_string(&file_path) {
            Ok(content) => match serde_json::from_str::<Value>(&content) {
                Ok(json_value) => json_value
                    .get("bundles")
                    .and_then(|v| v.as_array())
                    .map(|bundles_array| {
                        bundles_array
                            .iter()
                            .filter_map(|bundle_json| {
                                serde_json::from_value::<Bundle>(bundle_json.clone()).ok()
                            })
                            .collect()
                    })
                    .unwrap_or_default(),
                Err(e) => {
                    eprintln!("Error parsing bundles.json: {}", e);
                    Vec::new()
                }
            },
            Err(e) => {
                eprintln!("Error the reading of bundles.json didn't work : {}", e);
                Vec::new()
            }
        }
    }

    fn load_bundles_from_file(storage_dir: &PathBuf) -> Vec<Bundle> {
        let _guard = STORAGE_IO_LOCK.lock().unwrap();
        Self::load_bundles_from_file_unlocked(storage_dir)
    }

    fn refresh_from_disk(&mut self) {
        self.bundles = Self::load_bundles_from_file(&self.storage_dir);
    }

    pub fn save_ack_if_new_for_data(&mut self, ack: &Bundle) -> bool {
        let BundleKind::Ack { ack_bundle_id } = &ack.kind else {
            return false;
        };

        let _guard = STORAGE_IO_LOCK.lock().unwrap();

        self.bundles = Self::load_bundles_from_file_unlocked(&self.storage_dir);

        let already_exists = self.bundles.iter().any(|b| match &b.kind {
            BundleKind::Ack { ack_bundle_id: existing } => *existing == *ack_bundle_id,
            _ => false,
        });

        if already_exists {
            return false;
        }

        if self.bundles.len() >= self.capacity {
            return false;
        }

        self.bundles.push(ack.clone());

        let json_bundles: Vec<Value> = self
            .bundles
            .iter()
            .filter_map(|bundle| serde_json::to_value(bundle).ok())
            .collect();

        let json_content = json!({ "bundles": json_bundles });
        let file_path = self.storage_dir.join("bundles.json");
        let tmp_path = self.storage_dir.join("bundles.json.tmp");

        let payload = match serde_json::to_string_pretty(&json_content) {
            Ok(p) => p,
            Err(_) => return false,
        };

        fs::File::create(&tmp_path)
            .and_then(|mut f| {
                f.write_all(payload.as_bytes())?;
                f.sync_all()
            })
            .and_then(|_| fs::rename(&tmp_path, &file_path))
            .is_ok()
    }

    pub fn new(directory: String, capacity: usize) -> Self {
        // initialisation au demarrage
        let mut storage_dir = PathBuf::from(&directory); // constructeur prend chemin repertoir et retourne une instance jsonfilestorage
                                                         // convertit le string en PathBuf qui est un type rust optimisé pour les chemins

        if storage_dir.is_relative() {
            storage_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(storage_dir);
        }

        // SI LE REPERTOIRE N'EXISTE PAS IN LE CREE
        if !storage_dir.exists() {
            fs::create_dir_all(&storage_dir).expect("Failed to create storage directory");
            println!("created storzge directory at {}", storage_dir.display());
        }

        // on va construirte tout le chemain complet
        // bundels/bundles.json
        let bundles = Self::load_bundles_from_file(&storage_dir);
        StorageLayer {
            storage_dir,
            bundles,
            capacity,
        }
    }

    pub fn save_to_file(&self) -> Result<(), StorageError> {
        let _guard = STORAGE_IO_LOCK.lock().unwrap();
        let json_bundles: Vec<Value> = self
            .bundles
            .iter()
            .filter_map(|bundle| serde_json::to_value(bundle).ok())
            .collect();

        let json_content = json!({ "bundles": json_bundles });

        let file_path = self.storage_dir.join("bundles.json");
        let tmp_path = self.storage_dir.join("bundles.json.tmp");
        let payload = serde_json::to_string_pretty(&json_content).unwrap();

        match fs::File::create(&tmp_path)
            .and_then(|mut f| {
                f.write_all(payload.as_bytes())?;
                f.sync_all()
            })
            .and_then(|_| fs::rename(&tmp_path, &file_path))
        {
            Ok(_) => Ok(()),
            Err(e) => {
                let error = StorageError::SerializationError(format!(
                    "Failed to write bundles.json: {}",
                    e
                ));
                eprintln!("{}", error);
                Err(error)
            }
        }
    }

    // Issue #11
    // Duplicate detection logic
    pub fn save_bundle(&mut self, bundle: &Bundle) -> bool {
        self.refresh_from_disk();
        self.cleanup_expired_bundles();

        // If an ACK for this DATA already exists, do not resurrect the DATA bundle.
        if matches!(bundle.kind, BundleKind::Data { .. }) && self.has_ack_for_data_id(bundle.id) {
            return false;
        }

        // Check if we have reached storage capacity
        if self.bundles.len() >= self.capacity {
            let err = StorageError::StorageFull(bundle.id.to_string());
            eprintln!("{}", err);
            return false; // Storage is  full, reject new bundle and abort saving
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
    pub fn get_bundle(&mut self, bundle_id: Uuid) -> Option<Bundle> {
        self.refresh_from_disk();
        self.bundles.iter().find(|b| b.id == bundle_id).cloned()
    }

    //Retrieve all bundles
    pub fn get_all_bundles(&mut self) -> Vec<Bundle> {
        self.refresh_from_disk();
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
        self.refresh_from_disk();
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
        self.refresh_from_disk();
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
            println!(
                "Storage: Cleaned up {} expired bundle(s) in a single disk write.",
                removed_count
            );
        }

        removed_count
    }

    // Update an existing bundle in storage (used for status transitions).
    pub fn update_bundle(&mut self, bundle: &Bundle) -> bool {
        self.refresh_from_disk();

        // If an ACK for this DATA already exists, do not resurrect/update it.
        if matches!(bundle.kind, BundleKind::Data { .. }) && self.has_ack_for_data_id(bundle.id) {
            return false;
        }

        if let Some(existing) = self.bundles.iter_mut().find(|b| b.id == bundle.id) {
            *existing = bundle.clone();
            match self.save_to_file() {
                Ok(_) => true,
                Err(e) => {
                    eprintln!("Error saving updated bundle to file: {}", e);
                    false
                }
            }
        } else {
            eprintln!("{}", StorageError::NotFound(bundle.id.to_string()));
            false
        }
    }
}
