use chrono::Duration;
use deysis_trust_boundaries::{MockAttendanceProvider, crypto::KeyStore, queue::Job, worker};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter("info").init();
    let provider = Arc::new(MockAttendanceProvider::new(Duration::seconds(30)));
    let keys = KeyStore::new([7; 32]);
    let jobs = (0..3)
        .map(|n| Job::new(format!("demo-user-{n}"), "check-in", "region:demo"))
        .collect();
    let results = worker::run_bounded(provider, keys, jobs, 2).await;
    for result in results {
        println!(
            "{}",
            result
                .map(|r| format!("accepted: {}", r.receipt))
                .unwrap_or_else(|e| format!("rejected: {e}"))
        );
    }
}
