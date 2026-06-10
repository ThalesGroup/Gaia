use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use clap::{Args, Subcommand, ValueEnum};
use gaia_core::hf_catalog::{DiscoverSort, RefreshOptions, RefreshProgress, refresh_entries};
use gaia_core::model_catalog::ModelCatalog;

#[derive(Debug, Args)]
pub struct CatalogArgs {
    #[command(subcommand)]
    pub command: CatalogCommand,
}

#[derive(Debug, Subcommand)]
pub enum CatalogCommand {
    /// Refresh catalog entries from the Hugging Face API
    Refresh(RefreshArgs),
    /// Promote a reviewed generated catalog to the active catalog file
    Promote(PromoteArgs),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum SortMode {
    Trending,
    Downloads,
}

#[derive(Debug, Args)]
pub struct RefreshArgs {
    /// Discover top N text-generation models from the HF API
    #[arg(long, default_value_t = 0)]
    pub discover_limit: usize,
    /// Discovery ranking mode
    #[arg(long, value_enum, default_value_t = SortMode::Trending)]
    pub sort: SortMode,
    /// Explicit model ids to include (space-separated)
    #[arg(long, num_args = 1..)]
    pub ids: Vec<String>,
    /// Optional search query for discovery
    #[arg(long)]
    pub search: Option<String>,
    /// Read seed model ids from this catalog file
    #[arg(long, default_value = "catalog/models.yaml")]
    pub from_existing: PathBuf,
    /// Ignore --from-existing and only use --ids / --discover-limit
    #[arg(long)]
    pub no_existing: bool,
    /// Output YAML path
    #[arg(long, default_value = "catalog/models.generated.yaml")]
    pub output: PathBuf,
    /// Print YAML to stdout instead of writing the output file
    #[arg(long)]
    pub dry_run: bool,
    /// Keep models even when they do not look like text-generation models
    #[arg(long)]
    pub allow_non_text_generation: bool,
    /// HTTP timeout in seconds
    #[arg(long, default_value_t = 25)]
    pub timeout: u64,
}

#[derive(Debug, Args)]
pub struct PromoteArgs {
    /// Generated catalog to promote
    #[arg(long, default_value = "catalog/models.generated.yaml")]
    pub input: PathBuf,
    /// Active catalog destination
    #[arg(long, default_value = "catalog/models.yaml")]
    pub output: PathBuf,
}

pub fn run(args: CatalogArgs) -> Result<()> {
    match args.command {
        CatalogCommand::Refresh(args) => run_refresh(args),
        CatalogCommand::Promote(args) => run_promote(args),
    }
}

fn run_refresh(args: RefreshArgs) -> Result<()> {
    let mut seed_ids: Vec<String> = Vec::new();
    if !args.no_existing {
        seed_ids.extend(load_seed_ids(&args.from_existing)?);
    }
    seed_ids.extend(args.ids.iter().cloned());

    if seed_ids.is_empty() && args.discover_limit == 0 {
        bail!(
            "No model ids to refresh. Provide --ids, --discover-limit, or a valid --from-existing file."
        );
    }

    let options = RefreshOptions {
        seed_ids,
        discover_limit: args.discover_limit,
        discover_sort: match args.sort {
            SortMode::Trending => DiscoverSort::Trending,
            SortMode::Downloads => DiscoverSort::Downloads,
        },
        search: args.search.clone(),
        timeout_secs: args.timeout,
        allow_non_text_generation: args.allow_non_text_generation,
        ..RefreshOptions::default()
    };

    let report = refresh_entries(&options, |progress| match progress {
        RefreshProgress::Discovering => {
            println!("Discovering models from the Hugging Face API...");
        }
        RefreshProgress::Model { index, total, id } => {
            println!("[{index:>3}/{total}] {id}");
        }
    })?;

    let catalog = ModelCatalog {
        models: report.entries,
    };
    let yaml_output =
        serde_yaml::to_string(&catalog).context("Unable to serialize catalog as YAML")?;

    if args.dry_run {
        println!("{yaml_output}");
    } else {
        if let Some(parent) = args.output.parent()
            && !parent.as_os_str().is_empty()
        {
            fs::create_dir_all(parent)
                .with_context(|| format!("Unable to create {}", parent.display()))?;
        }
        fs::write(&args.output, &yaml_output)
            .with_context(|| format!("Unable to write {}", args.output.display()))?;
        println!();
        println!(
            "Wrote {} models to {}",
            catalog.models.len(),
            args.output.display()
        );
        println!(
            "Review the file, then promote it with: gaia catalog promote --input {} --output catalog/models.yaml",
            args.output.display()
        );
    }

    if !report.skipped.is_empty() {
        println!();
        println!(
            "Skipped {} non text-generation models:",
            report.skipped.len()
        );
        for id in &report.skipped {
            println!("  - {id}");
        }
    }
    if !report.errors.is_empty() {
        eprintln!();
        eprintln!("Completed with {} errors:", report.errors.len());
        for error in &report.errors {
            eprintln!("  - {error}");
        }
    }

    Ok(())
}

fn run_promote(args: PromoteArgs) -> Result<()> {
    let content = fs::read_to_string(&args.input)
        .with_context(|| format!("Unable to read {}", args.input.display()))?;
    let catalog = serde_yaml::from_str::<ModelCatalog>(&content)
        .with_context(|| format!("Invalid catalog file: {}", args.input.display()))?;

    if catalog.models.is_empty() {
        bail!(
            "Refusing to promote an empty catalog from {}.",
            args.input.display()
        );
    }

    fs::write(&args.output, &content)
        .with_context(|| format!("Unable to write {}", args.output.display()))?;
    println!(
        "Promoted {} models from {} to {}",
        catalog.models.len(),
        args.input.display(),
        args.output.display()
    );
    Ok(())
}

fn load_seed_ids(path: &Path) -> Result<Vec<String>> {
    let catalog = if path.exists() {
        ModelCatalog::load_from_path(path)?
    } else {
        // Fall back to the embedded catalog so the command works outside the repo.
        ModelCatalog::load_default()?
    };
    Ok(catalog.models.into_iter().map(|model| model.id).collect())
}
