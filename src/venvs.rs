use anyhow::{anyhow, Context, Result};
use dirs::home_dir;
use human_bytes::human_bytes;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use std::ffi::OsStr;

use std::collections::HashSet;
use std::fmt;
use std::fs::canonicalize;
use std::fs::symlink_metadata;
use std::fs::{read_dir, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct VirtualEnv {
    pub path: PathBuf,
    pub name: String,
    pub python_path: PathBuf,
    pub python_version: String,
    pub venv_size: u64,
    pub venv_size_str: String,
}

impl fmt::Display for VirtualEnv {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} - {} ({}) [{}]",
            self.name,
            self.path.display(),
            self.venv_size_str,
            self.python_version
        )
    }
}

pub fn get_venv_paths() -> Result<Vec<PathBuf>> {
    let home_dir = home_dir().ok_or_else(|| anyhow!("Could not find home directory"))?;

    let search_paths = vec![
        // pipx
        home_dir.join(".local/pipx/venvs"),
        // virtualenvwrapper
        home_dir.join(".virtualenvs"),
        // virtualenv
        home_dir.join(".local/share/virtualenvs"),
        "/usr/local/share/virtualenvs".into(),
        "/usr/share/virtualenvs".into(),
        "/opt/virtualenvs".into(),
        home_dir.join(".config/virtualenvs"),
        // poetry
        home_dir.join(".cache/pypoetry/virtualenvs"),
        // conda and its variants
        home_dir.join(".conda/envs"),
        home_dir.join(".miniconda/envs"),
        home_dir.join(".miniforge/envs"),
        home_dir.join("anaconda3/envs"),
        home_dir.join("miniconda3/envs"),
        home_dir.join("miniforge3/envs"),
        home_dir.join("mambaforge/envs"),
        home_dir.join("mambaforge3/envs"),
        // pyenv
        home_dir.join(".pyenv/versions/envs"),
        // asdf
        home_dir.join(".asdf/installs/python"),
        home_dir.join(".asdf/installs/python/versions"),
        // Enthought Canopy (for macOS/Linux)
        home_dir.join("Library/Enthought/Canopy/edm/envs"),
        // PyCharm (replace with appropriate paths if needed)
        home_dir.join(".PyCharmXXXX.X/config/virtualenvs"),
        // Additional system locations
        "/opt/anaconda3/envs".into(),
        "/opt/miniconda3/envs".into(),
    ];

    // Step 1: Canonicalize each search path to resolve symlinks
    let canonical_paths: Vec<PathBuf> = search_paths
        .iter() // Use parallel iteration for efficiency
        .filter_map(|path| canonicalize(path).ok()) // Resolve symlinks, skip if failed
        .collect();
    // Step 2: Deduplicate canonical paths using a HashSet
    let mut unique_canonical_paths = HashSet::new();
    let unique_paths: Vec<PathBuf> = canonical_paths
        .into_iter()
        .filter(|p| unique_canonical_paths.insert(p.clone())) // Insert returns false if already present
        .collect();

    let venv_roots: Vec<PathBuf> = unique_paths
        .into_par_iter()
        .map(|search_path| {
            WalkDir::new(search_path)
                .follow_links(false)
                .max_depth(4)
                .into_iter()
                .filter_map(Result::ok)
                .filter(|entry| {
                    // Compare OsStr directly without type mismatch
                    entry.file_name() == "python"
                        && entry
                            .path()
                            .parent()
                            .map_or(false, |p| p.file_name() == Some(OsStr::new("bin")))
                })
                .filter_map(|entry| {
                    entry
                        .path()
                        .parent() // bin_dir
                        .and_then(|bin_dir| bin_dir.parent()) // venv_root
                        .map(|venv_root| venv_root.to_path_buf())
                })
                .collect::<Vec<_>>() // Collect the inner iterator into a Vec
        })
        .flatten() // Flatten the Vec<Vec<PathBuf>> into Vec<PathBuf>
        .collect(); // Collect the final results into Vec<PathBuf>
                    // deduplicate the paths

    Ok(venv_roots)
}

