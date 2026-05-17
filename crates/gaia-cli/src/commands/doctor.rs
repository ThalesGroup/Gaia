use anyhow::Result;
use clap::Args;
use comfy_table::{Cell, Table, presets::UTF8_FULL};
use gaia_core::backend::all_backends;
use gaia_core::machine::MachineSpecs;
use gaia_core::model_catalog::ModelCatalog;
use gaia_core::recommendation::RecommendationEngine;

#[derive(Debug, Args, Default)]
pub struct DoctorArgs {}

pub fn run(_args: DoctorArgs) -> Result<()> {
    let machine = MachineSpecs::detect();
    println!("gaia doctor");
    println!("================");
    print_machine_section(&machine);
    print_backends_section(&machine);
    print_recommendations_section(&machine)?;
    Ok(())
}

fn print_machine_section(machine: &MachineSpecs) {
    println!();
    println!("Machine");
    println!("-------");
    println!("OS: {}", machine.os_name);
    if let Some(kernel) = &machine.kernel_version {
        println!("Kernel: {kernel}");
    }
    println!("CPU cores: {}", machine.cpu_cores);
    println!("RAM: {:.1} GB", machine.ram_total_gb);
    println!(
        "Docker: {}",
        if machine.docker.installed {
            machine
                .docker
                .version
                .as_deref()
                .unwrap_or("installed (version unknown)")
        } else {
            "not installed"
        }
    );
    println!(
        "Docker daemon: {}",
        if machine.docker.daemon_accessible {
            "reachable"
        } else {
            "not reachable"
        }
    );
    println!(
        "HF_TOKEN: {}",
        if machine.hf_token_present {
            "present"
        } else {
            "missing (optional unless using gated models)"
        }
    );

    if let Some(gpu) = &machine.gpu {
        println!("GPU: {} ({:.1} GB VRAM)", gpu.name, gpu.vram_gb);
        if let Some(driver) = &gpu.driver_version {
            println!("NVIDIA driver: {driver}");
        }
        if let Some(cuda) = &gpu.cuda_version {
            println!("CUDA version: {cuda}");
        }
    } else {
        println!("GPU: none detected");
    }

    if !machine.warnings.is_empty() {
        println!();
        println!("Warnings:");
        for warning in &machine.warnings {
            println!("  - {warning}");
        }
    }
}

fn print_backends_section(machine: &MachineSpecs) {
    println!();
    println!("Backend availability");
    println!("--------------------");

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Backend", "Available", "Reason"]);

    for backend in all_backends() {
        let availability = backend.is_available(machine);
        table.add_row(vec![
            Cell::new(backend.display_name()),
            Cell::new(if availability.available { "yes" } else { "no" }),
            Cell::new(availability.reason),
        ]);
    }

    println!("{table}");
}

fn print_recommendations_section(machine: &MachineSpecs) -> Result<()> {
    println!();
    println!("Recommendations");
    println!("---------------");

    let catalog = ModelCatalog::load_default()?;
    let preferred_backend = RecommendationEngine::preferred_backend(machine);
    let recommendations =
        RecommendationEngine::recommend_models(machine, &catalog, preferred_backend.as_str());

    println!("Preferred backend: {}", preferred_backend.display_name());

    let mut table = Table::new();
    table.load_preset(UTF8_FULL);
    table.set_header(vec!["Model", "Params", "Fit", "Why"]);

    for recommendation in recommendations.iter().take(8) {
        table.add_row(vec![
            Cell::new(&recommendation.model.id),
            Cell::new(format!("{:.1}B", recommendation.model.params_b)),
            Cell::new(recommendation.status.as_badge()),
            Cell::new(&recommendation.explanation),
        ]);
    }

    println!("{table}");
    Ok(())
}
