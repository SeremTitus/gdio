use anyhow::{Context, Result};
use std::path::{Path, PathBuf};

pub fn get_templates_dir(godot_version: &str) -> PathBuf {
    crate::config::Config::get_godot_templates_dir().join(godot_version)
}

pub fn scons_base_args(csharp: bool) -> Vec<String> {
    let mut args = Vec::new();
    if csharp {
        args.push("module_mono_enabled=yes".to_string());
    }
    args.push("production=yes".to_string());
    args.push("debug_symbols=no".to_string());
    args.push("lto=auto".to_string());
    args.push("disable_path_overrides=no".to_string());
    args
}

pub async fn run(
    platform: &crate::platform::PlatformFlags,
    csharp: bool,
    debug: bool,
    release: bool,
    extra_args: &[String],
) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;
    let godot_version = super::detect_godot_version(&cwd)?;

    let build_debug = debug || !release;
    let build_release = release || !debug;

    let platforms = if platform.any() {
        platform.to_platforms()
    } else {
        vec![
            "windows", "linux", "web", "macos", "ios", "android", "visionos",
        ]
        .into_iter()
        .map(String::from)
        .collect()
    };

    println!(
        "Building templates for Godot {} ({})",
        godot_version,
        platforms.join(", ")
    );

    for p in &platforms {
        build_templates_for_platform(p, csharp, build_debug, build_release, extra_args).await?;
    }

    println!("\nInstalling templates...");
    install_templates(&godot_version, &cwd, &platforms)?;

    println!("\nTemplate build complete.");
    Ok(())
}

async fn build_templates_for_platform(
    platform: &str,
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    println!("\n--- {} ---", platform);

    match platform {
        "windows" => build_windows(csharp, build_debug, build_release, extra_args).await,
        "linux" => build_linux(csharp, build_debug, build_release, extra_args).await,
        "web" => build_web(csharp, build_debug, build_release, extra_args).await,
        "android" => build_android(csharp, build_debug, build_release, extra_args).await,
        "macos" => build_apple("macos", csharp, build_debug, build_release, extra_args).await,
        "ios" => build_apple("ios", csharp, build_debug, build_release, extra_args).await,
        "visionos" => build_apple("visionos", csharp, build_debug, build_release, extra_args).await,
        _ => {
            println!("  Unknown platform '{}', skipping.", platform);
            Ok(())
        }
    }
}

async fn build_windows(
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    let archs = ["x86_32", "x86_64", "arm32", "arm64"];

    for arch in &archs {
        for target in &["template_debug", "template_release"] {
            if *target == "template_debug" && !build_debug {
                continue;
            }
            if *target == "template_release" && !build_release {
                continue;
            }

            let mut args = vec![
                "platform=windows".to_string(),
                format!("target={}", target),
                format!("arch={}", arch),
            ];
            args.extend(scons_base_args(csharp));
            super::run_scons(&args, extra_args).await?;

            // Console variant (release only)
            if *target == "template_release" {
                let mut console_args = args.clone();
                console_args.push("windows_subsystem=console".to_string());
                super::run_scons(&console_args, extra_args).await?;
            }
        }
    }

    Ok(())
}

async fn build_linux(
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    let archs = [
        "x86_32",
        "x86_64",
        "arm32",
        "arm64",
        "rv64",
        "ppc64",
        "loongarch64",
    ];

    for arch in &archs {
        for target in &["template_debug", "template_release"] {
            if *target == "template_debug" && !build_debug {
                continue;
            }
            if *target == "template_release" && !build_release {
                continue;
            }

            let mut args = vec![
                "platform=linuxbsd".to_string(),
                format!("target={}", target),
                format!("arch={}", arch),
            ];
            args.extend(scons_base_args(csharp));
            super::run_scons(&args, extra_args).await?;
        }
    }

    Ok(())
}

async fn build_web(
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    let variants: Vec<(&str, Vec<(&str, &str)>)> = vec![
        ("standard", vec![]),
        ("dlink", vec![("dlink_enabled", "yes")]),
        ("nothreads", vec![("threads", "no")]),
        (
            "dlink+nothreads",
            vec![("dlink_enabled", "yes"), ("threads", "no")],
        ),
    ];

    for target in &["template_debug", "template_release"] {
        if *target == "template_debug" && !build_debug {
            continue;
        }
        if *target == "template_release" && !build_release {
            continue;
        }

        for (variant_name, extra_flags) in &variants {
            println!("  Building web {} ({})", target, variant_name);

            let mut args = vec!["platform=web".to_string(), format!("target={}", target)];
            args.extend(scons_base_args(csharp));
            for (key, val) in extra_flags {
                args.push(format!("{}={}", key, val));
            }
            super::run_scons(&args, extra_args).await?;
        }
    }

    Ok(())
}

