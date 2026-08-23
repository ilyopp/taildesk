"use strict";

const Tauri = window.__TAURI__ ?? null;
const invoke = Tauri ? Tauri.core.invoke : null;
const t = (k, v) => window.BC.t(k, v);

const els = {
  content: document.getElementById("content"),
  banner: document.getElementById("banner"),
  search: document.getElementById("search"),
  btnRefresh: document.getElementById("btn-refresh"),
  stateDot: document.getElementById("state-dot"),
  statusText: document.getElementById("status-text"),
  updatedAt: document.getElementById("updated-at"),
  versionText: document.getElementById("version-text"),
  toast: document.getElementById("toast"),

  viewList: document.getElementById("view-list"),
  viewNet: document.getElementById("view-net"),
  netCanvas: document.getElementById("net-canvas"),
  netTip: document.getElementById("net-tip"),
  netEmpty: document.getElementById("net-empty"),

  btnPanel: document.getElementById("btn-panel"),
  backdrop: document.getElementById("backdrop"),
  panel: document.getElementById("panel"),
  panelClose: document.getElementById("panel-close"),
  connText: document.getElementById("conn-text"),
  btnConn: document.getElementById("btn-conn"),
  panelVersion: document.getElementById("panel-version"),
  exitSelect: document.getElementById("exit-select"),
  btnNetcheck: document.getElementById("btn-netcheck"),
  netcheckOut: document.getElementById("netcheck-out"),
  rowMenu: document.getElementById("row-menu"),

  tailnetSelect: document.getElementById("tailnet-select"),

  optRdp: document.getElementById("opt-rdp-embedded"),
  optLang: document.getElementById("opt-lang"),
  optUpdates: document.getElementById("opt-updates"),
  btnUpd: document.getElementById("btn-upd"),
  updLine: document.getElementById("upd-line"),

  rdp: document.getElementById("rdp"),
  rdpTitle: document.getElementById("rdp-title"),
  rdpState: document.getElementById("rdp-state"),
  rdpDot: document.getElementById("rdp-dot"),
  rdpClose: document.getElementById("rdp-close"),
  rdpStage: document.getElementById("rdp-stage"),
  rdpAuth: document.getElementById("rdp-auth"),
  rdpAuthHost: document.getElementById("rdp-auth-host"),
  rdpUser: document.getElementById("rdp-user"),
  rdpPass: document.getElementById("rdp-pass"),
  rdpErr: document.getElementById("rdp-err"),
  rdpImg: document.getElementById("rdp-img"),
};

const ICONS = {
  ping: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="22 12 18 12 15 21 9 3 6 12 2 12"/></svg>`,
  browser: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14.5 14.5 0 0 1 0 18a14.5 14.5 0 0 1 0-18"/></svg>`,
  ssh: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><polyline points="5 8 9 12 5 16"/><path d="M12 17h7"/></svg>`,
  copy: `<svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.75" stroke-linecap="round" stroke-linejoin="round"><rect x="9" y="9" width="12" height="12" rx="2.5"/><path d="M5 15V5a2 2 0 0 1 2-2h10"/></svg>`,
  more: `<svg viewBox="0 0 24 24" fill="currentColor"><circle cx="5" cy="12" r="1.7"/><circle cx="12" cy="12" r="1.7"/><circle cx="19" cy="12" r="1.7"/></svg>`,
};

let query = "";
let lastStatus = null;
let appVersion = "";

function esc(s) {
  return String(s).replace(/[&<>"']/g, (c) =>
    ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;", "'": "&#39;" }[c])
  );
}

function toast(msg, ms) {
  els.toast.textContent = msg;
  els.toast.classList.add("show");
  clearTimeout(toast._t);
  toast._t = setTimeout(() => els.toast.classList.remove("show"), ms || 1800);
}

async function copyText(text) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    const ta = document.createElement("textarea");
    ta.value = text;
    document.body.appendChild(ta);
    ta.select();
    document.execCommand("copy");
    ta.remove();
  }
}

function relTime(iso) {
  const d = Date.parse(iso);
  if (!Number.isFinite(d)) return "";
  const s = Math.max(0, (Date.now() - d) / 1000);
  if (s < 60) return t("rel.now");
  if (s < 3600) return t("rel.min", { n: Math.floor(s / 60) });
  if (s < 86400) return t("rel.h", { n: Math.floor(s / 3600) });
  return t("rel.d", { n: Math.floor(s / 86400) });
}

function formatMs(ms) {
  return ms >= 1000 ? `${(ms / 1000).toFixed(1)} s` : `${Math.round(ms)} ms`;
}

function showBanner(html, isError) {
  els.banner.innerHTML = html;
  els.banner.classList.toggle("err", Boolean(isError));
  els.banner.classList.remove("hidden");
}

function hideBanner() {
  els.banner.classList.add("hidden");
}

function setStatus(dotClass, text) {
  els.stateDot.className = `dot ${dotClass}`;
  els.statusText.textContent = text;
}

function langLocale() {
  return window.BC.lang === "fr" ? "fr-FR" : "en-GB";
}

