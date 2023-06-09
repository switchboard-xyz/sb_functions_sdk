use crate::*;
use anchor_client::anchor_lang::prelude::*;
use anchor_client::anchor_lang::{
    AnchorDeserialize, AnchorSerialize, Discriminator, InstructionData, ToAccountMetas,
};
use anchor_client::solana_sdk::instruction::Instruction;
use anchor_client::solana_sdk::signer::keypair::Keypair;
use anchor_client::Cluster;
use serde::{Deserialize, Serialize};
use sgx_quote::Quote;
use solana_sdk::{
    commitment_config::CommitmentConfig, instruction::AccountMeta, message::Message,
    pubkey::Pubkey, signature::Signer,
    transaction::Transaction,
};
use spl_token;
use std::result::Result;
use std::str::FromStr;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use std::env;

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct FunctionResult {
    pub version: u32,
    pub chain: Chain,
    pub key: [u8; 32],
    pub signer: [u8; 32],
    pub serialized_tx: Vec<u8>,
    pub quote: Vec<u8>,
    pub program: Vec<u8>,
    pub data: Vec<u8>,
}
impl FunctionResult {
    pub async fn generate_verifiable_solana_tx(
        enclave_signer: Arc<Keypair>,
        mut ixs: Vec<Instruction>,
    ) -> Result<FunctionResult, Err> {
        let current_time = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs() as i64;
        let client = anchor_client::Client::new_with_options(
            Cluster::Devnet,
            enclave_signer.clone(),
            CommitmentConfig::processed(),
        );
        let quote_raw = Sgx::gramine_generate_quote(&enclave_signer.pubkey().to_bytes()).unwrap();
        let quote = Quote::parse(&quote_raw).unwrap();

        let blockhash = client
            .program(ATTESTATION_PID)
            .rpc()
            .get_latest_blockhash()
            .unwrap();
        let function = Pubkey::from_str(&env::var("FUNCTION_KEY").unwrap()).unwrap();
        let fn_data: FunctionAccountData = load(&client, function).await?;
        let payer = Pubkey::from_str(&env::var("PAYER").unwrap()).unwrap();
        let verifier = &env::var("VERIFIER").unwrap_or(String::new());
        if verifier.is_empty() {
            return Err(Err::VerifierMissing);
        }
        let next_allowed_timestamp = fn_data
            .next_execution_timestamp()
            .map(|x| x.timestamp())
            .unwrap_or(i64::MAX);
        let ix = FunctionVerify::build(
            &client,
            FunctionVerifyArgs {
                function,
                fn_signer: enclave_signer.pubkey(),
                reward_receiver: Pubkey::from_str(&env::var("REWARD_RECEIVER").unwrap()).unwrap(),
                verifier: Pubkey::from_str(verifier).unwrap(),
                payer,
                timestamp: current_time,
                next_allowed_timestamp,
                is_failure: false,
                mr_enclave: quote.isv_report.mrenclave.try_into().unwrap(),
            },
        )
        .await
        .unwrap();
        ixs.insert(0, ix);
        let message = Message::new(&ixs, Some(&payer));
        let mut tx = Transaction::new_unsigned(message);
        tx.partial_sign(&[enclave_signer.as_ref()], blockhash);
        Ok(FunctionResult {
            version: 1,
            chain: Chain::Solana,
            key: function.to_bytes(),
            signer: enclave_signer.pubkey().to_bytes(),
            serialized_tx: bincode::serialize(&tx).unwrap(),
            quote: quote_raw,
            ..Default::default()
        })
    }

