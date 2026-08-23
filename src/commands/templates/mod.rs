/// Godot mirror API and concurrent download helpers.
pub mod api;

/// ZIP archive parsing (Central Directory, ZIP64).
pub mod storage;

/// `gdio templates add` — download and install export templates.
pub mod add;

/// `gdio templates list` — list installed export templates.
pub mod list;

/// `gdio templates remove` — remove export templates.
pub mod remove;
