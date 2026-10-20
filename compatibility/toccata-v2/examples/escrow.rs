//! Read-only inspection and unsigned previews. No wallet secrets or submission.
use std::{env, error::Error, fs, str::FromStr, time::Duration};

use kaspa_consensus_core::Hash;
use kaspa_wrpc_client::{KaspaRpcClient, WrpcEncoding};
use kaspascript_toccata_native::{TransactionOutpoint, escrow::*};
use serde_json::json;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn Error>> {
    let args: Vec<String> = env::args().skip(1).collect();
    if !matches!(args.len(), 1 | 2 | 6) {
        return Err("usage: escrow TERMS.json [BORSH_WRPC_URL [TXID:INDEX release|refund FEE_SOMPI COMPUTE_BUDGET]]".into());
    }
    let bytes = fs::read(&args[0]).map_err(|_| "cannot read public escrow terms file")?;
    let terms: EscrowTerms = serde_json::from_slice(&bytes).map_err(
        |_| "invalid public escrow terms; private keys and extra fields are not accepted",
    )?;
    let escrow = BuyerSellerEscrow::new(terms)?;
    let mut report = json!({
        "status": "experimental TN10; wallet integration and live lifecycle unverified",
        "terms": escrow.terms(),
        "locking_address": escrow.address().to_string(),
        "source_hash": escrow.source_hash(),
        "redeem_script_hex": hex::encode(escrow.redeem_script()),
        "broadcast": false,
        "node_acceptance_verified": false,
        "warning": "Never fund public fixture keys. Both participants must review the payout, deadline and fee cap."
    });
    if args.len() > 1 {
        let url = url::Url::parse(&args[1]).map_err(|_| "invalid wRPC URL")?;
        if !matches!(url.scheme(), "ws" | "wss")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
        {
            return Err("use a ws/wss node URL without credentials, query or fragment".into());
        }
        let client = KaspaRpcClient::new(WrpcEncoding::Borsh, Some(url.as_str()), None, None, None)
            .map_err(|_| "cannot initialize read-only wRPC client")?;
        let result = tokio::time::timeout(Duration::from_secs(15), async {
            client
                .connect(None)
                .await
                .map_err(|_| "TN10 node connection failed")?;
            let observed = observe(&client, &escrow).await?;
            report["node"] = serde_json::to_value(&observed.server_info)?;
            report["observed_utxos"] = serde_json::to_value(&observed.utxos)?;
            report["node_fee_estimate"] = serde_json::to_value(&observed.fee_estimate)?;
            if args.len() == 6 {
                let (txid, index) = args[2].split_once(':').ok_or("expected TXID:INDEX")?;
                let outpoint = TransactionOutpoint::new(Hash::from_str(txid)?, index.parse()?);
                let path = match args[3].as_str() {
                    "release" => EscrowPath::Release,
                    "refund" => EscrowPath::Refund,
                    _ => return Err("path must be release or refund".into()),
                };
                let (native, preview) = observed.unsigned_spend(
                    &escrow,
                    outpoint,
                    path,
                    args[4].parse()?,
                    args[5].parse()?,
                )?;
                report["signing_preview"] = serde_json::to_value(preview)?;
                report["unsigned_rpc_transaction"] =
                    serde_json::to_value(native.rpc_transaction())?;
            }
            Ok::<(), Box<dyn Error>>(())
        })
        .await;
        let _ = tokio::time::timeout(Duration::from_secs(5), client.disconnect()).await;
        result
            .map_err(|_| "read-only node operation timed out; nothing was signed or submitted")??;
    }
    println!("{}", serde_json::to_string_pretty(&report)?);
    Ok(())
}
