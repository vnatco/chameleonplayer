# Chameleon Player

A cover-first music player for Windows. The album cover is the player, and the whole interface takes its colors from the artwork: each edge of the cover lights up the desktop around it, and the app changes color with every track.

> **Status:** rewrite in progress. The engine (playback, library, covers, tag editing) works; the final interface is being designed. The current screen is a temporary test bench.
>
> The original 2017 WinForms version lives on the [`legacy`](../../tree/legacy) branch and the `v1-legacy` tag.

## Features

- **Chameleon colors.** A palette is extracted from every cover (in OKLab, so colors don't turn muddy), with text and accent colors checked against WCAG contrast. Each side of the cover is sampled separately for the ambient glow.
- **Music library.** Add folders; they are scanned in parallel, kept up to date automatically, and rescanned incrementally. Unplugged drives keep their songs.
- **Covers first.** Embedded art or `cover.jpg`/`folder.jpg`, cached once per unique image. A Missing Covers view lists everything without art.
- **Safe tag editing.** Title, artist, album, album artist, genre, year, track, disc and cover art. Changes are written to a copy, verified, then swapped in, so a failed save never damages the file. Works on the song that is currently playing.
- **Gapless playback** of MP3, M4A/AAC/ALAC, FLAC, Ogg Vorbis, WAV and AIFF.
- **Windows integration.** Media keys and the Windows media overlay, single instance, open files or folders from Explorer or by dropping them on the window.

Opus, APE, WMA, WavPack, Musepack and DSD files are not supported yet; scans report how many were skipped.

## Building

Requirements: Windows 10/11, [Rust](https://rustup.rs) (stable, MSVC), [Node.js](https://nodejs.org) 20+, and the WebView2 runtime (included with Windows 11).

```bash
npm install
npm run tauri dev      # run in development
npm run tauri build    # build the installer
```

Tests:

```bash
cd src-tauri
cargo test
```

`cargo run --example make_fixtures -- <folder>` generates a small test library of tagged tone files with covers chosen to stress the color extraction.

## Project Layout

| Path | What |
| --- | --- |
| `src-tauri/src/audio` | Playback engine (rodio/symphonia), gapless queue, loudness meter for the glow pulse |
| `src-tauri/src/palette` | Cover to palette extraction and color math |
| `src-tauri/src/library` | SQLite library, folder scanner, cover cache |
| `src-tauri/src/tags.rs` | Tag reading and safe writing (lofty) |
| `src-tauri/src/media.rs` | Windows media overlay and media keys |
| `src-tauri/src/watcher.rs` | Folder watching |
| `src-tauri/src/commands.rs` | Commands and events used by the interface |
| `src/` | Interface (SvelteKit + TypeScript) |

App data (library database, cover cache, settings, logs) is stored in `%LOCALAPPDATA%\co.vnat.chameleon`.

## License

[Apache 2.0](LICENSE)