    pub fn emit(&self) {
        println!(
            "FN_OUT: {}",
            hex::encode(&serde_json::to_string(&self).unwrap())
        );
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum Chain {
    Solana,
    Arbitrum,
    Bsc,
    Coredao,
    Aptos,
    Sui,
}
impl Default for Chain {
    fn default() -> Self {
        Self::Solana
    }
}

pub struct FunctionVerify {
    pub function: Pubkey,
    pub fn_signer: Pubkey,
    pub fn_quote: Pubkey,
    pub verifier_quote: Pubkey,
    pub secured_signer: Pubkey,
    pub attestation_queue: Pubkey,
    pub escrow: Pubkey,
    pub receiver: Pubkey,
    pub verifier_permission: Pubkey,
    pub fn_permission: Pubkey,
    pub state: Pubkey,
    pub token_program: Pubkey,
    pub payer: Pubkey,
    pub system_program: Pubkey,
}
#[derive(Clone, AnchorSerialize, AnchorDeserialize)]
pub struct FunctionVerifyParams {
    pub observed_time: i64,
    pub next_allowed_timestamp: i64,
    pub is_failure: bool,
    pub mr_enclave: [u8; 32],
}
pub struct FunctionVerifyArgs {
    pub function: Pubkey,
    pub fn_signer: Pubkey,
    pub reward_receiver: Pubkey,
    pub verifier: Pubkey,
    pub payer: Pubkey,
    pub timestamp: i64,
    pub next_allowed_timestamp: i64,
    pub is_failure: bool,
    pub mr_enclave: [u8; 32],
}

impl Discriminator for FunctionVerifyParams {
    const DISCRIMINATOR: [u8; 8] = [0; 8];
    fn discriminator() -> [u8; 8] {
        ix_discriminator("function_verify")
    }
}
impl InstructionData for FunctionVerifyParams {}
impl ToAccountMetas for FunctionVerify {
    fn to_account_metas(&self, _: Option<bool>) -> Vec<AccountMeta> {
        vec![
            AccountMeta {
                pubkey: self.function,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.fn_signer,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.fn_quote,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.verifier_quote,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.secured_signer,
                is_signer: true,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.attestation_queue,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.escrow,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.receiver,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.verifier_permission,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.fn_permission,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.state,
                is_signer: false,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.token_program,
                is_signer: false,
                is_writable: false,
            },
            AccountMeta {
                pubkey: self.payer,
                is_signer: true,
                is_writable: true,
            },
            AccountMeta {
                pubkey: self.system_program,
                is_signer: false,
                is_writable: false,
            },
        ]
    }
}

impl FunctionVerify {
    pub async fn build(
        client: &anchor_client::Client<Arc<Keypair>>,
        args: FunctionVerifyArgs,
    ) -> Result<Instruction, Err> {
        let fn_data: FunctionAccountData = load(client, args.function).await?;
        let queue = fn_data.attestation_queue;
        let queue_data: AttestationQueueAccountData = load(client, queue).await?;
        let quote_data: QuoteAccountData = load(client, args.verifier).await?;
        let escrow = fn_data.escrow;
        let (fn_quote, _) = Pubkey::find_program_address(
            &[b"QuoteAccountData", &args.function.to_bytes()],
            &ATTESTATION_PID,
        );
        let (verifier_permission, _) = Pubkey::find_program_address(
            &[
                b"PermissionAccountData",
                &queue_data.authority.to_bytes(),
                &queue.to_bytes(),
                &args.verifier.to_bytes(),
            ],
            &ATTESTATION_PID,
        );
        let (fn_permission, _) = Pubkey::find_program_address(
            &[
                b"PermissionAccountData",
                &queue_data.authority.to_bytes(),
                &queue.to_bytes(),
                &args.function.to_bytes(),
            ],
            &ATTESTATION_PID,
        );
        let (state, _) = Pubkey::find_program_address(&[b"STATE"], &ATTESTATION_PID);
        let accounts = Self {
            function: args.function,
            fn_signer: args.fn_signer,
            fn_quote,
            verifier_quote: args.verifier,
            secured_signer: quote_data.secured_signer,
            attestation_queue: queue,
            escrow,
            receiver: args.reward_receiver,
            verifier_permission,
            fn_permission,
            state,
            token_program: spl_token::ID,
            payer: args.payer,
            system_program: solana_sdk::system_program::ID,
        };
        Ok(build_ix(
            accounts,
            FunctionVerifyParams {
                observed_time: args.timestamp,
                next_allowed_timestamp: args.next_allowed_timestamp,
                is_failure: args.is_failure,
                mr_enclave: args.mr_enclave,
            },
        ))
    }
}

pub fn ix_discriminator(name: &str) -> [u8; 8] {
    let preimage = format!("global:{}", name);
    let mut sighash = [0u8; 8];
    sighash.copy_from_slice(&solana_sdk::hash::hash(preimage.as_bytes()).to_bytes()[..8]);
    sighash
}

pub fn build_ix<A: ToAccountMetas, I: InstructionData + Discriminator>(
    accounts: A,
    params: I,
) -> Instruction {
    Instruction {
        program_id: ATTESTATION_PID,
        accounts: accounts.to_account_metas(None),
        data: params.data(),
    }
}
