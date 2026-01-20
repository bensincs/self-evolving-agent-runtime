use serde::Deserialize;
use capability_common::serde_json::Value;

#[derive(Deserialize)]
struct ApiResponse {
    bitcoin: Currency,
}

#[derive(Deserialize)]
struct Currency {
    usd: f64,
}

fn main() {
    capability_common::run(|_: Value| {
        let body: ApiResponse = capability_common::http_get_json("https://api.coingecko.com/api/v3/simple/price?ids=bitcoin&vs_currencies=usd")?;
        let price_usd = body.bitcoin.usd;
        let output = serde_json::json!({"price_usd": price_usd});
        Ok(output)
    });
}