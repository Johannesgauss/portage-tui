# Planned Roadmap & Feature Status

This document tracks completed features and planned enhancements for `portage-tui`.

---

## Completed Features (v0.1.0)

- [x] **Collapsible @world Dependency Tree**: Full tree navigation with cycle detection and depth indentation.
- [x] **USE Flag Inspector**: Plain-English descriptions from `use.desc`, `metadata.xml`, and `desc/*.desc`.
- [x] **Flag Origin Tracking**: Visual indicators for `package.use`, `make.conf`, and profile defaults.
- [x] **Pre-Flight Query on Install (`[a]`)**: Verifies package existence before calling emerge.
- [x] **Pre-Install USE Flag Picker**: Allows configuring package flags *before* compilation begins.
- [x] **Package Mask Management (`[m]`, `[t]`)**: Support for `package.mask`, `package.unmask`, and `package.accept_keywords`.
- [x] **Safe Package Depclean (`[d]` / `[x]`)**: Interactive deselect and unmerge with automatic tree reload.
- [x] **System Update Workflow (`[u]`)**: Synchronizes repositories with `emaint` and updates `@world`.
- [x] **Non-Destructive Overrides**: Writes exclusively to `zz-portage-manager` files.
- [x] **Cooked Terminal Handover**: Transparent terminal mode restoration for `sudo` and Portage status output.
- [x] **Gentoo Packaging**: Provided automated scripts and standard EAPI 8 ebuild.

---

## Active Roadmap (Planned for v0.2.0 - v0.3.0)

### 1. Single-Package Rebuild with Changed USE Flags (`[r]`)
* **Priority**: High
* **Rationale**: When modifying USE flags on an installed package, users currently have to either run a full `@world` update or drop to a shell.
* **Proposed Implementation**:
  * Shortcut `[r]` (or `[e]`) on an installed package.
  * Prompts: "Re-emerge <atom> with updated USE flags? (`emerge -1avN <atom>`)"
  * Launches `sudo emerge --ask --verbose --newuse <atom>`.

### 2. Staged Configuration Diff Preview Modal
* **Priority**: High
* **Rationale**: Giving users a visual diff preview before writing to `/etc/portage/` builds trust and avoids accidental writes.
* **Proposed Implementation**:
  * When pressing `[s]` to save, display a diff modal showing added/removed lines in unified diff format:
    ```diff
    --- /etc/portage/package.use/zz-portage-manager
    +++ /etc/portage/package.use/zz-portage-manager
    +games-emulation/fceux opengl -lua
    ```
  * User confirms with `[Enter]` or cancels with `[Esc]`.

### 3. Raw Ebuild Viewer & Build Log Pager (`[v]` / `[l]`)
* **Priority**: Medium
* **Rationale**: Gentoo power users frequently inspect ebuild source code to see compile flags and dependencies.
* **Proposed Implementation**:
  * `[v]`: Reads the package's `.ebuild` from `/var/db/repos/<repo>/...` and renders it in a scrollable Ratatui text viewer with syntax highlighting.
  * `[l]`: Reads recent emerge logs from `/var/log/emerge.log` or the package's build log under `/var/tmp/portage/.../temp/build.log`.

### 4. View Mode Filters (`[F]`)
* **Priority**: Medium
* **Rationale**: Finding customized or masked packages in a large `@world` tree (150+ packages) can be tedious.
* **Proposed Implementation**:
  * Pressing `[F]` cycles through view modes:
    1. **All @world** (Default)
    2. **Customized Only**: Packages with entries in `package.use`, `package.mask`, or `package.accept_keywords`.
    3. **Testing / Masked Only**: Packages using `~arch` or masked by profiles.
    4. **Outdated / Upgradable Only**: Packages where a newer version is available in the repository.

### 5. Config File Dispatcher Integration (`[c]`)
* **Priority**: Low
* **Rationale**: After updates that modify config files in `/etc`, Portage outputs warnings about `._cfg0000_*` files.
* **Proposed Implementation**:
  * Shortcut `[c]` launches `etc-update` or `dispatch-conf` in the interactive runner session without leaving the TUI.