function machineHtml(p) {
  const host = p.hostname || p.dns_name || "sans-nom";
  const ip = p.ipv4 || p.ipv6 || "";
  const browserTarget = p.ipv4 || p.dns_name || "";
  const sshHost = p.dns_name || p.ipv4 || "";
  const id = p.ipv4 || p.dns_name || p.hostname;

  const sub = [];
  if (p.dns_name && p.dns_name !== host) sub.push(esc(p.dns_name));
  if (!p.is_self && !p.online && p.last_seen)
    sub.push(esc(t("sub.seen", { r: relTime(p.last_seen) })));
  if (p.os) sub.push(esc(p.os));

  const classes = ["machine"];
  if (p.is_self) classes.push("self-card");
  if (!p.is_self && !p.online) classes.push("offline");

  const searchIndex = [host, p.dns_name, p.ipv4, p.ipv6].join(" ").toLowerCase();

  return `
  <article class="${classes.join(" ")}" data-search="${esc(searchIndex)}" data-id="${esc(id)}">
    <span class="dot ${p.online ? "on" : "off"}"></span>
    <div class="m-id">
      <div class="m-name-row">
        <span class="m-name">${esc(host)}</span>
        ${p.is_self ? `<span class="chip self-chip">${esc(t("chip.self"))}</span>` : ""}
      </div>
      <p class="m-sub">${sub.join(" · ")}</p>
    </div>
    <div class="m-right">
      ${ip ? `<button class="ip-btn" data-action="copy" data-value="${esc(ip)}" title="Copy IP">${esc(ip)}</button>` : ""}
      <span class="latency"></span>
      <div class="actions">
        <button class="btn act-ping" data-action="ping" data-value="${esc(ip || sshHost)}" title="Ping">
          ${ICONS.ping}<span class="btn-label">ping</span>
        </button>
        <button class="btn icon-only" data-action="browser" data-value="${esc(browserTarget)}" title="http://${esc(browserTarget)}">${ICONS.browser}</button>
        <button class="btn icon-only" data-action="ssh" data-value="${esc(sshHost)}" title="SSH">${ICONS.ssh}</button>
        <button class="btn icon-only" data-action="copy" data-value="${esc(ip)}" title="Copy IP">${ICONS.copy}</button>
        <button class="btn icon-only" data-action="more" title="More">${ICONS.more}</button>
      </div>
    </div>
  </article>`;
}

function render(st) {
  lastStatus = st;

  const tsShort = (st.version || "").split("-")[0];
  els.versionText.textContent = `Taildesk ${appVersion} · tailscale ${tsShort}`;

  const running = st.backend_state === "Running";
  const online = st.peers.filter((p) => p.online);

  if (!running) {
    if (st.backend_state === "NeedsLogin" || st.backend_state === "NeedsMachineAuth") {
      setStatus("warn", t("st.needsLogin"));
      showBanner(
        `${esc(t("banner.loginText"))} ` +
        `<button class="banner-action" data-action="login">${esc(t("banner.adminBtn"))}</button>`,
        false
      );
    } else if (st.backend_state === "Stopped") {
      setStatus("warn", t("conn.stopped"));
      showBanner(
        `${esc(t("banner.stopped"))} ` +
        `<button class="banner-action" data-action="login">${esc(t("banner.consoleBtn"))}</button>`,
        false
      );
    } else {
      setStatus("err", `${st.backend_state}`);
      hideBanner();
    }
  } else {
    setStatus("on", t("st.connected", { n: st.peers.length, m: online.length }));
    hideBanner();
  }

  let html = '<div class="wrap">';
  if (st.self_device) html += machineHtml(st.self_device);

  if (!running) {
    html += `<div class="empty"><img src="assets/logo.png" alt=""><p>${esc(t("empty.notRunning"))}</p></div>`;
  } else if (st.peers.length === 0) {
    html += `
      <div class="empty">
        <img src="assets/logo.png" alt="">
        <p>${esc(t("empty.noneTitle"))}</p>
        <small>${esc(t("empty.noneSub"))}</small>
      </div>`;
  } else {
    const offline = st.peers.filter((p) => !p.online);
    if (online.length)
      html += `<div class="group"><p class="group-label">${esc(t("g.online"))}<span class="count">${online.length}</span></p>${online
        .map(machineHtml)
        .join("")}</div>`;
    if (offline.length)
      html += `<div class="group"><p class="group-label">${esc(t("g.offline"))}<span class="count">${offline.length}</span></p>${offline
        .map(machineHtml)
        .join("")}</div>`;
  }

  html += "</div>";
  els.content.innerHTML = html;

  els.updatedAt.textContent = t("footer.updatedAt", {
    t: new Date().toLocaleTimeString(langLocale()),
  });

  applyFilter();

  updateConnUI();
  netSync(st);
}

async function doPing(btn, row) {
  const lat = row.querySelector(".latency");
  btn.disabled = true;
  btn.classList.add("loading");
  try {
    const out = await invoke("ping_peer", { ip: btn.dataset.value });
    const m = out.match(/pong[^\n]*?\bin\s+([\d.]+)\s*(ms|s)\b/i);
    clearTimeout(lat._t);
    if (m) {
      let ms = parseFloat(m[1]);
      if ((m[2] || "").toLowerCase() === "s") ms *= 1000;
      lat.textContent = formatMs(ms);
      lat.className = `latency show ${ms < 50 ? "ok" : ms < 200 ? "warn" : "err"}`;
    } else if (/pong/i.test(out)) {
      lat.textContent = "pong";
      lat.className = "latency show ok";
    } else {
      lat.textContent = t("lat.fail");
      lat.className = "latency show err";
    }
    lat._t = setTimeout(() => lat.classList.remove("show"), 12000);
  } catch (e) {
    clearTimeout(lat._t);
    lat.textContent = t("lat.fail");
    lat.className = "latency show err";
    lat._t = setTimeout(() => lat.classList.remove("show"), 6000);
  } finally {
    btn.disabled = false;
    btn.classList.remove("loading");
  }
}

els.content.addEventListener("click", async (ev) => {
  const btn = ev.target.closest("[data-action]");
  if (!btn) return;
  const action = btn.dataset.action;
  if (action === "more") {
    openRowMenu(btn);
    return;
  }
  const row = btn.closest(".machine");

  try {
    switch (action) {
      case "copy":
        await copyText(btn.dataset.value);
        toast(t("toast.copied"));
        break;
      case "browser":
        toast(t("toast.browser"));
        await invoke("open_browser", { url: btn.dataset.value });
        break;
      case "ssh":
        toast(t("toast.ssh"));
        await invoke("open_ssh", { host: btn.dataset.value });
        break;
      case "login":
        await invoke("open_browser", { url: "https://login.tailscale.com/admin/machines" });
        break;
      case "ping":
        if (row) await doPing(btn, row);
        break;
    }
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"));
  }
});

els.banner.addEventListener("click", async (ev) => {
  const btn = ev.target.closest("[data-action=login]");
  if (!btn) return;
  try {
    await invoke("open_browser", { url: "https://login.tailscale.com/admin/machines" });
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"));
  }
});

els.btnRefresh.addEventListener("click", async () => {
  els.btnRefresh.classList.add("loading");
  await refresh();
  els.btnRefresh.classList.remove("loading");
});

