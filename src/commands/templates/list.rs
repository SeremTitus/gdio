use crate::config::Config;
use anyhow::Result;
use std::path::Path;

pub fn detect_platforms(dir: &Path) -> Result<Vec<String>> {
    let installed = get_installed_files(dir)?;
    let mut platforms = Vec::new();

    if installed.iter().any(|f| f.starts_with("windows_")) {
        platforms.push("windows".to_string());
    }
    if installed.iter().any(|f| f.starts_with("linux_")) {
        platforms.push("linux".to_string());
    }
    if installed.iter().any(|f| f.starts_with("macos")) {
        platforms.push("macos".to_string());
    }
    if installed.iter().any(|f| f.starts_with("web_")) {
        platforms.push("web".to_string());
    }
    if installed.iter().any(|f| f.starts_with("ios")) {
        platforms.push("ios".to_string());
    }
    if installed.iter().any(|f| f.starts_with("android")) {
        platforms.push("android".to_string());
    }

    Ok(platforms)
}

pub fn run(_config: &Config) -> Result<()> {
    let godot_dir = Config::get_godot_templates_dir();

    if !godot_dir.exists() {
        println!("No export templates installed.");
        println!("Godot stores templates in: {}", godot_dir.display());
        println!("Use `gdio templates add <version>` to install.");
        return Ok(());
    }

    let mut found_any = false;

    for entry in std::fs::read_dir(&godot_dir)? {
        let entry = entry?;
        if entry.path().is_dir() {
            let version = entry.file_name().to_string_lossy().to_string();
            let template_dir = entry.path();
            let installed = get_installed_files(&template_dir)?;

            if installed.is_empty() {
                continue;
            }

            if !found_any {
                found_any = true;
            }

            println!("\n{}", version);
            print_template_tree(&template_dir, &installed);
        }
    }

    if !found_any {
        println!("No export templates found.");
        println!("Use `gdio templates add <version>` to install.");
    }

    Ok(())
}

struct TemplateGroup<'a> {
    platform: &'a str,
    templates: Vec<TemplateInfo<'a>>,
}

struct TemplateInfo<'a> {
    name: &'a str,
    files: Vec<&'a str>,
}

fn get_template_groups() -> Vec<TemplateGroup<'static>> {
    vec![
        TemplateGroup {
            platform: "Windows",
            templates: vec![
                TemplateInfo {
                    name: "Windows x86_32",
                    files: vec![
                        "windows_debug_x86_32.exe",
                        "windows_debug_x86_32_console.exe",
                        "windows_release_x86_32.exe",
                        "windows_release_x86_32_console.exe",
                    ],
                },
                TemplateInfo {
                    name: "Windows x86_64",
                    files: vec![
                        "windows_debug_x86_64.exe",
                        "windows_debug_x86_64_console.exe",
                        "windows_release_x86_64.exe",
                        "windows_release_x86_64_console.exe",
                    ],
                },
                TemplateInfo {
                    name: "Windows arm64",
                    files: vec![
                        "windows_debug_arm64.exe",
                        "windows_debug_arm64_console.exe",
                        "windows_release_arm64.exe",
                        "windows_release_arm64_console.exe",
                    ],
                },
            ],
        },
        TemplateGroup {
            platform: "Linux",
            templates: vec![
                TemplateInfo {
                    name: "Linux x86_32",
                    files: vec!["linux_debug.x86_32", "linux_release.x86_32"],
                },
                TemplateInfo {
                    name: "Linux x86_64",
                    files: vec!["linux_debug.x86_64", "linux_release.x86_64"],
                },
                TemplateInfo {
                    name: "Linux arm32",
                    files: vec!["linux_debug.arm32", "linux_release.arm32"],
                },
                TemplateInfo {
                    name: "Linux arm64",
                    files: vec!["linux_debug.arm64", "linux_release.arm64"],
                },
            ],
        },
        TemplateGroup {
            platform: "macOS",
            templates: vec![TemplateInfo {
                name: "macOS",
                files: vec!["macos.zip"],
            }],
        },
        TemplateGroup {
            platform: "Web",
            templates: vec![
                TemplateInfo {
                    name: "Web",
                    files: vec!["web_debug.zip", "web_release.zip"],
                },
                TemplateInfo {
                    name: "Web with Extensions",
                    files: vec!["web_dlink_debug.zip", "web_dlink_release.zip"],
                },
                TemplateInfo {
                    name: "Web Single-Threaded",
                    files: vec!["web_nothreads_debug.zip", "web_nothreads_release.zip"],
                },
                TemplateInfo {
                    name: "Web with Extensions Single-Threaded",
                    files: vec![
                        "web_dlink_nothreads_debug.zip",
                        "web_dlink_nothreads_release.zip",
                    ],
                },
            ],
        },
        TemplateGroup {
            platform: "Android",
            templates: vec![TemplateInfo {
                name: "Android",
                files: vec![
                    "android_debug.apk",
                    "android_release.apk",
                    "android_source.zip",
                ],
            }],
        },
        TemplateGroup {
            platform: "iOS",
            templates: vec![TemplateInfo {
                name: "iOS",
                files: vec!["ios.zip"],
            }],
        },
    ]
}

pub fn get_installed_files(dir: &Path) -> Result<std::collections::HashSet<String>> {
    let mut files = std::collections::HashSet::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().is_file() {
            let name = entry.file_name().to_string_lossy().to_string();
            files.insert(name);
        }
    }
    Ok(files)
}

fn print_template_tree(_dir: &Path, installed: &std::collections::HashSet<String>) {
    let groups = get_template_groups();

    for group in &groups {
        let mut has_any = false;

        // Check if this platform has any templates
        for template in &group.templates {
            if template.files.iter().any(|f| installed.contains(*f)) {
                has_any = true;
                break;
            }
        }

        if !has_any {
            continue;
        }

        println!("  {}", group.platform);

        for template in &group.templates {
            let present: Vec<&str> = template
                .files
                .iter()
                .filter(|f| installed.contains(**f))
                .copied()
                .collect();

            if present.is_empty() {
                continue;
            }

            // For single-template platforms (macOS, Android, iOS), skip template level
            if group.templates.len() > 1 {
                println!("    {}", template.name);
            }
        }
    }
}