pub fn get_dir_size(path: &Path) -> u64 {
    // Get the metadata of the current path without following symlinks
    let metadata = match symlink_metadata(path) {
        Ok(meta) => meta,
        Err(_) => {
            eprintln!("Failed to get metadata for {}", path.display());
            return 0;
        }
    };

    // Check if the path is a symlink
    if metadata.file_type().is_symlink() {
        // Skip symlinks to avoid cycles and double-counting
        return 0;
    }

    // Start with the size of the current file
    let mut size = metadata.len();

    // If it's a directory, recursively get the size of its contents
    if metadata.is_dir() {
        let entries = match read_dir(path) {
            Ok(entries) => entries,
            Err(_) => {
                eprintln!("Failed to read directory {}", path.display());
                return size;
            }
        };

        // Process entries in parallel and accumulate sizes
        let dir_size: u64 = entries
            .par_bridge()
            .map(|entry_result| {
                match entry_result {
                    Ok(entry) => {
                        let entry_path = entry.path();
                        // Recursively calculate the size of each entry
                        get_dir_size(&entry_path)
                    }
                    Err(_) => {
                        eprintln!("Failed to read an entry in {}", path.display());
                        0
                    }
                }
            })
            .sum();

        // Add the size of the directory contents to the current directory size
        size += dir_size;
    }

    size
}

pub fn build_virtualenv(path: PathBuf) -> Result<VirtualEnv> {
    let bin_dir = path.join("bin");
    let python_path = bin_dir.join("python");

    // Ensure that the python executable exists
    if !python_path.exists() {
        return Err(anyhow!(
            "Python executable not found in {}",
            python_path.display()
        ));
    }

    // Get the python version
    let python_version = get_python_version(&path)?.unwrap_or_else(|| "Unknown".to_string());

    let name = path
        .file_name()
        .and_then(|os_str| os_str.to_str())
        .ok_or_else(|| anyhow!("Failed to parse virtual environment name"))?
        .to_string();

    let venv_size = get_dir_size(&path);
    let venv_size_str = human_bytes(venv_size as f64);

    Ok(VirtualEnv {
        path,
        name,
        python_path,
        python_version,
        venv_size,
        venv_size_str,
    })
}

pub fn get_python_version(venv_root: &Path) -> Result<Option<String>> {
    // Method 1: Read 'pyvenv.cfg' if it exists
    let pyvenv_cfg_path = venv_root.join("pyvenv.cfg");
    if pyvenv_cfg_path.exists() {
        let file = File::open(&pyvenv_cfg_path)
            .with_context(|| format!("Failed to open {}", pyvenv_cfg_path.display()))?;
        let version_line = BufReader::new(file)
            .lines()
            .filter_map(Result::ok)
            .find(|line| line.starts_with("version = "));
        if let Some(line) = version_line {
            let version = line["version = ".len()..].trim().to_string();
            return Ok(Some(version));
        }
    }

    // Method 2: Inspect the 'lib' directory
    let lib_dir = venv_root.join("lib");
    if lib_dir.exists() {
        if let Some(version) = read_dir(&lib_dir)
            .with_context(|| format!("Failed to read {}", lib_dir.display()))?
            .filter_map(Result::ok)
            .map(|entry| entry.file_name())
            .filter_map(|os_str| os_str.to_str().map(String::from))
            .find(|name| name.starts_with("python"))
            .map(|name| name.trim_start_matches("python").to_string())
        {
            if !version.is_empty() {
                return Ok(Some(version));
            }
        }
    }

    // Method 3: Check for 'conda-meta/history' file
    let conda_history_path = venv_root.join("conda-meta/history");
    if conda_history_path.exists() {
        let file = File::open(&conda_history_path)
            .with_context(|| format!("Failed to open {}", conda_history_path.display()))?;
        let version_line = BufReader::new(file)
            .lines()
            .filter_map(Result::ok)
            .find(|line| line.contains("python-"));
        if let Some(line) = version_line {
            if let Some(start) = line.find("python-") {
                let version_info = &line[start + "python-".len()..];
                let version = version_info
                    .split_whitespace()
                    .next()
                    .unwrap_or("Unknown")
                    .to_string();
                return Ok(Some(version));
            }
        }
    }

    // Method 4: Run 'python --version' (Most computational load)
    let python_exec = venv_root.join("bin/python");
    if python_exec.exists() {
        let output = Command::new(&python_exec)
            .arg("--version")
            .output()
            .with_context(|| format!("Failed to execute '{} --version'", python_exec.display()))?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let version_output = if !stdout.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        if version_output.starts_with("Python ") {
            let version = version_output["Python ".len()..].to_string();
            return Ok(Some(version));
        }
    }

    // If all methods fail, return None
    Ok(None)
}

