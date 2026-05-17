use anyhow::Result;
use clap::Args;
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_core::recommendation::RecommendationEngine;

#[derive(Debug, Args)]
pub struct RecommendArgs {
    #[arg(long)]
    pub backend: Option<String>,
    #[arg(long, default_value_t = 8)]
    pub top: usize,
}

pub fn run(args: RecommendArgs) -> Result<()> {
    let machine = MachineSpecs::detect();
    let catalog = ModelCatalog::load_default()?;

    let detected_preference = RecommendationEngine::preferred_backend(&machine);
    let backend = args.backend.as_deref().or(detected_preference.as_str());
    let recommendations = RecommendationEngine::recommend_models(&machine, &catalog, backend);

    println!("gaia recommend");
    println!("==================");
    if let Some(backend) = backend {
        println!("Backend filter: {backend}");
    } else {
        println!("Backend filter: none (showing all compatible models)");
    }
    println!(
        "GPU: {}",
        machine
            .gpu
            .as_ref()
            .map(|gpu| format!("{} ({:.1} GB)", gpu.name, gpu.vram_gb))
            .unwrap_or_else(|| "none detected".to_owned())
    );
    println!("RAM: {:.1} GB", machine.ram_total_gb);

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Model", "Params", "Status", "Explanation"]);

    for rec in recommendations.iter().take(args.top) {
        table.add_row(vec![
            Cell::new(&rec.model.id),
            Cell::new(format!("{:.1}B", rec.model.params_b)),
            Cell::new(rec.status.as_badge()),
            Cell::new(&rec.explanation),
        ]);
    }

    println!();
    println!("{table}");

    Ok(())
}
