use std::time::Instant;

use anyhow::{Context, Result, bail};
use clap::Args;
use rand::Rng;
use reqwest::blocking::Client;
use serde_json::json;

use gaia_core::config::AppConfig;

#[derive(Debug, Args)]
pub struct BenchmarkArgs {
    #[arg(long)]
    pub base_url: Option<String>,
    #[arg(long)]
    pub api_key: Option<String>,
    #[arg(long)]
    pub model: Option<String>,
    #[arg(long, default_value_t = 10)]
    pub requests: usize,
    #[arg(long, default_value = "Give me one concise sentence about Rust.")]
    pub prompt: String,
    #[arg(long)]
    pub mock: bool,
}

pub fn run(args: BenchmarkArgs) -> Result<()> {
    let config = AppConfig::load_or_transient_default()?;
    let serve_config = config.to_serve_config();
    let base_url = args
        .base_url
        .unwrap_or_else(|| serve_config.openai_base_url())
        .trim_end_matches('/')
        .to_owned();
    let api_key = args.api_key.unwrap_or_else(|| serve_config.api_key.clone());
    let model = args.model.unwrap_or_else(|| serve_config.model_id.clone());

    if args.requests == 0 {
        bail!("`--requests` must be greater than zero.");
    }

    println!("gaia benchmark");
    println!("==================");
    println!("Base URL: {base_url}");
    println!("Model: {model}");
    println!("Requests: {}", args.requests);
    println!();

    let mut latencies_ms = if args.mock {
        simulate_latencies(args.requests)
    } else {
        run_live_benchmark(&base_url, &api_key, &model, &args.prompt, args.requests)?
    };
    latencies_ms.sort_by(f64::total_cmp);

    let avg = latencies_ms.iter().sum::<f64>() / latencies_ms.len() as f64;
    let p50 = percentile(&latencies_ms, 0.50);
    let p95 = percentile(&latencies_ms, 0.95);
    let min = *latencies_ms.first().unwrap_or(&0.0);
    let max = *latencies_ms.last().unwrap_or(&0.0);

    println!("Results");
    println!("-------");
    println!("avg: {:.2} ms", avg);
    println!("p50: {:.2} ms", p50);
    println!("p95: {:.2} ms", p95);
    println!("min: {:.2} ms", min);
    println!("max: {:.2} ms", max);
    println!();
    println!(
        "throughput (approx): {:.2} req/s",
        if avg > 0.0 { 1000.0 / avg } else { 0.0 }
    );

    Ok(())
}

fn run_live_benchmark(
    base_url: &str,
    api_key: &str,
    model: &str,
    prompt: &str,
    requests: usize,
) -> Result<Vec<f64>> {
    let endpoint = format!("{base_url}/chat/completions");
    let client = Client::builder()
        .timeout(std::time::Duration::from_secs(120))
        .build()
        .context("Unable to build HTTP client")?;

    let mut latencies = Vec::with_capacity(requests);
    for index in 0..requests {
        let started = Instant::now();
        let response = client
            .post(&endpoint)
            .bearer_auth(api_key)
            .json(&json!({
                "model": model,
                "messages": [
                    { "role": "user", "content": prompt }
                ],
                "temperature": 0.7,
                "stream": false
            }))
            .send()
            .with_context(|| format!("Request {} failed", index + 1))?;

        if !response.status().is_success() {
            let status = response.status();
            let body = response.text().unwrap_or_else(|_| "<no body>".to_owned());
            bail!(
                "Request {} failed with status {}: {}",
                index + 1,
                status,
                body
            );
        }

        latencies.push(started.elapsed().as_secs_f64() * 1000.0);
        println!("Request {:>3}/{} ok", index + 1, requests);
    }

    Ok(latencies)
}

fn simulate_latencies(requests: usize) -> Vec<f64> {
    let mut rng = rand::rng();
    (0..requests)
        .map(|index| {
            let base = 110.0 + index as f64 * 1.5;
            let jitter: f64 = rng.random_range(-25.0..35.0);
            (base + jitter).max(40.0)
        })
        .collect()
}

fn percentile(values: &[f64], ratio: f64) -> f64 {
    if values.is_empty() {
        return 0.0;
    }
    let rank = ((values.len() - 1) as f64 * ratio).round() as usize;
    values[rank.min(values.len() - 1)]
}
