# Changelog

## [1.0.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-rust-v0.5.3...agent-rdp-rust-v1.0.0) (2026-01-24)


### ⚠ BREAKING CHANGES

* restructure to pnpm workspaces with platform-specific packages ([#33](https://github.com/thisnick/agent-rdp/issues/33))
* The `invoke` automation command has been renamed to `click`.
* Remove click/double-click/right-click/check commands in favor of native UI Automation patterns.
* JS API now uses object-style parameters

### Features

* add --process-timeout flag to automate run command ([fe2ca21](https://github.com/thisnick/agent-rdp/commit/fe2ca2170d518afba94e98971db04b1303a0bdcc))
* add --process-timeout flag to automate run command ([ef0625c](https://github.com/thisnick/agent-rdp/commit/ef0625cd24b6507063b560b6485ffcd1bc86d96c))
* add IPC schema type generation with ts-rs ([#48](https://github.com/thisnick/agent-rdp/issues/48)) ([2f2953a](https://github.com/thisnick/agent-rdp/commit/2f2953a3abe816b71b8d43252af622ac22a5ec5d))
* add OCR-based text location with locate command ([7a90d3c](https://github.com/thisnick/agent-rdp/commit/7a90d3c0ce960e599a8170940279890deb548ee1))
* add WebSocket clipboard support and serve viewer from daemon ([a44d4f9](https://github.com/thisnick/agent-rdp/commit/a44d4f983f743ce2615d2ee71a6dbbacadd50ca6))
* add WebSocket clipboard support and serve viewer from daemon ([058c8c5](https://github.com/thisnick/agent-rdp/commit/058c8c5888f50fb1f7ed6b51def50a8e86f5aacc))
* Add Windows RDPDR backend for drive redirection with associated build and release configurations. ([2143b45](https://github.com/thisnick/agent-rdp/commit/2143b45ca34445693b882697d75feb79853047a5))
* add Windows UI Automation support ([0ebde33](https://github.com/thisnick/agent-rdp/commit/0ebde334fb363ef4b381f4b959068b77880554dc))
* hide PowerShell agent window completely (no taskbar icon) ([3fb273c](https://github.com/thisnick/agent-rdp/commit/3fb273c91d5eaa439beae0c0ade94142b2595e65))
* Implement cross-platform native binary release workflow and runtime wrappers. ([ccf2d92](https://github.com/thisnick/agent-rdp/commit/ccf2d9283d14ecb5df1d37434cc1ddbf9804701d))
* improve CLI, docs, and fix window pattern matching ([ce2e82b](https://github.com/thisnick/agent-rdp/commit/ce2e82ba2e4c8fb37e2e22db87dafa3c4791749f))
* improve CLI, docs, and fix window pattern matching ([a2354f8](https://github.com/thisnick/agent-rdp/commit/a2354f8e05eed7c00be80909f2b245c20da7fc14))
* improve snapshot defaults with depth limit and truncation indicator ([ae01dcb](https://github.com/thisnick/agent-rdp/commit/ae01dcbcf5d3311d6cef8e60470ca81914d14b41))
* improve snapshot defaults with depth limit and truncation indicator ([8d2a84c](https://github.com/thisnick/agent-rdp/commit/8d2a84cc713f8cd1c60768b57b289901e6b9ef0d))
* improve snapshot to enumerate all windows via EnumWindows ([e6c8c40](https://github.com/thisnick/agent-rdp/commit/e6c8c409ef61c869f0c2b659670963bc7f7a6dac))
* rename invoke to click with mouse-based implementation ([#30](https://github.com/thisnick/agent-rdp/issues/30)) ([e265697](https://github.com/thisnick/agent-rdp/commit/e265697d8207429318170a960dfe7520e082563c))
* replace mouse-based automation with native UI Automation patterns ([715f9bb](https://github.com/thisnick/agent-rdp/commit/715f9bbf34e5d666b8f1ca8207a517f628676c0b))
* restructure to pnpm workspaces with platform-specific packages ([#33](https://github.com/thisnick/agent-rdp/issues/33)) ([79f6474](https://github.com/thisnick/agent-rdp/commit/79f647472479fc19db33383278116f03097d30ce))
* set PowerShell agent window title to "agent-rdp automation" ([d59acce](https://github.com/thisnick/agent-rdp/commit/d59acce867c106ed580d2c57b1b5716645503c69))
* v0.2.0 API improvements and release automation ([3fec7b5](https://github.com/thisnick/agent-rdp/commit/3fec7b5a0c3b90e4c2ae640eb5125d349b6656dd))
* Windows UI Automation and OCR text location ([73ff0d1](https://github.com/thisnick/agent-rdp/commit/73ff0d13ce332728fcbcc914782b5a01df2bc975))
* Windows UI Automation with improved IPC reliability ([c3180dc](https://github.com/thisnick/agent-rdp/commit/c3180dc37bfefb67cff5f1051b05324e69aa2586))


### Bug Fixes

* add explicit flush after IPC writes to prevent hanging ([330543f](https://github.com/thisnick/agent-rdp/commit/330543f5dfe2e9bb062b1ea9539b5cef3cfb23c6))
* add local logging for PowerShell agent debugging ([a272f2e](https://github.com/thisnick/agent-rdp/commit/a272f2e4635b942dec53228a9b2ebc1338ba70af))
* add postinstall script to set binary executable permission ([2e898da](https://github.com/thisnick/agent-rdp/commit/2e898da44388574dc303f7117266555a6467658c))
* add postinstall script to set binary executable permission ([#38](https://github.com/thisnick/agent-rdp/issues/38)) ([7e350d1](https://github.com/thisnick/agent-rdp/commit/7e350d15d43ecde420a30f6343b432c9b57b1243))
* bump patch for feat commits in pre-1.0 ([dd2cdec](https://github.com/thisnick/agent-rdp/commit/dd2cdec315758bed02807ad478d8873be1879e62))
* bundle models in platform packages and use pnpm publish ([#37](https://github.com/thisnick/agent-rdp/issues/37)) ([1010de2](https://github.com/thisnick/agent-rdp/commit/1010de2410b0e4881f3cfe39ebbd729b6fc19210))
* configure release-please to manage all platform packages ([#36](https://github.com/thisnick/agent-rdp/issues/36)) ([34b7fe5](https://github.com/thisnick/agent-rdp/commit/34b7fe5b4d81ad94501ec76c2ad152c547e975e3))
* configure release-please to update Cargo.toml ([#41](https://github.com/thisnick/agent-rdp/issues/41)) ([8805eb3](https://github.com/thisnick/agent-rdp/commit/8805eb3497ab9fe506a9878b26757b3315d6c6fa))
* improve automation error handling and reduce log noise ([a670a9e](https://github.com/thisnick/agent-rdp/commit/a670a9e6e4c98ee25df802341cdd9900985421af))
* improve keyboard typing reliability and release locks during sleeps ([50835c2](https://github.com/thisnick/agent-rdp/commit/50835c2a50f2c585fa5414556e0e4b2a0f90653c))
* improve keyboard typing reliability and release locks during sleeps ([5b0a066](https://github.com/thisnick/agent-rdp/commit/5b0a06695bef15f4a63a258db1c302bf78a51f55))
* integrate build and npm publish into release-please workflow ([c236942](https://github.com/thisnick/agent-rdp/commit/c236942a1d4ed52b0b93f5bd519a460c128a08e4))
* integrate build and npm publish into release-please workflow ([18f91cc](https://github.com/thisnick/agent-rdp/commit/18f91cc4623ff5261b3969fd4e6af343d2fca16d))
* IPC flush, Windows timeout, and ESLint ([9967b67](https://github.com/thisnick/agent-rdp/commit/9967b67c854abe478a2f890761b598fd18c3bca6))
* make request writes atomic and remove Rust-side file deletion ([e2c72e8](https://github.com/thisnick/agent-rdp/commit/e2c72e82e50b1657d2c8f10e0ebcd7dcf8dd3cef))
* remove invalid extra-files paths from release-please config ([7cce328](https://github.com/thisnick/agent-rdp/commit/7cce32800c9e6ffa35316d3559be7aeebfbb4931))
* remove postinstall script since binaries are bundled ([f6f57ba](https://github.com/thisnick/agent-rdp/commit/f6f57bab89263d06521cf8a0bb00373232affabf))
* remove truncation of name and value in snapshot output ([2e5b1fc](https://github.com/thisnick/agent-rdp/commit/2e5b1fcc15e9bd9122d23b8ab87f0a5da2dafa38))
* reset version to 0.1.4 for proper 0.2.0 release ([604789b](https://github.com/thisnick/agent-rdp/commit/604789bde09f35ca57bec53e45637d52b10f2a4d))
* reset version to 0.1.4 for proper 0.2.0 release ([c914a87](https://github.com/thisnick/agent-rdp/commit/c914a876aa8e517e339a7315c0931c8e9e40bdad))
* resolve RDPDR race conditions in file IPC ([db8fa5a](https://github.com/thisnick/agent-rdp/commit/db8fa5aeee36de5142996b85b9ef89527e0ccf00))
* set working directory to user profile for run command ([dde0b6c](https://github.com/thisnick/agent-rdp/commit/dde0b6c8522dfc2221eb50e1e4d45aac9a1add11))
* set working directory to user profile for run command ([e94ce33](https://github.com/thisnick/agent-rdp/commit/e94ce33556a73d354a702236c6175ea196bfc8b2))
* support manual and tag-based release triggers ([d769aaf](https://github.com/thisnick/agent-rdp/commit/d769aafc673e6cbf5c6888493612a4af97601a79))
* sync Cargo.toml version and add release-please marker ([ed43da7](https://github.com/thisnick/agent-rdp/commit/ed43da710134135ea845b72699c57d9039ca1d07))
* use simple release type with generic updater for Cargo.toml ([#42](https://github.com/thisnick/agent-rdp/issues/42)) ([92c22d1](https://github.com/thisnick/agent-rdp/commit/92c22d1a7ac17cc68668cb8cbe448b93c9948637))
* Windows IPC timeout and remove unused wrapper scripts ([950c761](https://github.com/thisnick/agent-rdp/commit/950c761f0818d42040991fe38c172c0ee405488e))

## [0.5.3](https://github.com/thisnick/agent-rdp/compare/agent-rdp-rust-v0.5.2...agent-rdp-rust-v0.5.3) (2026-01-24)


### Features

* add IPC schema type generation with ts-rs ([#48](https://github.com/thisnick/agent-rdp/issues/48)) ([2f2953a](https://github.com/thisnick/agent-rdp/commit/2f2953a3abe816b71b8d43252af622ac22a5ec5d))


### Bug Fixes

* bump patch for feat commits in pre-1.0 ([dd2cdec](https://github.com/thisnick/agent-rdp/commit/dd2cdec315758bed02807ad478d8873be1879e62))

## [0.5.2](https://github.com/thisnick/agent-rdp/compare/agent-rdp-rust-v0.5.1...agent-rdp-rust-v0.5.2) (2026-01-23)


### Miscellaneous Chores

* **agent-rdp-rust:** Synchronize agent-rdp packages versions
