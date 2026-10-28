#!/usr/bin/env bash
# ==============================================================================
# setup-portage-permissions.sh
#
# PURPOSE:
#   Grants a non-root user write access to /etc/portage by adding them to the
#   built-in Gentoo 'portage' group and configuring group permissions.
#   This allows managing USE flags, keywords, and package masks without running
#   the TUI as root.
#
# USAGE:
#   sudo ./setup-portage-permissions.sh [username]
#
#   If [username] is omitted, it defaults to the user who invoked sudo ($SUDO_USER),
#   or the current user if already root.
# ==============================================================================

# ------------------------------------------------------------------------------
# Bash Shell Options:
#   -e : Exit immediately if any command exits with a non-zero status.
#   -u : Treat unset variables and parameters as an error when expanding.
#   -o pipefail : The return value of a pipeline is the status of the last
#                 command to exit with a non-zero status (or 0 if all succeeded).
# ------------------------------------------------------------------------------
set -euo pipefail

# Determine target username using parameter expansion fallback:
#   1. $1                -> First CLI argument, if provided.
#   2. ${SUDO_USER:-...} -> Username of the caller before running sudo.
#   3. $(id -un)         -> Current user name (id -u: user ID, -n: print name).
TARGET_USER="${1:-${SUDO_USER:-$(id -un)}}"

# ------------------------------------------------------------------------------
# Check 1: Root Privileges
#   $EUID is the Effective User ID. Root always has EUID 0.
# ------------------------------------------------------------------------------
if [[ "$EUID" -ne 0 ]]; then
    # '>&2' redirects output to stderr (standard error stream)
    echo "[-] Error: This script must be run as root (or via sudo)." >&2
    echo "    Usage: sudo $0 [username]" >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Check 2: Target User Existence
#   'id <user>' looks up user in /etc/passwd.
#   '&>/dev/null' redirects both stdout and stderr to the null device (silent check).
# ------------------------------------------------------------------------------
if ! id "$TARGET_USER" &>/dev/null; then
    echo "[-] Error: User '$TARGET_USER' does not exist." >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Check 3: Portage Group Existence
#   'getent group <group>' queries the system group database (/etc/group).
# ------------------------------------------------------------------------------
if ! getent group portage &>/dev/null; then
    echo "[-] Error: Group 'portage' does not exist. Are you on a Gentoo system?" >&2
    exit 1
fi

# ------------------------------------------------------------------------------
# Step 1: Add user to portage group
#   Command: gpasswd
#   Flags:
#     -a <user> : Add (append) user to the specified named group.
# ------------------------------------------------------------------------------
echo "[+] Adding '$TARGET_USER' to group 'portage'..."
gpasswd -a "$TARGET_USER" portage

# ------------------------------------------------------------------------------
# Step 2: Configure /etc/portage filesystem permissions
# ------------------------------------------------------------------------------
if [[ -d /etc/portage ]]; then
    # Command: chgrp
    #   Changes the group ownership of files and directories.
    # Flags:
    #   -R : Recursive (apply to /etc/portage and all files/subdirectories inside).
    echo "[+] Setting group ownership on /etc/portage to 'portage'..."
    chgrp -R portage /etc/portage

    # Command: chmod
    #   Changes file mode bits (permissions).
    # Flags / Mode:
    #   -R   : Recursive.
    #   g+w  : 'g' = group, '+' = add, 'w' = write permission.
    #          Allows members of the group 'portage' to create, edit, and delete files.
    echo "[+] Granting group write permissions on /etc/portage..."
    chmod -R g+w /etc/portage

    # Command: find + chmod
    #   Applies the 'setgid' (Set Group ID) bit to all directories.
    # Find Flags:
    #   -type d : Only match directories (not regular files).
    #   -exec <command> {} + : Execute command on matching paths in batches ({} is placeholder).
    # Chmod Mode:
    #   g+s : Set Group ID bit.
    #         On Linux directories, setgid causes any newly created file or subfolder
    #         to automatically inherit the group of the parent directory ('portage')
    #         instead of the creating user's primary group.
    echo "[+] Setting setgid bit on /etc/portage directories..."
    find /etc/portage -type d -exec chmod g+s {} +
else
    echo "[!] Warning: /etc/portage not found (are you inside a container?). Permissions will need to be set on your Gentoo host."
fi

echo ""
echo "[✓] Setup complete!"
echo "    Note: If you are currently logged in as '$TARGET_USER', run:"
echo "          newgrp portage"
echo "    or log out and log back in for the new group membership to take effect."