async fn build_android(
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    let archs = ["arm32", "arm64", "x86_32", "x86_64"];

    for arch in &archs {
        for target in &["template_debug", "template_release"] {
            if *target == "template_debug" && !build_debug {
                continue;
            }
            if *target == "template_release" && !build_release {
                continue;
            }

            let mut args = vec![
                "platform=android".to_string(),
                format!("target={}", target),
                format!("arch={}", arch),
            ];
            args.extend(scons_base_args(csharp));
            super::run_scons(&args, extra_args).await?;
        }
    }

    Ok(())
}

async fn build_apple(
    platform: &str,
    csharp: bool,
    build_debug: bool,
    build_release: bool,
    extra_args: &[String],
) -> Result<()> {
    let archs = ["arm64", "x86_64"];

    for arch in &archs {
        for target in &["template_debug", "template_release"] {
            if *target == "template_debug" && !build_debug {
                continue;
            }
            if *target == "template_release" && !build_release {
                continue;
            }

            let mut args = vec![
                format!("platform={}", platform),
                format!("target={}", target),
                format!("arch={}", arch),
            ];

            // iOS and visionOS: x86_64 requires simulator=yes
            // macOS has no simulator concept
            if *arch == "x86_64" && platform != "macos" {
                args.push("simulator=yes".to_string());
            }

            args.extend(scons_base_args(csharp));
            super::run_scons(&args, extra_args).await?;
        }
    }

    Ok(())
}

fn install_templates(godot_version: &str, godot_dir: &Path, platforms: &[String]) -> Result<()> {
    let templates_dir = get_templates_dir(godot_version);
    std::fs::create_dir_all(&templates_dir)?;

    let bin_dir = godot_dir.join("bin");

    for platform in platforms {
        match platform.as_str() {
            "windows" => install_windows_templates(&bin_dir, &templates_dir)?,
            "linux" => install_linux_templates(&bin_dir, &templates_dir)?,
            "web" => install_web_templates(&bin_dir, &templates_dir)?,
            "android" => install_android_templates(&bin_dir, &templates_dir)?,
            "macos" => install_apple_templates("macos", &bin_dir, &templates_dir)?,
            "ios" => install_apple_templates("ios", &bin_dir, &templates_dir)?,
            "visionos" => install_apple_templates("visionos", &bin_dir, &templates_dir)?,
            _ => {}
        }
    }

    println!("\nTemplates installed to: {}", templates_dir.display());
    Ok(())
}

fn install_windows_templates(bin_dir: &Path, templates_dir: &Path) -> Result<()> {
    use std::fs;

    let archs = ["x86_32", "x86_64", "arm32", "arm64"];
    let targets = [("template_debug", "debug"), ("template_release", "release")];

    for arch in &archs {
        for (scons_target, file_target) in &targets {
            let name = format!("godot.windows.{}.{}", scons_target, arch);
            let src = bin_dir.join(&name);
            if src.exists() {
                let dst = templates_dir.join(format!("windows_{}_{}.exe", file_target, arch));
                fs::copy(&src, &dst)?;
                println!("  ✓ {}", dst.display());
            }

            // Console variant (release only)
            if *scons_target == "template_release" {
                let name_console = format!("{}.console", name);
                let src_console = bin_dir.join(&name_console);
                if src_console.exists() {
                    let dst =
                        templates_dir.join(format!("windows_{}_{}_console.exe", file_target, arch));
                    fs::copy(&src_console, &dst)?;
                    println!("  ✓ {}", dst.display());
                }
            }
        }
    }

    Ok(())
}

