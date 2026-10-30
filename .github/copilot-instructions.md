# Harper desktop implementation guidance

This branch is based on German-support PR #3402 at commit `75c77b497b10cad3b79251a198de410a4b68418b`. Read root `AGENTS.md` and `AGENT_POLICY.md` before changing code; language-module work also follows `harper-core/src/language/AGENTS.md`.

## Architecture

- The desktop application combines a Svelte 5/TypeScript/Vite frontend in `harper-desktop/` with a Rust/Tauri 2 backend in `harper-desktop/src-tauri/`. Tauri commands are the IPC boundary.
- Shared language behavior is implemented in `harper-core/`. The desktop backend owns persisted configuration and resolves the macOS locale; the frontend must display and update that backend state without reducing it to an English-only model.
- The Rust `Language` model supports BCP-47 parsing. German variants are `GermanDialect::Standard` (`de-DE`), `Austrian` (`de-AT`), and `Swiss` (`de-CH`); the desktop Cargo default `multilingual` feature enables German along with Portuguese, Slovak, and Polish.
- macOS artifacts are built through the root `justfile`. Universal builds require both `aarch64-apple-darwin` and `x86_64-apple-darwin`; the unsigned universal DMG is the expected PR artifact. Signing, notarization, and updater signing are release-only operations that require externally configured secrets.

## Commands

Run commands from the repository root unless a directory is included:

```bash
# Frontend type/Svelte validation
pnpm --dir harper-desktop check

# Frontend tests
pnpm --dir harper-desktop test
# Run one frontend test
pnpm --dir harper-desktop test -- <test-file-or-name>

# Desktop Rust formatting, clippy, and checks
just check-desktop

# Rust test suite
just test-rust
# Run one Rust test from the relevant crate directory
(cd harper-desktop/src-tauri && cargo test <test_name>)

# Universal, unsigned macOS application and DMG
rustup target add aarch64-apple-darwin x86_64-apple-darwin
just build-desktop-macos-unsigned
```

Use `just build-desktop-macos-arm64` for Apple Silicon-only artifacts and `just build-desktop-macos` for the signed/universal recipe when signing prerequisites are configured. After a universal build, locate artifacts rather than assuming paths:

```bash
find harper-desktop/src-tauri/target -name '*.dmg' -o -name 'Harper.app'
lipo -archs "$APP/Contents/MacOS/Harper"
```

## Language-selection conventions

- Do not use `harper.js` English `Dialect` integer values as the desktop-wide language setting. Expose a stable, explicit Tauri IPC DTO that represents `system` versus `explicit` selection and BCP-47 language/region values.
- Never silently map an unknown backend language to American English. Preserve the previous valid frontend state and surface an error if a backend response or user input cannot be represented.
- Resolve `system` from `tauri_plugin_os::locale()` and `parse_language()` on startup. Explicit user selections must persist across system locale changes.
- Preserve compatibility with configurations that store the old `dialect: Language` field. Migrate those values to an explicit selection and cover the migration with tests.
- Validate all public command inputs in Rust. Do not `unwrap()` untrusted string-to-enum conversions.
- The settings UI labels the control **Language**, offers Automatic/System plus the supported English and German regional variants, disables it while loading or saving, and commits UI state only after the backend write succeeds.

## Validation and CI expectations

- Language tests must prove that the configured language affects the actual Harper dictionary/rules, not only the selected UI value. Reuse real fixtures and rules in `harper-core/src/language/german/tests/`.
- Cover system resolution for `de-DE`, `de-AT`, and `de-CH`; explicit German and all existing English variants; invalid DTOs/locales; persistence/reload; and legacy configuration migration.
- Desktop frontend tests must cover DTO-to-selector mapping, selector-to-DTO mapping, system mode, all German variants, and the absence of an unknown-language-to-American fallback.
- PR workflow files belong at root `.github/workflows/`, not under `harper-desktop/.github/workflows/`. PR builds must remain unsigned and must not access release secrets.
- The macOS CI build should run desktop checks and tests, build `just build-desktop-macos-unsigned`, assert both `arm64` and `x86_64` with `lipo`, mount the DMG to confirm it contains `Harper.app`, and upload the DMG plus checksum.
- Do not add Apple credentials, Tauri private signing keys, or upstream Fastlane team/S3 configuration to the repository. Accessibility permission remains a manual macOS TCC step; notarization does not grant it.