function applyFilter() {
  const q = query.trim().toLowerCase();
  for (const group of els.content.querySelectorAll(".group")) {
    let visible = 0;
    for (const card of group.querySelectorAll(".machine")) {
      const hit = !q || card.dataset.search.includes(q);
      card.classList.toggle("hidden", !hit);
      if (hit) visible++;
    }
    group.classList.toggle("hidden", visible === 0);
  }
  for (const card of els.content.querySelectorAll(".wrap > .machine")) {
    const hit = !q || card.dataset.search.includes(q);
    card.classList.toggle("hidden", !hit);
  }
}

els.search.addEventListener("input", () => {
  query = els.search.value;
  applyFilter();
});

document.addEventListener("keydown", (e) => {
  if (e.key === "/" && document.activeElement !== els.search) {
    e.preventDefault();
    els.search.focus();
  }
});

const NODE_COLOR = "#e07a50";
const OFFLINE_COLOR = "#4a5057";

const NET = {
  allNodes: [],
  nodes: [],
  links: [],
  self: null,
  ctx: null,
  W: 900,
  H: 540,
  active: false,
  raf: 0,
  lastT: 0,
  filter: "all",
  hover: null,
  drag: null,
};

function hexA(hex, alpha) {
  const n = parseInt(hex.slice(1), 16);
  return `rgba(${(n >> 16) & 255}, ${(n >> 8) & 255}, ${n & 255}, ${alpha})`;
}

function netSync(st) {
  const all = [];
  if (st.self_device) all.push(st.self_device);
  for (const p of st.peers) all.push(p);

  const prev = new Map(NET.allNodes.map((n) => [n.id, n]));
  const nextAll = [];
  const now = performance.now();

  for (const p of all) {
    const id = p.ipv4 || p.dns_name || p.hostname;
    let n = prev.get(id);
    if (!n) {
      const ang = Math.random() * Math.PI * 2;
      const rad = Math.min(NET.W, NET.H) * 0.3;
      n = {
        id,
        born: now,
        x: NET.W / 2 + Math.cos(ang) * rad + (Math.random() * 40 - 20),
        y: NET.H / 2 + Math.sin(ang) * rad + (Math.random() * 40 - 20),
        vx: 0,
        vy: 0,
      };
    }
    n.peer = p;
    n.r = p.is_self ? 11 : 7.5;
    nextAll.push(n);
  }

  NET.allNodes = nextAll;
  applyNetFilter();
}

function applyNetFilter() {
  const f = NET.filter;

  NET.nodes = NET.allNodes.filter(
    (n) =>
      n.peer.is_self ||
      f === "all" ||
      (f === "online" ? n.peer.online : !n.peer.online)
  );
  NET.self = NET.nodes.find((n) => n.peer.is_self) || null;

  NET.links = [];
  if (NET.self) {
    for (const n of NET.nodes) {
      if (n !== NET.self) NET.links.push({ a: NET.self, b: n, mesh: false });
    }
    for (let i = 0; i < NET.nodes.length; i++) {
      for (let j = i + 1; j < NET.nodes.length; j++) {
        const a = NET.nodes[i], b = NET.nodes[j];
        if (a !== NET.self && b !== NET.self && a.peer.online && b.peer.online) {
          NET.links.push({ a, b, mesh: true });
        }
      }
    }
  }

  const hasPeers = NET.nodes.some((n) => !n.peer.is_self);
  if (!hasPeers) {
    NET.nodes = [];
    NET.self = null;
    NET.links = [];
  }

  els.netEmpty.classList.toggle("hidden", !NET.active || hasPeers);
}

function tick(dt) {
  const ns = NET.nodes;
  const friction = Math.pow(0.87, dt);

  for (let i = 0; i < ns.length; i++) {
    for (let j = i + 1; j < ns.length; j++) {
      const a = ns[i], b = ns[j];
      let dx = a.x - b.x, dy = a.y - b.y;
      if (dx === 0 && dy === 0) dx = 0.01;
      const d2 = dx * dx + dy * dy;
      const d = Math.sqrt(d2);
      const f = Math.min(2400 / Math.max(d2, 400), 1.1) * dt;
      const fx = (dx / d) * f, fy = (dy / d) * f;
      a.vx += fx; a.vy += fy;
      b.vx -= fx; b.vy -= fy;
    }
  }

  for (const l of NET.links) {
    const rest = l.mesh ? 185 : 140;
    const k = (l.mesh ? 0.005 : 0.018) * dt;
    let dx = l.b.x - l.a.x, dy = l.b.y - l.a.y;
    if (dx === 0 && dy === 0) dx = 0.01;
    const d = Math.hypot(dx, dy) || 0.01;
    const f = (d - rest) * k;
    const fx = (dx / d) * f, fy = (dy / d) * f;
    l.a.vx += fx; l.a.vy += fy;
    l.b.vx -= fx; l.b.vy -= fy;
  }

  for (const n of ns) {
    if (n === NET.drag) continue;

    n.vx += (NET.W / 2 - n.x) * 0.0025 * dt;
    n.vy += (NET.H / 2 - n.y) * 0.0038 * dt;
    n.vx *= friction;
    n.vy *= friction;
    if (Math.abs(n.vx) < 0.008) n.vx = 0;
    if (Math.abs(n.vy) < 0.008) n.vy = 0;
    const sp = Math.hypot(n.vx, n.vy);
    if (sp > 5.5 * dt) { n.vx *= (5.5 * dt) / sp; n.vy *= (5.5 * dt) / sp; }
    n.x += n.vx * dt;
    n.y += n.vy * dt;

    const padX = n.r + 30;
    const padTop = n.r + 34;
    const padBottom = n.r + 26;
    n.x = Math.max(padX, Math.min(NET.W - padX, n.x));
    n.y = Math.max(padTop, Math.min(NET.H - padBottom, n.y));
  }
}

function nodeAlpha(n, time) {
  const k = Math.min(1, Math.max(0, (time - n.born) / 600));
  return 1 - (1 - k) * (1 - k);
}

