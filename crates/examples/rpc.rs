use warp::Filter;

use serde_json::json;
use tokio::sync::mpsc::Sender;

use serde_derive::{Deserialize, Serialize};

#[derive(Deserialize, Serialize, Debug)]
struct RpcRequest {
    jsonrpc: String,
    method: String,
    params: serde_json::Value,
    id: u64,
}

#[derive(Deserialize, Serialize, Debug)]
struct RpcResponse {
    jsonrpc: String,
    result: serde_json::Value,
    id: u64,
}

/// Starts the RPC server
pub async fn start_rpc(rpc_port: u16, tx_send: tokio::sync::mpsc::Sender<Vec<u8>>) -> Result<()> {
    let tx_send_filter = warp::any().map(move || tx_send.clone());
    
    let jrpc = warp::post()
        .and(warp::body::content_length_limit(1024 * 16))
        .and(warp::body::json())
        .and(tx_send_filter)
        .and_then(handle_rpc_request);

    tracing::debug!("Starting RPC server on: 0.0.0.0:{}", rpc_port);

    warp::serve(jrpc).run(([0, 0, 0, 0], rpc_port)).await;

    Ok(())
}

async fn handle_rpc_request(req: RpcRequest, tx_send: Sender<Vec<u8>>) -> Result<impl warp::Reply, warp::Rejection> {
    match req.method.as_str() {
        "send_txs" => {
            if let Some(txs) = req.params.get("txs").and_then(|v| v.as_array()) {
                match rpc_send_txs(txs, tx_send).await {
                    Ok(_) => {
                        let response = RpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: json!(true),
                            id: req.id,
                        };
                        Ok(warp::reply::json(&response))
                    }
                    Err(e) => {
                        tracing::warn!("Failed to handle send_txs: {}", e);
                        let response = RpcResponse {
                            jsonrpc: "2.0".to_string(),
                            result: json!(e.to_string()), // TODO: avoid returning error message
                            id: req.id,
                        };
                        Ok(warp::reply::json(&response))
                    }
                }
            } else {
                Err(warp::reject())
            }
        }
        _ => Err(warp::reject()),
    }
}

async fn rpc_send_txs(txs: &Vec<serde_json::Value>, tx_send: Sender<Vec<u8>>) -> Result<()> {
    let txs_bytes = txs
        .iter()
        .map(|tx| {
            match tx.as_str() {
                Some(tx) => Ok(tx.as_bytes().to_vec()),
                None => Err(anyhow::anyhow!("Failed to parse tx as string")),
            }
        })
        .collect::<Result<Vec<_>, _>>()?;
    tracing::debug!("Processing {} RPC transactions", txs_bytes.len());
    // Send the transactions
    for tx in txs_bytes {
        tx_send.send(tx).await?;
    }
    Ok(())
}
