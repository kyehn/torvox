# Spec: Bootstrap SHA-256 Verification (Best-Effort)

> Status: Partially Implemented | Since: v7 (comprehensive-hardening-v7)

## Purpose

When downloading a bootstrap file (URL or local path), provide best-effort SHA-256
integrity verification to detect accidental corruption or tampering.

## Design

### Mechanism

A sidecar file (`<filename>.sha256`) may be placed alongside the bootstrap file
containing a hex-encoded SHA-256 hash. When the sidecar exists, the downloaded
file's hash is verified against it before use.

### Key invariants

1. **Best-effort only**: If no sidecar exists, the download proceeds without
   verification. This is a safety net, not a security gate.
2. **No network round-trip**: The SHA-256 sidecar must be provided locally (same
   directory or same archive). We do not fetch checksums from the network.
3. **Atomic replace**: If verification fails, the downloaded file is deleted and
   the user is notified via the bootstrap status callback.

### Files

- `native/src/terminal/` — Bootstrap downloader and file management

### Open questions

- BootstrapDownloader implementation is pending. The spec defines the interface;
  the actual verification logic will be added when the feature is implemented.

### Test contract

- Unit test: sidecar present + matching hash → download accepted
- Unit test: sidecar present + mismatched hash → download rejected + file deleted
- Unit test: no sidecar → download accepted (best-effort)