function drawNodeLabel(n, online, alpha) {
  const name = (n.peer.hostname || n.peer.dns_name || "?").slice(0, 22);
  const ctx = NET.ctx;
  ctx.font = "11px 'Segoe UI', system-ui";
  ctx.textAlign = "center";
  ctx.fillStyle = hexA(online ? "#a7acb3" : "#585e66", alpha);
  ctx.fillText(name, n.x, n.y + n.r + 15);
}

function draw(time) {
  const ctx = NET.ctx;
  ctx.clearRect(0, 0, NET.W, NET.H);

  ctx.lineWidth = 1;
  for (const l of NET.links) {
    if (!l.mesh) continue;
    const a = Math.min(nodeAlpha(l.a, time), nodeAlpha(l.b, time)) * 0.45;
    ctx.strokeStyle = `rgba(66, 72, 80, ${a})`;
    ctx.beginPath();
    ctx.moveTo(l.a.x, l.a.y);
    ctx.lineTo(l.b.x, l.b.y);
    ctx.stroke();
  }

  ctx.lineWidth = 1.25;
  for (const l of NET.links) {
    if (l.mesh) continue;
    const online = l.b.peer.online;
    const base = online ? 0.32 : 0.15;
    const a = base * nodeAlpha(l.b, time);
    ctx.strokeStyle = hexA(online ? NODE_COLOR : OFFLINE_COLOR, a);
    ctx.beginPath();
    ctx.moveTo(l.a.x, l.a.y);
    ctx.lineTo(l.b.x, l.b.y);
    ctx.stroke();
  }

  const pulse = 1 + Math.sin(time / 900) * 0.05;

  for (const n of NET.nodes) {
    const online = n.peer.is_self || n.peer.online;
    const col = online ? NODE_COLOR : OFFLINE_COLOR;
    const alpha = nodeAlpha(n, time);
    const r = n.r * (online ? pulse : 1);

    drawNode(n, r, col, alpha, online);
  }
}

function drawNode(n, r, col, alpha, online) {
  const ctx = NET.ctx;
  ctx.globalAlpha = alpha;
  if (online) {
    ctx.shadowColor = hexA(col, 0.5);
    ctx.shadowBlur = 10;
  }
  ctx.fillStyle = col;
  ctx.beginPath();
  ctx.arc(n.x, n.y, r, 0, Math.PI * 2);
  ctx.fill();
  ctx.shadowBlur = 0;

  ctx.fillStyle = "#131518";
  ctx.beginPath();
  ctx.arc(n.x, n.y, r * 0.42, 0, Math.PI * 2);
  ctx.fill();

  if (n === NET.hover) {
    ctx.strokeStyle = `rgba(231, 229, 225, ${0.55 * alpha})`;
    ctx.lineWidth = 1.5;
    ctx.beginPath();
    ctx.arc(n.x, n.y, r + 4.5, 0, Math.PI * 2);
    ctx.stroke();
  }

  drawNodeLabel(n, online, alpha);
  ctx.globalAlpha = 1;
}

function frame(ts) {
  if (!NET.active) return;
  const dt = Math.min(Math.max((ts - NET.lastT) / 16.67, 0.25), 2);
  NET.lastT = ts;
  tick(dt);
  draw(ts);
  NET.raf = requestAnimationFrame(frame);
}

function startNet() {
  if (NET.active) return;
  NET.active = true;
  sizeCanvas();
  applyNetFilter();
  NET.lastT = performance.now();
  NET.raf = requestAnimationFrame(frame);
}

function stopNet() {
  NET.active = false;
  cancelAnimationFrame(NET.raf);
  els.netTip.classList.add("hidden");
}

function sizeCanvas() {
  const rect = els.viewNet.getBoundingClientRect();
  const dpr = window.devicePixelRatio || 1;
  NET.W = Math.max(rect.width, 100);
  NET.H = Math.max(rect.height, 100);
  els.netCanvas.width = NET.W * dpr;
  els.netCanvas.height = NET.H * dpr;
  NET.ctx = els.netCanvas.getContext("2d");
  NET.ctx.setTransform(dpr, 0, 0, dpr, 0, 0);

  for (const n of NET.nodes) {
    n.x = Math.min(Math.max(n.x, n.r + 30), NET.W - n.r - 30);
    n.y = Math.min(Math.max(n.y, n.r + 34), NET.H - n.r - 26);
  }
}

window.addEventListener("resize", () => {
  if (NET.active) sizeCanvas();
});

function canvasPos(e) {
  const rect = els.netCanvas.getBoundingClientRect();
  return { x: e.clientX - rect.left, y: e.clientY - rect.top };
}

function pickNode(pos) {
  let best = null;
  let bestD = Infinity;
  for (const n of NET.nodes) {
    const d = Math.hypot(n.x - pos.x, n.y - pos.y);
    if (d < n.r + 8 && d < bestD) {
      best = n;
      bestD = d;
    }
  }
  return best;
}

function showTip(n) {
  const p = n.peer;
  const ip = p.ipv4 || p.ipv6 || "";
  const state = p.is_self
    ? t("tip.self")
    : p.online
      ? t("tip.online")
      : t("tip.offline");
  els.netTip.innerHTML =
    `<b>${esc(p.hostname || p.dns_name || "?")}</b>` +
    (ip ? `<span class="tip-ip">${esc(ip)}</span><br>` : "") +
    esc(state);
  els.netTip.style.left = `${n.x}px`;
  els.netTip.style.top = `${n.y}px`;
  els.netTip.classList.remove("hidden");
}

els.netCanvas.addEventListener("pointermove", (e) => {
  const pos = canvasPos(e);
  if (NET.drag) {
    NET.drag.x = pos.x;
    NET.drag.y = pos.y;
    NET.drag.vx = 0;
    NET.drag.vy = 0;
    showTip(NET.drag);
    return;
  }
  const picked = pickNode(pos);
  if (picked !== NET.hover) {
    NET.hover = picked;
    els.netCanvas.style.cursor = picked ? "grab" : "default";
    if (picked) showTip(picked);
    else els.netTip.classList.add("hidden");
  } else if (picked) {
    showTip(picked);
  }
});

els.netCanvas.addEventListener("pointerdown", (e) => {
  const pos = canvasPos(e);
  const picked = pickNode(pos);
  NET.downPos = picked ? pos : null;
  if (picked) {
    NET.drag = picked;
    els.netCanvas.setPointerCapture(e.pointerId);
    els.netCanvas.style.cursor = "grabbing";
  }
});

