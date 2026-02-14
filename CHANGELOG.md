# Changelog

All notable changes to this project are documented in this file.

The format is based on Keep a Changelog.

## [Unreleased]

### Changed
- Updated `bin/nebula-up.sh` with xtrace preflight validation to fail fast when `XTRACE_AUTH_MODE=service` but `XTRACE_TOKEN` is empty.

### Added
- Added a deployment checklist for preventing recurrent `{"message":"Unauthorized"}` on `/api/audit-logs`.
- Added remote runbook troubleshooting steps to auto-sync `XTRACE_TOKEN` from `~/github/xtrace/.env` and verify audit API health.

### Fixed
- Fixed a common operational misconfiguration where missing `deploy/nebula.env` or empty `XTRACE_TOKEN` caused Audit Logs to fail intermittently after restart.

## [2026-02-14]

### Added
- Added explicit BFF xtrace auth mode with `XTRACE_AUTH_MODE` (`service` / `internal`).
- Added `crates/nebula-bff/Dockerfile` for containerized BFF builds.
- Added optional `xtrace` service under Docker Compose profile `observe`.
- Added deployment guidance for BFF + xtrace auth strategy in docs.

### Changed
- Updated BFF xtrace proxy behavior to use explicit mode policy instead of caller-token fallback.
- Updated local startup scripts to support BFF startup and xtrace mode configuration.
- Updated `docker-compose.yml` to include BFF service and route Gateway to BFF.
- Updated deployment and README docs with dev/prod recommended auth settings.

### Fixed
- Fixed Audit Logs Unauthorized behavior by clarifying and enforcing service/internal auth configuration paths.

[Unreleased]: https://github.com/lipish/nebula/compare/555ddec...HEAD
