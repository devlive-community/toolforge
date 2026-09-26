<div align="center">
<img src="apps/desktop/src-tauri/icons/logo.svg" width="96" />

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
| **JSON Formatter** | Format, minify, validate, escape / unescape, tree view and structural diff; JSON5 support; errors pinpointed to line and column; tree view loads large documents on demand |
| **XML Formatter** | Format, minify and validate; keeps comments, CDATA and declarations; unclosed or mismatched tags reported with line and column |
| **SQL Formatter** | Beautify and minify; keyword casing, indentation and compact mode; generic SQL / PostgreSQL / SQL Server dialects |
| **Regex Tester** | Live match and group highlighting with replacement preview; lookaround, backreferences and named groups; common pattern presets |
| **Timestamp** | Seconds / milliseconds / microseconds / nanoseconds detected automatically; multiple time zones side by side; date strings to timestamps; live clock |
| **UUID Generator** | Batch generate UUID v1 / v3 / v4 / v5 / v7, ULID and NanoID; inspect versions and embedded timestamps |
| **Hash** | MD5, SHA-1, SHA-2, SHA3, SM3 and CRC32; live text hashing and HMAC with verification; drop files to hash them in a background task with live logs |
| **Encoder** | Base64 / Base64URL / Base32 / Hex, URL (component, full, form), HTML entities and Unicode escapes in both directions; files to Base64 or Data URIs |
| **JWT Decoder** | Decode headers and payloads, check expiry, verify HS / RS / PS / ES / EdDSA signatures and sign new tokens |
| **Certificate Viewer** | Inspect PEM / DER certificates and chains (names, SANs, validity, key, usages, fingerprints), or fetch the chain a server presents and verify it with the system trust store, showing the TLS version and cipher |
| **Encryption** | AES (GCM / CBC / CTR / ECB), SM4 and ChaCha20-Poly1305 with hex, Base64, text or PBKDF2 keys; RSA key generation, OAEP / PKCS#1 encryption and PKCS#1 / PSS signatures, compatible with OpenSSL |
| **Text Tools** | 13 naming styles and letter cases; trim, dedupe, sort (natural, by length), shuffle and number lines; character, word, CJK and reading-time statistics |
| **Text Diff** | Line, word or character diff with side-by-side highlighting, ignore case / whitespace and unified patch export |
| **Format Converter** | JSON ⇄ YAML ⇄ TOML ⇄ CSV with automatic detection, preserved key order and CSV delimiter / header / type options |
| **Base Converter** | Arbitrary-precision integers in bases 2–36, prefix detection, two's complement for 8–128 bits and a clickable 64-bit view |
| **Color Converter** | HEX / RGB / HSL / HSV / HWB / CMYK / Lab / OKLCH, WCAG contrast checks, tints, shades and harmonies |
| **Password Generator** | Cryptographically secure passwords with per-type options and exclusions, plus a local strength and crack-time check |
| **QR Code** | Generate QR codes as PNG / SVG with error correction, size and colors; read QR codes from images |
| **Background Remover** | Remove image backgrounds with a local AI model (U²-Net lite or IS-Net) running in Rust; models download on first use with resume and SHA-256 verification; transparent PNG or a solid color |
| **OCR** | Extract Chinese and English text from images and screenshots with PaddleOCR PP-OCRv4 models running in Rust (about 15 MB, downloaded on first use); paste screenshots with ⌘V, see recognized boxes and copy lines |
| **IP Calculator** | IPv4 / IPv6 subnets: network, broadcast, host range, counts, address type and reverse DNS; subnet splitting, membership check and range to CIDR |
| **HTTP Client** | Send requests with params, headers and JSON / text / form bodies; status, timing, headers and pretty-printed JSON; save responses and copy as cURL |
| **DNS Lookup** | A / AAAA / CNAME / MX / TXT / NS / SOA / SRV / CAA / PTR records from the system resolver or public DNS, compared side by side with timings; flags proxy fake IPs |
| **Port Manager** | See which process listens on each TCP / UDP port (with user, memory and command line), search, stop the process, and check remote ports in parallel with latency and failure reasons |
| **Local Server** | Serve a folder as a website on localhost or the local network (with QR codes) and watch every request live; directory listing, SPA fallback, CORS, byte ranges, dotfiles hidden and paths locked to the folder |
| **System Monitor** | Live CPU, memory, disk, network and temperature readings plus a searchable process list |
| **Unit Converter** | 14 categories including length, weight, temperature, area, volume, data size and rate, pressure, energy and power, plus traditional Chinese units |
| **Image Compressor** | Batch compress JPEG (mozjpeg), PNG (dithered palette with transparency, or lossless oxipng) and WebP offline; resize, strip EXIF, keep color profiles, never overwrite, and compare with the original using a drag divider |
| **Image Converter** | Batch convert PNG / JPEG / WebP / GIF / BMP / ICO / TIFF, resize by percent or bounding box, JPEG quality; runs as a task with live logs and never overwrites files |
| **Cron Expression** | Explain 5 / 6 / 7-field cron expressions field by field, preview upcoming runs in any time zone, with `L`, `W`, `#` and aliases such as `@daily` |
| **JSON to Code** | Generate TypeScript, Rust (serde), Go, Java records, Kotlin, Python (pydantic) and C# models from JSON samples, merging arrays and detecting optional and nullable fields |
| **cURL to Code** | Convert curl commands (including browser “Copy as cURL” in bash or cmd format) into JavaScript fetch, Python requests, Go, Rust reqwest, Java HttpClient, PHP and C# code, listing anything that cannot be converted |
| **Mock Data** | Generate up to 100,000 rows of realistic Chinese or English test data (consistent names, emails, valid ID numbers, real cities and 35 field types) as JSON, CSV or SQL INSERT, reproducible with a seed |
| **CSV Viewer** | Open CSV / TSV files up to 1 GB (a million rows in under a second) with automatic encoding, delimiter and header detection; sort, search, filter, column statistics and export to CSV / TSV / JSON / Markdown |
| **Image Info** | Format, dimensions, color type, EXIF camera settings and GPS location with a map link, dominant colors; strip EXIF / XMP / IPTC from JPEG and PNG losslessly |
| **Markdown Editor** | GitHub Flavored Markdown with live preview rendered in Rust, outline, word count, formatting toolbar and shortcuts; drafts are saved automatically; export standalone HTML |

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
