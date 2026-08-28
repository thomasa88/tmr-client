# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

<!--

Headers:

* "Added" for new features.
* "Changed" for changes in existing functionality.
* "Deprecated" for soon-to-be removed features.
* "Removed" for now removed features.
* "Fixed" for any bug fixes.
* "Security" in case of vulnerabilities. 

`cargo release` is used to bump the version.

-->

<!-- next-header -->

## [Unreleased] - ReleaseDate

### Added

- `Currency` helper functions to create common currencies.

### Fixed

- Handle issuer (`iss`) field in OAuth callback.

## [0.0.9] - 2026-07-19

### Added

- Support `==` with `str` for `AccountNumber` and `Currency`.

### Fixed

- Move Montrose API dump into southesk crate. Make the crate build without the workspace.

## [0.0.8] - 2026-07-19

### Fixed

- Avoid trying to update the list of API functions in the readme when building
  southesk as a dependency to another create. Avoids the build script failing to find the readme file.

## [0.0.7] - 2026-07-19

### Added

- Show currency positions in the `show_data` example.

### Changed

- Authentication handlers have their own error types that matches the errors
  betters and lets calling application disambiguate on the type of error. 
- Errors no longer don't duplicate the inner error's message and they are all
  lowercase, as prescribed by the Rust API guidelines.
- Collapse `ClientBuildError::BuildError` into `ClientBuildError`. The enum only
  had one member.
- Separate error for authentication handler failure during connect:
  `ClientConnectError::AuthHandlerError`.

### Fixed

- Use shared cred store JSON encode/decode functions in plaintext cred store.
- Build docs with all features and mark items with feature requirements.
- Plaintext cred store fallback compiles when the `keyring` feature is unset.
- All feature combinations build without errors or warnings.
- All code linted using clippy in pedantic mode. Removed unwraps in `Client::connect()`.

## [0.0.6] - 2026-07-03

### Fixed

- Point to correct README.md for southesk on crates.io.

## [0.0.5] - 2026-07-03

### Added

- Low-level API that provides automatically generated functions from the MCP schema using `southesk-macros`.
  Feature flag: `low-api`.
- Server version is now included in the MCP introspection dump.

### Changed

- Moved southesk into a Cargo workspace.
- southesk error types are now found in `error`.
- Move raw API behind the feature flag `raw-api`.

## [0.0.4] - 2026-06-26

### Added

- Timeout option to `BrowserAuth` authentication callback handler.
- `Client::disconnect()`, for clean disconnects.
- Rename `result` module to `error`.

## [0.0.3] - 2026-06-13

### Added

- Quickstart example added to the readme and top Rust documentation.
- no_auth() option, for non-interactive sessions. Requires an existing access token.
- Documentation of many more types.
- Banner/logo image.

### Changed

- Simplified module path to credential store implementations.
- Montrose API updated to the 2026-06-11 version. Includes `CurrencyPosition`.
- Renamed `Account` to `AccountHoldings`.
- Renamed `HoldingsSelector` to `AccountFilter`.
- Turn account number and currency into their own types.

### Fixed

- Removed left-over oauth2 dependency.

## [0.0.2] - 2026-06-02

### Added

- API: Add missing `add_to_watchlist()`.
- Raw API calls using `raw_api_call()`.
- Downstream libraries used in function signatures are re-exported.
- Added a lot more rustdoc documentation.

### Changed

- Recommended standard traits added to all API types.
- Feature `keyring-creds` renamed to `keyring`.
- Some API types are renamed to better names.
- `TradeCurrency` is now an enum.

## [0.0.1] - 2024-05-30

First public release.

<!-- next-url -->
[Unreleased]: https://github.com/thomasa88/southesk/compare/southesk-v0.0.9...HEAD
[0.0.9]: https://github.com/thomasa88/southesk/compare/southesk-v0.0.8...southesk-v0.0.9
[0.0.8]: https://github.com/thomasa88/southesk/compare/southesk-v0.0.7...southesk-v0.0.8
[0.0.7]: https://github.com/thomasa88/southesk/compare/southesk-v0.0.6...southesk-v0.0.7
[0.0.6]: https://github.com/thomasa88/southesk/compare/southesk-v0.0.5...southesk-v0.0.6
[0.0.5]: https://github.com/thomasa88/southesk/compare/v0.0.4...southesk-v0.0.5
[0.0.4]: https://github.com/thomasa88/southesk/compare/v0.0.3...v0.0.4
[0.0.3]: https://github.com/thomasa88/southesk/compare/v0.0.2...v0.0.3
[0.0.2]: https://github.com/thomasa88/southesk/compare/v0.0.1...v0.0.2
[0.0.1]: https://github.com/thomasa88/southesk/releases/tag/v0.0.1
