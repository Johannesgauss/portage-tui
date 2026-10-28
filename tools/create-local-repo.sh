#!/usr/bin/env bash
# ==============================================================================
# create-local-repo.sh
#
# PURPOSE:
#   Creates and registers a local Gentoo Portage repository (overlay) to house
#   custom or local ebuilds, such as 'portage-tui'.
#
# USAGE:
#   sudo ./tools/create-local-repo.sh [repo_name] [repo_path]
#
# DEFAULTS:
#   repo_name : localrepo
#   repo_path : /var/db/repos/localrepo
#
# ACTIONS PERFORMED:
#   1. Creates repository directory tree (/var/db/repos/<repo_name>).
#   2. Initializes 'profiles/repo_name' with the unique overlay name.
#   3. Initializes 'profiles/categories' with 'app-portage'.
#   4. Initializes 'metadata/layout.conf' with masters = gentoo.
#   5. Registers repository in /etc/portage/repos.conf/<repo_name>.conf.
#   6. Installs the 'portage-tui' ebuild into app-portage/portage-tui/.
#   7. Generates Manifest digest (if 'ebuild' or 'pkgdev' command exists).
#   8. Sets ownership to portage:portage with group-write permissions.
# ==============================================================================

# ------------------------------------------------------------------------------
# Shell Safety Flags:
#   -e          : Exit immediately if any command returns a non-zero exit code.
#   -u          : Treat references to unset variables as errors and exit.
#   -o pipefail : A pipeline fails if any command inside it fails.
# ------------------------------------------------------------------------------
set -euo pipefail

# ------------------------------------------------------------------------------
# Script Parameters & Defaults:
#   ${1:-localrepo}              : Name of the overlay (defaults to 'localrepo').
#   ${2:-/var/db/repos/$REPO_NAME}: Directory on disk where overlay files reside.
# ------------------------------------------------------------------------------
REPO_NAME="${1:-localrepo}"
REPO_PATH="${2:-/var/db/repos/${REPO_NAME}}"
REPOS_CONF_DIR="/etc/portage/repos.conf"
REPOS_CONF_FILE="${REPOS_CONF_DIR}/${REPO_NAME}.conf"
PROJECT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# ------------------------------------------------------------------------------
# Privilege Verification:
#   Writing to /var/db/repos and /etc/portage requires root/sudo privileges.
#   $EUID is the Effective User ID (0 for root).
# ------------------------------------------------------------------------------
if [[ "$EUID" -ne 0 ]]; then
    echo "[-] Error: Creating a Portage repository requires root privileges." >&2
    echo "    Usage: sudo $0 [repo_name] [repo_path]" >&2
    exit 1
fi

echo "[*] Initializing local Portage repository:"
echo "    Repository Name : ${REPO_NAME}"
echo "    Repository Path : ${REPO_PATH}"
echo "    Config File     : ${REPOS_CONF_FILE}"
echo ""

# ------------------------------------------------------------------------------
# Step 1: Create Repository Directory Structure
#   'mkdir -p':
#     -p (--parents) : Creates intermediate directories as needed without error.
# ------------------------------------------------------------------------------
echo "[+] Step 1: Creating directory layout..."
mkdir -p "${REPO_PATH}/profiles"
mkdir -p "${REPO_PATH}/metadata"
mkdir -p "${REPO_PATH}/app-portage/portage-tui"

# ------------------------------------------------------------------------------
# Step 2: Configure profiles/repo_name
#   Portage identifies overlays by the name written inside profiles/repo_name.
# ------------------------------------------------------------------------------
echo "[+] Step 2: Writing profiles/repo_name..."
echo "${REPO_NAME}" > "${REPO_PATH}/profiles/repo_name"

# ------------------------------------------------------------------------------
# Step 3: Configure profiles/categories
#   Declares categories supported by this overlay.
# ------------------------------------------------------------------------------
echo "[+] Step 3: Writing profiles/categories..."
cat << 'EOF' > "${REPO_PATH}/profiles/categories"
app-portage
EOF

# ------------------------------------------------------------------------------
# Step 4: Configure metadata/layout.conf
#   - masters = gentoo       : Inherits eclasses and licenses from the gentoo repo.
#   - auto-sync = false      : Prevents 'emaint sync' or 'emerge --sync' from wiping this local repo.
#   - thin-manifests = true  : Standard modern Portage manifest format (hashes distfiles only).
#   - sign-manifests = false : Disables GPG signing requirement for local ebuilds.
# ------------------------------------------------------------------------------
echo "[+] Step 4: Writing metadata/layout.conf..."
cat << 'EOF' > "${REPO_PATH}/metadata/layout.conf"
masters = gentoo
auto-sync = false
thin-manifests = true
sign-manifests = false
EOF

