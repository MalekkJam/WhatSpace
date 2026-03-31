use clap::{Parser, Subcommand};

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Bundle-related commands
    Bundle {
        #[command(subcommand)]
        action: BundleCommands,
    },
    /// Node-related commands
    Node {
        #[command(subcommand)]
        action: NodeCommands,
    },
}
