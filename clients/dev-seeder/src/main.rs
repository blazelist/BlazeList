//! BlazeList development seeder — generates deterministic test data and provisions
//! it to a BlazeList server via the QUIC protocol.
//!
//! Designed for development and testing. Uses a seeded RNG (ChaCha8) so all
//! generated data is reproducible by default.

mod client;
mod seed;

use std::net::SocketAddr;

use clap::Parser;

/// BlazeList dev seeder — generate and provision test data. 🌱
#[derive(Parser, Debug)]
#[command(name = "blazelist-dev-seeder", version, about)]
struct Cli {
    /// Server address to connect to.
    #[arg(long, default_value = "127.0.0.1:47200")]
    server: SocketAddr,

    /// RNG seed for deterministic generation.
    #[arg(long, default_value = "42")]
    seed: u64,

    /// Number of cards to generate.
    #[arg(long, default_value = "1200")]
    cards: usize,

    /// Number of tags to generate.
    #[arg(long, default_value = "50")]
    tags: usize,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let data = seed::generate(cli.seed, cli.tags, cli.cards);

    let tag_versions: usize = data.tag_chains.iter().map(Vec::len).sum();
    let card_versions: usize = data.card_chains.iter().map(Vec::len).sum();

    println!(
        "Generated {} tags ({} versions), {} cards ({} versions), \
         +{} deleted tags, +{} deleted cards, +{} extra ops (seed={})",
        data.tag_chains.len(),
        tag_versions,
        data.card_chains.len(),
        card_versions,
        data.deleted_tag_chains.len(),
        data.deleted_card_chains.len(),
        data.extra_ops.len(),
        cli.seed,
    );

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let client = client::Client::connect(cli.server).await?;
        client.push_seed_data(&data).await?;
        println!("Seed data pushed successfully.");
        Ok(())
    })
}
