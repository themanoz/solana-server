use axum::{
    routing::post,
    Router,
    Json,
    response::IntoResponse,
    http::StatusCode,
};
use solana_sdk::signer::{keypair::Keypair, Signer};
use serde::Serialize;
use bs58;

#[derive(Serialize)]
struct KeypairData {
    pubkey: String,
    secret: String,
}

#[derive(Serialize)]
struct ApiResponse {
    success: bool,
    data: Option<KeypairData>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

async fn generate_keypair() -> impl IntoResponse {
    let keypair = Keypair::new(); 
    let pubkey = keypair.pubkey().to_string(); 
    let secret = bs58::encode(keypair.to_bytes()).into_string(); 

    let data = KeypairData { pubkey, secret };
    let response = ApiResponse {
        success: true,
        data: Some(data),
        error: None,
    };

    (StatusCode::OK, Json(response))
}

#[tokio::main]
async fn main() {
    let app = Router::new().route("/generate-keypair", post(generate_keypair));

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000").await.unwrap();
    println!("Server running at http://localhost:3000");
    axum::serve(listener, app).await.unwrap();
}
