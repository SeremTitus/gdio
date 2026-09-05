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
python scripts/local_install.py
```

## Latest Release Installation

### Windows

```powershell
curl.exe -fsSL https://gdio.seremtitus.co.ke/install.ps1 | powershell -Command -
```

### Linux / macOS

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

### `gdio clone <url>`

Clone a Godot project from a git repository and open it.

```bash
gdio clone https://github.com/user/repo       # clone and open
gdio clone https://github.com/user/repo mydir # clone into ./mydir/
gdio clone https://github.com/user/repo -d 1  # shallow clone (--depth 1)
```

### `gdio build`

Export the current project. Reads presets from `export_presets.cfg`.

```bash
gdio build                                    # all platforms (default)
gdio build --windows                          # Windows only
gdio build --linux --web                      # Linux + Web
gdio build --macos --ios                      # macOS + iOS
gdio build --android                          # Android
gdio build --visionos                         # visionOS
```

Platforms: `--windows`, `--linux`, `--web`, `--macos`, `--ios`, `--android`, `--visionos`

Export debug flag : `--debug`/`-d`

Downloads export templates on-demand if not found. Templates are stored in Godot's native directory.

### `gdio up`

Upload the current project to itch.io using [butler](https://itch.io/docs/butler/). Automatically builds the project first, then uploads each platform to its configured channel.

```bash
gdio up setup myuser/mygame                   # configure game identifier for itch.io upload
gdio up                                       # build + upload all configured platforms
gdio up --windows                             # build + upload Windows only
gdio up --linux --web                         # build + upload Linux + Web
gdio up --name                                # interactive channel name customization per platform
```
First run `gdio up setup <Game identifier>`, game identifier is your itch.io project in `user/game` format (e.g. `myuser/mygame`)

Platforms: `--windows`, `--linux`, `--web`, `--macos`, `--ios`, `--android`, `--visionos`

Export debug flag : `--debug`/`-d`

Custom channel name flag : `--name`/`-n` (default channel: `{platform}-v{version}`)

### `gdio templates`

Manage export templates. Reads from and installs to Godot's native template directory.

```bash
gdio templates list                           # show installed templates + variations
gdio templates add 4.7                        # download all platforms
gdio templates add 4.7 --windows --web        # download specific platforms
gdio templates add 4.7 --csharp               # download C# (mono) templates
gdio templates add 4.7 --debug                # download debug templates only
gdio templates add 4.7 --release              # download release templates only
gdio templates remove 4.7                     # remove all templates
gdio templates remove 4.7 --web               # remove only web templates
```

`--debug`/`-d` and `--release`/`-r` are mutually exclusive flags for filtering template variants.

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

### `gdio news`

Show latest Godot news from the official blog.

### `gdio tags`

Manages Project tags.

```bash
gdio tags                                      # list current tags
gdio tags rpg                                  # toggle add/remove a tag
```

### `gdio args <args...>`

Run the editor with custom arguments `{editor} --path {project} <your args ...>`.

```bash
gdio args -e --verbose                         # open project with verbose logging
gdio args -e --quit --quit-after 10            # open, run for 10 frames, then quit
gdio args --import --headless                  # force reimport
```

### `gdio script <file>`

Run a script headless using the project's bound editor. If not inside a Godot project, interactively selects an editor.

```bash
gdio script res://scripts/my_script.gd         # run a GDScript file
gdio script tools/export_tool.gd               # run a tool script
```

### `gdio addons`

Manage addons from the [Godot Asset Store](https://store.godotengine.org/) and third-party repositories.

```bash
gdio addons add seremtitus/ruzta              # install addon to current project
gdio addons add seremtitus/ruzta --select     # interactively choose which version to install
gdio addons list                              # list addons in current project

gdio addons remove                            # interactive: select addon to remove
gdio addons remove ruzta                      # remove addon by folder name
gdio addons remove seremtitus/ruzta           # remove by identifier (also cleans .gdio tracking)

gdio addons search "2d platformer"             # search the asset store by name/description

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

NOTE: Edit/Change in one it will appear in every project linking the addon.

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

## `gdio ci`

Adds custom GitHub Actions workflow to the current project for exporting Godot projects.

- `export_pipeline_from_gdio.yml` - Standard export pipeline using gdio
- `export_pipeline_secure_from_gdio.yml` - Export pipeline with [Godot Secure](https://github.com/KnifeXRage/Godot-Secure) AES-256 script encryption

### Secrets and Variables

On Github navigating to: Settings -> Secrets and variables -> Actions -> New repository secret

| Secret / Variable | Value | Note | Why |
|-------------------|-------|-------------|---------|
| `SCRIPT_AES256_ENCRYPTION_KEY` | 64 hex char | `openssl rand -hex 32` or `python -c "import secrets; print(secrets.token_hex(32))"` | AES-256 encryption key for Godot-Secure script protection (secure pipeline only- not required but set it for consistency) |
| `ANDROID_KEYSTORE` | base64 encoded keystore file | Linux/Macos:`base64 -w 0 mygame.keystore` or Powershell: `[Convert]::ToBase64String([IO.File]::ReadAllBytes("mygame.keystore"))` | Android signing keystore for release and debug exports. How to [generate keystore](https://docs.godotengine.org/en/stable/tutorials/export/exporting_for_android.html#exporting-for-google-play-store) |
| `ANDROID_KEYSTORE_PASSWORD` | keystore password string |  | Required to open the keystore during Android export |
| `ANDROID_KEYSTORE_USER` | keystore user/alias string | e.g. `mygame` | Identifies the key entry used for signing the APK |

## `gdio builder`

Build Godot Engine from source. Run from within a Godot source directory.

```bash
gdio builder clone                           # Shallow clone Godot master
gdio builder clone --full                    # Full clone with history
gdio builder clone 4.7.2-stable              # Clone specific tag
gdio builder                                 # Build editor (auto-detects platform)
gdio builder --csharp                        # Build editor with C# support
gdio builder ccache=yes                      # Pass extra args to scons
gdio builder install                         # Install build dependencies
gdio builder secure --key <64-hex-chars>     # Apply Godot Secure encryption
gdio builder template                        # Build all templates
gdio builder template --windows              # Build Windows templates only
gdio builder template --linux -j8            # Build Linux templates with parallel jobs
gdio builder template --visionos             # Build visionOS templates only
gdio builder template --debug                # Build debug templates only
gdio builder template --release              # Build release templates only
gdio builder template --linux use_llvm=yes   # Pass extra scons flags
```

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
| visionOS | `--visionos` |

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