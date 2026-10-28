# Copyright 2026 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

EAPI=8

inherit cargo

DESCRIPTION="Interactive terminal user interface (TUI) for Gentoo Portage package and USE flag management"
HOMEPAGE="https://github.com/Johannesgauss/portage-tui"

# In production, Gentoo cargo ebuilds specify SRC_URI with crate vendoring or cargo_crate_uris.
# For local overlay development from local source, SRC_URI can point to a release tarball or local distfile.
SRC_URI=""

LICENSE="MIT Apache-2.0 Unicode-DFS-2016"
SLOT="0"
KEYWORDS="~amd64 ~arm64 ~x86"
IUSE=""

# Runtime dependencies:
# - sys-apps/portage: required to interact with Gentoo ebuild repositories, /etc/portage, and emerge.
RDEPEND="
	sys-apps/portage
"

# Build dependencies:
# - virtual/rust: Cargo and Rust compiler
BDEPEND="
	virtual/rust
"

src_unpack() {
	if [[ -n "${A}" ]]; then
		cargo_src_unpack
	else
		# When building from a local repository checkout or unpackdir
		mkdir -p "${S}" || die
		cp -a "${FILESDIR}/../../.."/* "${S}/" 2>/dev/null || true
	fi
}

src_compile() {
	cargo_src_compile
}

src_install() {
	# Installs the target/release/portage-tui binary into /usr/bin/portage-tui
	cargo_src_install

	# Install documentation and manual page if available
	dodoc README.md docs/*.md 2>/dev/null || true
	doman docs/portage-tui.1 2>/dev/null || true
}
