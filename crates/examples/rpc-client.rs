//! A simple RPC client that sends transactions to the RPC server on the validator
//! according to the specified transactions per second (TPS) rate

use anyhow::{Context, Result};
use clap::Parser;
use hotshot::helpers::initialize_logging_with_file;
use libp2p::Multiaddr;
use rand::Rng as _;
use std::sync::{atomic::AtomicU64, Arc};

include!("rpc.rs");

/// The RPC client service, used to dispatch transactions to the RPC server on the validator
#[derive(Parser, Debug)]
struct Args {
    /// The coordinator to use for working against multiple nodes.
    /// If set, the client will use the coordinator to get all the available RPCs to connect to.
    #[arg(long)]
    coordinator_url: Option<String>,

    /// The rpc url to connect to
    #[arg(long, default_value = "127.0.0.1:5000")]
    rpc_url: String,

    /// The rate of transactions per second to send
    #[arg(long, default_value = "2")]
    tps: u64,

    /// The size of each transaction in bytes
    /// The transaction will be filled with random bytes
    #[arg(long, default_value = "1024")]
    tx_size: u64,

    /// The number of transactions to send
    /// If not specified, the client will run indefinitely
    /// If specified, the client will stop after sending the specified number of transactions
    #[arg(long)]
    total_txs: Option<u64>,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize logging
    let _log_guard = initialize_logging_with_file();

    // Parse the command-line arguments
    let args = Args::parse();

    tracing::info!("Starting rpc-client with args: {:?}", args);

    let rpc_urls = if let Some(coorindator_url) = &args.coordinator_url {
        tracing::info!("Using coordinator at: {}", coorindator_url.clone());
        // Fetch the known libp2p nodes from the coordinator
        let p2p_info_url = format!("http://{}/libp2p-info", coorindator_url);
        let known_ips = reqwest::get(p2p_info_url.as_str())
            .await?
            .text()
            .await?
            .split('\n')
            .map(|s| {
                s.parse::<Multiaddr>()
                    .with_context(|| "Failed to parse Libp2p bootstrap address")
                    .unwrap()
            })
            .map(|addr| {
                let components = addr.iter().collect::<Vec<_>>();
                if components.len() < 2 {
                    return addr.to_string();
                }
                components[0].to_string()
            })
            .collect::<Vec<_>>();
        // Print the known libp2p nodes
        tracing::info!("Known libp2p nodes: {:?}", known_ips);

        let rpc_urls = known_ips
            .iter()
            .map(|ip| format!("http://{}:5000", ip.clone()))
            .collect::<Vec<_>>();

        rpc_urls
    } else {
        vec![args.rpc_url.clone()]
    };

    // every 1s, send a batch of `tps` transactions made of random bytes
    // if `total_txs` is defined, stop after sending the specified number of transactions
    // otherwise, run indefinitely
    let total_txs = args.total_txs;
    let tx_per_sec = args.tps;
    let tx_size = usize::try_from(args.tx_size)?;
    let txs_sent = Arc::new(AtomicU64::new(0));
    let mut iterations = 0;
    let num_rpcs = rpc_urls.len();
    if num_rpcs == 0 {
        return Err(anyhow::anyhow!("No RPCs to connect to"));
    }
    loop {
        let rpc_url = rpc_urls[get_next_rpc(iterations, num_rpcs)].clone();
        iterations += 1;
        tokio::spawn(send_txs(
            rpc_url,
            tx_per_sec,
            tx_size,
            Arc::clone(&txs_sent),
        ));

        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        let txs_sent_value = txs_sent.load(std::sync::atomic::Ordering::SeqCst);
        if let Some(total_txs) = total_txs {
            if txs_sent_value >= total_txs {
                tracing::debug!(
                    "Sent {} transactions while limit is {}, stopping",
                    txs_sent_value,
                    total_txs
                );
                return Ok(());
            }
        }
    }
}

fn get_next_rpc(iterations: u32, rpcs: usize) -> usize {
    if rpcs == 1 {
        return 0;
    }
    (iterations as usize) % rpcs
}

/// Sends transactions to the RPC server
async fn send_txs(
    rpc_url: String,
    tx_per_sec: u64,
    tx_size: usize,
    txs_sent: Arc<AtomicU64>,
) -> Result<()> {
    let mut txs: Vec<String> = Vec::new();
    for _ in 0..tx_per_sec {
        let mut transaction_bytes = vec![0u8; tx_size];
        rand::thread_rng().fill(&mut transaction_bytes[..]);
        txs.push(hex::encode(transaction_bytes));
    }

    let client = reqwest::Client::new();
    let txs_sent_value = txs_sent.load(std::sync::atomic::Ordering::SeqCst);
    let rpc_request = RpcRequest {
        jsonrpc: "2.0".to_string(),
        method: "send_txs".to_string(),
        params: serde_json::json!({ "txs": txs }),
        id: txs_sent_value,
    };

    let start_time = std::time::Instant::now();
    tracing::debug!("Sending {tx_per_sec} transactions");

    let response = client
        .post(format!("http://{rpc_url}"))
        .json(&rpc_request)
        .send()
        .await
        .with_context(|| "Failed to send RPC request")?;

    tracing::info!(
        "Got RPC response after {}ms",
        start_time.elapsed().as_millis()
    );

    let response: RpcResponse = response
        .json()
        .await
        .with_context(|| "Failed to parse RPC response")?;

    tracing::debug!("RPC response: {:?}", response);

    txs_sent.fetch_add(tx_per_sec, std::sync::atomic::Ordering::SeqCst);

    Ok(())
}
