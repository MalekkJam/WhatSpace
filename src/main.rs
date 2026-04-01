use crate::routing::model::Node;
mod cli;
mod network;
mod routing;
mod storage;

use clap::Parser;
use cli::cli::Cli;
use cli::cli::NodeCommands;
use cli::handler::handle_command;
use network::server::Server;
use std::io::{self, Write};
use std::time::Duration;

#[tokio::main]
async fn main() {
    let mut nodes = vec![
        Node::new("alice", "127.0.0.1", 9001, vec![]), // this creates node alice on a port that does not conflict with the registry server
        Node::new("bob", "127.0.0.1", 9002, vec![]), // this creates node bob as the second simulation participant
        Node::new("carol", "127.0.0.1", 9003, vec![]), // this creates node carol as the third simulation participant
    ];

    let all_ids: Vec<_> = nodes.iter().map(|node| node.id).collect(); // this captures node ids once to build a default full-mesh peer topology
    for node in &mut nodes {
        node.peers = all_ids
            .iter()
            .copied()
            .filter(|peer_id| *peer_id != node.id)
            .collect(); // this sets each node's peer list to every other node id for immediate epidemic routing
        if let Some(engine) = &node.routing_engine {
            if let Ok(mut guard) = engine.lock() {
                guard.peers = node.peers.clone(); // this synchronizes routing engine peer list with node-level peer configuration
            }
        }
    }

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && (args[1] == "serve" || args[1] == "server") {
        println!("Starting registry server on 127.0.0.1:9100"); // this prints the new default registry endpoint to avoid 8080 conflicts
        let server = Server::new();
        server.start_server();
        return;
    }

    // One-shot mode: subcommands passed directly on the command line.
    if args.len() > 1 {
        let cli = Cli::parse();
        let keep_alive = matches!(&cli.command, NodeCommands::Start { .. }); // this detects node startup commands that must keep process alive to preserve listener and registry stream
        handle_command(cli.command, &mut nodes).await;
        if keep_alive {
            println!("Node process is running. Press Ctrl+C to stop."); // this informs operator that start command intentionally keeps process alive
            loop {
                tokio::time::sleep(Duration::from_secs(3600)).await; // this keeps the process alive so the node remains reachable in multi-terminal simulations
            }
        }
        return;
    }

    // Interactive mode: keeps node state across many commands in one process.
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
            println!("  peers <name> add <peer-name>");
            println!("  peers <name> remove <peer-name>");
            println!("  send --from <name> --to <name> --message <msg> --ttl <seconds>");
            println!("  start <name> --server <ip:port>");
            println!("  stop <name>");
            continue;
        }

        let argv = std::iter::once("WhatSpace".to_string())
            .chain(input.split_whitespace().map(|s| s.to_string()))
            .collect::<Vec<String>>();

        match Cli::try_parse_from(argv) {
            Ok(cli) => handle_command(cli.command, &mut nodes).await,
            Err(e) => println!("{}", e),
        }
    }
    // _registry_stream drops here — server correctly marks node as disconnected
}