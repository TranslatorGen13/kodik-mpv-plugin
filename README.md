# kodik-mpv-plugin

MPV plugin that resolves indirect video links into direct playback URLs. It also integrates with [Shikimori](https://shikimori.one), including episode tracking with cookies.

This plugin is part of the [`kodik` Rust workspace](https://github.com/TranslatorGen13/kodik), alongside `kodik-rs` (CLI), `kodik-parser`, `kodik-utils`, and `kodik-shiki`.

## Installation

### Prerequisites

- Rust 2024 edition
- MPV media player
- A public Kodik API token (`KODIK_TOKEN`) for compiling the Kodik API client

### Build

The plugin depends on workspace crates and configuration from the `kodik` repository, so it cannot be built by cloning this repository alone. Clone the workspace with its submodules, then build the plugin from the workspace root:

```bash
git clone --recurse-submodules https://github.com/TranslatorGen13/kodik.git
cd kodik
KODIK_TOKEN=your_public_api_token cargo build --release -p kodik-mpv-plugin
```

`KODIK_TOKEN` is read at compile time by `kodik-parser` and embedded in the resulting binary. Replace `your_public_api_token` with your public Kodik API token. The token is needed to compile the workspace; it does not need to be set when running the plugin. Do not use a private credential here.

In PowerShell, set the variable for the build like this:

```powershell
$env:KODIK_TOKEN = "your_public_api_token"
cargo build --release -p kodik-mpv-plugin
```

The shared library is placed in `target/release/` (`kodik.dll` on Windows, `libkodik.so` on Linux, `libkodik.dylib` on macOS). For MPV installations that expect the name `kodik.so`, rename the Linux library accordingly.

### Setup

1. Copy the built shared library to MPV's `scripts/` directory:
   - Linux/macOS: `~/.config/mpv/scripts/`
   - Windows: `%APPDATA%\mpv\scripts\`

   MPV auto-loads `.so` / `.dll` files in that directory.

2. (Optional) Create a config file at `script-opts/kodik.conf`:

   ```ini
   # Specify video quality [default: 720] [possible values: 360, 480, 720]
   quality=720

   # Netscape formatted file to read cookies from (needed for media database integration)
   # cookies=~/cookies.txt

   # Specify translation title (regex)
   # translation_title=Subtitles

   # Specify translation type [possible values: voice, subtitles] (fallback if translation_title not found)
   # translation_type=voice

   # Expand a media database URL into all related URLs [default: none] [possible values: all, essential, none]
   related_mode=none
   ```

### Usage

```bash
mpv https://kodikplayer.com/video/91875/013ac13bfd06b08fabaefddce91a7107/720p
mpv https://shikimori.one/animes/z12345-anime-title
```

### Key bindings

Add to your MPV `input.conf`:

```text
Ctrl+ENTER script-binding "kodik/watched"
```

Or use the `input.conf` shipped with the plugin.

### Android installation guide

To use this plugin on mpv-android, you will need to manually inject the library into the APK package.

1. Download the latest official mpv-android APK and an APK editor/decompiler tool of your choice.
2. Decompile the APK.
3. Copy the Android-compiled plugin (`kodik.so`) into `lib/arm64-v8a/kodik.so` in the decompiled structure.
4. Recompile, sign, and install the modified APK.
5. Open mpv, navigate to **Settings → Advanced → Edit mpv.conf**, and append `script=kodik.so`.
6. (Optional) To use cookies, place `cookies.txt` in the application's media folder and link it via `mpv.conf`, for example: `script-opts=kodik-cookies=/sdcard/Android/media/is.xyz.mpv/cookies.txt`.
