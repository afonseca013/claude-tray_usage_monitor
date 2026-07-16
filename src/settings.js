const { invoke: rawInvoke } = window.__TAURI__.core;

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

const claudeTokenEl = document.getElementById("claude-token");
const claudeStatusEl = document.getElementById("claude-status");
const openaiKeyEl = document.getElementById("openai-key");
const openaiStatusEl = document.getElementById("openai-status");
const pollIntervalEl = document.getElementById("poll-interval");
const autostartEl = document.getElementById("autostart");

async function refreshClaudeStatus() {
  const has = await invoke("has_claude_token");
  claudeStatusEl.textContent = has ? "Token configurado." : "Nenhum token configurado ainda.";
}

async function refreshOpenAiStatus() {
  const has = await invoke("has_openai_key");
  openaiStatusEl.textContent = has ? "API key configurada." : "Nenhuma API key configurada.";
}

document.getElementById("claude-save").addEventListener("click", async () => {
  const token = claudeTokenEl.value.trim();
  if (!token) return;
  try {
    await invoke("set_claude_token", { token });
    claudeTokenEl.value = "";
    await refreshClaudeStatus();
    invoke("refresh_now");
  } catch (e) {
    claudeStatusEl.textContent = `Erro ao salvar: ${e}`;
  }
});

document.getElementById("claude-clear").addEventListener("click", async () => {
  try {
    await invoke("clear_claude_token");
    await refreshClaudeStatus();
  } catch (e) {
    claudeStatusEl.textContent = `Erro ao remover: ${e}`;
  }
});

document.getElementById("openai-save").addEventListener("click", async () => {
  const key = openaiKeyEl.value.trim();
  if (!key) return;
  try {
    await invoke("set_openai_key", { key });
    openaiKeyEl.value = "";
    await refreshOpenAiStatus();
    invoke("refresh_now");
  } catch (e) {
    openaiStatusEl.textContent = `Erro ao salvar: ${e}`;
  }
});

document.getElementById("openai-clear").addEventListener("click", async () => {
  try {
    await invoke("clear_openai_key");
    await refreshOpenAiStatus();
  } catch (e) {
    openaiStatusEl.textContent = `Erro ao remover: ${e}`;
  }
});

pollIntervalEl.addEventListener("change", async () => {
  const settings = await invoke("get_settings");
  settings.poll_interval_minutes = Math.max(1, Number(pollIntervalEl.value) || 5);
  await invoke("set_settings", { settings });
});

autostartEl.addEventListener("change", async () => {
  try {
    if (autostartEl.checked) {
      await invoke("plugin:autostart|enable");
    } else {
      await invoke("plugin:autostart|disable");
    }
  } catch (e) {
    console.error("autostart toggle failed", e);
  }
  const settings = await invoke("get_settings");
  settings.autostart = autostartEl.checked;
  await invoke("set_settings", { settings });
});

async function init() {
  await refreshClaudeStatus();
  await refreshOpenAiStatus();

  const settings = await invoke("get_settings");
  pollIntervalEl.value = settings.poll_interval_minutes;

  try {
    autostartEl.checked = await invoke("plugin:autostart|is_enabled");
  } catch (e) {
    autostartEl.checked = settings.autostart;
  }
}

init();
