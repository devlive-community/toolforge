<div align="center">
<img src="apps/desktop/src-tauri/icons/logo.png" width="96" />

<h1>ToolForge</h1>

**A fast, secure, lightweight and pluggable desktop toolbox for developers and power users**

Built with Tauri 2 and Rust: all data is processed locally in Rust, every tool is an independent plugin,
and the modern interface supports light / dark themes, Chinese and English, and in-app updates.

<p>
  <a href="https://github.com/devlive-community/toolforge/releases"><img alt="release" src="https://img.shields.io/github/v/release/devlive-community/toolforge?include_prereleases&label=release" /></a>
  <img alt="platform" src="https://img.shields.io/badge/platform-macOS%20%7C%20Windows%20%7C%20Linux-555" />
  <img alt="stack" src="https://img.shields.io/badge/Tauri%202-React%2019-FFC131" />
  <a href="https://github.com/devlive-community/toolforge/actions/workflows/ci.yml"><img alt="ci" src="https://github.com/devlive-community/toolforge/actions/workflows/ci.yml/badge.svg" /></a>
  <img alt="license" src="https://img.shields.io/badge/license-MIT-green" />
</p>

<p><strong>English</strong> · <a href="README.zh-CN.md">简体中文</a></p>
</div>

---

## ✨ Features

- **Your data stays local**: parsing, formatting, encoding, hashing and file I/O all run in Rust. The frontend only renders results, and nothing leaves your machine.
- **Everything is a plugin**: every tool, built-in ones included, is an independent plugin with its own manifest, Rust backend, React UI and translations. The host hard-codes no tools.
- **Live feedback for long tasks**: heavy work such as hashing large files runs as a background task with live progress and logs, and can be cancelled at any time. The task center keeps the history and logs.
- **Modern interface**: semantic design tokens, light / dark themes, custom components and window controls, a ⌘K command palette, favorites and recently used tools.
- **Clipboard-aware ⌘K**: open the command palette and it recognizes what you copied (JSON, JWT, timestamps, UUIDs, colors, curl commands, cron expressions, IPs, SQL, spreadsheet tables, certificates, screenshots and more) and offers the right tool, opened with the content already filled in. Detection runs locally in Rust and can be turned off.
- **Quick launch**: a global shortcut (⌥Space / Alt+Space by default, configurable) brings ToolForge forward with the command palette from any app; background mode keeps it in the menu bar or system tray.
- **Internationalized**: Simplified Chinese and English; plugins ship their own translations, and Rust only returns error codes that the UI translates.
- **On-demand resources**: large assets such as AI models are downloaded only when a tool needs them, from verified sources with resume support, and can be removed at any time.
- **In-app updates**: delivered through GitHub Releases with signed update packages; versions can be skipped and automatic checks turned off.
- **SQLite only for local storage**: preferences, favorites, usage and task history are stored in a local database.

## 🧰 Built-in tools

