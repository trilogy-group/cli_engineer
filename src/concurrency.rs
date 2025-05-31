use anyhow::Result;
use tokio::task::JoinHandle;

/// Runs tasks concurrently using tokio.
pub async fn run_parallel<T, F>(futs: Vec<F>) -> Result<Vec<T>>
where
    F: std::future::Future<Output = Result<T>> + Send + 'static,
    T: Send + 'static,
{
    let handles: Vec<JoinHandle<Result<T>>> = futs
        .into_iter()
        .map(|f| tokio::spawn(f))
        .collect();

    let mut out = Vec::new();
    for handle in handles {
        out.push(handle.await??);
    }
    Ok(out)
}
