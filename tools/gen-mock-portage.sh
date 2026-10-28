#!/usr/bin/env bash
# Regenerates ./mock_portage: a tiny fake Gentoo root for developing off-Gentoo.
# Usage: tools/gen-mock-portage.sh   (run from the project root)
set -euo pipefail

R="mock_portage"
rm -rf "$R"
mkdir -p "$R/var/lib/portage" "$R/etc/portage/package.use" "$R/etc/portage/repos.conf"
REPO="$R/var/db/repos/gentoo"
mkdir -p "$REPO/profiles/desc"

# ---------------------------------------------------------------- user config
cat > "$R/etc/portage/repos.conf/gentoo.conf" <<'EOF'
[DEFAULT]
main-repo = gentoo

[gentoo]
location = /var/db/repos/gentoo
EOF

cat > "$R/etc/portage/make.conf" <<'EOF'
COMMON_FLAGS="-O2 -pipe -march=native"
USE="lto wayland -X pulseaudio"
EOF

cat > "$R/etc/portage/package.use/00-custom" <<'EOF'
net-misc/curl ssl http2
app-editors/neovim LUA_SINGLE_TARGET: luajit
EOF

# ---------------------------------------------------------------- repo metadata
cat > "$REPO/profiles/use.desc" <<'EOF'
# Global USE flags
X - Add support for X11
debug - Enable extra debug codepaths, like asserts and extra output
doc - Add extra documentation (API, Javadoc, etc)
http2 - Enable support for the HTTP/2 protocol
ldap - Add LDAP support (Lightweight Directory Access Protocol)
lto - Enable Link-Time Optimization (LTO) to optimize the build
nls - Add Native Language Support (using gettext)
pulseaudio - Add support for PulseAudio sound server
ssh - Enable SSH support
ssl - Add support for SSL/TLS connections (Secure Socket Layer / Transport Layer Security)
static-libs - Build static versions of dynamic libraries as well
test - Enable dependencies and/or preparations necessary to run tests
wayland - Enable dev-libs/wayland backend
zstd - Enable support for ZSTD compression
pgo - Optimize the build using Profile Guided Optimization (PGO)
dbus - Enable dbus support for anything that needs it
vaapi - Enable Video Acceleration API for hardware decoding
EOF

cat > "$REPO/profiles/desc/lua_single_target.desc" <<'EOF'
luajit - Build for LuaJIT 2 (single only)
lua5-1 - Build for Lua 5.1 (single only)
lua5-4 - Build for Lua 5.4 (single only)
EOF

cat > "$REPO/profiles/desc/python_targets.desc" <<'EOF'
python3_12 - Build with Python 3.12
python3_13 - Build with Python 3.13
EOF

# metadata.xml with local flags:  meta <cat/pkg> "flag|desc" ...
meta() {
  local cp="$1"; shift
  mkdir -p "$REPO/$cp"
  {
    echo '<?xml version="1.0" encoding="UTF-8"?>'
    echo '<!DOCTYPE pkgmetadata SYSTEM "https://www.gentoo.org/dtd/metadata.dtd">'
    echo '<pkgmetadata>'
    echo '  <use>'
    for f in "$@"; do
      echo "    <flag name=\"${f%%|*}\">${f#*|}</flag>"
    done
    echo '  </use>'
    echo '</pkgmetadata>'
  } > "$REPO/$cp/metadata.xml"
}

meta app-editors/neovim "nvimpager|Install nvimpager symlink to less.sh macro"
meta dev-libs/openssl "asm|Enable hand-written assembly optimizations" "sslv3|Support for the old/insecure SSLv3 protocol"
meta sys-apps/ripgrep "pcre2|Add support for PCRE2 regular expressions via <pkg>dev-libs/libpcre2</pkg>"
meta sys-libs/zlib "minizip|Include the minizip library for quick and dirty zip extraction"
meta www-client/firefox \
  "clang|Use Clang compiler instead of GCC" \
  "eme-free|Disable EME (DRM plugin) capability at build time" \
  "geckodriver|Install geckodriver for WebDriver automation" \
  "hardened|Use hardened toolchain flags" \
  "hwaccel|Force-enable hardware-accelerated rendering" \
  "jumbo-build|Increase build speed with unified (jumbo) builds" \
  "openh264|Use <pkg>media-libs/openh264</pkg> for H264 support" \
  "screencast|Enable screencast support on Wayland via PipeWire" \
  "sndio|Enable sndio audio backend" \
  "system-av1|Use the system-wide AV1 libraries" \
  "system-harfbuzz|Use the system-wide harfbuzz" \
  "system-icu|Use the system-wide ICU" \
  "system-jpeg|Use the system-wide libjpeg-turbo" \
  "system-libevent|Use the system-wide libevent" \
  "system-libvpx|Use the system-wide libvpx" \
  "system-png|Use the system-wide libpng with APNG support" \
  "system-webp|Use the system-wide libwebp" \
  "telemetry|Send technical and interaction data to Mozilla" \
  "wifi|Enable network location via nearby WiFi access points"

