mod commands;
mod final_output;
mod mock_support;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Debug, Parser)]
#[command(
    name = "gaia",
    version,
    about = "LLM serving manager for Hugging Face models"
)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Debug, Subcommand)]
enum Commands {
    Doctor(commands::doctor::DoctorArgs),
    Models(commands::models::ModelsArgs),
    Recommend(commands::recommend::RecommendArgs),
    Select(commands::select::SelectArgs),
    Serve(commands::serve::ServeArgs),
    Stop(commands::stop::StopArgs),
    Status(commands::status::StatusArgs),
    Logs(commands::logs::LogsArgs),
    GenerateCompose(commands::compose::GenerateComposeArgs),
    GenerateK8s(commands::k8s::GenerateK8sArgs),
    GenerateSystemd(commands::systemd::GenerateSystemdArgs),
    Benchmark(commands::benchmark::BenchmarkArgs),
    #[command(name = "__mock-api", hide = true)]
    MockApi(commands::mock_api::MockApiArgs),
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    commands::run(cli.command)
}
