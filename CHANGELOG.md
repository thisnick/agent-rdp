# Changelog

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
