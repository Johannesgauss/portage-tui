#!/usr/bin/env bash
# ==============================================================================
# install-portage-tui.sh
#
# PURPOSE:
#   Compiles the optimized release binary of portage-tui and installs it
#   into /usr/local/bin (or a user-specified prefix directory).
#
# USAGE:
#   sudo ./tools/install-portage-tui.sh [install_prefix]
#
#   Example:
#     sudo ./tools/install-portage-tui.sh            # Installs to /usr/local/bin/portage-tui
#     sudo ./tools/install-portage-tui.sh /usr       # Installs to /usr/bin/portage-tui
# ==============================================================================

# ------------------------------------------------------------------------------
# Shell Safety Flags:
#   -e          : Exit immediately if any command returns a non-zero exit code.
#   -u          : Treat references to unset variables as errors and exit.
#   -o pipefail : A pipeline fails if any command inside it fails (not just the last).
# ------------------------------------------------------------------------------
set -euo pipefail

# ------------------------------------------------------------------------------
# Directory & Path Setup:
#   ${1:-/usr/local} : Parameter expansion; uses $1 if provided, else defaults to /usr/local.
#   dirname "${BASH_SOURCE[0]}" : Resolves the directory containing this script.
#   cd ... && pwd    : Resolves the canonical absolute path of the repository root.
# ------------------------------------------------------------------------------
PREFIX="${1:-/usr/local}"
BIN_DIR="${PREFIX}/bin"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ------------------------------------------------------------------------------
# Privilege Verification:
#   Attempt to create $BIN_DIR to verify write access.
#   If it fails and caller is not root ($EUID -ne 0), report error and exit.
# ------------------------------------------------------------------------------
if ! mkdir -p "$BIN_DIR" 2>/dev/null && [[ "$EUID" -ne 0 ]]; then
    # '>&2' redirects error message to stderr
    echo "[-] Error: Installing into '${PREFIX}' requires root privileges or write access." >&2
    echo "    Please run with sudo: sudo $0 [install_prefix]" >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Step 1: Build the Release Binary
#   'cargo build --release':
#     --release : Enables level 3 optimizations (LTO, inlining, strip symbols if configured)
#                 producing a fast, standalone production binary.
# ------------------------------------------------------------------------------
echo "[+] Navigating to project directory: ${PROJECT_DIR}"
cd "$PROJECT_DIR"

echo "[+] Building release binary using Cargo..."
cargo build --release

RELEASE_BIN="${PROJECT_DIR}/target/release/portage-tui"

# -f : File test operator verifying that the compiled binary actually exists.
if [[ ! -f "$RELEASE_BIN" ]]; then
    echo "[-] Error: Release binary was not found at '${RELEASE_BIN}'." >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Step 2: Ensure Destination Directory Exists
#   'mkdir -p "$BIN_DIR"':
#     -p (--parents) : Creates parent directories as needed without failing if they exist.
# ------------------------------------------------------------------------------
echo "[+] Ensuring destination directory exists: ${BIN_DIR}"
mkdir -p "$BIN_DIR"

# ------------------------------------------------------------------------------
# Step 3: Install the Binary with Executable Permissions
#   'install -m 755 [source] [destination]':
#     -m 755 (--mode) : Sets file mode bits to rwxr-xr-x (owner can read/write/execute,
#                       group and others can read and execute).
# ------------------------------------------------------------------------------
echo "[+] Installing binary to '${BIN_DIR}/portage-tui'..."
install -m 755 "$RELEASE_BIN" "${BIN_DIR}/portage-tui"

echo ""
echo "[✓] Successfully installed portage-tui to: ${BIN_DIR}/portage-tui"
echo "    You can now run 'portage-tui' from anywhere in your terminal."