els.netCanvas.addEventListener("pointerup", (e) => {
  const wasDrag = NET.drag;
  const startPos = NET.downPos;
  NET.drag = null;
  NET.downPos = null;
  els.netCanvas.style.cursor = NET.hover ? "grab" : "default";
  if (!wasDrag || !startPos) return;
  const pos = canvasPos(e);
  const ip = wasDrag.peer ? wasDrag.peer.ipv4 || wasDrag.peer.ipv6 || "" : "";
  if (Math.hypot(pos.x - startPos.x, pos.y - startPos.y) < 5 && ip) {
    copyText(ip);
    toast(`${ip} - ${t("toast.copied").toLowerCase()}`);
  }
});

els.netCanvas.addEventListener("pointerleave", () => {
  NET.hover = null;
  els.netTip.classList.add("hidden");
});

function setView(name) {
  const isNet = name === "net";
  for (const btn of document.querySelectorAll(".tab-btn")) {
    btn.classList.toggle("active", btn.dataset.view === name);
  }
  els.viewList.classList.toggle("hidden", isNet);
  els.viewNet.classList.toggle("hidden", !isNet);
  if (isNet) startNet();
  else stopNet();
}

for (const btn of document.querySelectorAll(".tab-btn")) {
  btn.addEventListener("click", () => setView(btn.dataset.view));
}

for (const btn of document.querySelectorAll(".nf-btn")) {
  btn.addEventListener("click", () => {
    if (NET.filter === btn.dataset.filter) return;
    NET.filter = btn.dataset.filter;
    for (const b of document.querySelectorAll(".nf-btn")) {
      b.classList.toggle("active", b === btn);
    }
    NET.hover = null;
    NET.drag = null;
    els.netTip.classList.add("hidden");
    applyNetFilter();
  });
}

function findPeer(id) {
  const n = NET.allNodes.find((n) => n.id === id);
  return n ? n.peer : null;
}

function hideRowMenu() {
  els.rowMenu.classList.add("hidden");
}

function openRowMenu(btn) {
  const card = btn.closest(".machine");
  const p = card ? findPeer(card.dataset.id) : null;
  if (!p) return;

  const items = [];
  if (p.ipv6)
    items.push({ label: t("menu.ipv6"), act: "copy", val: p.ipv6 });
  if (p.dns_name && p.dns_name !== p.hostname)
    items.push({ label: t("menu.dns"), act: "copy", val: p.dns_name });
  if ((p.os || "").toLowerCase() === "windows" && (p.ipv4 || p.ipv6))
    items.push({
      label: isEmbeddedRdp() ? t("menu.rdpEmb") : t("menu.rdpWin"),
      act: "rdp",
      val: p.ipv4 || p.ipv6,
    });
  items.push({ sep: true });
  items.push({ label: t("menu.taildrop"), act: "taildrop", val: card.dataset.id });

  els.rowMenu.innerHTML = items
    .map((it, i) =>
      it.sep
        ? '<div class="rm-sep"></div>'
        : `<button class="rm-item" data-i="${i}">${esc(it.label)}</button>`
    )
    .join("");
  els.rowMenu._items = items;

  const r = btn.getBoundingClientRect();
  const left = Math.max(8, Math.min(r.right - 235, window.innerWidth - 245));
  const top = Math.min(r.bottom + 6, window.innerHeight - 190);
  els.rowMenu.style.left = `${left}px`;
  els.rowMenu.style.top = `${top}px`;
  els.rowMenu.classList.remove("hidden");
}

els.rowMenu.addEventListener("click", async (ev) => {
  const btn = ev.target.closest("[data-i]");
  if (!btn) return;
  const item = (els.rowMenu._items || [])[Number(btn.dataset.i)];
  hideRowMenu();
  if (!item || item.sep) return;

  try {
    if (item.act === "copy") {
      await copyText(item.val);
      toast(t("toast.copied"));
    } else if (item.act === "rdp") {
      if (isEmbeddedRdp()) {
        openRdp(item.val);
      } else {
        toast(t("menu.rdpOpenWin"));
        await invoke("open_rdp", { ip: item.val });
      }
    } else if (item.act === "taildrop") {
      const p = findPeer(item.val);
      const host = p ? p.dns_name || p.ipv4 : null;
      if (!host) return;
      const path = await invoke("pick_file");
      if (!path) return;
      toast(t("taildrop.sending", { h: host }));
      await invoke("taildrop_send", { host, path });
      toast(t("taildrop.sent", { h: host }));
    }
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"));
  }
});

document.addEventListener("pointerdown", (ev) => {
  if (!els.rowMenu.classList.contains("hidden")) {
    if (!els.rowMenu.contains(ev.target) && !ev.target.closest('[data-action="more"]')) {
      hideRowMenu();
    }
  }
});

function updateConnUI() {
  const st = lastStatus;
  if (!st) {
    els.connText.textContent = "-";
    els.btnConn.textContent = "…";
    return;
  }
  const up = st.backend_state === "Running";
  const dotCls = up ? "on" : "warn";
  const txt = up
    ? t("conn.connected")
    : st.backend_state === "Stopped"
      ? t("conn.stopped")
      : t("conn.off");
  els.connText.innerHTML = `<span class="dot ${dotCls}"></span>${esc(txt)}`;
  els.btnConn.textContent = up ? t("conn.disconnectBtn") : t("conn.connectBtn");
  els.btnConn.classList.toggle("off", !up);
  els.panelVersion.textContent =
    `tailscale ${(st.version || "").split("-")[0]}` +
    (st.exit_node ? ` · ${t("exit.via", { ip: st.exit_node })}` : "");
}

