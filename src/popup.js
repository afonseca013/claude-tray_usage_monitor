function showFatalError(message) {
  document.body.style.background = "#17171b";
  document.body.innerHTML = `<div style="color:#f87171;padding:12px;font-size:11px;white-space:pre-wrap;font-family:monospace">${message}</div>`;
}
window.addEventListener("error", (e) => {
  showFatalError(`error: ${e.message}\n${e.filename}:${e.lineno}:${e.colno}`);
});
window.addEventListener("unhandledrejection", (e) => {
  showFatalError(`unhandled rejection: ${e.reason?.message ?? e.reason}`);
});

const { invoke: rawInvoke } = window.__TAURI__.core;
const { listen } = window.__TAURI__.event;

// Windows declared in tauri.conf.json start loading/running their scripts as
// soon as they're created, which can race ahead of `app.manage()` finishing
// in the Rust setup() hook. Retry commands briefly instead of failing hard.
async function invoke(cmd, args) {
  const maxAttempts = 20;
  for (let attempt = 0; ; attempt++) {
    try {
      return await rawInvoke(cmd, args);
    } catch (e) {
      const msg = typeof e === "string" ? e : (e?.message ?? "");
      if (msg.includes("state not managed") && attempt < maxAttempts - 1) {
        await new Promise((r) => setTimeout(r, 100));
        continue;
      }
      throw e;
    }
  }
}

const PROVIDER_LABELS = {
  claude: "Claude",
  codex: "Codex",
  openai: "ChatGPT",
  antigravity: "Antigravity",
};

const SEGMENTS = 18;

const cardsEl = document.getElementById("cards");
const lastUpdateEl = document.getElementById("last-update");
const snapshots = new Map();

function statusClass(status) {
  return status.toLowerCase();
}

function barClass(status) {
  if (status === "rejected" || status === "error") return "rejected";
  if (status === "warning") return "warning";
  return "";
}

function segmentColor(pct) {
  if (pct >= 70) return "seg-red";
  if (pct >= 50) return "seg-amber";
  return "seg-green";
}

function fmtCountdown(epochSeconds) {
  if (!epochSeconds) return null;
  const diffMs = epochSeconds * 1000 - Date.now();
  if (diffMs <= 0) return "reiniciando…";
  const totalSecs = Math.floor(diffMs / 1000);
  const h = Math.floor(totalSecs / 3600);
  const m = Math.floor((totalSecs % 3600) / 60);
  const s = totalSecs % 60;
  const pad = (n) => String(n).padStart(2, "0");
  return h > 0 ? `reinicia em ${h}:${pad(m)}:${pad(s)}` : `reinicia em ${m}:${pad(s)}`;
}

function renderMeter(label, percent, status, resetAt) {
  if (percent === null || percent === undefined) return "";
  const pct = Math.min(100, Math.max(0, percent));
  const lit = Math.round((pct / 100) * SEGMENTS);
  const segs = Array.from({ length: SEGMENTS }, (_, i) =>
    `<span class="seg ${i < lit ? segmentColor(pct) : "seg-off"}"></span>`
  ).join("");

  return `
    <div class="bar-row">
      <span class="bar-label">${label}</span>
      <div class="meter ${barClass(status)}">${segs}</div>
      <span class="bar-pct">${pct.toFixed(0)}%</span>
    </div>
    ${resetAt ? `<div class="card-detail countdown" data-reset="${resetAt}" style="margin-left:42px"></div>` : ""}
  `;
}

function renderCard(snapshot) {
  const name = PROVIDER_LABELS[snapshot.provider] ?? snapshot.provider;
  const hasBars = snapshot.percent_5h != null || snapshot.percent_7d != null;

  return `
    <div class="card">
      <div class="card-head">
        <span class="dot ${statusClass(snapshot.status)}"></span>
        <span class="card-name">${name}</span>
      </div>
      ${hasBars ? renderMeter("5h", snapshot.percent_5h, snapshot.status, snapshot.reset_5h) : ""}
      ${hasBars ? renderMeter("7d", snapshot.percent_7d, snapshot.status, snapshot.reset_7d) : ""}
      ${snapshot.detail ? `<div class="card-detail">${snapshot.detail}</div>` : ""}
      ${hasBars ? `<canvas class="chart" id="chart-${snapshot.provider}" width="288" height="34"></canvas>` : ""}
      ${hasBars ? `<canvas class="heatmap" id="heatmap-${snapshot.provider}" width="288" height="24"></canvas>` : ""}
    </div>
  `;
}

function render() {
  const order = ["claude", "codex", "openai", "antigravity"];
  const items = order
    .map((id) => snapshots.get(id))
    .filter((s) => s && s.status !== "unavailable");

  cardsEl.innerHTML = items.map(renderCard).join("");

  const latest = items.reduce((max, s) => Math.max(max, s.fetched_at), 0);
  if (latest > 0) {
    lastUpdateEl.textContent = `Atualizado às ${new Date(latest * 1000).toLocaleTimeString("pt-BR")}`;
  }

  for (const s of items) {
    if (s.percent_5h != null || s.percent_7d != null) {
      drawChart(s.provider);
      drawHeatmap(s.provider);
    }
  }

  updateCountdowns();
}

