use std::env;
use std::process::Command;

use serde::{Deserialize, Serialize};
use sysinfo::System;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DockerStatus {
    pub installed: bool,
    pub daemon_accessible: bool,
    pub version: Option<String>,
    pub details: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GpuInfo {
    pub name: String,
    pub vram_gb: f32,
    pub driver_version: Option<String>,
    pub cuda_version: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MachineSpecs {
    pub os_name: String,
    pub kernel_version: Option<String>,
    pub cpu_cores: usize,
    pub ram_total_gb: f32,
    pub docker: DockerStatus,
    pub gpu: Option<GpuInfo>,
    pub hf_token_present: bool,
    pub warnings: Vec<String>,
}

#[derive(Debug)]
struct ProbeResult {
    success: bool,
    stdout: String,
    stderr: String,
    error: Option<String>,
}

impl MachineSpecs {
    pub fn detect() -> Self {
        let mut system = System::new_all();
        system.refresh_all();

        let os_name = System::long_os_version()
            .or_else(System::name)
            .unwrap_or_else(|| "Unknown OS".to_owned());

        let kernel_version = System::kernel_version();
        let cpu_cores = system.cpus().len();
        let ram_total_gb = bytes_to_gb(system.total_memory());
        let docker = detect_docker();

        let mut warnings = Vec::new();
        let gpu = detect_gpu().map_err(|warning| {
            warnings.push(warning);
        });

        let hf_token_present = env::var("HF_TOKEN")
            .map(|value| !value.trim().is_empty())
            .unwrap_or(false);

        if !docker.installed {
            warnings.push("Docker is not installed.".to_owned());
        } else if !docker.daemon_accessible {
            warnings.push(
                "Docker daemon is not reachable (run as root or configure docker group)."
                    .to_owned(),
            );
        }

        Self {
            os_name,
            kernel_version,
            cpu_cores,
            ram_total_gb,
            docker,
            gpu: gpu.ok(),
            hf_token_present,
            warnings,
        }
    }

    pub fn gpu_vram_gb(&self) -> Option<f32> {
        self.gpu.as_ref().map(|gpu| gpu.vram_gb)
    }
}

fn detect_docker() -> DockerStatus {
    let version_probe = run_command("docker", &["--version"]);
    if version_probe.error.is_some() {
        return DockerStatus {
            installed: false,
            daemon_accessible: false,
            version: None,
            details: version_probe.error,
        };
    }

    let info_probe = run_command("docker", &["info"]);

    DockerStatus {
        installed: true,
        daemon_accessible: info_probe.success,
        version: trim_line(&version_probe.stdout),
        details: if info_probe.success {
            None
        } else if let Some(error) = info_probe.error {
            Some(error)
        } else {
            trim_line(&info_probe.stderr)
        },
    }
}

fn detect_gpu() -> Result<GpuInfo, String> {
    let query_probe = run_command(
        "nvidia-smi",
        &[
            "--query-gpu=name,memory.total,driver_version",
            "--format=csv,noheader",
        ],
    );

    if let Some(error) = query_probe.error {
        return Err(format!("NVIDIA GPU detection skipped ({error})."));
    }

    if !query_probe.success {
        return Err(format!(
            "NVIDIA GPU detection failed: {}",
            query_probe
                .stderr
                .lines()
                .next()
                .unwrap_or("nvidia-smi exited with an error")
        ));
    }

    let line = query_probe
        .stdout
        .lines()
        .next()
        .ok_or_else(|| "nvidia-smi returned an empty GPU list.".to_owned())?;

    let mut parts = line.split(',').map(str::trim);
    let name = parts
        .next()
        .ok_or_else(|| "Unable to parse GPU name.".to_owned())?
        .to_owned();
    let memory_raw = parts
        .next()
        .ok_or_else(|| "Unable to parse GPU memory.".to_owned())?;
    let driver_version = parts.next().map(str::to_owned);

    let vram_gb = parse_memory_mib(memory_raw)
        .map(bytes_to_gb)
        .ok_or_else(|| format!("Unable to parse GPU memory value `{memory_raw}`."))?;

    let cuda_version = detect_cuda_version();

    Ok(GpuInfo {
        name,
        vram_gb,
        driver_version,
        cuda_version,
    })
}

fn detect_cuda_version() -> Option<String> {
    let probe = run_command("nvidia-smi", &[]);
    if !probe.success {
        return None;
    }

    for line in probe.stdout.lines() {
        if let Some(cuda_fragment) = line.split("CUDA Version:").nth(1) {
            let version = cuda_fragment.split_whitespace().next()?;
            return Some(version.to_owned());
        }
    }

    None
}

fn parse_memory_mib(input: &str) -> Option<u64> {
    let numeric = input
        .chars()
        .filter(|ch| ch.is_ascii_digit() || *ch == '.')
        .collect::<String>();

    let value = numeric.parse::<f64>().ok()?;
    Some((value * 1024_f64 * 1024_f64) as u64)
}

fn bytes_to_gb(bytes: u64) -> f32 {
    bytes as f32 / 1024_f32 / 1024_f32 / 1024_f32
}

fn run_command(program: &str, args: &[&str]) -> ProbeResult {
    match Command::new(program).args(args).output() {
        Ok(output) => ProbeResult {
            success: output.status.success(),
            stdout: String::from_utf8_lossy(&output.stdout).trim().to_owned(),
            stderr: String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            error: None,
        },
        Err(error) => ProbeResult {
            success: false,
            stdout: String::new(),
            stderr: String::new(),
            error: Some(error.to_string()),
        },
    }
}

fn trim_line(value: &str) -> Option<String> {
    value.lines().next().map(str::trim).and_then(|line| {
        if line.is_empty() {
            None
        } else {
            Some(line.to_owned())
        }
    })
}
