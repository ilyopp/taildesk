# Taildesk

Tableau de bord de bureau pour ton réseau Tailscale - liste des machines, carte
réseau animée, client RDP intégré, Taildrop et diagnostics réseau. Rust + Tauri.

> English version: [README.md](README.md)

## Prérequis

- [Node.js](https://nodejs.org) (pour le CLI Tauri) - `node -v`
- [Rust](https://rustup.rs) - `cargo --version`

Les utilisateurs finaux n'ont besoin de rien de tout cela : le `Setup.exe`
embarque tout, y compris le client Tailscale officiel (l'app fait tourner son
propre `tailscaled` depuis son dossier d'installation, sans installation
Tailscale séparée).

## Lancer en développement

```bash
npm install                      # une seule fois
pwsh scripts/get-tailscale.ps1   # une seule fois : récupère le client Tailscale embarqué dans src-tauri/tailscale-bundle/
npm run dev
```

La fenêtre s'ouvre avec rafraîchissement auto toutes les 10 s.
Le premier lancement compile le backend Rust (~2 min), les suivants sont instantanés.

## Build de production

```bash
export RUST_MIN_STACK=33554432
export TAURI_SIGNING_PRIVATE_KEY=~/.tauri/taildesk.key
export TAURI_SIGNING_PRIVATE_KEY_PASSWORD=""
npm run build
```

Produits générés :

- Installeur Windows : `src-tauri/target/release/bundle/nsis/Taildesk_X.Y.Z_x64-setup.exe`
  (sélecteur de langue EN/FR et page de dossier d'installation ; la langue choisie
  devient celle de l'application au premier démarrage)
- Exécutable autonome : `src-tauri/target/release/taildesk.exe`

La clé privée signe les paquets de mise à jour : garde-la secrète et ne la perds
pas, sinon les mises à jour auto ne pourront plus être signées.

## Fonctions

| Action | Détail |
|---|---|
| Connexion guidée | L'écran d'accueil au premier lancement : un clic ouvre la page de connexion Tailscale dans ton navigateur, puis l'app bascule sur le tableau de bord dès que c'est connecté |
| Statut & ping | Liste des machines du tailnet, en ligne / hors ligne, latence via `tailscale ping` |
| Carte réseau | Graphe animé : chaque machine est un nœud braise relié à ce PC ; filtre Les 2 / En ligne / Hors ligne ; glisse les nœuds, clic = copie l'IP |
| Copier l'IP | Clic sur l'adresse IP (ou bouton presse-papiers, ou clic sur un nœud de la carte) |
| Navigateur | Ouvre `http://<ip-tailscale>` de la machine |
| SSH | Ouvre une console Windows avec `ssh <machine>` (utilise ton nom d'utilisateur Windows ; ajoute l'utilisateur dans `~/.ssh/config` si besoin) |
| Panneau Tailscale (bouton réglages) | Connecter/couper le tailnet (`tailscale up/down`), choix du nœud de sortie (`exit-node`), diagnostic réseau complet (`netcheck` : UDP, IPv4/IPv6, NAT, UPnP/PMP/PCP, latences DERP), langue, mises à jour |
| Menu « ⋯ » d'une machine | Bureau à distance, envoi de fichier via Taildrop, copie du nom MagicDNS ou de l'IPv6 |
| Bureau à distance intégré | Client RDP complet embarqué (IronRDP) : écran distant affiché dans l'app avec clavier/souris. Activable dans Panneau → Paramètres ; sinon utilise mstsc |
| Fichiers (Taildrop) | Onglet dédié pour envoyer des fichiers à n'importe quelle machine : glisser-déposer ou « Nouveau transfert », liste de progression, nettoyage des transferts terminés |
| Toujours connecté | Le service de connexion tourne en mode unattended : le PC se reconnecte seul au tailnet à l'ouverture de session, et l'app relance le service automatiquement s'il ne répond plus |
| Changement de réseau | Menu déroulant dans Panneau → Connexion : liste les réseaux (tailnets) déjà connectés sur ce PC (`tailscale switch --list`) et bascule instantanément de l'un à l'autre (`tailscale switch`). Pour ajouter un réseau : connecte-le d'abord une fois depuis l'application Tailscale ou `tailscale login` |

L'actualisation est automatique toutes les 10 s (aucun réglage).

## Langue

Anglais par défaut, français disponible dans Panneau Tailscale → Paramètres →
Language (mémorisé par appareil). Lors de l'installation via `Setup.exe`, la
langue choisie devient celle de l'application au premier démarrage.

## Mises à jour automatiques

Désactivables dans Panneau Tailscale → Paramètres → Automatic updates.

La vérification interroge les releases de ce dépôt :

1. L'endpoint se trouve dans `src-tauri/tauri.conf.json` (`plugins.updater.endpoints`)
   et pointe vers `https://github.com/ilyopp/taildesk/releases/latest/download/latest.json`.
2. Publie une Release GitHub (tag `vX.Y.Z`) avec en pièces jointes : l'installeur
   `Taildesk_X.Y.Z_x64-setup.exe`, son `.sig` et un manifeste `latest.json`.
   Les deux premiers sortent de `npm run build` sous
   `src-tauri/target/release/bundle/nsis/` ; le `latest.json` liste la version,
   la date, l'URL de téléchargement et la signature.
Des copies de travail de l'installeur, du `.sig`, du `latest.json` et des notes
de version sont préparées dans le dossier `release/` (ignoré par git).

### Notes sur le bureau à distance intégré

- La machine cible doit avoir le **Bureau à distance** activé et un compte protégé
  par mot de passe (exigence NLA).
- L'écran distant est diffusé en MJPEG sur `127.0.0.1` uniquement (jamais exposé) ;
  le certificat TLS du serveur RDP n'est pas vérifié.
- Si le build crashe avec `STATUS_STACK_BUFFER_OVERRUN`, relance avec
  `RUST_MIN_STACK=33554432` (crash connu de rustc sur la crate `windows`).

## Structure

```
Taildesk/
├── ui/                    # Interface (HTML/CSS/JS statique)
│   ├── index.html         # balises avec attributs data-i18n
│   ├── i18n.js            # dictionnaire EN/FR + helpers
│   ├── main.js            # logique UI, carte canvas, écran d'accueil, Taildrop & RDP
│   └── assets/logo.png
├── scripts/
│   └── get-tailscale.ps1  # récupère le client Tailscale officiel dans src-tauri/tailscale-bundle/
├── src-tauri/
│   ├── src/main.rs        # Commandes : status, ping, ssh, browser, toggle tailnet,
│   │                      # exit nodes, netcheck, updater
│   ├── src/embedded.rs    # Sonde client embarqué, connexion guidée, auto-réparation daemon
│   ├── src/xfer.rs        # Taildrop : destinations, envoi de fichiers, événements de progression
│   ├── src/rdp.rs         # Client IronRDP embarqué + serveur MJPEG
│   ├── windows/hooks.nsi  # Hooks NSIS : langue d'installation, règle pare-feu,
│   │                      # tâche planifiée lançant tailscaled à l'ouverture de session
│   ├── tauri.conf.json    # Fenêtre, bundle NSIS, config updater
│   └── icons/
└── package.json           # Scripts dev/build (@tauri-apps/cli)
```

## Notes

- Si `npm` n'est pas reconnu dans le terminal, redémarre-le (Node a été
  installé après l'ouverture de la session terminal).
- Ferme l'application avant de recompiler, sinon le linker échoue
  avec « Accès refusé (os error 5) ».
- Toutes les commandes Tailscale utilisent le client embarqué avec l'app
  (en dev : `src-tauri/tailscale-bundle/`, récupéré par
  `scripts/get-tailscale.ps1`) ; aucune installation système n'est utilisée.
- Les données de l'app et du service de connexion sont dans
  `C:\ProgramData\Taildesk` ; la désinstallation les efface, la prochaine
  installation redemandera un compte.

## Licence

[MIT](LICENSE) © ilyopp