# ------------------------------------------------------------------------------
# Step 5: Register Repository in /etc/portage/repos.conf
#   In Portage, overlays are registered in /etc/portage/repos.conf/ (or /etc/portage/repos.conf).
#   We create the directory if it doesn't exist and write the configuration block.
# ------------------------------------------------------------------------------
echo "[+] Step 5: Registering repository in ${REPOS_CONF_FILE}..."
mkdir -p "${REPOS_CONF_DIR}"

cat << EOF > "${REPOS_CONF_FILE}"
# Local repository configuration generated by portage-tui tools
[${REPO_NAME}]
location = ${REPO_PATH}
masters = gentoo
auto-sync = no
EOF

# ------------------------------------------------------------------------------
# Step 6: Copy portage-tui ebuild and metadata into repository
#   Locates the bundled ebuild and metadata from project directory and installs them.
# ------------------------------------------------------------------------------
SOURCE_PKG_DIR="${PROJECT_DIR}/ebuild/app-portage/portage-tui"
TARGET_PKG_DIR="${REPO_PATH}/app-portage/portage-tui"
SOURCE_EBUILD="${SOURCE_PKG_DIR}/portage-tui-0.1.0.ebuild"
TARGET_EBUILD="${TARGET_PKG_DIR}/portage-tui-0.1.0.ebuild"
SOURCE_META="${SOURCE_PKG_DIR}/metadata.xml"
TARGET_META="${TARGET_PKG_DIR}/metadata.xml"

if [[ -f "$SOURCE_EBUILD" ]]; then
    echo "[+] Step 6: Copying portage-tui ebuild and metadata to ${TARGET_PKG_DIR}..."
    # 'cp -f' (--force) overwrites destination if it already exists
    cp -f "$SOURCE_EBUILD" "$TARGET_EBUILD"
    if [[ -f "$SOURCE_META" ]]; then
        cp -f "$SOURCE_META" "$TARGET_META"
    fi
else
    echo "[!] Step 6: Bundled ebuild not found at '${SOURCE_EBUILD}'. Skipping ebuild copy."
fi

# ------------------------------------------------------------------------------
# Step 7: Generate Manifest Digest
#   Portage requires a Manifest file verifying integrity of files in package directory.
#   If 'ebuild' utility is available, we run 'ebuild ... digest'.
#   If 'pkgdev' utility is available, we run 'pkgdev manifest'.
# ------------------------------------------------------------------------------
echo "[+] Step 7: Generating Manifest..."
if command -v ebuild &>/dev/null && [[ -f "$TARGET_EBUILD" ]]; then
    echo "    Running 'ebuild ${TARGET_EBUILD} digest'..."
    ebuild "$TARGET_EBUILD" digest
elif command -v pkgdev &>/dev/null && [[ -f "$TARGET_EBUILD" ]]; then
    echo "    Running 'pkgdev manifest' in $(dirname "$TARGET_EBUILD")..."
    (cd "$(dirname "$TARGET_EBUILD")" && pkgdev manifest)
else
    echo "    [i] Neither 'ebuild' nor 'pkgdev' command found in PATH (or ebuild not copied)."
    echo "        Run 'ebuild ${TARGET_EBUILD} digest' on your Gentoo host to create Manifest."
fi

# ------------------------------------------------------------------------------
# Step 8: Configure Permissions
#   If group 'portage' exists, set ownership so portage and portage group users
#   can read and write to the repository.
# ------------------------------------------------------------------------------
if getent group portage &>/dev/null; then
    echo "[+] Step 8: Setting group ownership to 'portage'..."
    # 'chgrp -R portage' : Recursively change group ownership to portage
    chgrp -R portage "${REPO_PATH}"
    # 'chmod -R g+w'     : Grant group write permissions recursively
    chmod -R g+w "${REPO_PATH}"
    # 'find ... -exec chmod g+s' : Set setgid bit on directories so new files inherit portage group
    find "${REPO_PATH}" -type d -exec chmod g+s {} +
    echo "    Permissions configured for 'portage' group."
fi

echo ""
echo "[✓] Local Portage repository '${REPO_NAME}' is ready!"
echo "    Location : ${REPO_PATH}"
echo "    Config   : ${REPOS_CONF_FILE}"
echo ""
echo "    To install portage-tui using emerge from your new overlay, run:"
echo "      emerge --ask app-portage/portage-tui"
