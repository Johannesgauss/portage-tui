# portage-tui

> A fast, interactive Terminal User Interface (TUI) for Gentoo Portage package and USE flag management, built in pure Rust.

`portage-tui` provides Gentoo Linux users with a visual, keyboard-driven interface to explore their `@world` dependency tree, inspect and toggle USE flags with plain-English descriptions, unmask packages, configure flags *before* installation, and cleanly manage package lifecycles without leaving the terminal.

---

## Table of Contents
- [Architecture & Design Philosophy](#architecture--design-philosophy)
- [Features](#features)
  - [1. @world Dependency Tree](#1-world-dependency-tree)
  - [2. USE Flag Management with Descriptions & Origins](#2-use-flag-management-with-descriptions--origins)
  - [3. Pre-Flight Package Installation & USE Flag Configuration](#3-pre-flight-package-installation--use-flag-configuration)
  - [4. Package Mask & Keyword Management](#4-package-mask--keyword-management)
  - [5. Safe Package Uninstallation](#5-safe-package-uninstallation)
  - [6. Full System Sync & Update](#6-full-system-sync--update)
- [Keybindings Reference](#keybindings-reference)
- [Permissions & Setup](#permissions--setup)
- [Installation Methods](#installation-methods)
  - [Method 1: Direct Release Build & Install Script](#method-1-direct-release-build--install-script)
  - [Method 2: Local Portage Overlay & Ebuild (Recommended for Gentoo)](#method-2-local-portage-overlay--ebuild-recommended-for-gentoo)
- [Uninstalling](#uninstalling)
- [Missing Features & Planned Roadmap](#missing-features--planned-roadmap)
  - [1. Single-Package Re-emerge with Changed USE Flags (`[r]`)](#1-single-package-re-emerge-with-changed-use-flags-r)
  - [2. Staged Configuration Diff Review Modal](#2-staged-configuration-diff-review-modal)
  - [3. Ebuild Source Viewer & Build Log Pager](#3-ebuild-source-viewer--build-log-pager)
  - [4. View Mode Filters](#4-view-mode-filters)
  - [5. Configuration Dispatcher Integration (`etc-update` / `dispatch-conf`)](#5-configuration-dispatcher-integration-etc-update--dispatch-conf)
  - [6. Multi-Slot & Target Version Switcher](#6-multi-slot--target-version-switcher)
- [Developer Guide](#developer-guide)

---

## Architecture & Design Philosophy

`portage-tui` is designed from the ground up for Gentoo systems:
1. **Zero Privileges for Exploration & Staging**: Inspecting packages, reading ebuild repositories, searching, and configuring USE flags runs as your regular user.
2. **Group-Based `/etc/portage` Access**: By adding your user to the standard Gentoo `portage` group and setting group-write permissions on `/etc/portage`, configuration changes are written without running the TUI as `root`.
3. **Isolated Config Writes**: All saved changes are written to dedicated files named `zz-portage-manager` within `/etc/portage/package.use/`, `package.mask/`, `package.unmask/`, and `package.accept_keywords/`. This ensures alphabetical override priority while preserving your hand-written configurations intact.
4. **Clean Terminal Handover**: When invoking `emerge` or `emaint`, raw terminal mode and alternate screen buffers are suspended, giving `sudo` and Portage full control over stdout/stdin for interactive prompts, password entries, and compile status displays.

---

## Features

### 1. `@world` Dependency Tree
* Parses `/var/lib/portage/world` as root items and resolves direct runtime dependencies via `/var/db/pkg/`.
* Expand and collapse tree branches (`Enter`, `h` / `l`, or arrow keys).
* **Cycle Prevention**: Automatically detects circular package dependencies and marks them to prevent infinite recursive unfolding.
* **Live Search (`/`)**: Real-time filtering across package atoms and package descriptions.

### 2. USE Flag Management with Descriptions & Origins
* Displays all declared `IUSE` flags for the selected package.
* **Plain-English Explanations**: Resolves descriptions across three layers:
  1. Package-local `metadata.xml` `<use><flag name="...">` tags.
  2. Repository `profiles/desc/*.desc` (e.g. `python_targets.desc`, `cpu_flags_x86.desc`).
  3. Global `profiles/use.desc`.
* **Origin Badges**: Clearly shows where a flag's state originates:
  * `package.use`: Explicitly set in `/etc/portage/package.use/`.
  * `make.conf`: Set globally in `/etc/portage/make.conf`.
  * `default`: Ebuild default (e.g., `+flag` in `IUSE`) or profile default.
  * `staged`: Modified in the current session but not yet saved to disk.
* **Staged Changes**: Press `Space` to toggle flags. Changes remain in a staging buffer until you press `[s]` to save.

### 3. Pre-Flight Package Installation & USE Flag Configuration
* Press `[a]` to install a new package.
* **Pre-Flight Query**: Resolves package names (e.g. `fceux` -> `games-emulation/fceux`) against available repository ebuilds *before* invoking emerge. If the package does not exist or is ambiguous, you are notified immediately without running emerge.
* **Mask Interception**: If the target package is masked in profiles or requires `~testing` keywords, `portage-tui` warns you with exact reasons and asks whether you want to unmask it. If you choose **No**, it cancels immediately and returns to the TUI without calling emerge.
* **Pre-Install USE Flag Selection Modal**:
  * Before emerge is invoked, an interactive modal displays all flags declared in the package's ebuild with full descriptions.
  * You can toggle flags (`Space`), navigate (`j`/`k`, `PageUp`/`PageDown`), and customize options.
  * Pressing `Enter` automatically writes your customized flags to `/etc/portage/package.use/zz-portage-manager` and launches `sudo emerge --ask --verbose <atom>`.
  * Pressing `Esc` aborts the installation cleanly.

### 4. Package Mask & Keyword Management
* `[m]`: Toggles package mask in `/etc/portage/package.mask/zz-portage-manager`.
* `[t]`: Toggles `~testing` arch keyword in `/etc/portage/package.accept_keywords/zz-portage-manager`.
* If a package is masked by Gentoo profile masks, `portage-tui` provides unmasking via `/etc/portage/package.unmask/zz-portage-manager`.

### 5. Safe Package Uninstallation
* Press `[d]` (or `[x]`) on any package in the tree.
* Opens a confirmation modal outlining the exact commands that will execute:
  1. `sudo emerge --deselect <atom>` (removes from `@world` if present).
  2. `sudo emerge --ask --verbose --depclean <atom>` (safely uninstalls and cleans dependencies).
* If confirmed with `[y / Enter]`, the cooked terminal runs the depclean process, and upon returning, `portage-tui` refreshes the installed packages and `@world` tree automatically.

### 6. Full System Sync & Update
* Press `[u]` to trigger a full system upgrade.
* Automatically executes:
  1. `sudo emaint --auto sync` (synchronizes all configured repositories).
  2. `sudo emerge --ask --verbose --update --deep --newuse @world`.
* Reloads the package database upon return.

---

## Keybindings Reference

| Shortcut | Context | Action |
|:---|:---|:---|
| `Tab` / `BackTab` | Main View | Switch active pane (Tree ↔ USE Flags) |
| `↑` / `k`, `↓` / `j` | All Panes | Navigate up / down |
| `PageUp` / `PageDown` | All Panes | Page scroll |
| `Home` / `g`, `End` / `G` | All Panes | Jump to top / bottom |
| `Enter` | Tree Pane | Expand / collapse selected package dependencies |
| `l` / `h` | Tree Pane | Expand / collapse or jump to parent package |
| `Space` | Use Flags Pane | Toggle selected USE flag (staged) |
| `Space` | Tree Pane | Jump to USE flags pane |
| `[m]` | Main View | Toggle package mask in `package.mask` |
| `[t]` | Main View | Toggle `~testing` keyword in `package.accept_keywords` |
| `[s]` | Main View | Save all staged changes to `/etc/portage/` |
| `[a]` | Main View | Add / install package (resolves, checks mask, configures USE flags) |
| `[d]` / `[x]` | Main View | Uninstall package (`--deselect` + `--depclean`) |
| `[u]` | Main View | Sync repos (`emaint`) and update `@world` |
| `/` | Main View | Open live search filter |
| `Esc` | Search / Modal | Cancel modal or clear search filter |
| `[q]` | Main View | Quit `portage-tui` |

---

## Permissions & Setup

To manage `/etc/portage` without running the entire TUI as `root`, add your user to the `portage` group:

```bash
# Run the automated setup script
sudo ./tools/setup-portage-permissions.sh [username]
```

### What this script does:
1. Adds your user to the `portage` group: `gpasswd -a <user> portage`
2. Sets group ownership of `/etc/portage` to `portage`: `chgrp -R portage /etc/portage`
3. Grants group write permissions: `chmod -R g+w /etc/portage`
4. Sets the setgid bit on directories so new configuration files inherit group `portage`: `find /etc/portage -type d -exec chmod g+s {} +`

> **Note**: After running the permissions script, activate the new group in your current shell by running:
> ```bash
> newgrp portage
> ```
> *(or log out and log back in).*

---

## Installation Methods

### Method 1: Direct Release Build & Install Script

Compile the release binary and install to `/usr/local/bin/portage-tui`:

```bash
sudo ./tools/install-portage-tui.sh [/usr/local]
```

To install directly into `/usr/bin`:
```bash
sudo ./tools/install-portage-tui.sh /usr
```

---

### Method 2: Local Portage Overlay & Ebuild (Recommended for Gentoo)

Create a local Portage overlay, register it in `/etc/portage/repos.conf`, and install `portage-tui` via standard `emerge`:

```bash
# 1. Create and register local repository (defaults to /var/db/repos/localrepo)
sudo ./tools/create-local-repo.sh

# 2. Unmask ~testing keyword if needed, and emerge
sudo emerge --ask app-portage/portage-tui
```

The bundled ebuild is located at `ebuild/app-portage/portage-tui/portage-tui-0.1.0.ebuild` and adheres to modern Gentoo `EAPI=8` with `cargo.eclass`.

---

## Uninstalling

To cleanly remove the installed binary and optionally purge the managed `zz-portage-manager` configuration files:

```bash
sudo ./tools/uninstall-portage-tui.sh [/usr/local]
```

---

## Missing Features & Planned Roadmap

While `portage-tui` covers core daily package operations, the following features are actively planned to provide a complete Portage management experience:

### 1. Single-Package Re-emerge with Changed USE Flags (`[r]`)
* **Current Behavior**: When changing USE flags on an installed package and pressing `[s]`, flags are saved to `package.use`. The user must either run a full `@world` update or manually drop to shell and run emerge.
* **Planned Feature**: A dedicated keybinding `[r]` (or `[e]`) that immediately triggers:
  ```bash
  sudo emerge --ask --verbose --newuse <selected-package>
  ```
  *(or `--changed-use`)* to recompile only the selected package with its newly configured flags.

### 2. Staged Configuration Diff Review Modal
* **Current Behavior**: Pressing `[s]` writes all staged changes directly to `/etc/portage/*`.
* **Planned Feature**: A visual review popup before writing to disk, showing a unified diff preview of exactly which files and lines will be created or modified (e.g. `+ games-emulation/fceux opengl -lua`).

### 3. Ebuild Source Viewer & Build Log Pager
* **Current Behavior**: Homepage, slot, repository name, and description are displayed in the header.
* **Planned Feature**:
  * Press `[v]` to open the full raw `.ebuild` file from `/var/db/repos/<repo>/...` in a read-only scrollable pager with syntax highlighting.
  * Press `[l]` to view recent emerge logs from `/var/log/emerge.log` or the package's build log under `/var/tmp/portage/.../temp/build.log`.

### 4. View Mode Filters
* **Current Behavior**: The tree displays all packages in `@world` (and dependencies when expanded).
* **Planned Feature**: A cycle keybinding (e.g. `[F]`) to switch view filters:
  * **All @world** (default).
  * **Customized Only**: Packages that have explicit entries in `package.use`, `package.mask`, or `package.accept_keywords`.
  * **Masked / Testing Only**: Packages running under `~arch` or masked profiles.
  * **Outdated / Upgradable Only**: Packages where a newer version exists in the repository than what is recorded in `/var/db/pkg`.

### 5. Configuration Dispatcher Integration (`etc-update` / `dispatch-conf`)
* **Current Behavior**: When emerge completes, protected config file warnings are shown in emerge's terminal output.
* **Planned Feature**: A shortcut `[c]` to launch `etc-update` or `dispatch-conf` in the interactive runner session to resolve config updates in `/etc` without quitting the TUI.

### 6. Multi-Slot & Target Version Switcher
* **Current Behavior**: Shows the installed version from `/var/db/pkg` and the latest ebuild version in the repo.
* **Planned Feature**: A modal showing all available slots and versions in configured overlays (e.g. `gcc:13`, `gcc:14`, or `llvm:18`, `llvm:19`), allowing users to target a specific version slot when installing or rebuilding.

---

## Developer Guide

### Code Structure
```
src/
├── app.rs               # State machine, flattened tree navigation, staging, modals
├── main.rs              # Terminal event loop, keybinding dispatch, CLI flags
├── domain/
│   └── package.rs       # Core types: Package, UseFlag, FlagOrigin, TreeNode, MaskReport
├── backend/
│   ├── mod.rs           # Portage facade
│   ├── atom.rs          # Package atom string splitting and validation (split_pv, strip_to_cp)
│   ├── config.rs        # Parser & writer for /etc/portage/{package.use, package.mask, etc.}
│   ├── metadata.rs      # Parser for use.desc, metadata.xml, profiles/desc/*.desc
│   ├── repo_index.rs    # High-speed repository indexer and ebuild IUSE parser
│   ├── runner.rs        # Process execution runner (emaint, emerge, depclean)
│   └── vdb.rs           # VDB reader (/var/db/pkg and /var/lib/portage/world)
└── ui/
    └── mod.rs           # Ratatui layout and widget rendering
```

### Running Tests
Unit tests use an isolated temporary filesystem and mock trees:
```bash
cargo test
```

### Mock Development Mode
When running outside of a native Gentoo host (e.g. development in Debian, Fedora, or macOS), `portage-tui` automatically detects the absence of `/var/db/pkg` and loads `mock_portage/`, providing a complete sandbox containing simulated `@world`, VDB records, ebuild repositories, and `/etc/portage` configs.

You can also explicitly pass any root path:
```bash
cargo run -- /path/to/custom/root
```

### Coding Conventions
* **Brace Style**: Linux kernel / Linus coding style:
  * Function and method signatures have their opening brace `{` on the next line.
  * Control flow statements (`if`, `for`, `match`, `while`, `else`) have their opening brace on the same line.
* **Explicit Returns**: Explicit `return` statements are preferred for non-trivial exit points.
