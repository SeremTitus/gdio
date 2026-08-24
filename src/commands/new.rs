use crate::config::Config;
use crate::godot;
use anyhow::{Context, Result};
use std::fmt;
use std::fs;

enum Renderer {
    ForwardPlus,
    Mobile,
    Compatibility,
}

impl Renderer {
    fn all() -> &'static [Renderer] {
        &[
            Renderer::ForwardPlus,
            Renderer::Mobile,
            Renderer::Compatibility,
        ]
    }

    fn method(&self) -> &'static str {
        match self {
            Renderer::ForwardPlus => "forward_plus",
            Renderer::Mobile => "mobile",
            Renderer::Compatibility => "gl_compatibility",
        }
    }

    fn mobile_method(&self) -> &'static str {
        match self {
            Renderer::ForwardPlus => "mobile",
            Renderer::Mobile => "mobile",
            Renderer::Compatibility => "gl_compatibility",
        }
    }

    fn feature_name(&self) -> &'static str {
        match self {
            Renderer::ForwardPlus => "Forward Plus",
            Renderer::Mobile => "Mobile",
            Renderer::Compatibility => "GL Compatibility",
        }
    }
}

impl fmt::Display for Renderer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Renderer::ForwardPlus => write!(f, "Forward+"),
            Renderer::Mobile => write!(f, "Mobile"),
            Renderer::Compatibility => write!(f, "Compatibility"),
        }
    }
}

pub async fn run(name: &str, config: &mut Config) -> Result<()> {
    let cwd = std::env::current_dir().context("Failed to get current directory")?;

    if cwd.join("project.godot").exists() {
        anyhow::bail!(
            "Current directory is already a Godot project. \
             Create new projects from a parent directory."
        );
    }

    let project_dir = cwd.join(name);

    if project_dir.exists() {
        anyhow::bail!("Directory '{}' already exists", name);
    }

    // Select renderer
    let renderers = Renderer::all();
    let renderer_names: Vec<String> = renderers.iter().map(ToString::to_string).collect();
    let idx = dialoguer::FuzzySelect::new()
        .with_prompt("Renderer")
        .items(&renderer_names)
        .default(0)
        .interact()?;
    let renderer = &renderers[idx];

    // Create project directory
    fs::create_dir_all(&project_dir)
        .with_context(|| format!("Failed to create directory: {}", project_dir.display()))?;

    // Select editor and open
    let editor = super::shared::resolve_editor(config, None).await?;

    // Create project.godot with version from selected editor
    let (engine_version, _) = crate::config::parse_version_flavor(&editor.version);
    let project_file = project_dir.join("project.godot");
    let content = format!(
        r#"; Engine configuration file.
; It's best edited using the editor UI and not directly,
; since the parameters that go here are not all obvious.
;
; Format:
;   [section] ; section goes between []
;   param=value ; assign values to parameters

config_version=5

[application]

config/name="{}"
config/features=PackedStringArray("{}", "{}")

[rendering]

renderer/rendering_method="{}"
renderer/rendering_method.mobile="{}"
"#,
        name,
        engine_version,
        renderer.feature_name(),
        renderer.method(),
        renderer.mobile_method(),
    );
    fs::write(&project_file, content).context("Failed to write project.godot")?;

    println!("Created project: {}", project_dir.display());

    println!("Opening with {}...", editor.name);
    godot::open_project_editor_mode(&editor.path, &project_file)?;

    // Register project
    super::shared::register_opened_project(config, project_dir, name.to_string(), &editor.version)?;

    Ok(())
}
