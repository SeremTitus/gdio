use crate::config::Config;

pub fn run(config: &Config) {
    if config.editors.is_empty() {
        println!("No editors registered.");
        println!("Use `gdio add <version>` to download an editor.");
        return;
    }

    println!("{:<35} Path", "Name (version)");
    println!("{}", "-".repeat(100));

    let mut editors: Vec<_> = config.editors.values().collect();
    editors.sort_by(|a, b| a.version.cmp(&b.version));

    for editor in editors {
        println!(
            "{:<35} {}",
            format!("{} ({})", editor.name, editor.version),
            editor.path.display()
        );
    }

    println!("\nTotal: {} editor(s)", config.editors.len());
}
