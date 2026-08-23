# BrainConnect

Tableau de bord de bureau pour ton tailnet Tailscale — liste des machines, IP,
statut en ligne/hors ligne, ping, SSH et ouverture navigateur.

Stack : **Tauri 2** (backend Rust) + interface web statique HTML/CSS/JS.
Thème basalte / charbon, volontairement sobre.

## Prérequis

- [Node.js](https://nodejs.org) (pour le CLI Tauri) — `node -v`
- [Rust](https://rustup.rs) — `cargo --version`
- Tailscale installé et connecté sur le PC

## Lancer en développement

```bash
npm install        # une seule fois
npm run dev
```

La fenêtre s'ouvre avec rechargement auto toutes les 10 s.
Le premier lancement compile le backend Rust (~2 min), les suivants sont instantanés.

## Build de production

```bash
export RUST_MIN_STACK=33554432
export TAURI_SIGNING_PRIVATE_KEY_PATH=~/.tauri/brainconnect.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run build
```

Produits générés :

- Installeur Windows : `src-tauri/target/release/bundle/nsis/BrainConnect_0.1.0_x64-setup.exe`
  (choix de la langue EN/FR et du dossier d'installation ; la langue choisie
  devient celle de l'application au premier démarrage)
- Exécutable autonome : `src-tauri/target/release/brainconnect.exe`

La clé privée `~/.tauri/brainconnect.key` signe les paquets de mise à jour :
ne la publie pas et ne la perds pas (sinon les mises à jour auto ne fonctionnent plus).

## Langue

Anglais par défaut, français disponible. Le réglage se change dans
Tailscale → Paramètres → Language (mémorisé par appareil).

## Mises à jour automatiques

Désactivables dans Tailscale → Paramètres → Automatic updates.

Pour que la vérification fonctionne :

1. Remplace `YOUR_USERNAME/YOUR_REPO` dans `src-tauri/tauri.conf.json`
   (`plugins.updater.endpoints`) par ton dépôt GitHub réel.
2. Publie une Release GitHub contenant, en assets : le `.exe` autonome,
   le `.sig` associé et `latest.json` (générés par `npm run build` dans
   `target/release/bundle/`).

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

### Notes sur le bureau à distance intégré

- Nécessite que la machine cible ait le **Bureau à distance** activé
  (Paramètres Windows → Système → Bureau à distance) et un compte avec mot de passe.
- L'écran distant est servi en MJPEG sur `127.0.0.1` (local uniquement) par un
  petit serveur interne ; le certificat TLS du serveur RDP n'est pas vérifié.
- Si le build échoue avec `STATUS_STACK_BUFFER_OVERRUN`, relance avec
  `RUST_MIN_STACK=33554432 cargo build` (crash connu de rustc sur la crate `windows`).

## Structure

```
BrainConnect/
├── ui/                    # Interface (HTML/CSS/JS statique)
│   ├── index.html
│   ├── styles.css         # Thème basalte/charbon
│   ├── main.js            # Logique UI + appels invoke()
│   └── assets/logo.png
├── src-tauri/
│   ├── src/main.rs        # Commandes Rust : status, ping, ssh, rdp, browser,
│   │                      # toggle_tailscale, exit nodes, netcheck, taildrop
│   ├── tauri.conf.json    # Config fenêtre + bundle NSIS
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
