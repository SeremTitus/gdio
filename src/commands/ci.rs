use crate::config::{self, Config};
use anyhow::{Context, Result};
use console::Style;
use std::fs;

const STANDARD_TEMPLATE: &str = include_str!("../../scripts/ci/export_pipeline_from_gdio.yml");
const SECURE_TEMPLATE: &str = include_str!("../../scripts/ci/export_pipeline_secure_from_gdio.yml");
const CI_PACKAGE_SCRIPT: &str = include_str!("../../scripts/ci/ci_package.py");

pub fn run(project_dir: &std::path::Path, config: &Config) -> Result<()> {
    let ctx = super::shared::ProjectContext::from_dir(project_dir, "game")?;

    let workflows_dir = ctx.cwd.join(".github").join("workflows");
    fs::create_dir_all(&workflows_dir).context("Failed to create .github/workflows directory")?;

    let editor = ctx
        .bound_editor(config)
        .context("No editor bound to this project. Run `gdio bind` first.")?;
    let (base, flavor) = config::parse_version_flavor(&editor.version);
    let tag = format!("{}.{}", base, flavor);
    let csharp_bool = if editor.is_mono { "true" } else { "false" };

    let script_file = workflows_dir.join("ci_package.py");
    fs::write(&script_file, CI_PACKAGE_SCRIPT).context("Failed to write ci_package.py")?;

    let standard_yaml = STANDARD_TEMPLATE
        .replace("{tag}", &tag)
        .replace("{csharp_bool}", csharp_bool);
    let standard_file = workflows_dir.join("export_pipeline_from_gdio.yml");
    fs::write(&standard_file, &standard_yaml).context("Failed to write workflow file")?;
    let cyan = Style::new().cyan();

    if flavor == "stable" {
        let secure_yaml = SECURE_TEMPLATE
            .replace("{tag}", &tag)
            .replace("{csharp_bool}", csharp_bool);
        let secure_file = workflows_dir.join("export_pipeline_secure_from_gdio.yml");
        fs::write(&secure_file, &secure_yaml).context("Failed to write secure workflow file")?;
        println!("To use secure pipeline:");
        println!(
            "  {}",
            cyan.apply_to("Settings -> Secrets and variables -> Actions -> New repository secret")
        );
        println!("  {}", cyan.apply_to("Name:  SCRIPT_AES256_ENCRYPTION_KEY"));
        println!("  {}", cyan.apply_to("Value: <64 hex characters>"));
        println!(
            "      {}",
            cyan.apply_to("generate by running: `openssl rand -hex 32`")
        );
        println!(
            "                       {}",
            cyan.apply_to("or: `python -c \"import secrets; print(secrets.token_hex(32))\"`")
        );
    } else {
        let yellow = Style::new().yellow();
        println!(
            "\n{}",
            yellow.apply_to(
                "export_pipeline_secure_from_gdio.yml skipped (editor is not a stable version)"
            )
        );
    }

    println!("Trigger the workflow via:");
    println!(
        "  {}",
        cyan.apply_to(
            "Actions -> gdio Export Pipeline/gdio Secure Export Pipeline -> Run workflow"
        )
    );

    Ok(())
}
