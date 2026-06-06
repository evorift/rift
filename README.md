<div align="center">

# Rift

**Unblock Discord, Roblox & games — without a VPN.**

Rift is a lightweight Windows app that breaks through ISP DPI censorship using
surgical packet techniques (WinDivert), so blocked apps just work again — while
your normal traffic stays untouched and your voice/UDP keeps running.

*No VPN · no speed loss · only the sites you choose are affected.*

</div>

---

> ⚠️ **Status: early development.** Not ready for download yet. Star/watch to follow.

## Why Rift?
- 🚀 **One-click "Game Mode"** — turn protection on, everything optimized.
- 🎯 **Surgical, not a tunnel** — only chosen domains are de-blocked; the rest of your
  internet is normal and full-speed. Discord voice (UDP/QUIC) keeps working.
- 🔒 **Encrypted DNS (DoH)** — defeats ISP DNS hijacking.
- 🧰 **Network tools** — live monitor, per-app firewall, ping test, safe tweaks.
- 🪶 **Tiny & fast** — built in Rust + Tauri, near-zero idle resource use.

## How it works
Rift uses [WinDivert](https://reqrypt.org/windivert.html) to apply DPI-desync
strategies (inspired by [zapret](https://github.com/bol-van/zapret)) only to a
host-list you control. It is **not** a VPN and does **not** route your traffic
through any server.

## Download
Coming soon via [Releases](../../releases). The app auto-updates.

> First launch shows a Windows SmartScreen warning (unsigned build) —
> click **More info → Run anyway**. Verify the download against the published `SHA256SUMS`.

## Languages
English · Türkçe · Español · Русский

## Support
If Rift helps you, consider supporting development — see the **Sponsor** button.
Donations are optional; Rift is and will stay free.

## License
Code: [MIT](LICENSE). Third-party components: see [THIRD-PARTY.md](THIRD-PARTY.md)
(WinDivert is LGPLv3; zapret is MIT).
