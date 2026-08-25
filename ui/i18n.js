"use strict";

window.BC = (function () {
  const DICT = {
    en: {
      "nav.list": "List",
      "nav.files": "Files",
      "nav.map": "Map",

      "search.placeholder": "Search a device or an IP…",
      "loading": "Connecting to tailnet…",

      "st.connected": "Connected - {n} device(s) · {m} online",
      "st.needsLogin": "Tailscale login required",
      "st.err": "Tailscale error",
      "st.browserMode": "Browser mode",

      "banner.loginText": "This PC isn't connected to the tailnet.",
      "banner.stopped": "The Tailscale service is stopped on this machine. Run “tailscale up” or open the Tailscale app.",
      "banner.adminBtn": "Log in",
      "banner.consoleBtn": "Admin console",
      "banner.fail": "Failed to query Tailscale: {e}",
      "banner.noBridge": "Missing Tauri bridge: launch the app with npm run dev, not directly in a browser.",

      "footer.updatedAt": "Updated at {t}",

      "g.online": "Online",
      "g.offline": "Offline",
      "chip.self": "this device",
      "sub.seen": "last seen {r}",
      "rel.now": "just now",
      "rel.min": "{n} min ago",
      "rel.h": "{n} h ago",
      "rel.d": "{n} d ago",

      "lat.fail": "failed",

      "empty.noneTitle": "No other devices on your tailnet.",
      "empty.noneSub": "Add devices from the Tailscale admin console.",
      "empty.notRunning": "Tailnet unavailable until Tailscale is connected.",

      "toast.copied": "Copied to clipboard",
      "toast.browser": "Opening browser…",
      "toast.ssh": "Opening SSH session…",
      "toast.failed": "Action failed",

      "menu.ipv6": "Copy IPv6",
      "menu.dns": "Copy MagicDNS name",
      "menu.rdpEmb": "Remote desktop (in-app)",
      "menu.rdpWin": "Remote desktop (Windows)",
      "menu.send": "Send files",
      "menu.rdpOpenWin": "Opening Windows Remote Desktop…",
      "menu.rdpOpen": "Opening Remote Desktop…",

      "toast.sending": "Sending to {h}…",

      "xf.newSend": "New transfer",
      "xf.inbox": "Incoming",
      "xf.outbox": "Outgoing",
      "xf.emptyAll": "No transfers yet.",
      "xf.emptySub": "Send from a device menu or drop files anywhere in the window.",
      "xf.accept": "Accept",
      "xf.refuse": "Refuse",
      "xf.openFolder": "Open folder",
      "xf.clear": "Clear finished transfers",
      "xf.wants": "{p} wants to send you {n} file(s)",
      "xf.nFiles": "{n} file(s)",
      "xf.modeDirect": "direct",
      "xf.modeTaildrop": "Taildrop",
      "xf.stPending": "Pending",
      "xf.stActive": "Transferring",
      "xf.stDone": "Done",
      "xf.stFailed": "Failed",
      "xf.stRefused": "Refused",
      "xf.autoAccept": "Automatically accept incoming files",
      "xf.autoAcceptHint": "Files are saved without asking, from any tailnet device.",
      "xf.recvDir": "Reception folder",
      "xf.dirDefault": "Default: Downloads\\Taildesk",
      "xf.changeDir": "Change folder",
      "xf.dropHere": "Drop your files to send them",
      "xf.chooseDest": "Choose a destination",
      "xf.noneOnline": "No device online",

      "welcome.title": "Welcome to Taildesk",
      "welcome.sub": "Connect this PC to your tailnet with your Tailscale account.",
      "welcome.loginBtn": "Log in",
      "welcome.note": "Includes the official Tailscale client ({v}).",
      "welcome.serviceDown": "Connection service unreachable. Retrying…",
      "welcome.fixHint": "If this persists, reinstall Taildesk to restore the connection service.",
      "welcome.login": "Preparing sign-in…",
      "welcome.authReady": "Sign-in link ready. Open it to connect your account.",
      "welcome.authBtn": "Open Tailscale sign-in page",
      "welcome.connected": "Connected!",
      "welcome.errLogin": "Sign-in failed. Try again.",
      "welcome.errTimeout": "No connection detected in time. Try signing in again.",
      "welcome.errDaemon": "Connection service lost. Restart Taildesk and try again.",

      "filter.all": "All",
      "filter.online": "Online",
      "filter.offline": "Offline",
      "net.empty": "No devices to show on the map.",

      "tip.self": "this device",
      "tip.online": "Online",
      "tip.offline": "Offline",

      "p.conn": "Connection",
      "p.exit": "Exit node",
      "p.settings": "Settings",
      "p.diag": "Network diagnostics",
      "exit.hint": "Route this PC's Internet traffic through a tailnet device.",
      "diag.hint": "Test UDP, IPv4/IPv6, NAT type and DERP relay latency.",
      "diag.run": "Run diagnostics",
      "diag.running": "Running (~5 s)…",
      "diag.natVaries": "Variable NAT",
      "diag.pref": "Preferred relay",
      "diag.portMap": "Mapped port",
      "common.yes": "Yes",
      "common.no": "No",

      "conn.connected": "Connected to the tailnet",
      "conn.stopped": "Tailscale is stopped",
      "conn.off": "Disconnected from the tailnet",
      "conn.disconnectBtn": "Disconnect",
      "conn.connectBtn": "Connect",

      "exit.none": "None - direct access",
      "exit.loading": "Loading…",
      "exit.unavail": "Unavailable - {m}",
      "exit.current": "Current: {ip} (not listed)",
      "exit.noneShared": "No device shares its connection",
      "exit.applied": "Exit node: {h}",
      "exit.cleared": "Exit node disabled",
      "exit.via": "exit via {ip}",

      "set.lang": "Language",
      "switch.label": "Network (tailnet)",
      "switch.tip": "Switch tailnet",
      "switch.switching": "Switching network…",
      "switch.done": "Now on {n}",
      "switch.none": "No network",
      "set.updatesH": "Updates",
      "set.updates": "Automatic updates",
      "set.checkNow": "Check for updates",
      "set.checking": "Checking…",
      "set.uptodate": "Up to date",
      "set.available": "Update available: v{v}",
      "set.install": "Install & restart",
      "set.installing": "Installing…",
      "set.checkFail": "Check failed: {m}",
      "set.embeddedRdp": "Built-in remote desktop (in-app)",
      "set.rdpHint": "Disabled: opens the Windows Remote Desktop client (mstsc).",

      "rdp.formTitle": "Remote desktop connection",
      "rdp.credsHint": "Credentials of an authorized session on",
      "rdp.user": "Username",
      "rdp.pass": "Password",
      "rdp.submit": "Connect",
      "rdp.connecting": "Connecting…",
      "rdp.missingCreds": "Enter a username and password.",
      "rdp.waitingCreds": "Waiting for credentials",
      "rdp.active": "Session active",
      "rdp.failed": "Connection failed",
      "rdp.closeTip": "Close session",
    },

    fr: {
      "nav.list": "Liste",
      "nav.files": "Fichiers",
      "nav.map": "Carte",

      "search.placeholder": "Rechercher une machine ou une IP…",
      "loading": "Connexion au tailnet…",

      "st.connected": "Connecté - {n} machine(s) · {m} en ligne",
      "st.needsLogin": "Connexion à Tailscale requise",
      "st.err": "Erreur Tailscale",
      "st.browserMode": "Mode navigateur",

      "banner.loginText": "Ce PC n'est pas connecté au tailnet.",
      "banner.stopped": "Le service Tailscale est arrêté sur cette machine. Lance « tailscale up » ou ouvre l'application Tailscale.",
      "banner.adminBtn": "Se connecter",
      "banner.consoleBtn": "Console admin",
      "banner.fail": "Impossible d'interroger Tailscale : {e}",
      "banner.noBridge": "Bridge Tauri absent : lance l'application avec npm run dev, pas directement dans un navigateur.",

      "footer.updatedAt": "Actualisé à {t}",

      "g.online": "En ligne",
      "g.offline": "Hors ligne",
      "chip.self": "cette machine",
      "sub.seen": "vu {r}",
      "rel.now": "à l'instant",
      "rel.min": "il y a {n} min",
      "rel.h": "il y a {n} h",
      "rel.d": "il y a {n} j",

      "lat.fail": "échec",

      "empty.noneTitle": "Aucune autre machine sur ton tailnet.",
      "empty.noneSub": "Ajoute des appareils depuis la console d'administration Tailscale.",
      "empty.notRunning": "Tailnet indisponible tant que Tailscale n'est pas connecté.",

      "toast.copied": "Copié dans le presse-papiers",
      "toast.browser": "Ouverture du navigateur…",
      "toast.ssh": "Ouverture de la session SSH…",
      "toast.failed": "Action impossible",

      "menu.ipv6": "Copier l'IPv6",
      "menu.dns": "Copier le nom MagicDNS",
      "menu.rdpEmb": "Bureau à distance (dans l'app)",
      "menu.rdpWin": "Bureau à distance (Windows)",
      "menu.send": "Envoyer des fichiers",
      "menu.rdpOpenWin": "Ouverture du Bureau à distance Windows…",
      "menu.rdpOpen": "Ouverture du Bureau à distance…",

      "toast.sending": "Envoi vers {h}…",

      "xf.newSend": "Nouvel envoi",
      "xf.inbox": "Réceptions",
      "xf.outbox": "Envois",
      "xf.emptyAll": "Aucun transfert pour l'instant.",
      "xf.emptySub": "Envoie depuis le menu d'une machine ou dépose des fichiers n'importe où dans la fenêtre.",
      "xf.accept": "Accepter",
      "xf.refuse": "Refuser",
      "xf.openFolder": "Ouvrir le dossier",
      "xf.clear": "Nettoyer les transferts terminés",
      "xf.wants": "{p} veut t'envoyer {n} fichier(s)",
      "xf.nFiles": "{n} fichier(s)",
      "xf.modeDirect": "direct",
      "xf.modeTaildrop": "Taildrop",
      "xf.stPending": "En attente",
      "xf.stActive": "Transfert en cours",
      "xf.stDone": "Terminé",
      "xf.stFailed": "Échec",
      "xf.stRefused": "Refusé",
      "xf.autoAccept": "Accepter automatiquement les fichiers entrants",
      "xf.autoAcceptHint": "Les fichiers sont enregistrés sans demander, depuis toute machine du tailnet.",
      "xf.recvDir": "Dossier de réception",
      "xf.dirDefault": "Par défaut : Téléchargements\\Taildesk",
      "xf.changeDir": "Changer de dossier",
      "xf.dropHere": "Dépose tes fichiers pour les envoyer",
      "xf.chooseDest": "Choisis une destination",
      "xf.noneOnline": "Aucune machine en ligne",

      "welcome.title": "Bienvenue sur Taildesk",
      "welcome.sub": "Connecte ce PC à ton tailnet avec ton compte Tailscale.",
      "welcome.loginBtn": "Se connecter",
      "welcome.note": "Inclut le client Tailscale officiel ({v}).",
      "welcome.serviceDown": "Service de connexion injoignable. Nouvel essai…",
      "welcome.fixHint": "Si ça persiste, réinstalle Taildesk pour rétablir le service de connexion.",
      "welcome.login": "Préparation de la connexion…",
      "welcome.authReady": "Lien de connexion prêt. Ouvre-le pour connecter ton compte.",
      "welcome.authBtn": "Ouvrir la page de connexion Tailscale",
      "welcome.connected": "Connecté !",
      "welcome.errLogin": "Échec de la connexion. Réessaie.",
      "welcome.errTimeout": "Pas de connexion détectée à temps. Relance la connexion.",
      "welcome.errDaemon": "Service de connexion perdu. Redémarre Taildesk et réessaie.",

      "filter.all": "Les 2",
      "filter.online": "En ligne",
      "filter.offline": "Hors ligne",
      "net.empty": "Aucune machine à afficher sur la carte.",

      "tip.self": "cette machine",
      "tip.online": "En ligne",
      "tip.offline": "Hors ligne",

      "p.conn": "Connexion",
      "p.exit": "Nœud de sortie",
      "p.settings": "Paramètres",
      "p.diag": "Diagnostic réseau",
      "exit.hint": "Route le trafic Internet de ce PC via une machine du tailnet.",
      "diag.hint": "Teste UDP, IPv4/IPv6, le type de NAT et la latence des relais DERP.",
      "diag.run": "Lancer le diagnostic",
      "diag.running": "Analyse en cours (~5 s)…",
      "diag.natVaries": "NAT variable",
      "diag.pref": "Relai préféré",
      "diag.portMap": "Port mappé",
      "common.yes": "Oui",
      "common.no": "Non",

      "conn.connected": "Connecté au tailnet",
      "conn.stopped": "Tailscale est arrêté",
      "conn.off": "Déconnecté du tailnet",
      "conn.disconnectBtn": "Couper",
      "conn.connectBtn": "Connecter",

      "exit.none": "Aucun - accès direct",
      "exit.loading": "Chargement…",
      "exit.unavail": "Indisponible - {m}",
      "exit.current": "Actuel : {ip} (hors liste)",
      "exit.noneShared": "Aucune machine ne partage de connexion",
      "exit.applied": "Nœud de sortie : {h}",
      "exit.cleared": "Nœud de sortie désactivé",
      "exit.via": "sortie via {ip}",

      "set.lang": "Langue",
      "switch.label": "Réseau (tailnet)",
      "switch.tip": "Changer de réseau",
      "switch.switching": "Changement de réseau…",
      "switch.done": "Réseau : {n}",
      "switch.none": "Aucun réseau",
      "set.updatesH": "Mises à jour",
      "set.updates": "Mises à jour automatiques",
      "set.checkNow": "Vérifier les mises à jour",
      "set.checking": "Vérification…",
      "set.uptodate": "À jour",
      "set.available": "Mise à jour disponible : v{v}",
      "set.install": "Installer et redémarrer",
      "set.installing": "Installation…",
      "set.checkFail": "Vérification impossible : {m}",
      "set.embeddedRdp": "Bureau à distance intégré (dans l'app)",
      "set.rdpHint": "Désactivé : ouvre le client Bureau à distance de Windows (mstsc).",

      "rdp.formTitle": "Connexion au bureau à distance",
      "rdp.credsHint": "Identifiants d'une session autorisée sur",
      "rdp.user": "Utilisateur",
      "rdp.pass": "Mot de passe",
      "rdp.submit": "Se connecter",
      "rdp.connecting": "Connexion en cours…",
      "rdp.missingCreds": "Renseigne l'utilisateur et le mot de passe.",
      "rdp.waitingCreds": "En attente d'identifiants",
      "rdp.active": "Session en cours",
      "rdp.failed": "Échec de connexion",
      "rdp.closeTip": "Fermer la session",
    },
  };

  let lang = "en";

  function t(key, vars) {
    let s = (DICT[lang] && DICT[lang][key]) || DICT.en[key] || key;
    if (vars) {
      for (const k in vars) s = s.split("{" + k + "}").join(String(vars[k]));
    }
    return s;
  }

  function applyI18n() {
    document.documentElement.lang = lang;
    document.querySelectorAll("[data-i18n]").forEach((el) => {
      el.textContent = t(el.getAttribute("data-i18n"));
    });
    document.querySelectorAll("[data-i18n-ph]").forEach((el) => {
      el.setAttribute("placeholder", t(el.getAttribute("data-i18n-ph")));
    });
    document.querySelectorAll("[data-i18n-title]").forEach((el) => {
      el.setAttribute("title", t(el.getAttribute("data-i18n-title")));
    });
  }

  function setLang(l) {
    lang = l === "fr" ? "fr" : "en";
    applyI18n();
  }

  return { t, setLang, applyI18n, get lang() { return lang; }, DICT };
})();
