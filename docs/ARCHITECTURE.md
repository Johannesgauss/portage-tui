# Architecture Overview

`portage-tui` is structured into five distinct, decoupled layers. This separation ensures that filesystem parsing, business logic, user interface rendering, and process execution never leak into one another.

```
┌────────────────────────────────────────────────────────┐
│                        Terminal                        │
└───────────────────────────┬────────────────────────────┘
                            │ (crossterm events)
┌───────────────────────────▼────────────────────────────┐
│                       main.rs                          │
│   Event Loop • Terminal Raw Mode • Emerge Handover     │
└──────────────┬──────────────────────────▲──────────────┘
               │                          │
┌──────────────▼─────────────┐ ┌──────────┴──────────────┐
│          ui/mod.rs         │ │        runner.rs        │
│   Ratatui Layouts & Boxes  │ │   Interactive Emerge    │
│   No Business Logic        │ │   Cooked Terminal       │
└──────────────▲─────────────┘ └─────────────────────────┘
               │ reads state
┌──────────────┴─────────────────────────────────────────┐
│                       app.rs                           │
│   State Machine • Modal Routing • Tree Navigation      │
│   Staging Buffer • Filter Logic                        │
└──────────────┬─────────────────────────────────────────┘
               │ calls operations
┌──────────────▼─────────────────────────────────────────┐
│                      backend/                          │
│   Portage Facade (mod.rs)                              │
│   ├── vdb.rs         → Reads /var/db/pkg & @world      │
│   ├── repo_index.rs  → Indexes ebuilds & parses IUSE   │
│   ├── metadata.rs    → use.desc & metadata.xml         │
│   ├── config.rs      → /etc/portage reader & writer    │
│   └── atom.rs        → Version & atom string parsing   │
└────────────────────────────────────────────────────────┘
```

---

## 1. Domain Layer (`src/domain/`)

Defines the core data structures used throughout the application:
* **`Package`**: Represents an installed or repo package (category, name, version, slot, repository, description, homepage, dependencies, use flags, mask state).
* **`UseFlag`**: Represents a single USE flag with its name, description, enabled status, ebuild default (`+flag`), and origin.
* **`FlagOrigin`**: Tracks whether a flag's state is derived from `package.use`, `make.conf`, or `ProfileDefault`.
* **`TreeNode`**: Recursive tree structure representing `@world` packages and their child dependencies.
* **`StagedAction`**: Action queued in memory awaiting a save command (`ToggleUse`, `ToggleMask`, `ToggleTesting`, `UnmaskPackage`).

---

## 2. Backend Layer (`src/backend/`)

The backend layer is completely independent of Ratatui or terminal libraries. It can be compiled into a headless CLI or library without UI dependencies.

* **`Portage` (`mod.rs`)**: A unified facade presenting a clean API to `app.rs`.
* **`vdb.rs` (`VdbReader`)**: Parses installed packages directly from `/var/db/pkg/<cat>/<pkg-ver>/` files (`IUSE`, `USE`, `RDEPEND`, `DEPEND`, `REPOSITORY`, `SLOT`, `HOMEPAGE`, `DESCRIPTION`).
* **`repo_index.rs` (`RepoIndex`)**:
  * Indexes category and package directories across all configured repositories in `/var/db/repos/` in under 20ms.
  * Parses `IUSE` and `DESCRIPTION` from raw `.ebuild` files to support pre-install USE flag selection.
* **`metadata.rs` (`MetadataDb`)**:
  * Reads `profiles/use.desc` for global flag descriptions.
  * Reads `profiles/desc/*.desc` for `USE_EXPAND` variables (e.g. `python_targets`, `cpu_flags_x86`).
  * Parses package-specific `metadata.xml` using `roxmltree` with DTD support.
  * Reads profile masks from `profiles/package.mask`.
* **`config.rs` (`UserConfig`)**:
  * Reads `/etc/portage/package.use`, `package.mask`, `package.unmask`, and `package.accept_keywords`.
  * Writes staged changes exclusively into `zz-portage-manager` files within those directories.
* **`runner.rs`**: Manages interactive process execution for `emaint` and `emerge`.

---

## 3. Terminal Handover & Process Runner

One of the most critical components of `portage-tui` is `run_emerge_interactively`:
1. When an emerge action is triggered (e.g., install, uninstall, update):
   * Disables terminal raw mode via `crossterm::terminal::disable_raw_mode()`.
   * Leaves alternate screen buffer via `LeaveAlternateScreen`.
2. Spawns `sudo emerge` or `sudo emaint` with standard I/O inheritance.
   * Prompts for passwords and yes/no confirmations work naturally.
   * Color output and build status bars render properly.
3. Upon process termination:
   * Pauses with `Press [Enter] to return...`.
   * Re-enters alternate screen buffer and re-enables raw mode for Ratatui.
   * Reloads the package database so the TUI immediately reflects any changes.

---

## 4. Application State (`src/app.rs`)

`AppState` acts as the single source of truth:
* **Tree Navigation**: Flattens recursive `TreeNode` structures into a linear `FlatTreeItem` vector for fast rendering and scrolling.
* **Cycle Prevention**: Ancestor stacks are maintained during tree traversal to catch and break circular dependencies.
* **Modal States (`InputMode`)**:
  * `Normal`: Standard navigation and hotkeys.
  * `Search`: Real-time filtering.
  * `InstallInput`: Typing package name to install.
  * `MaskPrompt`: Warning popup asking whether to unmask a package.
  * `InstallUseFlags`: Interactive USE flag picker prior to emerge.
  * `UninstallPrompt`: Confirmation popup for depcleaning.
