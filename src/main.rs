mod routing;
mod network;
mod storage;

use storage::JsonFileStorage;

fn main() {
    // creer le,ficheir de storage automatiquement au lancer de l'app
    let storage = Box ::new(JsonFileStorage ::new("./bundles".to_string()));
    
}
