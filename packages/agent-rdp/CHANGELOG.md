# Changelog

## [1.0.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.5.2...agent-rdp-v1.0.0) (2026-01-23)


### Maintenance

* **agent-rdp:** Synchronize agent-rdp packages versions

## [0.5.2](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.5.1...agent-rdp-v0.5.2) (2026-01-23)


### Maintenance

* **agent-rdp:** Synchronize agent-rdp packages versions

## [0.5.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.4.0...agent-rdp-v0.5.0) (2026-01-23)


### ⚠ BREAKING CHANGES

* restructure to pnpm workspaces with platform-specific packages ([#33](https://github.com/thisnick/agent-rdp/issues/33))

### Features

* restructure to pnpm workspaces with platform-specific packages ([#33](https://github.com/thisnick/agent-rdp/issues/33)) ([79f6474](https://github.com/thisnick/agent-rdp/commit/79f647472479fc19db33383278116f03097d30ce))

## [0.4.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.5...agent-rdp-v0.4.0) (2026-01-23)


### ⚠ BREAKING CHANGES

* The `invoke` automation command has been renamed to `click`.

### Features

* rename invoke to click with mouse-based implementation ([#30](https://github.com/thisnick/agent-rdp/issues/30)) ([e265697](https://github.com/thisnick/agent-rdp/commit/e265697d8207429318170a960dfe7520e082563c))


### Bug Fixes

* set working directory to user profile for run command ([dde0b6c](https://github.com/thisnick/agent-rdp/commit/dde0b6c8522dfc2221eb50e1e4d45aac9a1add11))
* set working directory to user profile for run command ([e94ce33](https://github.com/thisnick/agent-rdp/commit/e94ce33556a73d354a702236c6175ea196bfc8b2))

## [0.3.5](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.4...agent-rdp-v0.3.5) (2026-01-23)


### Features

* improve snapshot defaults with depth limit and truncation indicator ([ae01dcb](https://github.com/thisnick/agent-rdp/commit/ae01dcbcf5d3311d6cef8e60470ca81914d14b41))
* improve snapshot defaults with depth limit and truncation indicator ([8d2a84c](https://github.com/thisnick/agent-rdp/commit/8d2a84cc713f8cd1c60768b57b289901e6b9ef0d))

## [0.3.4](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.3...agent-rdp-v0.3.4) (2026-01-23)


### Features

* add --process-timeout flag to automate run command ([fe2ca21](https://github.com/thisnick/agent-rdp/commit/fe2ca2170d518afba94e98971db04b1303a0bdcc))
* add --process-timeout flag to automate run command ([ef0625c](https://github.com/thisnick/agent-rdp/commit/ef0625cd24b6507063b560b6485ffcd1bc86d96c))
* add WebSocket clipboard support and serve viewer from daemon ([a44d4f9](https://github.com/thisnick/agent-rdp/commit/a44d4f983f743ce2615d2ee71a6dbbacadd50ca6))
* add WebSocket clipboard support and serve viewer from daemon ([058c8c5](https://github.com/thisnick/agent-rdp/commit/058c8c5888f50fb1f7ed6b51def50a8e86f5aacc))

## [0.3.3](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.2...agent-rdp-v0.3.3) (2026-01-22)


### Features

* improve CLI, docs, and fix window pattern matching ([ce2e82b](https://github.com/thisnick/agent-rdp/commit/ce2e82ba2e4c8fb37e2e22db87dafa3c4791749f))
* improve CLI, docs, and fix window pattern matching ([a2354f8](https://github.com/thisnick/agent-rdp/commit/a2354f8e05eed7c00be80909f2b245c20da7fc14))

## [0.3.2](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.1...agent-rdp-v0.3.2) (2026-01-22)


### Refactoring

* consolidate locate and locateAll into single locate method ([165edd6](https://github.com/thisnick/agent-rdp/commit/165edd65b0d4cabde3a8550a20d41ca24aa2e713))
* consolidate locate and locateAll into single locate method ([8336215](https://github.com/thisnick/agent-rdp/commit/83362152a427ce88472d50efad4115602ff551dc))

## [0.3.1](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.3.0...agent-rdp-v0.3.1) (2026-01-22)


### Bug Fixes

* improve keyboard typing reliability and release locks during sleeps ([50835c2](https://github.com/thisnick/agent-rdp/commit/50835c2a50f2c585fa5414556e0e4b2a0f90653c))
* improve keyboard typing reliability and release locks during sleeps ([5b0a066](https://github.com/thisnick/agent-rdp/commit/5b0a06695bef15f4a63a258db1c302bf78a51f55))

## [0.3.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.2.3...agent-rdp-v0.3.0) (2026-01-22)


### ⚠ BREAKING CHANGES

* Remove click/double-click/right-click/check commands in favor of native UI Automation patterns.

### Features

* replace mouse-based automation with native UI Automation patterns ([715f9bb](https://github.com/thisnick/agent-rdp/commit/715f9bbf34e5d666b8f1ca8207a517f628676c0b))

## [0.2.3](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.2.2...agent-rdp-v0.2.3) (2026-01-21)


### Bug Fixes

* sync Cargo.toml version and add release-please marker ([ed43da7](https://github.com/thisnick/agent-rdp/commit/ed43da710134135ea845b72699c57d9039ca1d07))


### Refactoring

* split rdpdr_backend.rs and add ocrs documentation ([bf9dbc5](https://github.com/thisnick/agent-rdp/commit/bf9dbc5b9856a84aa12d3f63a3db63df8ec37547))
* split rdpdr_backend.rs into smaller modules ([e98bccd](https://github.com/thisnick/agent-rdp/commit/e98bccd525767fc560bdee2767dc6c0c45b8bef1))


### Documentation

* add automation TypeScript API and architecture explanation ([bd1ab85](https://github.com/thisnick/agent-rdp/commit/bd1ab85bece551eb2ddb8f5b80358a75eb9d01f4))
* add automation TypeScript API and architecture explanation ([107e672](https://github.com/thisnick/agent-rdp/commit/107e672f4743ebc6b7ddbd3c53012cdb4a7277e8))
* mention ocrs library for OCR ([a35a8a4](https://github.com/thisnick/agent-rdp/commit/a35a8a45e7ae0e995e89a53a167ccc35d348e742))

## [0.2.2](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.2.1...agent-rdp-v0.2.2) (2026-01-21)


### Features

* add OCR-based text location with locate command ([7a90d3c](https://github.com/thisnick/agent-rdp/commit/7a90d3c0ce960e599a8170940279890deb548ee1))
* Windows UI Automation and OCR text location ([73ff0d1](https://github.com/thisnick/agent-rdp/commit/73ff0d13ce332728fcbcc914782b5a01df2bc975))


### Bug Fixes

* add local logging for PowerShell agent debugging ([a272f2e](https://github.com/thisnick/agent-rdp/commit/a272f2e4635b942dec53228a9b2ebc1338ba70af))
* make request writes atomic and remove Rust-side file deletion ([e2c72e8](https://github.com/thisnick/agent-rdp/commit/e2c72e82e50b1657d2c8f10e0ebcd7dcf8dd3cef))
* remove postinstall script since binaries are bundled ([f6f57ba](https://github.com/thisnick/agent-rdp/commit/f6f57bab89263d06521cf8a0bb00373232affabf))
* remove truncation of name and value in snapshot output ([2e5b1fc](https://github.com/thisnick/agent-rdp/commit/2e5b1fcc15e9bd9122d23b8ab87f0a5da2dafa38))

## [0.2.1](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.2.0...agent-rdp-v0.2.1) (2026-01-21)


### Features

* add Windows UI Automation support ([0ebde33](https://github.com/thisnick/agent-rdp/commit/0ebde334fb363ef4b381f4b959068b77880554dc))
* hide PowerShell agent window completely (no taskbar icon) ([3fb273c](https://github.com/thisnick/agent-rdp/commit/3fb273c91d5eaa439beae0c0ade94142b2595e65))
* improve snapshot to enumerate all windows via EnumWindows ([e6c8c40](https://github.com/thisnick/agent-rdp/commit/e6c8c409ef61c869f0c2b659670963bc7f7a6dac))
* set PowerShell agent window title to "agent-rdp automation" ([d59acce](https://github.com/thisnick/agent-rdp/commit/d59acce867c106ed580d2c57b1b5716645503c69))
* Windows UI Automation with improved IPC reliability ([c3180dc](https://github.com/thisnick/agent-rdp/commit/c3180dc37bfefb67cff5f1051b05324e69aa2586))


### Bug Fixes

* add explicit flush after IPC writes to prevent hanging ([330543f](https://github.com/thisnick/agent-rdp/commit/330543f5dfe2e9bb062b1ea9539b5cef3cfb23c6))
* improve automation error handling and reduce log noise ([a670a9e](https://github.com/thisnick/agent-rdp/commit/a670a9e6e4c98ee25df802341cdd9900985421af))
* IPC flush, Windows timeout, and ESLint ([9967b67](https://github.com/thisnick/agent-rdp/commit/9967b67c854abe478a2f890761b598fd18c3bca6))
* resolve RDPDR race conditions in file IPC ([db8fa5a](https://github.com/thisnick/agent-rdp/commit/db8fa5aeee36de5142996b85b9ef89527e0ccf00))
* Windows IPC timeout and remove unused wrapper scripts ([950c761](https://github.com/thisnick/agent-rdp/commit/950c761f0818d42040991fe38c172c0ee405488e))


### Refactoring

* modularize PowerShell agent and add linting ([2513dbf](https://github.com/thisnick/agent-rdp/commit/2513dbf802dbcf4b1005238f3e34bc2f61ba5790))
* simplify snapshot to use RootElement + RawViewWalker ([60a61d0](https://github.com/thisnick/agent-rdp/commit/60a61d058c7010b99931e2124d82c50c54f5ca82))


### Documentation

* document Start menu limitation and improve run examples ([79df70b](https://github.com/thisnick/agent-rdp/commit/79df70bbe717239c179ac294ca729432450aa3a1))

## [0.2.0](https://github.com/thisnick/agent-rdp/compare/agent-rdp-v0.1.4...agent-rdp-v0.2.0) (2026-01-20)


### ⚠ BREAKING CHANGES

* JS API now uses object-style parameters

### Features

* Add Windows RDPDR backend for drive redirection with associated build and release configurations. ([2143b45](https://github.com/thisnick/agent-rdp/commit/2143b45ca34445693b882697d75feb79853047a5))
* Implement cross-platform native binary release workflow and runtime wrappers. ([ccf2d92](https://github.com/thisnick/agent-rdp/commit/ccf2d9283d14ecb5df1d37434cc1ddbf9804701d))
* v0.2.0 API improvements and release automation ([3fec7b5](https://github.com/thisnick/agent-rdp/commit/3fec7b5a0c3b90e4c2ae640eb5125d349b6656dd))


### Bug Fixes

* integrate build and npm publish into release-please workflow ([c236942](https://github.com/thisnick/agent-rdp/commit/c236942a1d4ed52b0b93f5bd519a460c128a08e4))
* integrate build and npm publish into release-please workflow ([18f91cc](https://github.com/thisnick/agent-rdp/commit/18f91cc4623ff5261b3969fd4e6af343d2fca16d))
* reset version to 0.1.4 for proper 0.2.0 release ([604789b](https://github.com/thisnick/agent-rdp/commit/604789bde09f35ca57bec53e45637d52b10f2a4d))
* reset version to 0.1.4 for proper 0.2.0 release ([c914a87](https://github.com/thisnick/agent-rdp/commit/c914a876aa8e517e339a7315c0931c8e9e40bdad))


### Documentation

* update SKILL.md for v0.2.0 API changes ([b6bfea6](https://github.com/thisnick/agent-rdp/commit/b6bfea621be360d7f0685f01bed86dc000834dc9))

## Changelog
