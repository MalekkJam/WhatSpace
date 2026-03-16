pub mod engine;
pub mod epidemic;

pub use engine::RoutingEngine;
pub use epidemic::NetworkGraph;
// this file,is to tell Rust that the directory bundle is a module 
// every directory in rust should have a mod.rs file to tell rust that this directory is a module and to include the different files in this directory as submodules of the bundle module
pub mod model; // this line tells rust to include the model.rs file as a submodule
