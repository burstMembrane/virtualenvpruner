mod venvs;

use anyhow::{Context, Result};
use clap::Parser;
use console::style;
use human_bytes::human_bytes;
use indicatif::{ProgressBar, ProgressStyle};
use inquire::list_option::ListOption;
use inquire::{Confirm, MultiSelect};

use std::sync::Arc;
use std::time::Instant;
use std::{fs, time::Duration};
use venvs::{get_venvs, VirtualEnv};

#[derive(Parser)]
#[command(name = "venvpruner")]
#[command(author = "Liam Power <liamfpower@gmail.com>")]
#[command(version = "1.0")]
#[command(
    about = "Search and delete Python virtual environments at common search paths",
    long_about = "Search and delete Python virtual environments at common search paths."
)]

struct Cli {}

fn select_venvs_to_delete(venvs: &Vec<VirtualEnv>) -> Result<Vec<VirtualEnv>> {
    // Create a vector of tuples (original index, formatted string)
    let options = venvs
        .into_iter()
        .enumerate()
        .map(|(i, venv)| ListOption::new(i, venv))
        .collect::<Vec<_>>();

    let selected = MultiSelect::new("Select the virtualenvs to delete:", options).prompt();
    let selected = match selected {
        Ok(selected) => selected,
        Err(_) => {
            return Ok(vec![]);
        }
    };

    let selected_venvs = selected
        .iter()
        .map(|option: &ListOption<&VirtualEnv>| (*option.value).clone())
        .collect::<Vec<VirtualEnv>>();

    Ok(selected_venvs)
}

fn print_success_message(message: &str) {
    println!("{}", style(message).green());
}

fn print_info_message(message: &str) {
    println!("{}", style(message).cyan());
}

fn confirm_deletion() -> Result<bool> {
    Confirm::new("Are you sure you want to delete the selected virtual environments?")
        .with_default(false)
        .prompt()
        .map_err(|e| anyhow::anyhow!(e))
}

fn delete_venvs(venvs: &[VirtualEnv]) -> Result<()> {
    // Provide a custom bar style
    let pb = ProgressBar::new(venvs.len() as u64);
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
        pb.inc(1);
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
    let _cli = Cli::parse();

    let spinner = get_spinner();

    let start = Instant::now();

    loop {
        let mut venvs = get_venvs().context("Failed to search for virtual environments")?;

        venvs.sort_by(|a, b| b.venv_size.cmp(&a.venv_size));

        // total size
        let total_size: u64 = venvs.iter().map(|venv| venv.venv_size).sum();
        let total_size_str = human_bytes(total_size as f32);

        spinner.finish_with_message(
            style(format!(
                "Found {} virtual environments in {}",
                venvs.len(),
                format!("{:.4}s", start.elapsed().as_secs_f32())
            ))
            .green()
            .to_string(),
        );

        print_info_message(&format!(
            "Total size of all virtual environments: {}",
            total_size_str
        ));

        if venvs.is_empty() {
            print_info_message("No virtual environments found.");
            break;
        }

        let selected_venvs = select_venvs_to_delete(&venvs)?;

        match selected_venvs.is_empty() {
            true => {
                print_info_message("No virtual environments selected for deletion.");
                break;
            }
            false => {
                if !confirm_deletion()? {
                    print_info_message("Deletion cancelled.");
                    break;
                }
            }
        }

        delete_venvs(&selected_venvs)?;

        // Update the cache
        let remaining_venvs: Vec<VirtualEnv> = venvs
            .into_iter()
            .filter(|venv| !selected_venvs.contains(venv))
            .collect();

        if remaining_venvs.is_empty() {
            print_success_message("All virtual environments have been deleted.");
            break;
        }

        let repeat = Confirm::new("\nDo you want to delete more virtual environments?")
            .with_default(false)
            .prompt()
            .unwrap_or(false);

        if !repeat {
            break;
        }
    }

    Ok(())
}

fn get_spinner() -> Arc<ProgressBar> {
    let spinner = Arc::new(ProgressBar::new_spinner());
    spinner.set_style(ProgressStyle::with_template("{spinner:.green} {msg}").unwrap());
    spinner.set_message("Searching for virtual environments...");
    spinner.enable_steady_tick(Duration::from_millis(100));
    spinner
}
