use crate::routing::model::Node;
mod cli;
mod network;
mod routing;
mod storage;

use clap::Parser;
use cli::cli::Cli;
use cli::handler::handle_command;
use network::server::Server;
use std::io::{self, Write};
use uuid::Uuid;

fn node_id(name: &str) -> Uuid {
    let namespace = Uuid::parse_str("6ba7b810-9dad-11d1-80b4-00c04fd430c8").unwrap();
    Uuid::new_v5(&namespace, name.as_bytes())
}

#[tokio::main]
async fn main() {
    let alice_id = node_id("alice");
    let bob_id = node_id("bob");
    let carol_id = node_id("carol");
    let syrine_id = node_id("syrine");

    let alice = Node::new("alice", "127.0.0.1", 9001, vec![bob_id]);
    let bob = Node::new("bob", "127.0.0.1", 9002, vec![carol_id]);
    let carol = Node::new("carol", "127.0.0.1", 9003, vec![syrine_id]);
    let syrine = Node::new("syrine", "127.0.0.1", 9004, vec![]);
    let mut nodes = vec![alice, bob, carol, syrine];

    let args: Vec<String> = std::env::args().collect();

    if args.len() > 1 && (args[1] == "serve" || args[1] == "server") {
        println!("Starting registry server on 127.0.0.1:8080");
        let server = Server::new();
        server.start_server();
        return;
    }

    // One-shot mode: subcommands passed directly on the command line.
    if args.len() > 1 {
        let cli = Cli::parse();
        handle_command(cli.command, &mut nodes).await;
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
