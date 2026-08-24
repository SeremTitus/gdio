/// Asset Store HTTP client and version compatibility checking.
pub mod api;

/// File operations: ZIP extraction, symlinks, .gdio and .gitignore management.
pub mod storage;

/// `gdio addons add` — download and install addons from the asset store.
pub mod add;

/// `gdio addons list` — list addons in a project or globally.
pub mod list;

/// `gdio addons remove` — remove addons by folder name.
pub mod remove;

/// `gdio addons globals` — manage global addons (synced to all projects).
pub mod globals;

/// `gdio addons exclude` — manage project exclusions for global addons.
pub mod exclude;

/// `gdio addons sync` — synchronize global and every-project addons.
pub mod sync;

/// `gdio addons repository` — manage third-party addon repositories.
pub mod repository;

/// `gdio addons search` — search the asset store by name/description.
pub mod search;