fn install_linux_templates(bin_dir: &Path, templates_dir: &Path) -> Result<()> {
    use std::fs;

    let archs = [
        "x86_32",
        "x86_64",
        "arm32",
        "arm64",
        "rv64",
        "ppc64",
        "loongarch64",
    ];
    let targets = [("template_debug", "debug"), ("template_release", "release")];

    for arch in &archs {
        for (scons_target, file_target) in &targets {
            let name = format!("godot.linuxbsd.{}.{}", scons_target, arch);
            let src = bin_dir.join(&name);
            if src.exists() {
                let dst = templates_dir.join(format!("linux_{}_{}", file_target, arch));
                fs::copy(&src, &dst)?;
                println!("  ✓ {}", dst.display());
            }
        }
    }

    Ok(())
}

fn install_web_templates(bin_dir: &Path, templates_dir: &Path) -> Result<()> {
    use std::fs;

    let variants = [
        ("", ""),
        (".nothreads", "_nothreads"),
        (".dlink", "_dlink"),
        (".nothreads.dlink", "_dlink_nothreads"),
    ];
    let targets = [("template_debug", "debug"), ("template_release", "release")];

    for (variant_suffix, file_suffix) in &variants {
        for (scons_target, file_target) in &targets {
            let name = format!("godot.web.{}.wasm32{}.zip", scons_target, variant_suffix);
            let src = bin_dir.join(&name);
            if src.exists() {
                let dst = templates_dir.join(format!("web_{}{}.zip", file_target, file_suffix));
                fs::copy(&src, &dst)?;
                println!("  ✓ {}", dst.display());
            }
        }
    }

    Ok(())
}

fn install_android_templates(bin_dir: &Path, templates_dir: &Path) -> Result<()> {
    use std::fs;

    let archs = ["arm32", "arm64", "x86_32", "x86_64"];
    let targets = [("template_debug", "debug"), ("template_release", "release")];

    for arch in &archs {
        for (scons_target, file_target) in &targets {
            let name = format!("libgodot.android.{}.{}.so", scons_target, arch);
            let src = bin_dir.join(&name);
            if src.exists() {
                let dst = templates_dir.join(format!("android_{}_{}.so", file_target, arch));
                fs::copy(&src, &dst)?;
                println!("  ✓ {}", dst.display());
            }
        }
    }

    // android_source.zip
    let src = bin_dir.join("android_source.zip");
    if src.exists() {
        let dst = templates_dir.join("android_source.zip");
        fs::copy(&src, &dst)?;
        println!("  ✓ {}", dst.display());
    }

    Ok(())
}

fn install_apple_templates(platform: &str, bin_dir: &Path, templates_dir: &Path) -> Result<()> {
    use std::fs;
    use std::io::Write;

    let zip_name = format!("{}.zip", platform);
    let zip_src = bin_dir.join(&zip_name);

    if zip_src.exists() {
        let dst = templates_dir.join(&zip_name);
        fs::copy(&zip_src, &dst)?;
        println!("  ✓ {}", dst.display());
        return Ok(());
    }

    // iOS and visionOS use libgodot prefix; macOS uses godot prefix
    let prefix = if platform == "ios" || platform == "visionos" {
        "libgodot"
    } else {
        "godot"
    };

    // If no zip, look for the individual binaries and create one
    // Apple platforms build for arm64 (device) and x86_64 (simulator)
    let archs = ["arm64", "x86_64"];
    let targets = [("template_debug", "debug"), ("template_release", "release")];

    let mut zip_files: Vec<PathBuf> = Vec::new();
    for arch in &archs {
        for (scons_target, _) in &targets {
            // iOS/visionOS simulator builds have .simulator suffix
            let extra_suffix = if *arch == "x86_64" && platform != "macos" {
                ".simulator"
            } else {
                ""
            };

            // iOS/visionOS produce .a static libs; macOS produces executables
            let ext = if platform == "macos" { "" } else { ".a" };

            let name = format!(
                "{}.{}.{}.{}{}.{}",
                prefix, platform, scons_target, arch, extra_suffix, ext
            );
            let src = bin_dir.join(&name);
            if src.exists() {
                zip_files.push(src);
            }
        }
    }

    if !zip_files.is_empty() {
        let dst = templates_dir.join(&zip_name);
        let file = fs::File::create(&dst)?;
        let mut zip = zip::ZipWriter::new(file);
        for file_path in &zip_files {
            let name = file_path.file_name().unwrap().to_string_lossy();
            zip.start_file(&name, zip::write::SimpleFileOptions::default())?;
            let data = fs::read(file_path)?;
            zip.write_all(&data)?;
        }
        zip.finish()?;
        println!("  ✓ {}", dst.display());
    }

    Ok(())
}
