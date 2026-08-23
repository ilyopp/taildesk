# BrainConnect

Tableau de bord de bureau pour ton réseau Tailscale — liste des machines, carte
réseau animée, client RDP intégré, Taildrop et diagnostics réseau. Rust + Tauri.

> English version: [README.md](README.md)

## Prérequis

- [Node.js](https://nodejs.org) (pour le CLI Tauri) — `node -v`
- [Rust](https://rustup.rs) — `cargo --version`
- Tailscale installé et connecté sur le PC

Les utilisateurs finaux n'ont besoin de rien de tout cela : le `Setup.exe`
embarque tout.

## Lancer en développement

```bash
npm install        # une seule fois
npm run dev
```

La fenêtre s'ouvre avec rafraîchissement auto toutes les 10 s.
Le premier lancement compile le backend Rust (~2 min), les suivants sont instantanés.

## Build de production

```bash
export RUST_MIN_STACK=33554432
export TAURI_SIGNING_PRIVATE_KEY=~/.tauri/brainconnect.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run build
```

Produits générés :

- Installeur Windows : `src-tauri/target/release/bundle/nsis/BrainConnect_0.1.0_x64-setup.exe`
  (sélecteur de langue EN/FR et page de dossier d'installation ; la langue choisie
  devient celle de l'application au premier démarrage)
- Exécutable autonome : `src-tauri/target/release/brainconnect.exe`

La clé privée signe les paquets de mise à jour : garde-la secrète et ne la perds
pas, sinon les mises à jour auto ne pourront plus être signées.

## Fonctions

| Action | Détail |
|---|---|
| Statut & ping | Liste des machines du tailnet, en ligne / hors ligne, latence via `tailscale ping` |
| Carte réseau | Graphe animé : chaque machine est un nœud braise relié à ce PC ; filtre Les 2 / En ligne / Hors ligne ; glisse les nœuds, clic = copie l'IP |
| Copier l'IP | Clic sur l'adresse IP (ou bouton presse-papiers, ou clic sur un nœud de la carte) |
| Navigateur | Ouvre `http://<ip-tailscale>` de la machine |
| SSH | Ouvre une console Windows avec `ssh <machine>` (utilise ton nom d'utilisateur Windows ; ajoute l'utilisateur dans `~/.ssh/config` si besoin) |
| Panneau Tailscale (bouton réglages) | Connecter/couper le tailnet (`tailscale up/down`), choix du nœud de sortie (`exit-node`), diagnostic réseau complet (`netcheck` : UDP, IPv4/IPv6, NAT, UPnP/PMP/PCP, latences DERP), langue, mises à jour |
| Menu « ⋯ » d'une machine | Bureau à distance, envoi de fichier via Taildrop, copie du nom MagicDNS ou de l'IPv6 |
| Bureau à distance intégré | Client RDP complet embarqué (IronRDP) : écran distant affiché dans l'app avec clavier/souris. Activable dans Panneau → Paramètres ; sinon utilise mstsc |

L'actualisation est automatique toutes les 10 s (aucun réglage).

## Langue

Anglais par défaut, français disponible dans Panneau Tailscale → Paramètres →
Language (mémorisé par appareil). Lors de l'installation via `Setup.exe`, la
langue choisie devient celle de l'application au premier démarrage.

## Mises à jour automatiques

Désactivables dans Panneau Tailscale → Paramètres → Automatic updates.

La vérification interroge les releases de ce dépôt :

1. L'endpoint se trouve dans `src-tauri/tauri.conf.json` (`plugins.updater.endpoints`)
   et pointe vers `https://github.com/ilyopp/brainconnect/releases/latest/download/latest.json`.
2. Publie une Release GitHub (tag `vX.Y.Z`) avec en pièces jointes : l'exécutable
   `.exe` autonome, son `.sig` et un manifeste `latest.json` — les trois sont
   produits par `npm run build` sous `src-tauri/target/release/bundle/`.

### Notes sur le bureau à distance intégré

- La machine cible doit avoir le **Bureau à distance** activé et un compte protégé
  par mot de passe (exigence NLA).
- L'écran distant est diffusé en MJPEG sur `127.0.0.1` uniquement (jamais exposé) ;
  le certificat TLS du serveur RDP n'est pas vérifié.
- Si le build crashe avec `STATUS_STACK_BUFFER_OVERRUN`, relance avec
  `RUST_MIN_STACK=33554432` (crash connu de rustc sur la crate `windows`).

## Structure

```
BrainConnect/
├── ui/                    # Interface (HTML/CSS/JS statique)
│   ├── index.html         # balises avec attributs data-i18n
│   ├── i18n.js            # dictionnaire EN/FR + helpers
│   ├── main.js            # logique UI, carte canvas, visionneuse RDP
│   └── assets/logo.png
├── src-tauri/
│   ├── src/main.rs        # Commandes : status, ping, ssh, browser, toggle tailnet,
│   │                      # exit nodes, netcheck, taildrop, updater
│   ├── src/rdp.rs         # Client IronRDP embarqué + serveur MJPEG
│   ├── windows/hooks.nsi  # Hook NSIS : stocke la langue d'installation choisie
│   ├── tauri.conf.json    # Fenêtre, bundle NSIS, config updater
│   └── icons/
└── package.json           # Scripts dev/build (@tauri-apps/cli)
```

## Notes

- Si `npm` n'est pas reconnu dans le terminal, redémarre-le (Node a été
  installé après l'ouverture de la session terminal).
- Ferme l'application avant de recompiler, sinon le linker échoue
  avec « Accès refusé (os error 5) ».
- Le backend localise le CLI Tailscale dans `C:\Program Files\Tailscale\`
  puis dans le PATH.

## Licence

[MIT](LICENSE) © ilyopp