async function loadExitNodes() {
  els.exitSelect.innerHTML = `<option value="">${esc(t("exit.loading"))}</option>`;
  try {
    const list = await invoke("list_exit_nodes");
    const cur = (lastStatus && lastStatus.exit_node) || "";
    let html = `<option value="">${esc(t("exit.none"))}</option>`;
    for (const n of list) {
      const label = `${n.location ? n.location + " · " : ""}${n.host}`;
      html += `<option value="${esc(n.host)}"${n.ip && n.ip === cur ? " selected" : ""}>${esc(label)}</option>`;
    }
    if (cur && !list.some((n) => n.ip === cur)) {
      html += `<option value="" selected disabled>${esc(t("exit.current", { ip: cur }))}</option>`;
    }
    if (!list.length) {
      html += `<option value="" disabled>${esc(t("exit.noneShared"))}</option>`;
    }
    els.exitSelect.innerHTML = html;
  } catch (e) {
    const msg = esc(String(e)).slice(0, 80);
    els.exitSelect.innerHTML = `<option value="">${esc(t("exit.unavail", { m: msg }))}</option>`;
  }
}

els.exitSelect.addEventListener("change", async () => {
  const v = els.exitSelect.value;
  try {
    await invoke("set_exit_node", { node: v });
    toast(v ? t("exit.applied", { h: v }) : t("exit.cleared"));
    await refresh();
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"));
    loadExitNodes();
  }
});

els.btnConn.addEventListener("click", async () => {
  const goingUp = !(lastStatus && lastStatus.backend_state === "Running");
  els.btnConn.disabled = true;
  try {
    await invoke("toggle_tailscale", { up: goingUp });
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"));
  }
  await refresh();
  updateConnUI();
  els.btnConn.disabled = false;
});

els.btnNetcheck.addEventListener("click", async () => {
  els.btnNetcheck.disabled = true;
  els.btnNetcheck.textContent = t("diag.running");
  els.netcheckOut.innerHTML = "";
  try {
    const nc = await invoke("netcheck");
    els.netcheckOut.innerHTML = netcheckHtml(nc);
  } catch (e) {
    els.netcheckOut.innerHTML = `<p class="hint">${esc(String(e))}</p>`;
  } finally {
    els.btnNetcheck.disabled = false;
    els.btnNetcheck.textContent = t("diag.run");
  }
});

function ynCell(v, label) {
  if (v === true)
    return `<span class="nc-k">${label}</span><span class="nc-v ok"><span class="nc-dot"></span>${esc(t("common.yes"))}</span>`;
  if (v === false)
    return `<span class="nc-k">${label}</span><span class="nc-v ko"><span class="nc-dot"></span>${esc(t("common.no"))}</span>`;
  return `<span class="nc-k">${label}</span><span class="nc-v">-</span>`;
}

function netcheckHtml(nc) {
  let h = '<div class="nc-grid">';
  h += ynCell(nc.udp, "UDP");
  h += ynCell(nc.ipv4, "IPv4");
  h += ynCell(nc.ipv6, "IPv6");
  h += ynCell(nc.nat_varies, t("diag.natVaries"));
  for (const [k, v] of [["UPnP", nc.upnp], ["PMP", nc.pmp], ["PCP", nc.pcp]]) {
    if (v) h += `<span class="nc-k">${k}</span><span class="nc-v">${esc(v)}</span>`;
  }
  if (nc.preferred)
    h += `<span class="nc-k">${esc(t("diag.pref"))}</span><span class="nc-v">${esc(nc.preferred)}</span>`;
  if (nc.port_map)
    h += `<span class="nc-k">${esc(t("diag.portMap"))}</span><span class="nc-v">${esc(nc.port_map)}</span>`;
  h += "</div>";
  if (nc.derps && nc.derps.length) {
    h +=
      '<div class="derp-chips">' +
      nc.derps
        .map(
          (d) =>
            `<span class="derp-chip${d.region === nc.preferred ? " pref" : ""}">${esc(d.region)}${d.name ? ` <i>${esc(d.name)}</i>` : ""}<b>${Math.round(d.ms)} ms</b></span>`
        )
        .join("") +
      "</div>";
  }
  return h;
}

function openPanel() {
  els.panel.classList.add("open");
  els.backdrop.classList.remove("hidden");
  updateConnUI();
  loadExitNodes();
  loadProfiles();
}

function closePanel() {
  els.panel.classList.remove("open");
  els.backdrop.classList.add("hidden");
}

els.btnPanel.addEventListener("click", openPanel);
els.panelClose.addEventListener("click", closePanel);
els.backdrop.addEventListener("click", closePanel);

document.addEventListener("keydown", (e) => {
  if (e.key === "Escape") {
    closePanel();
    hideRowMenu();
  }
});

function isEmbeddedRdp() {
  return localStorage.getItem("bc_rdp_embedded") !== "0";
}

els.optRdp.checked = isEmbeddedRdp();
els.optRdp.addEventListener("change", () => {
  localStorage.setItem("bc_rdp_embedded", els.optRdp.checked ? "1" : "0");
});

els.optLang.addEventListener("change", () => {
  const l = els.optLang.value === "fr" ? "fr" : "en";
  localStorage.setItem("bc_lang", l);
  window.BC.setLang(l);
  loadProfiles();
  if (lastStatus) render(lastStatus);
  updateConnUI();
});

function updatesEnabled() {
  return localStorage.getItem("bc_autoupdate") !== "0";
}
els.optUpdates.checked = updatesEnabled();
els.optUpdates.addEventListener("change", () => {
  localStorage.setItem("bc_autoupdate", els.optUpdates.checked ? "1" : "0");
});

let updVersion = null;

function setUpdLine(txt) {
  els.updLine.textContent = txt;
}

async function checkUpdates(manual) {
  if (!invoke) return;
  els.btnUpd.disabled = true;
  els.btnUpd.textContent = t("set.checking");
  try {
    const v = await invoke("updater_check");
    if (v) {
      updVersion = v;
      setUpdLine(t("set.available", { v }));
      els.btnUpd.textContent = t("set.install");
      els.btnUpd.dataset.mode = "install";
    } else {
      updVersion = null;
      if (manual) setUpdLine(t("set.uptodate"));
      els.btnUpd.textContent = t("set.checkNow");
      els.btnUpd.dataset.mode = "check";
    }
  } catch (e) {
    if (manual) setUpdLine(t("set.checkFail", { m: String(e) }));
    els.btnUpd.textContent = t("set.checkNow");
    els.btnUpd.dataset.mode = "check";
  } finally {
    els.btnUpd.disabled = false;
  }
}

