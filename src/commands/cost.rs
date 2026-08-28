use crate::config::Config;
use anyhow::Result;
use console::Style;

fn dir_size(path: &std::path::Path) -> u64 {
    if !path.exists() {
        return 0;
    }
    walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter_map(|e| e.metadata().ok())
        .filter(|m| m.is_file())
        .map(|m| m.len())
        .sum()
}

fn format_size(bytes: u64) -> String {
    if bytes < 1024 {
        format!("{} B", bytes)
    } else if bytes < 1024 * 1024 {
        format!("{:.1} KB", bytes as f64 / 1024.0)
    } else if bytes < 1024 * 1024 * 1024 {
        format!("{:.1} MB", bytes as f64 / (1024.0 * 1024.0))
    } else {
        format!("{:.2} GB", bytes as f64 / (1024.0 * 1024.0 * 1024.0))
    }
}

pub fn run(_config: &Config) -> Result<()> {
    let config_dir = Config::config_dir();
    let global_addons_dir = Config::get_global_addons_dir();
    let editors_dir = Config::get_editors_dir();
    let templates_dir = Config::get_godot_templates_dir();
    let butler_dir = Config::get_butler_dir();

    let binary_path = std::env::current_exe().ok().map_or_else(
        || "unknown".to_string(),
        |p| p.to_string_lossy().to_string(),
    );
    let binary_size = std::env::current_exe()
        .ok()
        .and_then(|p| std::fs::metadata(p).ok())
        .map_or(0, |m| m.len());

    let config_path = config_dir.join("config.json");
    let config_size = dir_size(&config_path);
    let global_addons_size = dir_size(&global_addons_dir);
    let editors_size = dir_size(&editors_dir);
    let templates_size = dir_size(&templates_dir);
    let butler_size = dir_size(&butler_dir);

    let total = binary_size + config_size + editors_size + templates_size + butler_size;

    let header = Style::new().bold();
    let blue = Style::new().blue();
    let dim = Style::new().dim();

    println!();
    println!("{}", header.apply_to("gdio disk usage"));
    println!("{}", "-".repeat(70));
    println!("  {:<25} {:<12} Path", "Component", "Size");
    println!("{}", "-".repeat(70));
    println!(
        "  {:<25} {:<12} {}",
        "Binary (gdio)",
        format_size(binary_size),
        dim.apply_to(&binary_path)
    );
    println!(
        "  {:<25} {:<12} {}",
        "Config file",
        format_size(config_size),
        dim.apply_to(config_path.display())
    );
    println!(
        "  {:<25} {:<12} {}",
        "Linked addons",
        format_size(global_addons_size),
        dim.apply_to(global_addons_dir.display())
    );
    println!(
        "  {:<25} {:<12} {}",
        "Downloaded editors",
        format_size(editors_size),
        dim.apply_to(editors_dir.display())
    );
    println!(
        "  {:<25} {:<12} {}",
        "Export templates",
        format_size(templates_size),
        dim.apply_to(templates_dir.display())
    );
    println!(
        "  {:<25} {:<12} {}",
        "Butler",
        format_size(butler_size),
        dim.apply_to(butler_dir.display())
    );
    println!("{}", "-".repeat(70));
    println!("  {} {:<12}", blue.apply_to("Total"), format_size(total));
    println!();
    Ok(())
}
