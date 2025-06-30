// main.rs

use axum::{
    routing::post,
    Router,
    Json,
    http::StatusCode,
    response::IntoResponse,
};
use serde::{Serialize, Deserialize}; 
use solana_sdk::{
    pubkey::Pubkey,
    signer::{keypair::Keypair, Signer},
    instruction::Instruction,
    system_instruction,
};
use spl_token::instruction::{initialize_mint, mint_to, transfer as spl_transfer};
use spl_token::id as spl_token_program_id;
use bs58;
use base64;
use std::str::FromStr;
use ed25519_dalek::{Signer as DalekSigner, Verifier, Keypair as DalekKeypair, PublicKey as DalekPubkey, Signature as DalekSignature, PUBLIC_KEY_LENGTH};

#[derive(Serialize)]
struct ApiResponse<T> {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    data: Option<T>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

impl<T: Serialize> ApiResponse<T> {
    fn ok(data: T) -> Self {
        Self { success: true, data: Some(data), error: None }
    }
    fn err(msg: &str) -> Self {
        Self { success: false, data: None, error: Some(msg.to_string()) }
    }
}

type ApiResult<T> = Result<Json<ApiResponse<T>>, Json<ApiResponse<()>>>;

#[derive(Serialize)]
struct KeypairData {
    pubkey: String,
    secret: String,
}

async fn generate_keypair() -> ApiResult<KeypairData> {
    let keypair = Keypair::new();
    let pubkey = keypair.pubkey().to_string();
    let secret = bs58::encode(keypair.to_bytes()).into_string();
    Ok(Json(ApiResponse::ok(KeypairData { pubkey, secret })))
}

#[derive(Deserialize)]
struct CreateTokenRequest {
    mintAuthority: String,
    mint: String,
    decimals: u8,
}

#[derive(Serialize)]
struct AccountMetaInfo {
    pubkey: String,
    is_signer: bool,
    is_writable: bool,
}

#[derive(Serialize)]
struct InstructionData {
    program_id: String,
    accounts: Vec<AccountMetaInfo>,
    instruction_data: String,
}

async fn create_token(Json(payload): Json<CreateTokenRequest>) -> ApiResult<InstructionData> {
    let mint = Pubkey::from_str(&payload.mint).map_err(|_| Json(ApiResponse::err("Invalid mint pubkey")))?;
    let authority = Pubkey::from_str(&payload.mintAuthority).map_err(|_| Json(ApiResponse::err("Invalid authority pubkey")))?;

    let instr = initialize_mint(&spl_token_program_id(), &mint, &authority, None, payload.decimals)
        .map_err(|e| Json(ApiResponse::err(&format!("Instruction error: {e}"))))?;

    Ok(Json(ApiResponse::ok(InstructionData {
        program_id: instr.program_id.to_string(),
        accounts: instr.accounts.iter().map(|a| AccountMetaInfo {
            pubkey: a.pubkey.to_string(),
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        }).collect(),
        instruction_data: base64::encode(&instr.data),
    })))
}

#[derive(Deserialize)]
struct MintTokenRequest {
    mint: String,
    destination: String,
    authority: String,
    amount: u64,
}

async fn mint_token(Json(payload): Json<MintTokenRequest>) -> ApiResult<InstructionData> {
    let mint = Pubkey::from_str(&payload.mint).map_err(|_| Json(ApiResponse::err("Invalid mint pubkey")))?;
    let dest = Pubkey::from_str(&payload.destination).map_err(|_| Json(ApiResponse::err("Invalid destination pubkey")))?;
    let auth = Pubkey::from_str(&payload.authority).map_err(|_| Json(ApiResponse::err("Invalid authority pubkey")))?;

    let instr = mint_to(&spl_token_program_id(), &mint, &dest, &auth, &[], payload.amount)
        .map_err(|e| Json(ApiResponse::err(&format!("Instruction error: {e}"))))?;

    Ok(Json(ApiResponse::ok(InstructionData {
        program_id: instr.program_id.to_string(),
        accounts: instr.accounts.iter().map(|a| AccountMetaInfo {
            pubkey: a.pubkey.to_string(),
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        }).collect(),
        instruction_data: base64::encode(&instr.data),
    })))
}

#[derive(Deserialize)]
struct SignMessageRequest {
    message: String,
    secret: String,
}

#[derive(Serialize)]
struct SignMessageData {
    signature: String,
    public_key: String,
    message: String,
}

async fn sign_message(Json(payload): Json<SignMessageRequest>) -> ApiResult<SignMessageData> {
    let secret_bytes = bs58::decode(&payload.secret).into_vec().map_err(|_| Json(ApiResponse::err("Invalid secret")))?;
    let keypair = DalekKeypair::from_bytes(&secret_bytes).map_err(|_| Json(ApiResponse::err("Invalid secret bytes")))?;
    let sig = keypair.sign(payload.message.as_bytes());

    Ok(Json(ApiResponse::ok(SignMessageData {
        signature: base64::encode(sig.to_bytes()),
        public_key: bs58::encode(keypair.public.to_bytes()).into_string(),
        message: payload.message,
    })))
}

#[derive(Deserialize)]
struct VerifyMessageRequest {
    message: String,
    signature: String,
    pubkey: String,
}

#[derive(Serialize)]
struct VerifyMessageData {
    valid: bool,
    message: String,
    pubkey: String,
}

async fn verify_message(Json(payload): Json<VerifyMessageRequest>) -> ApiResult<VerifyMessageData> {
    let pubkey_bytes = bs58::decode(&payload.pubkey).into_vec().map_err(|_| Json(ApiResponse::err("Invalid pubkey")))?;
    let sig_bytes = base64::decode(&payload.signature).map_err(|_| Json(ApiResponse::err("Invalid signature")))?;
    let pubkey = DalekPubkey::from_bytes(&pubkey_bytes).map_err(|_| Json(ApiResponse::err("Invalid pubkey bytes")))?;
    let sig = DalekSignature::from_bytes(&sig_bytes).map_err(|_| Json(ApiResponse::err("Invalid signature bytes")))?;

    let valid = pubkey.verify(payload.message.as_bytes(), &sig).is_ok();

    Ok(Json(ApiResponse::ok(VerifyMessageData {
        valid,
        message: payload.message,
        pubkey: payload.pubkey,
    })))
}

#[derive(Deserialize)]
struct SendSolRequest {
    from: String,
    to: String,
    lamports: u64,
}

#[derive(Serialize)]
struct SendSolData {
    program_id: String,
    accounts: Vec<String>,
    instruction_data: String,
}

async fn send_sol(Json(payload): Json<SendSolRequest>) -> ApiResult<SendSolData> {
    let from = Pubkey::from_str(&payload.from).map_err(|_| Json(ApiResponse::err("Invalid from")))?;
    let to = Pubkey::from_str(&payload.to).map_err(|_| Json(ApiResponse::err("Invalid to")))?;

    let instr = system_instruction::transfer(&from, &to, payload.lamports);

    Ok(Json(ApiResponse::ok(SendSolData {
        program_id: instr.program_id.to_string(),
        accounts: instr.accounts.iter().map(|a| a.pubkey.to_string()).collect(),
        instruction_data: base64::encode(&instr.data),
    })))
}

#[derive(Deserialize)]
struct SendTokenRequest {
    destination: String,
    mint: String,
    owner: String,
    amount: u64,
}

async fn send_token(Json(payload): Json<SendTokenRequest>) -> ApiResult<InstructionData> {
    let mint = Pubkey::from_str(&payload.mint).map_err(|_| Json(ApiResponse::err("Invalid mint")))?;
    let dest = Pubkey::from_str(&payload.destination).map_err(|_| Json(ApiResponse::err("Invalid destination")))?;
    let owner = Pubkey::from_str(&payload.owner).map_err(|_| Json(ApiResponse::err("Invalid owner")))?;

    let instr = spl_transfer(&spl_token_program_id(), &mint, &dest, &owner, &[], payload.amount)
        .map_err(|e| Json(ApiResponse::err(&format!("Instruction error: {e}"))))?;

    Ok(Json(ApiResponse::ok(InstructionData {
        program_id: instr.program_id.to_string(),
        accounts: instr.accounts.iter().map(|a| AccountMetaInfo {
            pubkey: a.pubkey.to_string(),
            is_signer: a.is_signer,
            is_writable: a.is_writable,
        }).collect(),
        instruction_data: base64::encode(&instr.data),
    })))
}

#[tokio::main]
async fn main() {
    let app = Router::new()
        .route("/keypair", post(generate_keypair))
        .route("/token/create", post(create_token))
        .route("/token/mint", post(mint_token))
        .route("/message/sign", post(sign_message))
        .route("/message/verify", post(verify_message))
        .route("/send/sol", post(send_sol))
        .route("/send/token", post(send_token));

    println!("🚀 Solana API server running at http://0.0.0.0:3000");
    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}
