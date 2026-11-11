#![windows_subsystem = "windows"]

#[tokio::main]
async fn main() -> humboldt_core::Result<()> {
    humboldt_server::run_app().await
}