# ---------------------------------------------------------------- installed pkgs
# pkg <cat/name-ver> <slot> <description> <homepage> <IUSE> <USE> <RDEPEND>
pkg() {
  local d="$R/var/db/pkg/$1"
  mkdir -p "$d"
  echo "$2" > "$d/SLOT"
  echo "$3" > "$d/DESCRIPTION"
  echo "$4" > "$d/HOMEPAGE"
  echo "$5" > "$d/IUSE"
  echo "$6" > "$d/USE"
  echo "$7" > "$d/RDEPEND"
  echo "gentoo" > "$d/repository"
}

pkg app-editors/neovim-0.10.4 0 "Vim-fork focused on extensibility and agility" "https://neovim.io" \
  "+lua_single_target_luajit lua_single_target_lua5-1 lto nvimpager test" \
  "lua_single_target_luajit lto" \
  "dev-libs/libuv:0/1= >=dev-libs/msgpack-c-6.0.0 dev-lua/lua-lpeg[lua_single_target_luajit(-)] dev-lang/luajit:2"
pkg dev-libs/libuv-1.48.0 0/1 "Cross-platform asychronous I/O" "https://github.com/libuv/libuv" "" "" ""
pkg dev-libs/msgpack-c-6.0.0 0/2 "MessagePack is a binary-based efficient data interchange format" "https://msgpack.org" "doc test" "" ""
pkg dev-lua/lua-lpeg-1.1.0 0 "Parsing Expression Grammars for Lua" "https://www.inf.puc-rio.br/~roberto/lpeg/" \
  "debug doc +lua_single_target_luajit" "lua_single_target_luajit" "dev-lang/luajit:2"
pkg dev-lang/luajit-2.1.1720049189 2/2.1.1720049189 "Just-In-Time Compiler for the Lua programming language" "https://luajit.org/" "lua52compat static-libs" "" ""

pkg net-misc/curl-8.11.1 0 "A Client that groks URLs" "https://curl.se/" \
  "ssl http2 ldap ssh test zstd static-libs" "ssl http2 zstd" \
  ">=net-libs/nghttp2-1.0:= dev-libs/openssl:0/3= sys-libs/zlib app-arch/zstd ssh? ( net-libs/libssh2 )"
pkg net-libs/nghttp2-1.64.0 0/1.14 "HTTP/2 C Library" "https://nghttp2.org/" "debug static-libs test" "" "dev-libs/openssl"
pkg dev-libs/openssl-3.3.2-r1 0/3 "Robust, full-featured Open Source Toolkit for the TLS and SSL protocols" "https://openssl-library.org/" \
  "+asm sslv3 static-libs test zlib" "asm zlib" "sys-libs/zlib"
pkg sys-libs/zlib-1.3.1-r1 0/1 "Standard (de)compression library" "https://zlib.net/" "minizip static-libs" "" ""
pkg app-arch/zstd-1.5.6 0/1 "zstd fast compression library" "https://facebook.github.io/zstd/" "lz4 lzma static-libs +zlib" "zlib" "sys-libs/zlib"

pkg sys-apps/ripgrep-14.1.0 0 "Search tool that combines the usability of ag with the raw speed of grep" "https://github.com/BurntSushi/ripgrep" "+pcre2" "pcre2" "dev-libs/libpcre2"
pkg dev-libs/libpcre2-10.44 0/3 "Perl-compatible regular expression library" "https://www.pcre.org/" "bzip2 +jit static-libs unicode zlib" "jit unicode zlib" "sys-libs/zlib"