| Tool | Description |
|------|-------------|
| **Regex Tester** | Live match and group highlighting with replacement preview; lookaround, backreferences and named groups; common pattern presets |
| **Hash** | MD5, SHA-1, SHA-2, SHA3, SM3 and CRC32; live text hashing and HMAC with verification; drop files to hash them in a background task with live logs |
| **JWT Decoder** | Decode headers and payloads, check expiry, verify HS / RS / PS / ES / EdDSA signatures and sign new tokens |
| **Certificate Viewer** | Inspect PEM / DER certificates and chains (names, SANs, validity, key, usages, fingerprints), or fetch the chain a server presents and verify it with the system trust store, showing the TLS version and cipher |
| **Encoding Converter** | Detect the encoding of many files at once (BOM, UTF-16, UTF-8, GBK, Big5, Shift_JIS, …) and their line endings; preview a file in any encoding to fix wrong guesses; convert to UTF-8 or another encoding and LF / CRLF in place with backups or into a folder |
| **Batch Rename** | Rename many files with chained rules (replace / regex, insert, remove, case, numbering, templates with dates and EXIF capture time), a live conflict-checked preview, safe two-phase renaming and undo |
| **Archive Manager** | Browse, preview and extract ZIP, 7z, TAR and tar.gz / bz2 / xz / zst archives (encrypted ZIP and 7z included) with path-traversal protection, and create ZIP, 7z, tar.gz or tar.xz archives with AES-256 passwords — all in pure Rust |
| **Document Converter** | Excel → Word and Word → Excel in pure Rust (sheets become tables and tables become sheets, numbers stay numbers), plus Word → PDF and PDF → Word through a locally installed LibreOffice, found automatically; batch conversion that never overwrites |
| **SSH Keys** | Generate Ed25519, ECDSA and RSA keys in the OpenSSH format with passphrases, inspect keys, authorized_keys and known_hosts with SHA256 / MD5 fingerprints and randomart identical to ssh-keygen, convert PEM RSA keys and change passphrases |
| **QR Code** | Generate QR codes as PNG / SVG with error correction, size and colors; read QR codes from images |
| **Barcode** | Generate EAN-13, EAN-8, UPC-A, ITF-14, Code 128, Code 39, Code 93, Codabar, Data Matrix, PDF417 and Aztec codes; retail check digits are added or verified automatically, EAN/UPC are drawn with guard bars and grouped digits; save as SVG or PNG, and read every barcode in an image or a clipboard screenshot |
| **Background Remover** | Remove image backgrounds with a local AI model (U²-Net lite or IS-Net) running in Rust; models download on first use with resume and SHA-256 verification; transparent PNG or a solid color |
| **OCR** | Extract Chinese and English text from images, screenshots and PDFs with PaddleOCR PP-OCRv4 models running in Rust (about 15 MB, downloaded on first use); paste screenshots with ⌘V, see recognized boxes and copy lines; PDFs are rendered and recognized page by page (up to 200 pages) |
| **WebSocket Client** | Connect to ws:// and wss:// servers with custom headers and subprotocols, watch every received and sent frame live, format JSON, and send text, hex / base64 binary or pings with saved snippets |
| **DNS Lookup** | A / AAAA / CNAME / MX / TXT / NS / SOA / SRV / CAA / PTR records from the system resolver or public DNS, compared side by side with timings; flags proxy fake IPs |
| **Port Manager** | See which process listens on each TCP / UDP port (with user, memory and command line), search, stop the process, and check remote ports in parallel with latency and failure reasons |
| **Local Server** | Serve a folder as a website on localhost or the local network (with QR codes) and watch every request live; directory listing, SPA fallback, CORS, byte ranges, dotfiles hidden and paths locked to the folder |
| **System Monitor** | Live CPU, memory, disk, network and temperature readings plus a searchable process list |
| **Input Stats** | Count daily keystrokes, mouse clicks, travel distance and scrolling in the background, with a keyboard heatmap (macOS or PC, laptop or full size), favorite shortcuts, a per-display screen click heatmap and active time per app; only counts are kept, never what you type (macOS needs Input Monitoring; Linux requires X11) |
| **Disk Usage** | Scan a folder or disk in parallel and see what takes up space in a zoomable treemap colored by file type, with folder contents, largest files and a type breakdown; move items to the Trash |
| **Duplicate Finder** | Find files with identical content across folders (size → first 64 KB → full BLAKE3 hash, in parallel), ignore hard links, preselect extra copies by date, path or folder and move them to the Trash after re-verifying |
| **Folder Compare** | Compare two folders by size and time or by content, with ignore patterns; browse the differences as a tree, see changed text files side by side and copy files or folders to the other side, keeping modification times |
| **Context Menu Manager** (Windows) | List the right-click menu entries for files, folders, folder background, desktop and drives; disable and enable them without deleting anything, with a double-click .reg undo file for every change; switch the Windows 11 classic menu |
| **Image Compressor** | Batch compress JPEG (mozjpeg), PNG (dithered palette with transparency, or lossless oxipng) and WebP offline; resize, strip EXIF, keep color profiles, never overwrite, and compare with the original using a drag divider |
| **Image Stitcher** | Combine images in the order added into a row, a column or a grid with spacing and a background (transparent, solid or custom color); PNG and WebP keep transparency, JPEG flattens to white; runs as a task with live logs and never overwrites files |
| **App Icon Generator** | Turn one image or SVG into icon sets for the web (favicon.ico, PNGs, webmanifest and HTML snippet), iOS (Xcode AppIcon.appiconset), Android (mipmaps and round icons), macOS (.icns) and Windows (multi-size .ico), with padding, background color and corner radius; or drop the image into 37 templates such as folders, drives, documents, disc cases, photos and cartridges, export PNG / ICNS / ICO and set it as a folder icon (on macOS also on drives and any file), with one click to restore the default |

