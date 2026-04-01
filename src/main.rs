mod cli;
mod network;
mod routing;
mod storage;

use clap::Parser;
use cli::nodeCli::Cli;
use cli::handler::handle_command;
use network::server::Server;
use routing::model::Node;
use std::io::{self, Write};

#[tokio::main]
async fn main() {
    println!("🚀 WhatSpace - Delay-Tolerant Network Demo\n");
    
    // Create demo nodes for testing
    let mut nodes = vec![
        Node::new("alice", "127.0.0.1", 8080, vec![]),
        Node::new("bob",   "127.0.0.1", 8081, vec![]),
        Node::new("carol", "127.0.0.1", 8082, vec![]),
    ];

    println!("✅ Created 3 demo nodes: alice, bob, carol\n");
    println!("📋 Available commands:\n");
    println!("   cargo run -- all");
    println!("      → List all available nodes\n");
    println!("   cargo run -- status alice");
    println!("      → Check status of a specific node\n");
    println!("   cargo run -- peers alice list-peers");
    println!("      → List peers for a node\n");
    println!("   cargo run -- peers alice add bob");
    println!("      → Add a peer to a node\n");
    println!("   cargo run -- send --from alice --to bob --message \"Hello\" --ttl 3600");
    println!("      → Send a message from one node to another\n");
    println!("   cargo run");
    println!("      → Start interactive real-time mode (state is kept in memory)\n");
    println!("   cargo run -- serve");
    println!("      → Start the central registry server on 127.0.0.1:8080\n");

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && args[1] == "serve" {
        println!("Starting registry server on 127.0.0.1:8080");
        let server = Server::new();
        server.start_server();
        return;
    }

    // If subcommands are passed, run one-shot mode.
    if args.len() > 1 {
        let cli = Cli::parse();
        handle_command(cli.command, &mut nodes).await;
        return;
    }

    // Interactive mode keeps node state across many commands in one process.
    println!("Entering interactive mode. Type 'help' for examples, 'exit' to quit.");
    loop {
        print!("ws> ");
        let _ = io::stdout().flush();

        let mut line = String::new();
        if io::stdin().read_line(&mut line).is_err() {
            eprintln!("Failed to read input");
            continue;
        }

        let input = line.trim();
        if input.is_empty() {
            continue;
        }

        if input.eq_ignore_ascii_case("exit") || input.eq_ignore_ascii_case("quit") {
            println!("Bye.");
            break;
        }

        if input.eq_ignore_ascii_case("help") {
            println!("Commands:");
            println!("  all");
            println!("  status <name>");
            println!("  peers <name> list-peers");
            println!("  peers <name> add <uuid>");
            println!("  peers <name> remove <uuid>");
            println!("  send --from <name> --to <name> --message <msg> --ttl <seconds>");
            println!("  start <name> --server <ip:port>");
            println!("  stop <name>");
            continue;
        }

        // Build argv for clap: binary_name + tokens from input line.
        let argv = std::iter::once("WhatSpace".to_string())
            .chain(input.split_whitespace().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        match Cli::try_parse_from(argv) {
            Ok(cli) => handle_command(cli.command, &mut nodes).await,
            Err(e) => println!("{}", e),
        }
    }
}