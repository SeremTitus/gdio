use anyhow::{Context, Result};
use serde::Deserialize;
use std::path::Path;
use std::process::Command;

#[derive(Deserialize)]
struct GhRelease {
    assets: Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name: String,
    browser_download_url: String,
}

fn generate_key() -> Result<String> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).context("Failed to generate random bytes")?;
    Ok(bytes.iter().map(|b| format!("{:02x}", b)).collect())
}

pub async fn run(key: Option<&str>) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    // Validate we're in a Godot source directory
    let godot_version = super::detect_godot_version(&cwd)?;

    // Check if already applied
    let security_token = cwd.join("core/crypto/security_token.h");
    if security_token.exists() && std::fs::metadata(&security_token)?.len() > 0 {
        anyhow::bail!(
            "Godot Secure already applied (core/crypto/security_token.h exists).\n\
             To reapply, remove the godot source directory and clone again."
        );
    }

    // Generate or use provided key
    let (key, generated) = match key {
        Some(k) => (k.to_string(), false),
        None => {
            println!("No key provided, generating AES-256 key...");
            (generate_key()?, true)
        }
    };

    println!("Applying Godot Secure to Godot {}...", godot_version);

    // Validate key format
    if key.len() != 64 || !key.chars().all(|c| c.is_ascii_hexdigit()) {
        anyhow::bail!(
            "Invalid AES-256 key. Must be exactly 64 hex characters.\n\
             Generate one with: openssl rand -hex 32"
        );
    }

    // Download Godot Secure script
    println!("Downloading Godot Secure script...");
    let script_name = download_godot_secure(&cwd).await?;

    // Save baseline
    println!("Saving baseline files...");
    save_baseline(&cwd)?;

    // Apply Godot Secure
    println!("Applying Godot Secure...");
    apply_godot_secure(&cwd, &script_name, &key)?;

    // Verify
    println!("Verifying...");
    verify_applied(&cwd)?;

    // Save generated key after successful application
    if generated {
        let key_path = cwd.join("gdio_generated_key_for_godot_secure.txt");
        std::fs::write(&key_path, &key).context("Failed to write generated key file")?;
        println!("  Key saved to: {}", key_path.display());
    }

    println!("\nGodot Secure applied successfully.");
    Ok(())
}

async fn download_godot_secure(dest: &Path) -> Result<String> {
    let client = reqwest::Client::builder()
        .user_agent("gdio")
        .build()
        .context("Failed to build HTTP client")?;

    let resp = client
        .get("https://api.github.com/repos/KnifeXRage/Godot-Secure/releases/latest")
        .send()
        .await
        .context("Failed to fetch Godot Secure releases from GitHub")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to fetch Godot Secure releases (HTTP {})",
            resp.status()
        );
    }

    let release: GhRelease = resp
        .json()
        .await
        .context("Failed to parse GitHub API response")?;

    let asset = release
        .assets
        .iter()
        .find(|a| a.name.contains("Godot.Secure.AES-256.Universal") && a.name.ends_with(".py"))
        .context("Could not find Godot Secure AES-256 Universal script in latest release")?;

    let dest_path = dest.join(&asset.name);

    let resp = client
        .get(&asset.browser_download_url)
        .send()
        .await
        .context("Failed to download Godot Secure script")?;

    if !resp.status().is_success() {
        anyhow::bail!(
            "Failed to download Godot Secure script (HTTP {})",
            resp.status()
        );
    }

    let bytes = resp.bytes().await.context("Failed to read response body")?;
    std::fs::write(&dest_path, &bytes).context("Failed to write script file")?;

    println!("  Downloaded {}", asset.name);
    Ok(asset.name.clone())
}

fn save_baseline(godot_dir: &Path) -> Result<()> {
    let baseline_dir = godot_dir.join(".godot_secure_baseline");
    std::fs::create_dir_all(&baseline_dir)?;

    let files_to_backup = [
        "core/io/file_access_pack.h",
        "core/io/file_access_encrypted.h",
        "core/io/file_access_encrypted.cpp",
    ];

    for file in &files_to_backup {
        let src = godot_dir.join(file);
        if src.exists() {
            let dest = baseline_dir.join(file);
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&src, &dest)?;
        }
    }

    Ok(())
}

fn apply_godot_secure(godot_dir: &Path, script_name: &str, key: &str) -> Result<()> {
    let script_path = godot_dir.join(script_name);
    if !script_path.exists() {
        anyhow::bail!("Godot Secure script not found at {}", script_path.display());
    }

    // Determine python command
    let python_cmd = if cfg!(target_os = "windows") {
        "python"
    } else {
        "python3"
    };

    // Build the interactive input: y\nn\ny\n<key>\n\n\n
    // Last \n handles "Press Enter key to exit..." prompt
    let input = format!("y\nn\ny\n{}\n\n\n", key);

    let mut cmd = Command::new(python_cmd);
    cmd.arg(&script_path).arg(godot_dir).current_dir(godot_dir);

    if cfg!(target_os = "windows") {
        cmd.env("PYTHONIOENCODING", "utf-8");
    }
    cmd.env("SCRIPT_AES256_ENCRYPTION_KEY", key);

    // Use stdin to pipe interactive answers
    use std::io::Write;
    let mut child = cmd
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .context("Failed to run Godot Secure script. Is Python installed?")?;

    if let Some(mut stdin) = child.stdin.take() {
        stdin
            .write_all(input.as_bytes())
            .context("Failed to write to Godot Secure script stdin")?;
        // Flush and drop to ensure all input is sent and EOF is signaled
        let _ = stdin.flush();
    }

    let _output = child
        .wait_with_output()
        .context("Failed to wait for Godot Secure script")?;

    Ok(())
}

fn verify_applied(godot_dir: &Path) -> Result<()> {
    // Check security_token.h was generated
    let security_token = godot_dir.join("core/crypto/security_token.h");
    if !security_token.exists() || std::fs::metadata(&security_token)?.len() == 0 {
        anyhow::bail!(
            "Verification failed: core/crypto/security_token.h was not generated.\n\
             Godot Secure may not have applied correctly."
        );
    }

    // Check file_access_pack.h was modified
    let baseline_dir = godot_dir.join(".godot_secure_baseline");
    let baseline_pack = baseline_dir.join("core/io/file_access_pack.h");
    let current_pack = godot_dir.join("core/io/file_access_pack.h");

    if baseline_pack.exists() && current_pack.exists() {
        let baseline = std::fs::read(&baseline_pack)?;
        let current = std::fs::read(&current_pack)?;
        if baseline == current {
            anyhow::bail!(
                "Verification failed: core/io/file_access_pack.h was not modified.\n\
                 Godot Secure may not have applied correctly."
            );
        }
    }

    // Clean up baseline
    let _ = std::fs::remove_dir_all(&baseline_dir);

    Ok(())
}
