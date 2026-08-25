# Taildesk

A lightweight desktop dashboard for your Tailscale network - device list, animated
network map, built-in RDP client, Taildrop & network diagnostics. Built with Rust + Tauri.

> Version française : [README.fr.md](README.fr.md)

## Requirements

- [Node.js](https://nodejs.org) (for the Tauri CLI) - `node -v`
- [Rust](https://rustup.rs) - `cargo --version`

End users don't need any of these: the `Setup.exe` ships everything, including
the official Tailscale client (the app runs its own `tailscaled` from the
install folder, no separate Tailscale install needed).

## Run in development

```bash
npm install                      # once
pwsh scripts/get-tailscale.ps1   # once: fetches the bundled Tailscale client into src-tauri/tailscale-bundle/
npm run dev
```

The window opens with a 10 s auto-refresh. The first launch compiles the Rust
backend (~2 min); next ones are instant.

## Production build

```bash
export RUST_MIN_STACK=33554432
export TAURI_SIGNING_PRIVATE_KEY=~/.tauri/taildesk.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run build
```

Generated artifacts:

- Windows installer: `src-tauri/target/release/bundle/nsis/Taildesk_X.Y.Z_x64-setup.exe`
  (language picker EN/FR and install-directory page; the language chosen in the
  installer becomes the app's language on first launch)
- Standalone executable: `src-tauri/target/release/taildesk.exe`

The private key signs the update packages: keep it secret and don't lose it,
otherwise future auto-updates can't be signed.

## Features

| Action | Detail |
|---|---|
| Guided sign-in | First launch shows a welcome screen: one click opens the Tailscale sign-in page in your browser, then the app switches to the dashboard once connected |
| Status & ping | Tailnet device list, online/offline, latency through `tailscale ping` |
| Network map | Animated graph: every device is an ember node linked to this PC; All/Online/Offline filter; drag nodes around, click a node copies its IP |
| Copy IP | Click the IP chip (clipboard button, or click a map node) |
| Browser | Opens `http://<tailscale-ip>` for the selected device |
| SSH | Opens a Windows console running `ssh <device>` (uses your Windows username; add per-device users in `~/.ssh/config` if needed) |
| Files (Taildrop) | Send files to any device from its « ⋯ » menu, or drop them onto a device; progress list and clear finished transfers |
| Remote control | From the device « ⋯ » menu: the other PC shows an Accept/Refuse popup where you grant keyboard and mouse - no Windows account, no RDP setup. Disabled in settings it falls back to Windows mstsc |
| Always connected | The connection service runs unattended: the PC reconnects to the tailnet by itself at logon, and the app relaunches the service automatically if it stops responding |
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
   and targets `https://github.com/ilyopp/taildesk/releases/latest/download/latest.json`.
2. Publish a GitHub Release (tag `vX.Y.Z`) attaching: the installer
   `Taildesk_X.Y.Z_x64-setup.exe`, its `.sig` sidecar and a `latest.json`
   manifest. The first two come from `npm run build` under
   `src-tauri/target/release/bundle/nsis/`; `latest.json` lists the version,
   release date, download URL and the signature contents.

Staging copies of the installer, `.sig`, `latest.json` and release notes live
in the gitignored `release/` folder.

### Remote control notes

- The remote display is streamed as MJPEG through the tailnet and re-served to
  the viewer over `127.0.0.1` only; access is granted per session from a popup
  on the target PC (screen always, keyboard and mouse optional) and the
  requester can be ignored for 15 minutes.
- The mstsc fallback still requires **Remote Desktop** enabled on the target
  with a password-protected account (NLA requirement).

## Project layout

```
Taildesk/
├── ui/                    # Frontend (static HTML/CSS/JS)
│   ├── index.html         # markup with data-i18n attributes
│   ├── i18n.js            # EN/FR dictionary + helpers
│   ├── main.js            # UI logic, canvas map, welcome flow, Taildrop & RDP viewer
│   └── assets/logo.png
├── scripts/
│   └── get-tailscale.ps1  # fetches the official Tailscale client into src-tauri/tailscale-bundle/
├── src-tauri/
│   ├── src/main.rs        # Commands: status, ping, ssh, browser, tailnet toggle,
│   │                      # exit nodes, netcheck, updater helpers
│   ├── src/embedded.rs    # Bundled client probe, guided sign-in, daemon self-heal
│   ├── src/xfer.rs        # Taildrop: browse destinations, send files, progress events
│   ├── src/rc.rs          # Screen sharing: consent popup, capture, input injection
│   ├── windows/hooks.nsi  # NSIS hooks: install language, firewall rule,
│   │                      # scheduled task running the bundled tailscaled at logon
│   ├── tauri.conf.json    # Window, NSIS bundle, updater config
│   └── icons/
└── package.json           # dev/build scripts (@tauri-apps/cli)
```

## Notes

- If `npm` isn't recognized in a terminal, restart it (Node.js was installed
  after the session started).
- Close the running app before rebuilding, otherwise the linker fails with
  “Access denied (os error 5)”.
- All Tailscale commands use the client bundled with the app (dev:
  `src-tauri/tailscale-bundle/`, fetched by `scripts/get-tailscale.ps1`);
  no system Tailscale install is used.
- App and connection-service data live in `C:\ProgramData\Taildesk`;
  uninstalling wipes them, so the next install asks for an account again.

## License

[MIT](LICENSE) © ilyopp
