use crate::config::{Config, EditorSource};
use anyhow::{Context, Result};

pub fn run(target: Option<&str>, config: &mut Config) -> Result<()> {
    if config.editors.is_empty() {
        println!("No editors installed.");
        return Ok(());
    }

    let key = match target {
        Some(version_or_name) => {
            let matching: Vec<String> = config
                .editors
                .iter()
                .filter(|(k, e)| {
                    k.contains(version_or_name)
                        || e.name
                            .to_lowercase()
                            .contains(&version_or_name.to_lowercase())
                })
                .map(|(k, _)| k.clone())
                .collect();

            match matching.len() {
                0 => {
                    anyhow::bail!("No editor found matching '{}'", version_or_name);
                }
                1 => matching[0].clone(),
                _ => {
                    let names: Vec<String> = matching
                        .iter()
                        .filter_map(|k| config.editors.get(k))
                        .map(|e| format!("{} ({})", e.name, e.version))
                        .collect();
                    let idx = dialoguer::FuzzySelect::new()
                        .with_prompt("Multiple matches found. Select one to remove")
                        .items(&names)
                        .default(0)
                        .interact()?;
                    matching[idx].clone()
                }
            }
        }
        None => {
            // Interactive: show all editors
            let editors: Vec<_> = config.editors.iter().collect();
            let names: Vec<String> = editors
                .iter()
                .map(|(_, e)| format!("{} ({})", e.name, e.version))
                .collect();

            let idx = dialoguer::FuzzySelect::new()
                .with_prompt("Select editor to remove")
                .items(&names)
                .default(0)
                .interact()?;
            editors[idx].0.clone()
        }
    };

    let source = config.editors[&key].source.clone();
    let path = config.editors[&key].path.clone();
    let name = config.editors[&key].name.clone();

    if source == EditorSource::Downloaded {
        if path.exists() {
            if path.is_dir() {
                std::fs::remove_dir_all(&path)
                    .with_context(|| format!("Failed to delete {}", path.display()))?;
            } else {
                std::fs::remove_file(&path)
                    .with_context(|| format!("Failed to delete {}", path.display()))?;
            }
            println!("Deleted: {}", path.display());
        }
    } else {
        println!(
            "Local editor not deleted (only unregistered): {}",
            path.display()
        );
    }

    config.remove_editor(&key);
    config.save()?;
    println!("Removed: {}", name);

    Ok(())
}
