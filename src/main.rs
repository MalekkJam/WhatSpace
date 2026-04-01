use crate::routing::model::Node;
mod cli;
mod network;
mod routing;
mod storage;

use clap::Parser;
use cli::cli::Cli;
use cli::handler::handle_command;
use network::client::connect_to_server;
use network::server::Server;
use std::io::{self, Write};

#[tokio::main]
async fn main() {
    let node1 = Node::new("alice", "127.0.0.1", 8080, vec![]);
    let node2 = Node::new("bob", "127.0.0.1", 8081, vec![]);
    let node3 = Node::new("carol", "127.0.0.1", 8082, vec![]);
    let node4 = Node::new("syrine", "127.0.0.1", 8083, vec![]);
    let mut nodes = vec![node1, node2, node3, node4];

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