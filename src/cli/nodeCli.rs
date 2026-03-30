use clap::{Parser, Subcommand};

enum NodeCommands {
    /// Register the node in the server registry
    Register,
    /// Stop the node server
    GetPeers {
        /// Optional list of peer IDs to retrieve (comma-separated)
        #[arg(short, long)]
        ids: Option<String>,
    },
    /// Get the status of the node
    Status,
}