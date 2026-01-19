//! Wait command implementation.

use std::time::Duration;

use tokio::time::sleep;

pub async fn run(ms: u64) -> anyhow::Result<()> {
    sleep(Duration::from_millis(ms)).await;
    Ok(())
}
