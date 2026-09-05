use anyhow::{Context, Result};

pub async fn run(csharp: bool, extra_args: &[String]) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    let godot_version = super::detect_godot_version(&cwd)?;
    println!("Building Godot {} editor...", godot_version);

    let mut args = vec!["target=editor".to_string(), "debug_symbols=no".to_string()];

    if csharp {
        args.push("module_mono_enabled=yes".to_string());
    }

    super::run_scons(&args, extra_args).await?;

    println!("\nEditor build complete.");
    Ok(())
}
