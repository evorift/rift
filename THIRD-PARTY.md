# Third-Party Notices

evorift bundles or builds upon the following third-party components.

## WinDivert
- License: **LGPLv3** (commercial license available from the author).
- Source: https://github.com/basil00/Divert / https://reqrypt.org/windivert.html
- Usage: evorift dynamically uses the upstream, **Microsoft-signed** `WinDivert64.sys` / `WinDivert.dll`
  binaries **without modification**, in compliance with the LGPL (dynamic linking + this notice).
  Users may replace these binaries with their own build of WinDivert.

## zapret / winws (DPI-bypass engine — bundled binary)
- License: **MIT** — Copyright (c) 2016-2024 bol-van.
- Source: https://github.com/bol-van/zapret
- Usage: evorift bundles and runs the upstream **`winws.exe`** binary (plus its `windivert.filter/*`
  raw-part files, the `quic_initial_*.bin` fake-QUIC payload, and the host-list) **unmodified** as the
  DPI-bypass engine. The MIT copyright + permission notice is retained in the bundled `winws/` folder.

## Cygwin runtime (cygwin1.dll)
- License: **LGPLv3** (Cygwin runtime library).
- Source: https://www.cygwin.com/ / https://sourceware.org/cygwin/
- Usage: `winws.exe` links the Cygwin runtime, so the unmodified upstream `cygwin1.dll` is bundled
  alongside it (dynamic link + this notice; users may replace it with their own Cygwin build).

## wgcf (Cloudflare WARP account/config generator — bundled binary)
- License: **MIT** — Copyright (c) 2020 ViRb3.
- Source: https://github.com/ViRb3/wgcf
- Usage: evorift bundles and runs the upstream **`wgcf.exe`** binary **unmodified** to register a free
  Cloudflare WARP account and generate a WireGuard profile on first run (Engine B). The generated config
  is rewritten locally to a Discord-only split-tunnel and stored under `%PROGRAMDATA%\evorift`. The MIT
  copyright + permission notice is retained alongside the bundled binary.

## WireGuard for Windows (split-tunnel — bundled binary)
- License: **MIT** — Copyright (c) 2015-2024 Jason A. Donenfeld and contributors.
- Source: https://github.com/WireGuard/wireguard-windows / https://www.wireguard.com/
- Usage: evorift bundles and runs the upstream **`wireguard.exe`** binary **unmodified** to install/remove
  a WireGuard tunnel service (`/installtunnelservice` / `/uninstalltunnelservice`) that routes Discord's
  IP ranges through Cloudflare WARP (Engine B). "WireGuard" is a registered trademark of Jason A. Donenfeld.

## Wintun (TUN driver for WireGuard — bundled binary)
- License: **Prebuilt redistributable** under the Wintun license (Copyright (c) 2018-2024 WireGuard LLC).
  Redistribution of the unmodified upstream `wintun.dll` is permitted; see https://www.wintun.net/.
- Source: https://www.wintun.net/ / https://git.zx2c4.com/wintun/
- Usage: `wireguard.exe` loads the unmodified upstream, **Microsoft-signed** `wintun.dll` as its TUN
  data-plane. Bundled without modification.

## Cloudflare WARP (network service)
- evorift's Engine B connects to Cloudflare's public **WARP** endpoint
  (`engage.cloudflareclient.com:2408`). Use is subject to Cloudflare's WARP Terms of Service, which the
  user accepts on first run via `wgcf register --accept-tos`. No Cloudflare code is bundled.

## Tauri, Rust crates, and frontend libraries
- Their respective licenses (MIT/Apache-2.0, etc.) apply. See `Cargo.toml` / `package.json`
  and generated SBOM for the full dependency list.
