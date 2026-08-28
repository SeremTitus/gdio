use anyhow::{Context, Result};

pub fn run(tag: Option<&str>) -> Result<()> {
    let ctx = super::shared::ProjectContext::detect("Unknown Project")?;

    let mut tags = crate::project::parse_tags(&ctx.project_file);

    match tag {
        None => {
            if tags.is_empty() {
                println!("No tags");
            } else {
                println!("Tags: {}", tags.join(", "));
            }
        }
        Some(tag_name) => {
            let tag_lower = tag_name.to_lowercase();
            if let Some(pos) = tags.iter().position(|t| t == &tag_lower) {
                tags.remove(pos);
                crate::project::write_tags(&ctx.project_file, &tags)
                    .context("Failed to write tags to project.godot")?;
                println!("Removed tag: {}", tag_lower);
            } else {
                tags.push(tag_lower.clone());
                crate::project::write_tags(&ctx.project_file, &tags)
                    .context("Failed to write tags to project.godot")?;
                println!("Added tag: {}", tag_lower);
            }

            let mut tags = crate::project::parse_tags(&ctx.project_file);
            tags.sort();
            if tags.is_empty() {
                println!("No tags");
            } else {
                println!("Tags: {}", tags.join(", "));
            }
        }
    }

    Ok(())
}