els.btnUpd.addEventListener("click", async () => {
  if (els.btnUpd.dataset.mode === "install" && updVersion) {
    els.btnUpd.disabled = true;
    els.btnUpd.textContent = t("set.installing");
    try {
      await invoke("updater_install");
    } catch (e) {
      setUpdLine(String(e));
      els.btnUpd.disabled = false;
      els.btnUpd.textContent = t("set.install");
    }
  } else {
    await checkUpdates(true);
  }
});

async function loadProfiles() {
  if (!invoke) return;
  try {
    const list = await invoke("list_profiles");
    if (!list.length) {
      els.tailnetSelect.disabled = true;
      els.tailnetSelect.innerHTML = `<option>${esc(t("switch.none"))}</option>`;
      return;
    }
    els.tailnetSelect.disabled = false;
    els.tailnetSelect.innerHTML = list
      .map(
        (p) =>
          `<option value="${esc(p.id)}"${p.current ? " selected" : ""}>${esc(p.tailnet)}</option>`
      )
      .join("");
  } catch {
    els.tailnetSelect.disabled = true;
    els.tailnetSelect.innerHTML = `<option value="">tailnet</option>`;
  }
}

els.tailnetSelect.addEventListener("change", async () => {
  const id = els.tailnetSelect.value;
  if (!id) return;
  els.tailnetSelect.disabled = true;
  toast(t("switch.switching"));
  try {
    await invoke("switch_profile", { id });
    await Promise.all([refresh(), loadProfiles()]);
    toast(t("switch.done", { n: els.tailnetSelect.selectedOptions[0]?.textContent || "" }));
    setTimeout(() => {
      if (!document.hidden) refresh();
    }, 4000);
  } catch (e) {
    toast(typeof e === "string" ? e : t("toast.failed"), 6000);
    loadProfiles();
  } finally {
    if (els.tailnetSelect.value) els.tailnetSelect.disabled = false;
  }
});

const SCANCODES = {
  Escape: 0x01,
  Digit1: 0x02, Digit2: 0x03, Digit3: 0x04, Digit4: 0x05, Digit5: 0x06,
  Digit6: 0x07, Digit7: 0x08, Digit8: 0x09, Digit9: 0x0a, Digit0: 0x0b,
  Minus: 0x0c, Equal: 0x0d, Backspace: 0x0e, Tab: 0x0f,
  KeyQ: 0x10, KeyW: 0x11, KeyE: 0x12, KeyR: 0x13, KeyT: 0x14, KeyY: 0x15,
  KeyU: 0x16, KeyI: 0x17, KeyO: 0x18, KeyP: 0x19,
  BracketLeft: 0x1a, BracketRight: 0x1b, Enter: 0x1c, ControlLeft: 0x1d,
  KeyA: 0x1e, KeyS: 0x1f, KeyD: 0x20, KeyF: 0x21, KeyG: 0x22, KeyH: 0x23,
  KeyJ: 0x24, KeyK: 0x25, KeyL: 0x26,
  Semicolon: 0x27, Quote: 0x28, Backquote: 0x29, ShiftLeft: 0x2a, Backslash: 0x2b,
  KeyZ: 0x2c, KeyX: 0x2d, KeyC: 0x2e, KeyV: 0x2f, KeyB: 0x30, KeyN: 0x31, KeyM: 0x32,
  Comma: 0x33, Period: 0x34, Slash: 0x35, ShiftRight: 0x36, NumpadMultiply: 0x37,
  AltLeft: 0x38, Space: 0x39, CapsLock: 0x3a,
  F1: 0x3b, F2: 0x3c, F3: 0x3d, F4: 0x3e, F5: 0x3f, F6: 0x40,
  F7: 0x41, F8: 0x42, F9: 0x43, F10: 0x44,
  NumLock: 0x45, ScrollLock: 0x46,
  ArrowUp: 0x48, ArrowDown: 0x50, ArrowLeft: 0x4b, ArrowRight: 0x4d,
  Home: 0x47, End: 0x4f, PageUp: 0x49, PageDown: 0x51, Insert: 0x52, Delete: 0x53,
  Numpad7: 0x47, Numpad8: 0x48, Numpad9: 0x49, NumpadSubtract: 0x4a,
  Numpad4: 0x4b, Numpad5: 0x4c, Numpad6: 0x4d, NumpadAdd: 0x4e,
  Numpad1: 0x4f, Numpad2: 0x50, Numpad3: 0x51, Numpad0: 0x52, NumpadDecimal: 0x53,
  NumpadEnter: 0x1c, NumpadDivide: 0x35,
  ControlRight: 0x1d, AltRight: 0x38, MetaLeft: 0x5b, MetaRight: 0x5c, ContextMenu: 0x5d,
  F11: 0x57, F12: 0x58,
};

const EXTENDED_KEYS = new Set([
  "ControlRight", "AltRight", "MetaLeft", "MetaRight", "ContextMenu",
  "ArrowUp", "ArrowDown", "ArrowLeft", "ArrowRight",
  "Home", "End", "PageUp", "PageDown", "Insert", "Delete",
  "NumpadEnter", "NumpadDivide",
]);

const RDP = { target: null, remote: { w: 0, h: 0 }, connected: false };

function setRdpState(text, dotClass) {
  els.rdpState.textContent = text;
  els.rdpDot.className = `dot ${dotClass || "warn"}`;
}

function openRdp(target) {
  RDP.target = target;
  els.rdpTitle.textContent = `Taildesk - ${target}`;
  els.rdpAuthHost.textContent = target;
  els.rdpPass.value = "";
  els.rdpErr.textContent = "";
  els.rdpImg.classList.add("hidden");
  els.rdpImg.src = "";
  els.rdpAuth.classList.remove("hidden");
  RDP.connected = false;
  setRdpState(t("rdp.waitingCreds"), "warn");
  els.rdp.classList.remove("hidden");
  els.rdpUser.focus();
}

function closeRdp() {
  if (!els.rdp.classList.contains("hidden")) {
    invoke && invoke("rdp_stop").catch(() => {});
  }
  RDP.connected = false;
  RDP.target = null;
  els.rdp.classList.add("hidden");
  els.rdpImg.src = "";
}

els.rdpClose.addEventListener("click", closeRdp);

