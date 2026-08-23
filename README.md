<p align="center">
  <img src="gdio.svg" alt="gdio logo" width="128" height="128" />
</p>

<h1 align="center">gdio</h1>

<p align="center">
  CLI tool for managing Godot Engine projects, editor versions, addons and export templates.
</p>

## Build and Install
Clone this repository, `cd` into it's directory and run:
```bash
python scripts\local_install.py
```

## Latest Release Installation

### PowerShell (Windows)

```powershell
powershell -c "irm https://gdio.seremtitus.co.ke/install.ps1 | iex"
```

### Curl (Linux / macOS / Windows)

```bash
curl -fsSL https://gdio.seremtitus.co.ke/install.sh | bash
```

### Quick Start

```bash
cd my_godot_project
gdio                                          # Open a project in the current directory


gdio add 4.7                                  # Add an editor


gdio list                                     # List editors
```

## Commands

### `gdio` (no args)

Open the current directory's Godot project. Detects the required version from `project.godot` and opens it with the matching editor.

- If previously opened, uses the same editor
- If not, finds a matching editor or prompts to download
- Auto-detects version: `gdio add 4.7.2`

### `gdio add <version or path>`

Download or register a Godot editor.

```bash
gdio add 4.7                                  # download stable (or latest available)
gdio add 4.7-dev3                             # download a specific build
gdio add 4.7 --csharp                         # download the C# (mono) variant
gdio add /path/to/Godot.exe                   # register a local editor
```

### `gdio list`

List all registered editors.

### `gdio remove <version or name>`

Remove an editor. Downloaded editors are deleted from disk; local ones are just unregistered.

```bash
gdio remove                                   # interactive: select from all editors
gdio remove 4.7                               # remove by version (interactive if multiple match)
gdio remove "Godot v4.7"                      # remove by name
```

### `gdio bind <version or name>`

Bind a specific editor to the current project.

```bash
gdio bind                                     # interactive editor selection
gdio bind 4.7                                 # bind by version (downloads if not found)
gdio bind "Godot v4.7"                        # bind by name
```

### `gdio game`

Open the current project in game mode.

### `gdio recent` / `gdio -r`

Open the last project opened by gdio.

### `gdio projects`

List all known projects with interactive selection to open in edit or game mode.

### `gdio new <name>`

Create a new Godot project in a subdirectory.

```bash
gdio new MyGame                               # create project in ./MyGame/
```

### `gdio build`

Export the current project. Reads presets from `export_presets.cfg`.

```bash
gdio build                                    # all platforms (default)
gdio build --windows                          # Windows only
gdio build --linux --web                      # Linux + Web
gdio build --macos --ios                      # macOS + iOS
gdio build --android                          # Android
```

Platforms: `--windows`, `--linux`, `--web`, `--macos`, `--ios`, `--android`

Export debug flag : `--debug`/`-d`

Downloads export templates on-demand if not found. Templates are stored in Godot's native directory.

### `gdio up`

