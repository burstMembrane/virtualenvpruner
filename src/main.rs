mod venvs;

use anyhow::{Context, Result};
use clap::Parser;
use human_bytes::human_bytes;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::list_option::ListOption;
use inquire::{Confirm, MultiSelect};
use std::sync::Arc;
use std::{fs, time::Duration};
use venvs::{get_venvs, VirtualEnv};

#[derive(Parser)]
#[command(name = "venv-cleaner")]
#[command(author = "Liam Power <your.email@example.com>")]
#[command(version = "1.0")]
#[command(about = "Search and delete Python virtual environments", long_about = None)]
struct Cli {}
fn select_venvs_to_delete(venvs: &Vec<VirtualEnv>) -> Result<Vec<VirtualEnv>> {
    // Create a vector of tuples (original index, formatted string)
    let options = venvs
        .into_iter()
        .enumerate()
        .map(|(i, venv)| ListOption::new(i, venv))
        .collect::<Vec<_>>();

    let selected = MultiSelect::new("Select the virtualenvs to delete:", options)
        // .with_validator(validator)
        // .with_formatter(formatter)
        .prompt()
        .unwrap();

    let selected_venvs = selected
        .iter()
        .map(|option: &ListOption<&VirtualEnv>| (*option.value).clone())
        .collect::<Vec<VirtualEnv>>();

    Ok(selected_venvs)
}

fn confirm_deletion() -> Result<bool> {
    Confirm::new("Are you sure you want to delete the selected virtual environments?")
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!(e))
}

fn delete_venvs(venvs: &[VirtualEnv]) -> Result<()> {
    // Provide a custom bar style
    let pb = ProgressBar::new(1000);
    let mut total_size: u64 = 0;
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] [{bar:40.cyan/blue}] ({pos}/{len}, ETA {eta}) {msg}",
        )
        .unwrap(),
    );

    for venv in venvs {
        pb.set_message(format!(
            "Deleting virtual environment at: {}",
            &venv.path.display()
        ));
        fs::remove_dir_all(&venv.path)
            .with_context(|| format!("Failed to delete {}", venv.path.display()))?;

        total_size += venv.venv_size;
    }
    let total_size_hr = human_bytes(total_size as f32);
    pb.finish_with_message(format!(
        "All selected virtual environments have been deleted. Total size reclaimed: {}",
        total_size_hr
    ));
    Ok(())
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    let progress_bar = Arc::new(ProgressBar::new_spinner());
    progress_bar.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
    progress_bar.set_message("Searching for virtual environments...");
    progress_bar.enable_steady_tick(Duration::from_millis(100));

    let mut venvs_cache: Option<Vec<VirtualEnv>> = None;

    loop {
        let mut venvs = get_venvs().context("Failed to search for virtual environments")?;

        venvs.sort_by(|a, b| b.venv_size.cmp(&a.venv_size));
        // sort by size

        progress_bar.finish_with_message(format!("Found {} virtual environments", venvs.len()));

        // total size
        let total_size: u64 = venvs.iter().map(|venv| venv.venv_size).sum();
        let total_size_str = human_bytes(total_size as f32);

        println!("Total size of all virtual environments: {}", total_size_str);

        if venvs.is_empty() {
            println!("No virtual environments found.");
            break;
        }

        let selected_venvs = select_venvs_to_delete(&venvs)?;
        if selected_venvs.is_empty() {
            println!("No virtual environments selected for deletion.");
            break;
        }

        if !confirm_deletion()? {
            println!("Deletion cancelled.");
            break;
        }

        delete_venvs(&selected_venvs)?;

        // Update the cache
        let remaining_venvs: Vec<VirtualEnv> = venvs
            .into_iter()
            .filter(|venv| !selected_venvs.contains(venv))
            .collect();

        if remaining_venvs.is_empty() {
            println!("All virtual environments have been deleted.");
            break;
        }

        venvs_cache = Some(remaining_venvs);

        let repeat = Confirm::new("Do you want to delete more virtual environments?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);

        if !repeat {
            break;
        }
    }

    Ok(())
}
