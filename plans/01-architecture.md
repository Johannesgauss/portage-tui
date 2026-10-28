# Portage Manager TUI — System Architecture (v0.2 - Focused on USE & World Tree)

## 1. Problem Statement & Primary Goal

Managing Gentoo Portage manually is notoriously high-friction:
1. **Understanding USE flags:** Users must manually hunt down USE flag descriptions spread across `use.desc` and `metadata.xml`.
2. **Configuration file clutter:** Manually editing `/etc/portage/package.use`, `package.accept_keywords`, and `package.mask` leads to fragmented, messy, and error-prone configurations.
3. **Package visibility:** `/var/db/pkg` contains over a thousand packages, making it hard to see what the user *actually* chose to install vs what was pulled in purely as a transitive dependency.

**Primary Goal:** 
A fast, keyboard-driven TUI that displays the **`@world` dependency tree**, explains **every USE flag in plain English**, and lets the user toggle USE flags and keywords interactively while safely managing `/etc/portage/` changes.

---

## 2. Visual Layout Architecture

```text
┌─ Header: System Profile & Status ──────────────────────────────────────────┐
├──────────────────────────────────────┬─────────────────────────────────────┤
│ Left Pane (40% width)                │ Right Pane (60% width)              │
│                                      │                                     │
│  [World Dependency Tree]             │  [Package Header & Keywords/Mask]   │
│  ▼ app-editors/neovim                │  Slot: 0  |  Keywords: ~amd64       │
│    ├── dev-lua/lua-lpeg              │                                     │
│    ├── dev-libs/libuv                │  [Interactive USE Flag List]        │
│    └── dev-libs/msgpack-c            │  [✓] lua_single_target_luajit       │
│  ▶ net-misc/curl                     │      Description from metadata.xml  │
│  ▶ www-client/firefox                │  [ ] nvimpager                      │
│                                      │      Description from use.desc      │
├──────────────────────────────────────┴─────────────────────────────────────┤
│ Staging Bar: Pending changes ready to be written to /etc/portage/          │
├────────────────────────────────────────────────────────────────────────────┤
│ Footer: Hotkey help ([Space] Toggle, [s] Save, [p] Pretend Emerge, [/] Search)
└────────────────────────────────────────────────────────────────────────────┘
```

---

## 3. Top-Down Technical Stack

```
┌────────────────────────────────────────────────────────────────────────┐
│                        1. PRESENTATION LAYER (TUI)                     │
│                  Ratatui Widgets & Crossterm Terminal                  │
│                                                                        │
│   • WorldTreeView: Tree widget rendering @world & resolved RDEPENDs    │
│   • UseFlagListView: Interactive list with origin badges & descriptions │
│   • KeywordMaskWidget: Toggle stable / ~testing / masked status        │
│   • StagingDiffBar: Shows pending edits before writing to disk         │
└───────────────────────────────────▲────────────────────────────────────┘
                                    │ Input Events & View Models
┌───────────────────────────────────┴────────────────────────────────────┐
│                    2. APP CONTROLLER & STATE MACHINE                   │
│                                                                        │
│   • AppState:                                                          │
│       - world_tree: Tree<PackageNode>                                  │
│       - selected_package: Option<Atom>                                 │
│       - staging_changes: StagingBuffer (unwritten /etc/portage diffs)  │
│   • Event Loop: 60Hz tick + crossterm KeyEvent + Background Worker MPSC│
└───────────────────────────────────▲────────────────────────────────────┘
                                    │ Calls methods
┌───────────────────────────────────┴────────────────────────────────────┐
│               3. DOMAIN ABSTRACTIONS (RUST TRAITS)                     │
│                                                                        │
│   • trait WorldTreeProvider: Load @world and resolve RDEPEND relations │
│   • trait UseFlagProvider: Get available flags, descriptions & origins │
│   • trait PortageConfigWriter: Safely commit staging changes to disk   │
└───────────────────────────────────▲────────────────────────────────────┘
                                    │ Implemented by
┌───────────────────────────────────┴────────────────────────────────────┐
│                       4. BACKEND IMPLEMENTATIONS                       │
│                                                                        │
│   ┌────────────────────┐ ┌────────────────────┐ ┌──────────────────┐   │
│   │ Native World / VDB │ │  Portage IPC Bridge│ │ Config Modifier │   │
│   │ /var/lib/portage/  │ │ (python3 daemon)   │ │ Atomic write to  │   │
│   │ world & /var/db/pkg│ │ Descriptions & USE │ │ /etc/portage/    │   │
│   │ Pure Rust (~5ms)   │ │ resolution         │ │ package.*        │   │
│   └────────────────────┘ └────────────────────┘ └──────────────────┘   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 4. Key Data Flows

### A. Loading the World Dependency Tree
1. Read `/var/lib/portage/world` (plain text list of explicitly installed package atoms).
2. For each root package, read `/var/db/pkg/<cat>/<pkg-ver>/RDEPEND` to find direct child dependencies.
3. Build an in-memory `Tree<PackageNode>`.

### B. Inspecting USE Flags for Selected Package
1. Read `/var/db/pkg/<cat>/<pkg-ver>/IUSE` (all available flags).
2. Read `/var/db/pkg/<cat>/<pkg-ver>/USE` (flags active when built).
3. Query the Python IPC bridge for descriptions:
   * Local flag descriptions from `/var/db/repos/gentoo/<cat>/<pkg>/metadata.xml`.
   * Global flag descriptions from `/var/db/repos/gentoo/profiles/use.desc`.
4. Determine origin:
   * Is it in `/etc/portage/package.use`? (User override)
   * Is it in `/etc/portage/make.conf`? (Global override)
   * Is it from profile defaults?

### C. Staging and Committing Changes
1. User presses `Space` on `test` flag for `app-editors/neovim`.
2. App records a staged change: `AddFlag { package: "app-editors/neovim", flag: "test" }`.
3. Staging bar updates visually with `+ app-editors/neovim: +test`.
4. User presses `s`:
   * The config engine reads `/etc/portage/package.use`.
   * Formats a clean entry: `app-editors/neovim test`.
   * Atomically writes to `/etc/portage/package.use/custom` (or file).

---

## 5. C-Developer Concepts Used Here

* **Tree Representation:** Instead of manual `struct Node { void *data; struct Node **children; int num_children; }` with manual malloc/free, Rust uses `Vec<PackageNode>` with typed ownership.
* **Staging Buffer (Command Pattern):** Staging changes are stored as an enum:
  ```rust
  enum StagedChange {
      ToggleUse { atom: String, flag: String, enable: bool },
      ChangeKeyword { atom: String, keyword: KeywordStatus },
      ToggleMask { atom: String, unmask: bool },
  }
  ```
* **Thread Communication:** A background worker parses the ebuild metadata while the UI stays responsive at 60 FPS using an `mpsc::channel`.