Upload the current project to itch.io using [butler](https://itch.io/docs/butler/). Automatically builds the project first, then uploads each platform to its configured channel.

```bash
gdio up --setup                               # interactive setup (butler path, game ID)
gdio up                                       # build + upload all configured platforms
gdio up --windows                             # build + upload Windows only
gdio up --linux --web                         # build + upload Linux + Web
gdio up --name                                # interactive channel name customization per platform
```

Platforms: `--windows`, `--linux`, `--web`, `--macos`, `--ios`, `--android`

Export debug flag : `--debug`/`-d`

Custom channel name flag : `--name`/`-n` (default channel: `{platform}-v{version}`)

#### Setup

Run `gdio up --setup` from a Godot project directory to configure itch.io upload settings:

1. **Butler path**: enter path [butler](https://itch.io/docs/butler/).
2. **Game identifier**: your itch.io project in `user/game` format (e.g. `myuser/mygame`)

### `gdio templates`

Manage export templates. Reads from and installs to Godot's native template directory.

```bash
gdio templates list                           # show installed templates + variations
gdio templates add 4.7                        # download all platforms
gdio templates add 4.7 --windows --web        # download specific platforms
gdio templates remove 4.7                     # remove all templates
gdio templates remove 4.7 --web               # remove only web templates
```

Templates are stored where Godot expects them:
- Windows: `%APPDATA%/Godot/export_templates/`
- Linux: `~/.local/share/godot/export_templates/`
- macOS: `~/Library/Application Support/Godot/export_templates/`

This means templates installed by the Godot editor are also visible to gdio.

### `gdio uninstall`

Remove gdio and all its files from your system.

```bash
gdio uninstall                                # remove everything
gdio uninstall --keep                         # keep config and editor binaries
```

### `gdio cost`

Show disk space used by gdio components.

### `gdio test`

Run [GUT](https://github.com/bitwes/Gut) tests for the current project.

```bash
gdio test --init                              # install GUT addon
gdio test                                     # run all tests (headless)
gdio test test                                # run tests in res://test/
gdio test test/unit                           # run tests in res://test/unit/
gdio test --visual                            # open GUT GUI window
gdio test test --visual                       # visual mode with specific folder
```

`--init`/`-i` installs GUT. Without arguments, GUT reads `.gutconfig.json` if present, otherwise scans the project for `test_*.gd` files. 

`--visual`/`-v` opens the GUT test runner window instead of running headless.

### `gdio addons`

Manage addons from the [Godot Asset Store](https://store.godotengine.org/) and third-party repositories.

```bash
gdio addons add seremtitus/ruzta              # install addon to current project
gdio addons add seremtitus/ruzta --select     # interactively choose which version to install
gdio addons list                              # list addons in current project

gdio addons remove                            # interactive: select addon to remove
gdio addons remove ruzta                      # remove addon by folder name
gdio addons remove seremtitus/ruzta           # remove by identifier (also cleans .gdio tracking)

gdio addons repository                        # list registered repositories
gdio addons repository https://example.com    # toggle add/remove a repository
```

**Identifier format**: `publisher/asset` (e.g. `bitwes/gut`, `seremtitus/ruzta`)

#### Linked addons (`--linked` / `-l`) [ADVANCED]

```bash
gdio addons add seremtitus/ruzta --linked     # Stores addon globally (symlinked into project)
gdio addons list --linked                     # list linked addons

gdio addons sync                              # sync linked and global addons
```

Stores the addon in gdio's global store (`<gdio config dir>/addons/`) and creates a symlink in the project. The symlink is added to `.gitignore` and tracked in a `.gdio` project file.

`gdio addons sync` ensure `.gdio` addons are added. Runs automatically when you open a project with `gdio` (if a `.gdio` file exists).

Switching modes: Re-running `gdio addons add` with or without `--linked` switches the installation mode. The previous install (symlink or local copy) is automatically removed.

RISK: Not tracked by git, developer may delete addon from Asset Store.

#### Global addons (`gdio addons globals`) [ADVANCED]

```bash
gdio addons globals seremtitus/ruzta          # add addon as global (synced to all projects, copied)
gdio addons globals seremtitus/ruzta --linked # add as global, symlinked into each project
gdio addons globals seremtitus/ruzta --select # add as global, interactively pick version

gdio addons globals                           # list global addons (synced to all projects)
gdio addons globals --remove                  # interactive: stop syncing a global addon

gdio addons exclude seremtitus/ruzta          # exclude this project from a global addon
gdio addons exclude --revert                  # interactive: revert an exclusion
gdio addons exclude seremtitus/ruzta --revert # revert exclusion for a specific addon

gdio addons sync                              # sync linked and global addons
```

Addons marked as global are synced to all projects unless excluded. Use `gdio addons globals <identifier>` to add an addon as global. Use `--remove` to interactively stop syncing an addon (does not remove it from existing projects).

`gdio addons exclude`: Excludes the current project from receiving a global addon during sync. Use `--revert` to undo the exclusion and re-enable syncing.

- **`--linked` / `-l`**: Store the addon in the global cache and symlink it into each project during sync (instead of copying).
- **`--select` / `-s`**: Interactively pick a specific version to pin. Without this flag, each project gets the highest compatible version for its bound Godot version.

## Helper Flags

```bash
-h, --help                                    # Print help (works at all levels)
-V, --version                                 # Print version
```

## Platforms Flags

| Platform | Flag |
|----------|-----------|
| Windows | `--windows` |
| Linux | `--linux` |
| macOS | `--macos` |
| Web | `--web` |
| iOS | `--ios` |
| Android | `--android` |

## Config

Editor and project data stored in `%APPDATA%/gdio/` (Windows) or `~/.config/gdio/` (Linux/macOS):

```bash
gdio/
├── config.json                               # editor registry + project registry + addon repos
├── editors/                                  # downloaded editor binaries
├── downloads/                                # temporary download directory
└── addons/                                   # global addon store (for --global addons)
```

## Alternatives

- [godots](https://github.com/MakovWait/godots) - GUI based
- [GodotHub](https://github.com/RykoTheDev/GodotHub) - GUI based
- [godotenv](https://github.com/chickensoft-games/godotenv) - CLI based