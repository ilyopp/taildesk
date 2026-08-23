# BrainConnect

A lightweight desktop dashboard for your Tailscale network - device list, animated
network map, built-in RDP client, Taildrop & network diagnostics. Built with Rust + Tauri.

> Version française : [README.fr.md](README.fr.md)

## Requirements

- [Node.js](https://nodejs.org) (for the Tauri CLI) - `node -v`
- [Rust](https://rustup.rs) - `cargo --version`
- Tailscale installed and connected on the machine

End users don't need any of these: the `Setup.exe` ships everything.

## Run in development

```bash
npm install        # once
npm run dev
```

The window opens with a 10 s auto-refresh. The first launch compiles the Rust
backend (~2 min); next ones are instant.

## Production build

```bash
export RUST_MIN_STACK=33554432
export TAURI_SIGNING_PRIVATE_KEY=~/.tauri/brainconnect.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run build
```

Generated artifacts:

- Windows installer: `src-tauri/target/release/bundle/nsis/BrainConnect_0.1.0_x64-setup.exe`
  (language picker EN/FR and install-directory page; the language chosen in the
  installer becomes the app's language on first launch)
- Standalone executable: `src-tauri/target/release/brainconnect.exe`

The private key signs the update packages: keep it secret and don't lose it,
otherwise future auto-updates can't be signed.

## Features

| Action | Detail |
|---|---|
| Status & ping | Tailnet device list, online/offline, latency through `tailscale ping` |
| Network map | Animated graph: every device is an ember node linked to this PC; All/Online/Offline filter; drag nodes around, click a node copies its IP |
| Copy IP | Click the IP chip (clipboard button, or click a map node) |
| Browser | Opens `http://<tailscale-ip>` for the selected device |
| SSH | Opens a Windows console running `ssh <device>` (uses your Windows username; add per-device users in `~/.ssh/config` if needed) |
| Tailscale panel (settings button) | Connect/disconnect the tailnet (`tailscale up/down`), exit-node switch (`exit-node list` / `set --exit-node`), network diagnostics (`netcheck`: UDP, IPv4/IPv6, NAT, UPnP/PMP/PCP, DERP latencies), language, updates |
| Device « ⋯ » menu | Remote desktop, Taildrop file transfer, copy MagicDNS name or IPv6 |
| Built-in remote desktop | Full embedded RDP client (IronRDP): the remote screen renders inside the app with keyboard & mouse. Toggle in Panel → Settings; disabled it falls back to Windows mstsc |
| Network switcher | Dropdown in Panel → Connection: lists the tailnets already signed in on this PC (`tailscale switch --list`) and switches between them instantly (`tailscale switch`). To add a network: sign in once from the Tailscale app or `tailscale login` |

Data refreshes automatically every 10 seconds (no setting required).

## Language

English by default; French available in Tailscale panel → Settings → Language
(stored per machine). When installed through `Setup.exe`, the installer language
becomes the app's language on first launch.

## Automatic updates

Can be turned off in Tailscale panel → Settings → Automatic updates.

Checks hit this repository's releases:

1. The endpoint lives in `src-tauri/tauri.conf.json` (`plugins.updater.endpoints`)
   and targets `https://github.com/ilyopp/brainconnect/releases/latest/download/latest.json`.
2. Publish a GitHub Release (tag `vX.Y.Z`) attaching: the standalone `.exe`,
   its `.sig` sidecar and a `latest.json` manifest - all three are produced by
   `npm run build` under `src-tauri/target/release/bundle/`.

### Built-in remote desktop notes

- The target machine needs **Remote Desktop** enabled and an account protected
  by a password (NLA requirement).
- The remote display is streamed as MJPEG over `127.0.0.1` only (never exposed);
  the RDP server TLS certificate is not verified.
- If the build crashes with `STATUS_STACK_BUFFER_OVERRUN`, run it with
  `RUST_MIN_STACK=33554432` (known rustc issue on the `windows` crate).

## Project layout

```
BrainConnect/
├── ui/                    # Frontend (static HTML/CSS/JS)
│   ├── index.html         # markup with data-i18n attributes
│   ├── i18n.js            # EN/FR dictionary + helpers
│   ├── main.js            # UI logic, canvas map, RDP viewer
│   └── assets/logo.png
├── src-tauri/
│   ├── src/main.rs        # Commands: status, ping, ssh, browser, tailnet toggle,
│   │                      # exit nodes, netcheck, taildrop, updater helpers
│   ├── src/rdp.rs         # Embedded IronRDP client + MJPEG streamer
│   ├── windows/hooks.nsi  # NSIS hook: stores the chosen install language
│   ├── tauri.conf.json    # Window, NSIS bundle, updater config
│   └── icons/
└── package.json           # dev/build scripts (@tauri-apps/cli)
```

## Notes

- If `npm` isn't recognized in a terminal, restart it (Node.js was installed
  after the session started).
- Close the running app before rebuilding, otherwise the linker fails with
  “Access denied (os error 5)”.
- The backend locates the Tailscale CLI in `C:\Program Files\Tailscale\`
  first, then falls back to `PATH`.

## License

[MIT](LICENSE) © ilyopp
