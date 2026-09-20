#!/bin/sh
# Install reel2mp3: a release binary when one fits, otherwise build from source.
# Usage: curl -fsSL <raw url>/install.sh | sh
set -eu

REPO="3monthsfree/instagram-reel-recorder"
BIN_DIR="${BIN_DIR:-$HOME/.local/bin}"
APP_DIR="$HOME/.local/share/applications"
OUT_DIR="${OUT_DIR:-$HOME/Music/reels}"

say() { printf '%s\n' "$*"; }
have() { command -v "$1" >/dev/null 2>&1; }

# --- runtime dependencies -------------------------------------------------

missing=""
have yt-dlp || missing="yt-dlp"
have ffmpeg || missing="$missing ffmpeg"
if [ -n "$missing" ]; then
    say "reel2mp3 needs:$missing"
    if have pacman; then say "  sudo pacman -S$missing"
    elif have apt; then say "  sudo apt install$missing"
    elif have dnf; then say "  sudo dnf install$missing"
    elif have brew; then say "  brew install$missing"
    fi
    say "Install those, then run this script again."
    exit 1
fi

# --- the binary itself ----------------------------------------------------

mkdir -p "$BIN_DIR"
asset="reel2mp3-$(uname -m)-linux"
url="https://github.com/$REPO/releases/latest/download/$asset"

if [ "$(uname -s)" = "Linux" ] && have curl && curl -fsSLo "$BIN_DIR/reel2mp3.tmp" "$url" 2>/dev/null; then
    chmod +x "$BIN_DIR/reel2mp3.tmp"
    mv "$BIN_DIR/reel2mp3.tmp" "$BIN_DIR/reel2mp3"
    say "Installed the release binary to $BIN_DIR/reel2mp3"
elif have cargo; then
    rm -f "$BIN_DIR/reel2mp3.tmp"
    say "No release binary for this system — building from source."
    cargo install --git "https://github.com/$REPO" --root "${CARGO_ROOT:-$HOME/.local}"
else
    rm -f "$BIN_DIR/reel2mp3.tmp"
    say "No release binary for this system, and cargo isn't installed."
    say "Install Rust from https://rustup.rs and run this again."
    exit 1
fi

# --- desktop launcher -----------------------------------------------------

mkdir -p "$OUT_DIR" "$APP_DIR"
cat > "$APP_DIR/reel2mp3.desktop" <<EOF
[Desktop Entry]
Type=Application
Name=Reel to MP3
GenericName=Instagram Audio Downloader
Comment=Queue Instagram reels and save them as MP3s
Exec=$BIN_DIR/reel2mp3 --out $OUT_DIR
Path=$OUT_DIR
Icon=audio-mpeg
Terminal=true
Categories=AudioVideo;Audio;
Keywords=instagram;reel;mp3;download;yt-dlp;
EOF
have update-desktop-database && update-desktop-database "$APP_DIR" 2>/dev/null || true

say ""
say "Done. MP3s will be saved to $OUT_DIR"
say "Run it with:  reel2mp3   (or find \"Reel to MP3\" in your app menu)"
case ":$PATH:" in
    *":$BIN_DIR:"*) ;;
    *) say "Note: $BIN_DIR isn't on your PATH — add it to your shell profile." ;;
esac