pkg www-client/firefox-133.0 rapid "Firefox Web Browser" "https://www.mozilla.org/firefox" \
  "clang dbus debug eme-free geckodriver hardened hwaccel jumbo-build lto openh264 pgo pulseaudio screencast sndio system-av1 system-harfbuzz system-icu system-jpeg system-libevent system-libvpx system-png system-webp telemetry test vaapi wayland wifi X" \
  "clang dbus hwaccel jumbo-build lto pgo pulseaudio screencast system-av1 system-harfbuzz system-icu system-jpeg system-libevent system-libvpx system-png system-webp vaapi wayland X" \
  "dev-libs/openssl sys-libs/zlib media-libs/harfbuzz"
pkg media-libs/harfbuzz-10.1.0 0/6.0.0 "An OpenType text shaping engine" "https://harfbuzz.github.io/" "+cairo debug doc +glib +graphite icu +introspection test +truetype" "cairo glib graphite introspection truetype" "media-libs/freetype"
# freetype <-> harfbuzz is a real-world dependency cycle
pkg media-libs/freetype-2.13.3 2 "High-quality and portable font engine" "https://www.freetype.org/" "+adobe-cff brotli bzip2 +cleartype-hinting debug doc +harfbuzz +png static-libs svg utils" "adobe-cff cleartype-hinting harfbuzz png" "sys-libs/zlib harfbuzz? ( media-libs/harfbuzz )"

# A few simple world packages so the list is long enough to page through.
for spec in \
  "app-shells/zsh-5.9-r6|UNIX Shell similar to the Korn shell|https://www.zsh.org/|caps debug doc examples gdbm maildir pcre static|pcre" \
  "dev-vcs/git-2.47.1|stupid content tracker: distributed VCS designed for speed|https://www.git-scm.com/|+curl cgi doc gpg keyring +iconv mediawiki +nls +pcre perl +safe-directory selinux subversion test tk webdav xinetd|curl iconv nls pcre safe-directory" \
  "app-admin/sudo-1.9.16_p2|Allows users or groups to run commands as other users|https://www.sudo.ws/|gcrypt ldap nls offensive pam sasl +secure-path selinux +sendmail skey ssl sssd|nls pam secure-path sendmail ssl" \
  "net-misc/openssh-9.9_p1|Port of OpenBSD's free SSH release|https://www.openssh.com/|abi_mips_n32 audit debug kerberos ldns libedit livecd pam +pie security-key selinux +ssl static test xmss|pam pie ssl" \
  "media-video/mpv-0.39.0|Media player for the command line|https://mpv.io/|+X +alsa aqua archive bluray cdda +cli coreaudio debug +drm dvb dvd +egl gamepad +iconv jack javascript jpeg lcms libcaca +libmpv +lua mmal nvenc openal +opengl pipewire pulseaudio rubberband sdl selinux sixel sndio soc test tools +uchardet vaapi vdpau vulkan wayland +zimg zlib|X alsa cli drm egl iconv libmpv lua opengl pipewire pulseaudio uchardet vaapi vulkan wayland zimg zlib" \
  "app-misc/tmux-3.5a|Terminal multiplexer|https://tmux.github.io/|debug selinux systemd utempter vim-syntax|" \
  "sys-process/htop-3.3.0|Interactive process viewer|https://htop.dev/|caps debug delayacct hwloc lm-sensors llvm-libunwind openvz unicode unwind vserver|unicode" \
  "app-misc/fastfetch-2.31.0|Fast neofetch-like system information tool|https://github.com/fastfetch-cli/fastfetch|X chafa dbus ddcutil drm elf gnome imagemagick opencl opengl osmesa pulseaudio sqlite test vulkan wayland xfce xrandr|X dbus drm wayland" \
  "dev-lang/rust-bin-1.83.0|Systems programming language from Mozilla|https://www.rust-lang.org/|big-endian clippy doc prefix rust-analyzer rust-src rustfmt verify-sig|clippy rustfmt" \
  "app-text/tree-2.2.1|Lists directories recursively, and produces an indented listing of files|https://oldmanprogrammer.net/source.php?dir=projects/tree||"
do
  IFS='|' read -r cpv desc home iuse use <<< "$spec"
  pkg "$cpv" 0 "$desc" "$home" "$iuse" "$use" "sys-libs/zlib"
done

cat > "$R/var/lib/portage/world" <<'EOF'
app-admin/sudo
app-editors/neovim
app-misc/fastfetch
app-misc/tmux
app-shells/zsh
app-text/tree
dev-lang/rust-bin
dev-vcs/git
media-video/mpv
net-misc/curl
net-misc/openssh
sys-apps/ripgrep
sys-process/htop
www-client/firefox:rapid
EOF

echo "mock Portage tree generated in $R/"
