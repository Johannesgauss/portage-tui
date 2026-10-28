#!/usr/bin/env bash
# ==============================================================================
# uninstall-portage-tui.sh
#
# PURPOSE:
#   Cleanly uninstalls portage-tui by deleting the installed binary and
#   optionally cleaning up any configuration files created by portage-tui
#   in /etc/portage/.
#
# USAGE:
#   sudo ./tools/uninstall-portage-tui.sh [install_prefix]
#
#   Example:
#     sudo ./tools/uninstall-portage-tui.sh            # Removes /usr/local/bin/portage-tui
#     sudo ./tools/uninstall-portage-tui.sh /usr       # Removes /usr/bin/portage-tui
# ==============================================================================

# ------------------------------------------------------------------------------
# Shell Safety Flags:
#   -e          : Exit immediately if any command returns a non-zero exit code.
#   -u          : Treat references to unset variables as errors and exit.
#   -o pipefail : A pipeline fails if any command inside it fails.
# ------------------------------------------------------------------------------
set -euo pipefail

# ------------------------------------------------------------------------------
# Target Definitions:
#   ${1:-/usr/local} : Uses $1 if specified, defaults to /usr/local.
# ------------------------------------------------------------------------------
PREFIX="${1:-/usr/local}"
TARGET_BIN="${PREFIX}/bin/portage-tui"

# ------------------------------------------------------------------------------
# Privilege Check:
#   Root is required to remove binaries from system directories (/usr/local/bin, /usr/bin)
#   and to remove files from /etc/portage.
# ------------------------------------------------------------------------------
if [[ "$EUID" -ne 0 && ( "$PREFIX" == "/usr" || "$PREFIX" == "/usr/local" || ! -w "$PREFIX" ) ]]; then
    echo "[-] Error: Uninstalling from '${PREFIX}' requires root privileges." >&2
    echo "    Please run with sudo: sudo $0 [install_prefix]" >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Step 1: Remove the Installed Binary
#   -f : File test operator verifying file exists before attempting removal.
#   'rm -f "$TARGET_BIN"':
#     -f (--force) : Never prompt before removing; do not return error if file missing.
# ------------------------------------------------------------------------------
echo "[*] Checking for binary at '${TARGET_BIN}'..."
if [[ -f "$TARGET_BIN" ]]; then
    rm -f "$TARGET_BIN"
    echo "[✓] Successfully removed binary: ${TARGET_BIN}"
else
    echo "[-] Binary not found at '${TARGET_BIN}'. Skipping binary deletion."
fi

# ------------------------------------------------------------------------------
# Step 2: Search for Managed Configuration Files
#   portage-tui stores its saved changes in files named 'zz-portage-manager'
#   inside /etc/portage/ subdirectories.
# ------------------------------------------------------------------------------
echo ""
echo "[*] Scanning for portage-tui configuration files in /etc/portage/..."

CONFIG_TARGETS=(
    "/etc/portage/package.use/zz-portage-manager"
    "/etc/portage/package.mask/zz-portage-manager"
    "/etc/portage/package.unmask/zz-portage-manager"
    "/etc/portage/package.accept_keywords/zz-portage-manager"
)

FOUND_CONFIGS=()
for cfg in "${CONFIG_TARGETS[@]}"; do
    # -f : Check if regular file exists
    if [[ -f "$cfg" ]]; then
        FOUND_CONFIGS+=("$cfg")
    fi
done

# ------------------------------------------------------------------------------
# Step 3: Interactive Prompt for Configuration Removal
#   ${#FOUND_CONFIGS[@]} : Evaluates to the count of elements in the array.
#   'read -r -p ...':
#     -r : Raw input (prevents backslash escapes from being interpreted).
#     -p : Displays the prompt string on the terminal.
# ------------------------------------------------------------------------------
if [[ "${#FOUND_CONFIGS[@]}" -gt 0 ]]; then
    echo "    Found the following managed configuration files:"
    for cfg in "${FOUND_CONFIGS[@]}"; do
        echo "      - $cfg"
    done
    
    read -r -p "    Do you want to delete these configuration files? [y/N]: " REPLY
    
    # Regex test: matches 'y' or 'Y'
    if [[ "$REPLY" =~ ^[Yy]$ ]]; then
        for cfg in "${FOUND_CONFIGS[@]}"; do
            rm -f "$cfg"
            echo "    [✓] Deleted: $cfg"
        done
    else
        echo "    [*] Preserved configuration files in /etc/portage."
    fi
else
    echo "    [✓] No portage-tui configuration files found in /etc/portage."
fi

echo ""
echo "[✓] portage-tui has been uninstalled cleanly."
