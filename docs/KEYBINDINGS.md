# Keybindings Reference

`portage-tui` is entirely keyboard-driven. Most shortcuts follow standard Vim and terminal conventions.

---

## 1. Global Navigation

| Keybinding | Action | Description |
|:---|:---|:---|
| `Tab` / `BackTab` | Switch Pane | Toggles focus between the **@world tree** (left) and the **USE Flags / Details** (right). |
| `↑` / `k` | Move Up | Moves selection up by one item. |
| `↓` / `j` | Move Down | Moves selection down by one item. |
| `PageUp` | Page Up | Scrolls the active list up by one page. |
| `PageDown` | Page Down | Scrolls the active list down by one page. |
| `Home` / `g` | Jump to Start | Jumps to the very first item in the active list. |
| `End` / `G` | Jump to End | Jumps to the very last item in the active list. |
| `/` | Live Search | Opens search filter bar at bottom to filter packages or flags. |
| `Esc` | Clear / Dismiss | Clears search filter or closes open modals. |
| `[q]` | Quit | Cleanly restores terminal and exits `portage-tui`. |

---

## 2. Tree Pane Controls (Left Pane)

| Keybinding | Action | Description |
|:---|:---|:---|
| `Enter` | Expand / Collapse | Expands or collapses direct dependencies of the selected package. |
| `l` / `Right` | Expand | Expands the selected node's dependencies. |
| `h` / `Left` | Collapse / Parent | Collapses current node, or jumps to its parent node if already collapsed. |
| `Space` | Jump to Flags | Moves focus directly to the USE Flags pane for the selected package. |

---

## 3. USE Flag Controls (Right Pane)

| Keybinding | Action | Description |
|:---|:---|:---|
| `Space` | Toggle Flag | Toggles the selected USE flag between enabled `[✓]` and disabled `[ ]`. Marks the flag with a yellow `[staged]` badge. |
| `[s]` | Save Staged | Commits all queued USE flag and mask changes to `/etc/portage/package.use/zz-portage-manager`. |

---

## 4. Package Management Actions

| Keybinding | Action | Description |
|:---|:---|:---|
| `[a]` | Add / Install | Opens the package installation modal. |
| `[d]` / `[x]` | Uninstall | Opens confirmation modal to run `emerge --deselect` and `emerge --depclean`. |
| `[m]` | Toggle Mask | Adds or removes package mask in `/etc/portage/package.mask/zz-portage-manager`. |
| `[t]` | Toggle ~Testing | Adds or removes `~testing` arch keyword in `package.accept_keywords`. |
| `[u]` | System Update | Runs `sudo emaint --auto sync` followed by `emerge -avuDN @world`. |

---

## 5. Modal Navigation

### Install Package Modal (`[a]`)
* Type package name (e.g., `fceux` or `games-emulation/fceux`).
* `Enter`: Queries repository and checks mask status.
* `Esc`: Cancels installation.

### Pre-Install USE Flag Modal
* `Space`: Toggle USE flag on or off.
* `↑` / `k`, `↓` / `j`: Navigate flags list.
* `PageUp` / `PageDown`: Page scroll.
* `Enter`: **Save & Emerge** — Writes customized flags to `package.use` and launches `sudo emerge --ask --verbose <atom>`.
* `Esc`: Cancel without invoking emerge or writing changes.

### Uninstall Confirmation Modal (`[d]` / `[x]`)
* `y` / `Enter`: Confirm uninstallation (`emerge --deselect` + `emerge --depclean`).
* `n` / `Esc`: Cancel and return safely to the TUI.
