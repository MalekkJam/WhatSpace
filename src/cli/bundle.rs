use clap::Subcommand;

#[derive(Subcommand)]
enum BundleCommands {
    // Send a bundle
    Send {
        #[arg(short, long)]
        bundle_id : 
    },
    // status of the bundle
    status,
    // TTL of the bundle
    expiring_in,

}