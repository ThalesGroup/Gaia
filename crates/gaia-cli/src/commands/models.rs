use anyhow::Result;
use clap::Args;
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_core::recommendation::RecommendationEngine;

#[derive(Debug, Args)]
pub struct ModelsArgs {
    #[arg(long)]
    pub category: Option<String>,
    #[arg(long)]
    pub max_params: Option<f32>,
    #[arg(long)]
    pub backend: Option<String>,
    #[arg(long)]
    pub recommended_only: bool,
}

pub fn run(args: ModelsArgs) -> Result<()> {
    let catalog = ModelCatalog::load_default()?;
    let mut models = catalog.filtered(
        args.category.as_deref(),
        args.max_params,
        args.backend.as_deref(),
    );
    models.sort_by(|left, right| left.params_b.total_cmp(&right.params_b));

    let machine = if args.recommended_only {
        Some(MachineSpecs::detect())
    } else {
        None
    };

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec![
        "Model",
        "Family",
        "Params",
        "vLLM",
        "TGI",
        "Categories",
        "Fit",
    ]);

    let mut displayed = 0usize;
    for model in models {
        let fit = machine
            .as_ref()
            .map(|m| RecommendationEngine::evaluate_fit(m, &model).0);

        if args.recommended_only && !fit.is_some_and(|status| status.is_positive()) {
            continue;
        }

        displayed += 1;
        let categories = model.categories_label();
        table.add_row(vec![
            Cell::new(&model.id),
            Cell::new(&model.family),
            Cell::new(format!("{:.1}B", model.params_b)),
            Cell::new(if model.supports_vllm { "yes" } else { "no" }),
            Cell::new(if model.supports_tgi { "yes" } else { "no" }),
            Cell::new(categories),
            Cell::new(fit.map(|status| status.as_badge()).unwrap_or("-")),
        ]);
    }

    println!("{table}");
    println!("Total models shown: {displayed}");

    Ok(())
}
