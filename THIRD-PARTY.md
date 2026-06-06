# Third-Party Notices

Rift bundles or builds upon the following third-party components.

## WinDivert
- License: **LGPLv3** (commercial license available from the author).
- Source: https://github.com/basil00/Divert / https://reqrypt.org/windivert.html
- Usage: Rift dynamically uses the upstream, **Microsoft-signed** `WinDivert64.sys` / `WinDivert.dll`
  binaries **without modification**, in compliance with the LGPL (dynamic linking + this notice).
  Users may replace these binaries with their own build of WinDivert.

## zapret (DPI bypass logic / reference)
- License: **MIT**
- Source: https://github.com/bol-van/zapret
- Usage: DPI-desync strategies and host-list concepts are derived from / inspired by zapret.

## Tauri, Rust crates, and frontend libraries
- Their respective licenses (MIT/Apache-2.0, etc.) apply. See `Cargo.toml` / `package.json`
  and generated SBOM for the full dependency list.