More tools are on the way.

## 📦 Download

Grab the installer for your platform from [Releases](https://github.com/devlive-community/toolforge/releases):

| Platform | Installer |
|----------|-----------|
| macOS (Apple Silicon / Intel) | `.dmg` |
| Windows | `.msi` / `-setup.exe` |
| Linux | `.AppImage` / `.deb` / `.rpm` |

> Builds are not code-signed yet. If macOS says the app is damaged or from an unidentified developer, right-click it and choose **Open**, or run:
>
> ```bash
> xattr -cr /Applications/ToolForge.app
> ```
>
> If Windows SmartScreen appears, choose **More info → Run anyway**. In-app updates are not affected.

## 🛠 Development

### Requirements

- [Node.js](https://nodejs.org/) 22+ and [pnpm](https://pnpm.io/) 10+
- [Rust](https://rustup.rs/) stable (edition 2024)
- Tauri 2 [system prerequisites](https://tauri.app/start/prerequisites/) (Linux needs `libwebkit2gtk-4.1-dev` and friends)

### Common commands

```bash
pnpm install          # install frontend dependencies
pnpm dev              # run the desktop app in development mode with hot reload
pnpm build            # build installers
pnpm icons:generate   # rebuild desktop / tray icons from the selected TF logo PNG

cargo xtask check     # run exactly the checks CI runs
cargo xtask check rust|web|rules   # run a subset
pnpm lint             # ESLint (zero warnings)
```

`cargo xtask check` runs, in order: convention rules → rustfmt → clippy → Rust tests → TypeScript type checking → ESLint → frontend build.

### Plugin layout

```
plugins/<name>/
├── manifest.json          # id (org.devlive.toolforge.<name>), category, functions, permissions
├── icon.svg
├── locales/en-US.json     # UI text, log and error code translations
├── backend/               # Rust: implements ToolPlugin; all data processing lives here
└── ui/                    # React: presentation only, calls the backend through usePlugin / useTask
```

- Regular functions are invoked with `call()` for instant results. Functions declared with `"task": true` in the manifest must run as tasks and report logs and progress, and honor cancellation, through `TaskContext`.
- Plugin UIs may only use `@toolforge/ui` components and semantic tokens; files, clipboard and other capabilities go through the SDK's `host`.

### Conventions

- Data processing must happen in Rust. The frontend must not pull in data processing libraries or use browser storage (enforced by ESLint and the convention scanner).
- Styles use semantic tokens only: no raw palette classes and no `dark:`. Native form controls and dialogs are not used.
- All UI text goes through i18n; Rust returns only error codes and parameters.
- Rust tests live in dedicated `xxx_test.rs` files.
- Commits follow [Conventional Commits](https://www.conventionalcommits.org/), one commit per feature.

## 🚀 Releasing

```bash
scripts/release.sh --dry-run        # preview the release plan and notes
scripts/release.sh                  # release the current version
scripts/release.sh minor --wait     # bump the minor version, release and wait for CI
scripts/release.sh 1.0.0 --next none
scripts/release.sh --help           # all options
```

The release script checks the repository state, runs every check, creates an annotated tag whose message is the version's commit history, and pushes it.
GitHub Actions then builds the macOS / Windows / Linux installers and, once every platform succeeds, publishes the release together with the update manifest.
Finally the script bumps the project to the next development version and commits it.

Before releasing, add the updater signing key to the **repository** secrets as `TOOLFORGE_UPDATER_PRIVATE_KEY` (the private key file content, paired with the public key in `tauri.conf.json`), plus `TOOLFORGE_UPDATER_KEY_PASSWORD` if the key has a password.

## 📄 License

[MIT](https://opensource.org/licenses/MIT) © devlive-community
