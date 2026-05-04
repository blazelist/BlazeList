//! BlazeList development seeder — generates deterministic test data and provisions
//! it to a BlazeList server via the QUIC protocol.
//!
//! Designed for development and testing. Uses a seeded RNG (ChaCha8) so all
//! generated data is reproducible by default.

mod client;
mod seed;

use std::net::SocketAddr;

use clap::{Parser, ValueEnum};

/// Pre-defined environment sizes for quick dev setup.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum Preset {
    /// Minimal dataset for fast iteration (120 cards, 8 tags).
    Small,
    /// Everyday development (400 cards, 18 tags). Default.
    Medium,
    /// Full stress-test dataset (1200 cards, 50 tags).
    Large,
}

impl Preset {
    const fn cards(self) -> usize {
        match self {
            Self::Small => 120,
            Self::Medium => 400,
            Self::Large => 1200,
        }
    }

    const fn tags(self) -> usize {
        match self {
            Self::Small => 8,
            Self::Medium => 18,
            Self::Large => 50,
        }
    }
}

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

    /// Environment size preset (small, medium, large).
    #[arg(long, default_value = "medium")]
    preset: Preset,

    /// Number of cards to generate (overrides preset).
    #[arg(long)]
    cards: Option<usize>,

    /// Number of tags to generate (overrides preset).
    #[arg(long)]
    tags: Option<usize>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cli = Cli::parse();

    let num_cards = cli.cards.unwrap_or_else(|| cli.preset.cards());
    let num_tags = cli.tags.unwrap_or_else(|| cli.preset.tags());

    let data = seed::generate(cli.seed, num_tags, num_cards);

    let tag_versions: usize = data.tag_chains.iter().map(Vec::len).sum();
    let card_versions: usize = data.card_chains.iter().map(Vec::len).sum();

    tracing::info!(
        tags = data.tag_chains.len(),
        tag_versions,
        cards = data.card_chains.len(),
        card_versions,
        deleted_tags = data.deleted_tag_chains.len(),
        deleted_cards = data.deleted_card_chains.len(),
        extra_ops = data.extra_ops.len(),
        seed = cli.seed,
        preset = ?cli.preset,
        "Generated seed data"
    );

    let rt = tokio::runtime::Runtime::new()?;
    rt.block_on(async {
        let client = client::Client::connect(cli.server).await?;
        client.push_seed_data(&data).await?;
        tracing::info!("Seed data pushed successfully");
        Ok(())
    })
}
