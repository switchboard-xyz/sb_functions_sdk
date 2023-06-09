# sb_functions_sdk
[![crates.io](https://img.shields.io/crates/v/my_library.svg)](https://crates.io/crates/sb_functions_sdk)


This SDK is the master utility SDK for writing verifiable functions off chain.

Switchboard functions provide TEEs as a blockchain primitive.

Using Switchboard functions, you can validate that any code being signed by
Switchboard was run inside SGX using Switchboard's oracle network.

## Fire and Forget

Switchboard allows you to run any code on a cron or on-demand schedule.

## Solana Function Example

``` rust
const DEMO_PID: Pubkey = pubkey!("8kjszBCEgkzAsU6QySHSZvr9yFaboau2RnarCQFFvasS");

#[derive(Clone, AnchorSerialize, AnchorDeserialize, Debug, Default)]
pub struct PingParams {
    pub prices: Vec<BorshDecimal>,
    pub volumes: Vec<BorshDecimal>,
    pub twaps: Vec<BorshDecimal>,
}
impl Discriminator for PingParams {
    const DISCRIMINATOR: [u8; 8] = [0; 8];
    fn discriminator() -> [u8; 8] {
        ix_discriminator("ping")
    }
}
impl InstructionData for PingParams {}

#[allow(non_snake_case)]
#[derive(Deserialize, Clone, Debug)]
struct Ticker {
    symbol: String,
    weightedAvgPrice: String,
    lastPrice: String,
    volume: String,
}

#[tokio::main(worker_threads = 12)]
async fn main() {
    let symbols = ["BTCUSDC", "ETHUSDC", "SOLUSDT"];

    let symbols = symbols.map(|x| format!("\"{}\"", x)).join(",");
    let tickers = reqwest::get(format!(
        "https://api.binance.com/api/v3/ticker?symbols=[{}]&windowSize=1h",
        symbols
    ))
    .await
    .unwrap()
    .json::<Vec<Ticker>>()
    .await
    .unwrap();
    println!("{:#?}", tickers);

    let enclave_signer = generate_signer();
    let (fn_key, fn_quote) = fn_accounts();
    let ix = Instruction {
        program_id: DEMO_PID,
        accounts: vec![
            AccountMeta {
                pubkey: fn_key,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: fn_quote,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: enclave_signer.pubkey(),
                is_signer: true,
                is_writable: false,
            },
        ],
        data: PingParams {
            prices: tickers
                .iter()
                .map(|x| BorshDecimal::from(&x.lastPrice))
                .collect(),
            volumes: tickers
                .iter()
                .map(|x| BorshDecimal::from(&x.volume))
                .collect(),
            twaps: tickers
                .iter()
                .map(|x| BorshDecimal::from(&x.weightedAvgPrice))
                .collect(),
        }
        .data(),
    };
    FunctionResult::generate_verifiable_solana_tx(enclave_signer, vec![ix])
        .await
        .unwrap()
        .emit();
}
```

## Solana On-chain verification

For Receiving the result and verifying the SGX quote passed, please use this crate on chain:
<a href="https://crates.io/crates/solana_attestation_sdk">https://crates.io/crates/solana_attestation_sdk</a>

To see example output of such a function: See https://explorer.solana.com/tx/FnJ13SxdKmMadsnUg884msNnM76QuJkV8gxj9CEikBYbcJzgS3x1KLBiZzrav3tntJezhfYyn2KqrA7AoLRpf9k?cluster=devnet


## Example Function

To see an example function container, refer to: https://github.com/switchboard-xyz/sbv3-function-example

## Adding your function to a cron schedule

For running your function at a regular cadence, use our typescript sdk to attach it to an attestation queue:

``` Typescript
import {
  SwitchboardProgram,
  FunctionAccount,
  AttestationQueueAccount,
} from "@switchboard-xyz/solana.js";

// ...
const functionKeypair = anchor.web3.Keypair.generate();
const [functionAccount] = await FunctionAccount.create(switchboard, {
  name: "FUNCTION_NAME",
  metadata: "FUNCTION_METADATA",
  schedule: "30 * * * * *", // every 30 seconds
  container: "switchboardlabs/function-example",
  version: "v1",
  mrEnclave: new Uint8Array(0), // Leave blank to auto-populate after first run
  attestationQueue: new AttestationQueueAccount(
    switchboard,
    <QUEUE_PUBKEY>
  ),
  keypair: functionKeypair,
});
console.log(`Function: ${functionAccount.publicKey.toString()}`);
```
