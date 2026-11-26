// Each line marked with a number is a finding of http-outside-network.
use reqwest::Client; // 1
pub(crate) use reqwest::StatusCode; // 2
use reqwest_eventsource::EventSource; // 3

async fn fetch(url: &str) -> Result<(), String> {
    let client = reqwest::Client::new(); // 4
    let _ = reqwest::get(url).await; // 5
    let _: Option<reqwest::Response> = None; // 6
    let _ = client;
    Ok(())
}
