//! BlazeList server binary — QUIC + WebTransport.

use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

use blazelist_protocol::RootState;
use blazelist_server::SqliteStorage;
use blazelist_server::https::{hex_encode, run_cert_hash_server, run_https_server, tls_acceptor};
use blazelist_server::quic::{run_server, self_signed_server_config};
use blazelist_server::webtransport::{run_webtransport_server, webtransport_server_config};
use clap::Parser;
use quinn::Endpoint;
use tokio::sync::broadcast;

/// BlazeList server — QUIC + WebTransport.
#[derive(Parser, Debug)]
#[command(name = "blazelist-server", version, about)]
struct Cli {
    /// QUIC listen port.
    #[arg(long, default_value = "47200")]
    quic_port: u16,

    /// WebTransport listen port.
    #[arg(long, default_value = "47400")]
    wt_port: u16,

    /// HTTP cert-hash endpoint port.
    #[arg(long, default_value = "47600")]
    http_port: u16,

    /// Bind address (applies to all listeners).
    #[arg(long, default_value = "127.0.0.1")]
    bind: String,

    /// Path to a directory of static files to serve over HTTPS.
    /// When set, serves the WASM frontend on --https-port using the
    /// same self-signed certificate as WebTransport.
    #[arg(long)]
    static_dir: Option<PathBuf>,

    /// HTTPS static-file server port (used only with --static-dir).
    #[arg(long, default_value = "47800")]
    https_port: u16,

    /// Path to the SQLite database file.
    #[arg(long, default_value = "blazelist.db")]
    db: PathBuf,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let quic_addr: SocketAddr = format!("{}:{}", cli.bind, cli.quic_port).parse()?;
    let wt_addr: SocketAddr = format!("{}:{}", cli.bind, cli.wt_port).parse()?;
    let http_addr: SocketAddr = format!("{}:{}", cli.bind, cli.http_port).parse()?;
    let db_path = cli.db;

    println!("Opening database at {}", db_path.display());
    let allow_migration = std::env::var("BLAZELIST_ALLOW_IRREVERSIBLE_AUTOMATIC_UPGRADE_MIGRATION")
        .map(|v| v.eq_ignore_ascii_case("true"))
        .unwrap_or(false);
    let storage = Arc::new(SqliteStorage::open(&db_path, allow_migration)?);

    // Shared broadcast channel for notifications across both transports.
    let (notify_tx, _) = broadcast::channel::<RootState>(64);

    // Generate self-signed cert shared by both QUIC and WebTransport.
    let (quic_server_config, cert_material) = self_signed_server_config()?;

    // --- QUIC server ---
    let quic_endpoint = Endpoint::server(quic_server_config, quic_addr)?;
    println!("QUIC server listening on {quic_addr}");

    let quic_storage = Arc::clone(&storage);
    let quic_notify_tx = notify_tx.clone();
    let quic_handle = tokio::spawn(async move {
        run_server(quic_endpoint, quic_storage, quic_notify_tx).await;
    });

    // --- WebTransport server ---
    let wt = webtransport_server_config(&cert_material.cert_der, &cert_material.key_der, wt_addr)?;
    println!("WebTransport server listening on {wt_addr}");

    let wt_storage = Arc::clone(&storage);
    let wt_notify_tx = notify_tx.clone();
    let wt_handle = tokio::spawn(async move {
        run_webtransport_server(wt.config, wt_storage, wt_notify_tx).await;
    });

    // --- HTTP cert-hash endpoint ---
    // Serves the certificate SHA-256 hash as hex so WASM clients can
    // auto-fetch it for `serverCertificateHashes`.
    let cert_hash_hex = hex_encode(&wt.cert_hash);
    let http_handle = tokio::spawn(async move {
        run_cert_hash_server(http_addr, cert_hash_hex).await;
    });
    println!("Cert-hash HTTP endpoint on http://{http_addr}/cert-hash");

    // --- HTTPS static-file server (opt-in) ---
    let https_handle = if let Some(ref static_dir) = cli.static_dir {
        let https_addr: SocketAddr = format!("{}:{}", cli.bind, cli.https_port).parse()?;
        let acceptor = tls_acceptor(&cert_material.cert_der, &cert_material.key_der)?;
        let cert_hash_for_https = hex_encode(&wt.cert_hash);
        let dir = static_dir.clone();

        println!(
            "HTTPS static-file server on https://{https_addr} (serving {})",
            dir.display()
        );
        Some(tokio::spawn(async move {
            run_https_server(https_addr, dir, cert_hash_for_https, acceptor).await;
        }))
    } else {
        None
    };

    // Wait for any server to exit (they run forever in normal operation).
    tokio::select! {
        _ = quic_handle => eprintln!("QUIC server exited"),
        _ = wt_handle => eprintln!("WebTransport server exited"),
        _ = http_handle => eprintln!("HTTP cert-hash server exited"),
        _ = async {
            match https_handle {
                Some(h) => { let _ = h.await; }
                None => std::future::pending().await,
            }
        } => eprintln!("HTTPS static server exited"),
    }

    Ok(())
}
