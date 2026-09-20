<div align="center">

# reel2mp3

**Paste a wall of Instagram links. Press start. Walk away with MP3s.**

A little control room for reel audio: stage a batch, watch it run, keep the
sound and bin the video.

[![ci](https://github.com/3monthsfree/instagram-reel-recorder/actions/workflows/ci.yml/badge.svg)](https://github.com/3monthsfree/instagram-reel-recorder/actions/workflows/ci.yml)
[![release](https://img.shields.io/github/v/release/3monthsfree/instagram-reel-recorder?color=d62976&label=release)](https://github.com/3monthsfree/instagram-reel-recorder/releases/latest)
[![license](https://img.shields.io/badge/license-MIT-9e76e2.svg)](LICENSE)

<img src="https://raw.githubusercontent.com/3monthsfree/instagram-reel-recorder/main/docs/running.webp" width="900" alt="The queue running, with the speed waveform under load">

</div>

Eleven links pasted in one go, two downloading, the rest waiting their turn.
Progress meters per row, a braille waveform of transfer rate with worker load
filling in behind it, and the file sizes as they land.

<img src="https://raw.githubusercontent.com/3monthsfree/instagram-reel-recorder/main/docs/staged.webp" width="900" alt="Eleven links staged, waiting on the start button">

Nothing moves until you press start. Links sit in staging where you can look
them over, drop the ones you did not mean to paste, and fire the batch when you
are ready.

## Features

- **Paste a wall of links.** Every URL in the blob gets staged, however messy
  the text around it is. Keep pasting while downloads run.
- **Nothing starts behind your back.** Links wait in staging until you press
  `enter` or click start, so a stray paste never costs you bandwidth.
- **A dashboard, not a log.** Live progress meters, per-row sparklines, queue
  stats, and a speed waveform that shows load even while ffmpeg is converting.
- **Logged-in when it has to be.** Cycle cookies from Chromium, Firefox, Chrome
  or Brave with one key, then retry the row that failed.
- **Never downloads twice.** Files already in the folder are marked `have` and
  reused instead of fetched again.
- **Tune it while it runs.** Two jobs at a time by default, one to six on a
  keypress, to stay the right side of rate limits.
- **Keys or mouse, your call.** Click the start button and the rows, scroll the
  lists, or never touch the mouse at all.
- **One small binary.** About 1 MB, statically linked, no runtime beyond yt-dlp
  and ffmpeg.

Built with [ratatui](https://ratatui.rs) on top of
[yt-dlp](https://github.com/yt-dlp/yt-dlp) and ffmpeg.

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

## Contributing

Issues and pull requests are welcome. `cargo fmt` and
`cargo clippy --all-targets -- -D warnings` both have to pass, which CI checks
on every push. If you are reporting a download that fails, include the yt-dlp
version and what the info panel said.

## License

MIT. See [LICENSE](LICENSE).