function rdpSize() {
  const scale = Math.min(window.devicePixelRatio || 1, 1.5);
  const width = Math.max(1024, Math.min(1600, Math.round((window.innerWidth - 40) * scale)));
  const height = Math.max(640, Math.min(1000, Math.round((window.innerHeight - 90) * scale)));
  return { width, height };
}

els.rdpAuth.addEventListener("submit", async (ev) => {
  ev.preventDefault();
  const user = els.rdpUser.value.trim();
  const pass = els.rdpPass.value;
  if (!user || !pass) {
    els.rdpErr.textContent = t("rdp.missingCreds");
    return;
  }
  const submitBtn = els.rdpAuth.querySelector(".rdp-submit");
  submitBtn.disabled = true;
  submitBtn.textContent = t("rdp.connecting");
  els.rdpErr.textContent = "";
  setRdpState(t("rdp.connecting"), "warn");
  try {
    const size = rdpSize();
    const resp = await invoke("rdp_start", {
      target: RDP.target,
      username: user,
      password: pass,
      width: size.width,
      height: size.height,
    });
    RDP.remote = { w: resp.width, h: resp.height };
    els.rdpImg.src = `http://127.0.0.1:${resp.port}/stream`;
    els.rdpAuth.classList.add("hidden");
    els.rdpImg.classList.remove("hidden");
    RDP.connected = true;
    setRdpState(t("rdp.active"), "on");
    els.rdpStage.focus();
  } catch (e) {
    els.rdpErr.textContent = typeof e === "string" ? e : t("rdp.failed");
    setRdpState(t("rdp.failed"), "err");
  } finally {
    submitBtn.disabled = false;
    submitBtn.textContent = t("rdp.submit");
  }
});

if (Tauri && Tauri.event) {
  Tauri.event.listen("rdp-status", (ev) => {
    const p = ev.payload || {};
    if (els.rdp.classList.contains("hidden")) return;
    const stages = {
      connect: t("rdp.connecting"),
      negotiate: t("rdp.connecting"),
      tls: t("rdp.connecting"),
      auth: t("rdp.connecting"),
      active: t("rdp.active"),
    };
    const localized = !p.error && stages[p.stage] !== undefined ? stages[p.stage] : p.message;
    setRdpState(localized || "", p.error ? "err" : p.stage === "active" ? "on" : "warn");
    if (p.error) {
      if (els.rdpAuth.classList.contains("hidden")) {
        els.rdpImg.classList.add("hidden");
        els.rdpImg.src = "";
        els.rdpAuth.classList.remove("hidden");
        RDP.connected = false;
      }
      els.rdpErr.textContent = p.message || "";
    }
  });
}

function rdpSend(evt) {
  if (RDP.connected && invoke) invoke("rdp_input", evt).catch(() => {});
}

function imgPos(e) {
  const r = els.rdpImg.getBoundingClientRect();
  const nw = els.rdpImg.naturalWidth || RDP.remote.w || r.width;
  const nh = els.rdpImg.naturalHeight || RDP.remote.h || r.height;
  return {
    x: (e.clientX - r.left) * (nw / r.width),
    y: (e.clientY - r.top) * (nh / r.height),
  };
}

els.rdpImg.addEventListener("contextmenu", (e) => e.preventDefault());

els.rdpImg.addEventListener("pointermove", (e) => {
  const p = imgPos(e);
  rdpSend({ kind: "move", x: p.x, y: p.y });
});

els.rdpImg.addEventListener("pointerdown", (e) => {
  e.preventDefault();
  els.rdpStage.focus();
  const p = imgPos(e);
  rdpSend({
    kind: "button",
    button: ["left", "middle", "right"][e.button] || "left",
    down: true,
    x: p.x,
    y: p.y,
  });
});

els.rdpImg.addEventListener("pointerup", (e) => {
  const p = imgPos(e);
  rdpSend({
    kind: "button",
    button: ["left", "middle", "right"][e.button] || "left",
    down: false,
    x: p.x,
    y: p.y,
  });
});

els.rdpImg.addEventListener(
  "wheel",
  (e) => {
    e.preventDefault();
    rdpSend({ kind: "wheel", dy: e.deltaY < 0 ? 3 : -3 });
  },
  { passive: false }
);

els.rdpStage.addEventListener("keydown", (e) => {
  if (!els.rdpAuth.classList.contains("hidden")) return;
  const sc = SCANCODES[e.code];
  if (sc === undefined) return;
  e.preventDefault();
  rdpSend({
    kind: "key",
    scancode: sc,
    extended: EXTENDED_KEYS.has(e.code),
    release: false,
  });
});

els.rdpStage.addEventListener("keyup", (e) => {
  const sc = SCANCODES[e.code];
  if (sc === undefined) return;
  e.preventDefault();
  rdpSend({
    kind: "key",
    scancode: sc,
    extended: EXTENDED_KEYS.has(e.code),
    release: true,
  });
});

let refreshing = false;

async function refresh() {
  if (!invoke) {
    showBanner(t("banner.noBridge"), true);
    setStatus("err", t("st.browserMode"));
    return;
  }
  if (refreshing) return;
  refreshing = true;
  try {
    const st = await invoke("get_status");
    render(st);
  } catch (e) {
    setStatus("err", t("st.err"));
    showBanner(t("banner.fail", { e: esc(String(e)) }), true);
  } finally {
    refreshing = false;
  }
}

setInterval(() => {
  if (!document.hidden) refresh();
}, 10000);

document.addEventListener("visibilitychange", () => {
  if (!document.hidden) refresh();
});

(async () => {
  let stored = null;
  try {
    stored = localStorage.getItem("bc_lang");
  } catch {}

  if (stored !== "en" && stored !== "fr") {
    stored = invoke ? await invoke("get_default_lang").catch(() => "en") : "en";
    if (stored !== "fr") stored = "en";
  }

  window.BC.setLang(stored);
  els.optLang.value = stored;

  if (Tauri && Tauri.app) {
    try { appVersion = await Tauri.app.getVersion(); } catch {}
  }

  loadProfiles();
  await refresh();

  if (updatesEnabled() && invoke) {
    checkUpdates(false);
  }
})();
