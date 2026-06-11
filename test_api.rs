#[tokio::main]
async fn main() {
    let client = reqwest::Client::new();
    let res = client.get("https://fapi.binance.com/fapi/v1/exchangeInfo").send().await.unwrap().text().await.unwrap();
    println!("{}", res.len());
}