function updateCountdowns() {
  document.querySelectorAll(".countdown[data-reset]").forEach((el) => {
    const text = fmtCountdown(Number(el.dataset.reset));
    el.textContent = text ?? "";
  });
}
setInterval(updateCountdowns, 1000);

async function drawChart(provider) {
  const canvas = document.getElementById(`chart-${provider}`);
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const { width, height } = canvas;
  ctx.clearRect(0, 0, width, height);

  const points = await invoke("get_history", { provider, sinceHours: 24 });
  if (!points || points.length < 2) return;

  const snapshot = snapshots.get(provider);

  const drawLine = (values, color, dashed) => {
    const valid = values.filter((v) => v[1] != null);
    if (valid.length < 2) return;
    const minT = valid[0][0];
    const maxT = valid[valid.length - 1][0];
    const span = maxT - minT || 1;
    ctx.beginPath();
    ctx.strokeStyle = color;
    ctx.lineWidth = 1.5;
    ctx.setLineDash(dashed ? [3, 3] : []);
    valid.forEach(([t, v], i) => {
      const x = ((t - minT) / span) * width;
      const y = height - (Math.min(100, v) / 100) * height;
      if (i === 0) ctx.moveTo(x, y);
      else ctx.lineTo(x, y);
    });
    ctx.stroke();
    ctx.setLineDash([]);
    return valid;
  };

  const primary = drawLine(points.map((p) => [p[0], p[1]]), "#8b8bff", false);
  drawLine(points.map((p) => [p[0], p[2]]), "rgba(139, 139, 255, 0.4)", false);

  // Projection: extrapolate the recent burn rate to the 5h reset time.
  if (primary && primary.length >= 2 && snapshot?.reset_5h) {
    const recent = primary.slice(-Math.min(6, primary.length));
    const [t0, v0] = recent[0];
    const [t1, v1] = recent[recent.length - 1];
    const dt = t1 - t0;
    if (dt > 0) {
      const rate = (v1 - v0) / dt;
      const minT = primary[0][0];
      const span = (points[points.length - 1][0] - minT) || 1;
      const projected = Math.max(0, Math.min(100, v1 + rate * (snapshot.reset_5h - t1)));
      const x0 = ((t1 - minT) / span) * width;
      const y0 = height - (Math.min(100, v1) / 100) * height;
      const x1 = width;
      const y1 = height - (projected / 100) * height;
      ctx.beginPath();
      ctx.strokeStyle = "rgba(248, 113, 113, 0.7)";
      ctx.lineWidth = 1.5;
      ctx.setLineDash([2, 3]);
      ctx.moveTo(x0, y0);
      ctx.lineTo(x1, y1);
      ctx.stroke();
      ctx.setLineDash([]);
    }
  }
}

async function drawHeatmap(provider) {
  const canvas = document.getElementById(`heatmap-${provider}`);
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  const { width, height } = canvas;
  ctx.clearRect(0, 0, width, height);

  const points = await invoke("get_history", { provider, sinceHours: 24 });
  if (!points || points.length === 0) return;

  const now = Math.floor(Date.now() / 1000);
  const bucketSecs = 3600;
  const buckets = new Array(24).fill(null);
  for (const [t, v5h] of points) {
    if (v5h == null) continue;
    const hoursAgo = Math.floor((now - t) / bucketSecs);
    if (hoursAgo < 0 || hoursAgo >= 24) continue;
    const idx = 23 - hoursAgo;
    buckets[idx] = buckets[idx] == null ? v5h : Math.max(buckets[idx], v5h);
  }

  const barW = width / 24;
  const currentIdx = 23;
  buckets.forEach((v, i) => {
    const pct = v ?? 0;
    const h = Math.max(2, (Math.min(100, pct) / 100) * height);
    const alpha = v == null ? 0.08 : 0.25 + (Math.min(100, pct) / 100) * 0.75;
    ctx.fillStyle = i === currentIdx ? `rgba(139, 139, 255, ${alpha})` : `rgba(255, 255, 255, ${alpha * 0.5})`;
    ctx.fillRect(i * barW + 1, height - h, barW - 2, h);
  });
}

async function loadInitial() {
  const list = await invoke("get_latest_snapshots");
  for (const snapshot of list) snapshots.set(snapshot.provider, snapshot);
  render();
}

listen("usage-updated", (event) => {
  snapshots.set(event.payload.provider, event.payload);
  render();
});

document.getElementById("refresh-btn").addEventListener("click", () => {
  invoke("refresh_now");
});

document.getElementById("settings-btn").addEventListener("click", () => {
  invoke("hide_window", { label: "popup" });
  invoke("show_settings_window");
});

loadInitial();
