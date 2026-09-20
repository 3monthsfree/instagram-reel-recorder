# reel2mp3

A terminal dashboard for turning Instagram reels into MP3s. Paste a pile of
links, look them over, press start. The video is downloaded, the audio is kept,
the video is thrown away.

![Eleven links staged, waiting on the start button](https://raw.githubusercontent.com/3monthsfree/instagram-reel-recorder/main/docs/staged.webp)

*Paste a block of links: they wait in the add panel until you press start.*

![The queue running, with the speed waveform under load](https://raw.githubusercontent.com/3monthsfree/instagram-reel-recorder/main/docs/running.webp)

*Under way: progress meters per row, and the speed waveform with worker load
filling in behind it.*

Built with [ratatui](https://ratatui.rs) on top of
[yt-dlp](https://github.com/yt-dlp/yt-dlp) and ffmpeg.

## What it does

- **Bulk paste.** Drop a whole block of links at once; every URL in the text is
  staged. Keep adding while downloads run.
- **Staging, then start.** Links wait in the add panel until you press `enter`
  or click **▶ start**, so a stray paste never kicks off a download.
- **Live dashboard.** Per-row progress meters and sparklines, a braille speed
  waveform with worker load behind it, queue stats, and details for the
  selected row.
- **Cookies on a keypress.** Instagram often wants a logged-in session. `c`
  cycles between no cookies and your browser's, and `r` retries the row.
- **Knows what it already has.** A row marked `✓ have` was already in the
  folder; nothing was re-downloaded.

## Requirements

`yt-dlp` and `ffmpeg` must be on your PATH.

```bash
sudo pacman -S yt-dlp ffmpeg        # Arch, CachyOS, Manjaro
sudo apt install yt-dlp ffmpeg      # Debian, Ubuntu
brew install yt-dlp ffmpeg          # macOS
```

Instagram changes often, so keep yt-dlp current. That fixes most breakage.

## Install

**Prebuilt binary** (Linux x86_64, no toolchain needed):

```bash
curl -fsSL https://raw.githubusercontent.com/3monthsfree/instagram-reel-recorder/main/install.sh | sh
```

It fetches the latest release binary into `~/.local/bin` and installs a desktop
launcher. Read the script first if you'd rather not pipe to a shell; it's short.

**With cargo** (any platform with Rust):

```bash
cargo install --git https://github.com/3monthsfree/instagram-reel-recorder
```

**From source:**

```bash
git clone https://github.com/3monthsfree/instagram-reel-recorder
cd instagram-reel-recorder
cargo build --release
./target/release/reel2mp3
```

**Arch users** can build a package with the included `packaging/PKGBUILD`:

```bash
cd packaging && makepkg -si
```

## Using it

```
reel2mp3 [--cookies BROWSER] [--out DIR] [URL…]
         browsers: chromium, firefox, chrome, brave
```

MP3s are saved to the folder you run it in, or wherever `--out` points.

| key | does |
|-----|------|
| paste (ctrl+shift+v) | stage every link in the pasted text |
| `enter` | start everything staged · `v` stage from clipboard · `a` type one |
| `d` | drop the selected staged link · `x` clear staging |
| `tab` | switch between the add and queue panels |
| `↑ ↓` | pick a row · `p` play it · `o` open the folder |
| `r` | retry the selected row · `R` retry every failure |
| `d` | in the queue: cancel a running row, or remove a finished one |
| `c` | cycle cookies · `- +` fewer or more downloads at once (default 2) |
| `q` | quit |

The mouse works too: click **start**, click a row, scroll either list.

### Reading the speed panel

A reel's audio is about 1 MB and arrives in roughly 30ms, so raw
bytes-per-tick is a spike rather than a curve. The panel low-passes that into a
flow rate (the area under the wave is still the bytes moved) and draws it as
a braille waveform over a 15-second window. Behind it, in rose, is how many
workers are busy; that one stays up through the ffmpeg convert phase, when no
bytes are moving at all. A flat violet wave over a full rose one means the
files were already on disk.

## Troubleshooting

**A row fails with "Instagram sent an empty media response".** Instagram wants a
logged-in session for that post. Press `c` until the cookie source matches your
browser, then `r` to retry the row.

**Cookies still don't work.** The browser name has to match the profile on disk,
not the brand you think you use. Chrome and Chromium are different browsers with
different profile folders, so if yours lives in `~/.config/chromium`, pick
`chromium`. Snap and Flatpak builds hide their profiles somewhere yt-dlp usually
cannot reach.

**Rows finish instantly as `have` and nothing downloads.** Those files are
already in the output folder, so yt-dlp skips the transfer and reuses what is on
disk. Delete the MP3 or point `--out` elsewhere for a fresh copy.

**The speed wave is flat while the load wave is full.** Same cause as above, or
the jobs are in the ffmpeg stage, where no bytes move.

**Everything breaks at once.** Instagram changed something. Update yt-dlp first:
`sudo pacman -Syu yt-dlp`, or `yt-dlp -U` if you installed it outside your
package manager. Most breakage is fixed upstream within days.

**Downloads start failing partway through a long queue.** That is rate limiting.
Press `-` to run fewer at once, and use cookies.

**Pasting does nothing.** Your terminal may not send bracketed paste. Press `v`
to pull the clipboard instead, which needs `wl-paste`, `xclip` or `xsel`.

**You cannot select text with the mouse.** The app captures the mouse so the
start button and rows are clickable. Hold Shift while dragging, the standard
override in most terminals.

**The side panels are missing.** Below 108 columns the layout falls back to
queue plus details. Widen the window.

**The waveform draws as boxes or question marks.** Your font has no braille
glyphs. DejaVu Sans Mono, Fira Code, JetBrains Mono and any Nerd Font build
cover them.

**Colors look flat.** The panels need a truecolor terminal. Konsole, GNOME
Terminal, Alacritty, Kitty, WezTerm and foot all qualify; a bare Linux console
does not.

**The desktop launcher flashes and closes.** Usually a missing yt-dlp or ffmpeg.
Run `reel2mp3` from a terminal to read the error.

**Where did the files go?** Into the folder you started it in, or `--out`. Press
`o` to open that folder and `p` to play the selected row.

Still stuck? Open an issue with your yt-dlp version (`yt-dlp --version`), your
distro, and whatever the info panel showed for the failing row.

## Desktop launcher

`packaging/reel2mp3.desktop` adds it to your app menu. The installer script
does this for you; by hand:

```bash
install -Dm644 packaging/reel2mp3.desktop ~/.local/share/applications/reel2mp3.desktop
```

Edit `Exec=` in that file to set the download folder and your browser for
cookies.

## Asking an AI assistant to set it up

Paste this into Claude Code, Codex, Cursor or similar:

> Install reel2mp3 from https://github.com/3monthsfree/instagram-reel-recorder.
> Check whether yt-dlp and ffmpeg are already installed and install whichever is
> missing with my system's package manager. Then install reel2mp3 itself, using
> the release binary from install.sh if I have no Rust toolchain and
> `cargo install --git` otherwise. Add the desktop launcher, set it to save MP3s in
> ~/Music/reels, and tell me how to launch it.

## How it works

One `yt-dlp` process per job, in its own process group so a cancel takes ffmpeg
with it. Progress is read from tagged `--progress-template` and `--print` lines
instead of scraped from yt-dlp's human output, which keeps parsing stable
across versions.

| file | holds |
|------|-------|
| `src/main.rs` | state, keys, mouse, scheduling |
| `src/job.rs` | spawning yt-dlp and parsing its output |
| `src/ui.rs` | the five panels |
| `src/graph.rs` | braille waveforms and sparklines |
| `src/theme.rs` | palette |

## Fair use

Made for saving audio you have the right to keep: your own posts, things you
have permission to use, or personal offline listening where that's allowed
where you live. Respect the rights of whoever made what you're downloading.

## License

MIT. See [LICENSE](LICENSE).