pub fn build_virtualenvs(venv_paths: Vec<PathBuf>) -> Result<Vec<VirtualEnv>> {
    let venvs: Vec<VirtualEnv> = venv_paths
        .into_par_iter()
        .filter_map(|path| match build_virtualenv(path) {
            Ok(venv) => Some(venv),
            Err(err) => {
                eprintln!("Error building virtualenv: {}", err);
                None
            }
        })
        .collect();
    Ok(venvs)
}

pub fn get_venvs() -> Result<Vec<VirtualEnv>> {
    let venv_paths = get_venv_paths().context("Failed to get virtual environment paths")?;
    let venvs = build_virtualenvs(venv_paths).context("Failed to build virtual environments")?;
    Ok(venvs)
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    pub fn test_get_venv_paths() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        assert!(!venv_paths.is_empty(), "No virtual environments found");
    }

    #[test]
    pub fn test_build_virtualenv() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let venv =
            build_virtualenv(venv_paths[0].clone()).expect("Failed to build virtual environment");
        assert_eq!(venv.path, venv_paths[0]);
    }

    #[test]
    pub fn test_get_python_version() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let python_version = get_python_version(&venv_paths[0])
            .expect("Failed to get Python version")
            .expect("Python version not found");
        assert!(!python_version.is_empty(), "Python version is empty");
    }

    #[test]
    pub fn test_serialize_venv() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let venv =
            build_virtualenv(venv_paths[0].clone()).expect("Failed to build virtual environment");
        let serialized = serde_json::to_string(&venv).expect("Failed to serialize virtual env");
        assert!(!serialized.is_empty(), "Serialized virtual env is empty");
    }

    #[test]
    pub fn test_serialize_all_venvs() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let venvs: Vec<VirtualEnv> = venv_paths
            .into_iter()
            .filter_map(|path| build_virtualenv(path).ok())
            .collect();
        let serialized =
            serde_json::to_string(&venvs).expect("Failed to serialize virtual environments");
        assert!(
            !serialized.is_empty(),
            "Serialized virtual environments are empty"
        );
    }

    #[test]
    pub fn test_get_size() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let size = get_dir_size(&venv_paths[0]);

        assert!(size > 0, "Virtual environment size is zero");
    }

    #[test]
    pub fn test_build_all_virtualenvs() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let venvs = build_virtualenvs(venv_paths).expect("Failed to build virtual environments");
        assert!(!venvs.is_empty(), "No virtual environments built");
    }

    #[test]
    pub fn test_get_size_human() {
        let venv_paths = get_venv_paths().expect("Failed to get virtual environment paths");
        let size = get_dir_size(&venv_paths[0]);
        let size_str = human_bytes(size as f64);
        dbg!(&size_str);
        assert!(!size_str.is_empty(), "Human-readable size is empty");
    }
}
