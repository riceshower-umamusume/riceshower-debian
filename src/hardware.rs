use crate::process::Runner;
use anyhow::Result;
use std::fmt;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GpuVendor {
    Amd,
    Intel,
    Nvidia,
    Unknown,
}

impl GpuVendor {
    pub fn recommended_packages(self) -> &'static [&'static str] {
        match self {
            GpuVendor::Amd => &[
                "firmware-amd-graphics",
                "mesa-vulkan-drivers",
                "vulkan-tools",
            ],
            GpuVendor::Intel => &[
                "firmware-intel-graphics",
                "intel-media-va-driver",
                "mesa-vulkan-drivers",
                "vulkan-tools",
            ],
            GpuVendor::Nvidia => &["nvidia-driver", "nvidia-vulkan-icd"],
            GpuVendor::Unknown => &[],
        }
    }
}

impl fmt::Display for GpuVendor {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            GpuVendor::Amd => "amd",
            GpuVendor::Intel => "intel",
            GpuVendor::Nvidia => "nvidia",
            GpuVendor::Unknown => "unknown",
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Mode {
    pub width: u32,
    pub height: u32,
    pub refresh: f64,
}

impl Mode {
    pub fn label(&self) -> String {
        let refresh = if self.refresh.fract() == 0.0 {
            format!("{}", self.refresh as u64)
        } else {
            format!("{}", self.refresh)
        };
        format!("{}x{}@{}", self.width, self.height, refresh)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Output {
    pub connector: String,
    pub modes: Vec<Mode>,
}

impl Output {
    pub fn best_mode(&self) -> Option<&Mode> {
        self.modes.iter().max_by_key(|mode| {
            (
                mode.width.saturating_mul(mode.height),
                (mode.refresh * 1000.0) as u64,
            )
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Hardware {
    pub gpu: GpuVendor,
    pub outputs: Vec<Output>,
}

pub fn detect(runner: &dyn Runner) -> Result<Hardware> {
    let gpu = detect_gpu(Path::new("/sys/class/drm"));
    let outputs = match runner.run("umbriel", &["outputs"]) {
        Ok(output) if output.success => parse_umbriel_outputs(&output.stdout),
        _ => Vec::new(),
    };
    Ok(Hardware { gpu, outputs })
}

pub fn detect_gpu(sysfs_root: &Path) -> GpuVendor {
    let Ok(entries) = std::fs::read_dir(sysfs_root) else {
        return GpuVendor::Unknown;
    };
    for entry in entries.flatten() {
        let name = entry.file_name();
        if !name.to_string_lossy().starts_with("card") {
            continue;
        }
        let vendor_path = entry.path().join("device/vendor");
        let Ok(vendor) = std::fs::read_to_string(vendor_path) else {
            continue;
        };
        match vendor.trim().to_ascii_lowercase().as_str() {
            "0x1002" => return GpuVendor::Amd,
            "0x8086" => return GpuVendor::Intel,
            "0x10de" => return GpuVendor::Nvidia,
            _ => {}
        }
    }
    GpuVendor::Unknown
}

pub fn parse_umbriel_outputs(text: &str) -> Vec<Output> {
    let mut outputs = Vec::new();
    let mut current: Option<Output> = None;
    for line in text.lines() {
        let indented = line.starts_with(' ') || line.starts_with('\t');
        let trimmed = line.trim();
        if !indented {
            if let Some(output) = current.take() {
                outputs.push(output);
            }
            if let Some(connector) = trimmed.split_whitespace().next() {
                if !connector.is_empty() {
                    current = Some(Output {
                        connector: connector.to_string(),
                        modes: Vec::new(),
                    });
                }
            }
            continue;
        }
        let Some(output) = current.as_mut() else {
            continue;
        };
        let Some((resolution, rest)) = trimmed.split_once(" @ ") else {
            continue;
        };
        let Some((width, height)) = resolution.split_once('x') else {
            continue;
        };
        let Some(refresh) = rest.split_whitespace().next() else {
            continue;
        };
        let (Ok(width), Ok(height), Ok(refresh)) = (
            width.parse::<u32>(),
            height.parse::<u32>(),
            refresh.parse::<f64>(),
        ) else {
            continue;
        };
        output.modes.push(Mode {
            width,
            height,
            refresh,
        });
    }
    if let Some(output) = current {
        outputs.push(output);
    }
    outputs
}

pub fn display_toml(hardware: &Hardware) -> String {
    let mut result = String::new();
    for output in &hardware.outputs {
        if let Some(mode) = output.best_mode() {
            result.push_str(&format!(
                "[output.{}]\nmode = \"{}\"\nposition = [0, 0]\nscale = 1.0\n\n",
                output.connector,
                mode.label()
            ));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    const OUTPUTS: &str = r#"DP-1 "Acer Technologies KB242Y P6 1522085FC4216 (DP-1)"
  Enabled: yes
  Modes:
    1920x1080 @ 60.000 Hz (preferred, current)
    1920x1080 @ 144.000 Hz
    1920x1080 @ 119.880 Hz
    1680x1050 @ 59.954 Hz
"#;

    #[test]
    fn parses_outputs_and_picks_highest_refresh_at_native_resolution() {
        let outputs = parse_umbriel_outputs(OUTPUTS);
        assert_eq!(outputs.len(), 1);
        assert_eq!(outputs[0].connector, "DP-1");
        assert_eq!(outputs[0].modes.len(), 4);
        assert_eq!(outputs[0].best_mode().unwrap().label(), "1920x1080@144");
    }

    #[test]
    fn detects_amd_gpu_from_sysfs() {
        let directory = tempfile::tempdir().unwrap();
        let device = directory.path().join("card0/device");
        fs::create_dir_all(&device).unwrap();
        fs::write(device.join("vendor"), "0x1002\n").unwrap();
        assert_eq!(detect_gpu(directory.path()), GpuVendor::Amd);
    }

    #[test]
    fn recommends_amd_graphics_packages() {
        assert!(GpuVendor::Amd
            .recommended_packages()
            .contains(&"mesa-vulkan-drivers"));
    }

    #[test]
    fn display_toml_contains_output_mode() {
        let hardware = Hardware {
            gpu: GpuVendor::Amd,
            outputs: parse_umbriel_outputs(OUTPUTS),
        };
        let toml = display_toml(&hardware);
        assert!(toml.contains("[output.DP-1]"));
        assert!(toml.contains("mode = \"1920x1080@144\""));
    }
}
