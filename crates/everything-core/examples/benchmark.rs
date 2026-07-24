use everything_core::{EverythingEngine, QueryRequest, SortSpec};
use std::env;
use std::time::Instant;

#[cfg(windows)]
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let query = env::var("EVERYTHING_BENCH_QUERY").unwrap_or_else(|_| "*.rs".into());
    let iterations = env::var("EVERYTHING_BENCH_ITERATIONS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(40)
        .max(5);
    let mut engine = EverythingEngine::new()?;

    // Warm-up : chargement IPC/DLL et première mise en cache Everything.
    let _ = engine.query(QueryRequest {
        query: query.clone(),
        offset: 0,
        limit: 256,
        sort: SortSpec::default(),
        request_id: 0,
    })?;

    let mut durations = Vec::with_capacity(iterations);
    let mut total_results = 0;
    for request_id in 1..=iterations {
        let started = Instant::now();
        let page = engine.query(QueryRequest {
            query: query.clone(),
            offset: 0,
            limit: 256,
            sort: SortSpec::default(),
            request_id: request_id as u32,
        })?;
        durations.push(started.elapsed().as_secs_f64() * 1_000.0);
        total_results = page.total;
    }

    durations.sort_by(f64::total_cmp);
    let percentile = |ratio: f64| {
        let index = ((durations.len() - 1) as f64 * ratio).round() as usize;
        durations[index]
    };
    println!("Query: {query}");
    println!("Résultats: {total_results}");
    println!("Itérations: {}", durations.len());
    println!("p50: {:.2} ms", percentile(0.50));
    println!("p95: {:.2} ms", percentile(0.95));
    println!("max: {:.2} ms", durations[durations.len() - 1]);
    Ok(())
}

#[cfg(not(windows))]
fn main() {
    eprintln!("Ce benchmark nécessite Windows et une instance Everything lancée.");
}
