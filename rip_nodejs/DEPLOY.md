# Deploying rip-receipt to npm

Native Node.js addon with prebuilt binaries for 6 platforms. Users don't need Rust.

## How it works

The npm package `rip-receipt` has `optionalDependencies` on platform-specific packages. When a user runs `npm install rip-receipt`, npm only downloads the binary for their OS/arch:

| npm package | Platform |
|---|---|
| `rip-receipt-win32-x64-msvc` | Windows x64 |
| `rip-receipt-darwin-x64` | macOS Intel |
| `rip-receipt-darwin-arm64` | macOS Apple Silicon |
| `rip-receipt-linux-x64-gnu` | Linux x64 (glibc) |
| `rip-receipt-linux-arm64-gnu` | Linux ARM64 (glibc) |
| `rip-receipt-linux-x64-musl` | Linux x64 (Alpine/musl) |

The auto-generated `index.js` detects the platform and loads the right `.node` binary.

## CI pipeline

Everything is automated via GitHub Actions (`.github/workflows/nodejs.yml`):

- **On every push/PR to main**: builds all 6 targets, runs tests on native hosts
- **On version tag (`v*`)**: builds, tests, and publishes to npm

### What CI does

1. **Build** — matrix of 6 targets across Windows, macOS, and Ubuntu runners
2. **Test** — downloads built artifacts, runs `test_smoke.mjs` and `test_resources.mjs` on native hosts (+ Alpine Docker for musl)
3. **Rust tests** — full `cargo test --workspace` to catch regressions
4. **Publish** — only on `v*` tags: moves artifacts into platform package dirs, publishes all packages to npm

## Publishing a new version

### One-time setup

1. Create an npm access token at https://www.npmjs.com/settings/tokens
2. Add it as a GitHub Actions secret named `NPM_TOKEN`

### Each release

```bash
# 1. Bump version in package.json (and optionally Cargo.toml)
cd rip_nodejs
# edit package.json version

# 2. Commit the version bump
git add package.json
git commit -m "0.2.0"

# 3. Tag and push
git tag v0.2.0
git push && git push --tags
```

CI builds all 6 platforms, tests them, and publishes to npm. Check the Actions tab for progress.

### What gets published

The `napi prepublish` command handles publishing the 6 platform packages. Then `npm publish` publishes the root `rip-receipt` package which references them as `optionalDependencies`.

## Local development

For local testing, you don't need the full CI pipeline:

```bash
cd rip_nodejs
npm install

# Build for current platform
npm run build

# Test
node test_smoke.mjs
node test_resources.mjs
```

The `--platform` flag in the build script names the output `rip-nodejs.<platform>.node` (e.g., `rip-nodejs.win32-x64-msvc.node`). The auto-generated `index.js` tries the local file first before falling back to the npm package.

## Regenerating platform dirs

If you add or remove targets in `package.json`, regenerate the `npm/` directory:

```bash
cd rip_nodejs
npx napi create-npm-dirs
```

## Files overview

| File | Checked in | Published | Notes |
|---|---|---|---|
| `src/lib.rs` | yes | no | Rust source |
| `Cargo.toml` | yes | no | Rust config |
| `build.rs` | yes | no | napi-rs build script |
| `index.js` | generated | yes | Platform loader (napi-rs) |
| `index.d.ts` | generated | yes | TypeScript types (napi-rs) |
| `npm/` | generated | yes | Platform package dirs |
| `rip-nodejs.*.node` | no | yes (in platform pkgs) | Native binaries |
| `README.md` | yes | yes | npm page |
| `package.json` | yes | yes | Package metadata |
| `test_smoke.mjs` | yes | no | Tests |
| `test_resources.mjs` | yes | no | Tests |

## Troubleshooting

**"Cannot find native binding"** — run `npm run build` to build for your current platform.

**CI build fails on Linux targets** — the GNU targets use `--use-napi-cross` (napi-rs cross-compilation sysroot). Musl targets use `-x` (cargo-zigbuild). Make sure zig is installed for musl.

**Publish fails** — check that `NPM_TOKEN` is set in GitHub Actions secrets and has publish access.

**Version mismatch error** — platform package versions must match the root package version. The `napi version` script can sync them. CI handles this automatically.
