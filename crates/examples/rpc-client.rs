//! A simple RPC client that sends transactions to the RPC server on the validator
//! according to the specified transactions per second (TPS) rate

use anyhow::{Context, Result};
use clap::Parser;
use std::{
    net::SocketAddr,
    sync::{atomic::AtomicU64, Arc},
};

use hotshot::helpers::initialize_logging;

include!("rpc.rs");

/// The RPC client service, used to dispatch transactions to the RPC server on the validator
#[derive(Parser)]
struct Args {
    /// The rpc url to connect to
    #[arg(long, default_value = "127.0.0.1:5000")]
    rpc_url: String,

    /// The rate of transactions per second to send
    #[arg(long, default_value = "100")]
    tps: u64,

    /// The number of transactions to send
    /// If not specified, the client will run indefinitely
    /// If specified, the client will stop after sending the specified number of transactions
    #[arg(long)]
    total_txs: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    initialize_logging();

    // Parse the command-line arguments
    let args = Args::parse();

    // Parse the RPC URL
    let rpc_url = args
        .rpc_url
        .parse::<SocketAddr>()
        .with_context(|| "Failed to parse RPC URL")?;

    // every 1s, send a batch of `tps` transactions made of random bytes
    // if `total_txs` is defined, stop after sending the specified number of transactions
    // otherwise, run indefinitely
    let total_txs = args.total_txs.unwrap_or(0);
    let tps = args.tps;
    let txs_sent = Arc::new(AtomicU64::new(0));
    loop {
        tokio::spawn(send_txs(rpc_url, tps, total_txs, Arc::clone(&txs_sent)));

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let txs_sent_value = txs_sent.load(std::sync::atomic::Ordering::SeqCst);
        if total_txs > 0 && txs_sent_value >= total_txs {
            tracing::debug!(
                "Sent {} transactions while limit is {}, stopping",
                txs_sent_value,
                total_txs
            );
            return Ok(());
        }
    }
}

async fn send_txs(
    rpc_url: SocketAddr,
    tps: u64,
    total_txs: u64,
    txs_sent: Arc<AtomicU64>,
) -> Result<()> {
    let mut txs = Vec::new();
    for _ in 0..tps {
        txs.push(rand::random::<[u8; 32]>().to_vec());
    }

    let client = reqwest::Client::new();
    let txs_sent_value = txs_sent.load(std::sync::atomic::Ordering::SeqCst);
    if total_txs > 0 && txs_sent_value >= total_txs {
        tracing::debug!("Sent {} transactions, stopping", txs_sent_value);
        return Ok(());
    }
    let rpc_request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "send_txs".to_string(),
        params: serde_json::json!({ "txs": txs }),
        id: txs_sent_value,
    };

    tracing::debug!("Sending {} transactions", tps);

    let response = client
        .post(format!("http://{}", rpc_url))
        .json(&rpc_request)
        .send()
        .await
        .with_context(|| "Failed to send RPC request")?;

    let response: RpcResponse = response
        .json()
        .await
        .with_context(|| "Failed to parse RPC response")?;

    tracing::debug!("RPC response: {:?}", response);

    txs_sent.fetch_add(tps, std::sync::atomic::Ordering::SeqCst);

    Ok(())
}
