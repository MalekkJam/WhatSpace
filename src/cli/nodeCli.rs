use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "node")]

#[derive(Parser)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}


#[derive(Subcommand)]
pub enum NodeCommands {
    
    /// List all nodes created in the demo
    All,
    /// Start the node and register it with the server
    Start {
        /// Name of the node to start
        name: String,

        /// Server address to register with (ip:port)
        #[arg(short, long)]
        server: String,
    },
    /// Stop the node server
    Stop{
        /// Name of the node to stop
        name: String,
    },
    
    /// Get the status of the node
    Status{
        /// Name of the node to check status for
        name: String,
    },

    /// Manage peers of a node
    Peers {
        /// Name of the node
        name: String,

        #[command(subcommand)]
        command: PeerCommands,
    },

    Send {
        /// Name of the sender node
        #[arg(long)]
        from: String,

        /// name of the destination node
        #[arg(long)]
        to: String,

        /// The message content
        #[arg(long)]
        message: String,

        /// Time-to-live in seconds
        #[arg(long)]
        ttl: u64,
    },

    #[cfg(feature = "debug")]
    Debug {
        /// Name of the node to dump, or all if omitted
        name: Option<String>,
    },

}

#[derive(Subcommand)]
pub enum PeerCommands {
    /// List all known peers
    ListPeers,

    /// Fetch one or more peers from the server by UUID
    GetConnectedPeers {
        /// One or more peer UUIDs
        #[arg(required = true, num_args = 1..)]
        ids: Vec<String>,
    },

    /// Add a peer UUID to your local peer list
    Add {
        /// UUID of the peer to add
        id: String,
    },

    /// Remove a peer UUID from your local peer list
    Remove {
        /// UUID of the peer to remove
        id: String,
    },
}

