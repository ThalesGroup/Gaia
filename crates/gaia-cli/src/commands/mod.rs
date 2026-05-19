pub mod benchmark;
pub mod compose;
pub mod doctor;
pub mod k8s;
pub mod logs;
pub mod mock_api;
pub mod models;
pub mod recommend;
pub mod select;
pub mod serve;
pub mod status;
pub mod stop;
pub mod systemd;

use anyhow::Result;

use crate::Commands;

pub fn run(command: Commands) -> Result<()> {
    match command {
        Commands::Doctor(args) => doctor::run(args),
        Commands::Models(args) => models::run(args),
        Commands::Recommend(args) => recommend::run(args),
        Commands::Select(args) => select::run(args),
        Commands::Serve(args) => serve::run(args),
        Commands::Stop(args) => stop::run(args),
        Commands::Status(args) => status::run(args),
        Commands::Logs(args) => logs::run(args),
        Commands::GenerateCompose(args) => compose::run(args),
        Commands::GenerateK8s(args) => k8s::run(args),
        Commands::GenerateSystemd(args) => systemd::run(args),
        Commands::Benchmark(args) => benchmark::run(args),
        Commands::MockApi(args) => mock_api::run(args),
    }
}
