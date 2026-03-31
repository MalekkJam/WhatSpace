use std::fs; // Filesystem manipulation operations
use std:: path::PathBuf;
use uuid::Uuid;
use serde_json::{Value, json};


use crate::routing::model::Bundle;
use super::storage_interface::{StorageLayer, StorageError};

#[derive(Debug)]
pub struct JsonFileStorage {
    pub storage_dir: PathBuf,
    pub bundles: Vec<Bundle>,
    pub capacity: usize, //maximum number of bundles this node can store
}

impl JsonFileStorage {
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
        
        JsonFileStorage { storage_dir, bundles, capacity }
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
}
