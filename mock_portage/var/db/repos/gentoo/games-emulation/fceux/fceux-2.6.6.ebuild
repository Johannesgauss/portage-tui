# Copyright 1999-2024 Gentoo Authors
# Distributed under the terms of the GNU General Public License v2

EAPI=8

DESCRIPTION="A portable, highly compatible NES/Famicom emulator"
HOMEPAGE="https://fceux.com/"
SRC_URI="https://github.com/TASEmulators/fceux/archive/v${PV}.tar.gz"

LICENSE="GPL-2+"
SLOT="0"
KEYWORDS="~amd64 ~x86"
IUSE="lua opengl"
