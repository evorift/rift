<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import Segmented from "$lib/components/ui/Segmented.svelte";
  import { t } from "$lib/i18n.svelte";
  import { app } from "$lib/state.svelte";
  import { eventLog } from "$lib/log.svelte";

  let { onBack }: { onBack?: () => void } = $props();

  // Gerçek olay günlüğü (eski SAHTE statik satırların yerine). Servis IPC bağlanınca
  // canlı motor olaylarıyla da beslenecek (docs/03 §10 Health Check).
  const lines = $derived(eventLog.lines);

  const filters = $derived([
    { value: "all", label: t("logs.fAll") },
    { value: "info", label: t("logs.fInfo") },
    { value: "warn", label: t("logs.fWarn") },
    { value: "error", label: t("logs.fError") },
  ]);
  let filter = $state("all");

  const shown = $derived(
    filter === "all"
      ? lines
      : lines.filter((l) =>
          filter === "warn" ? l.level === "warn" : filter === "error" ? l.level === "error" : l.level === "info" || l.level === "ok"
        )
  );

  let copied = $state(false);
  // Tanılamayı kopyala — GERÇEK sistem/ağ durumu + olay günlüğü (eski sürüm sahte veri kopyalıyordu).
  async function copyDiag() {
    let net: string;
    try {
      const d = await invoke<{ dns_ok: boolean; reachable: boolean; ms: number }>("connectivity_test");
      net = `Ağ: DNS ${d.dns_ok ? "OK" : "BAŞARISIZ"} · 1.1.1.1:443 ${d.reachable ? `OK (${d.ms}ms)` : "BAŞARISIZ"}`;
    } catch (_) {
      net = "Ağ testi: çalıştırılamadı (Tauri dışı)";
    }
    let dns: string;
    try {
      const s = await invoke<{ servers: string[]; secure: boolean; provider: string }>("dns_status");
      dns = `Sistem DNS: ${s.servers.join(", ") || "—"} (${s.secure ? "güvenli: " + s.provider : "ISS/güvensiz"})`;
    } catch (_) {
      dns = "Sistem DNS: alınamadı";
    }
    const head = [
      "evorift v0.1.0 — tanılama",
      `Zaman: ${new Date().toISOString()}`,
      `Platform: ${typeof navigator !== "undefined" ? navigator.userAgent : "?"}`,
      `Dil: ${app.language} · ISS: ${app.isp} · Durum: ${app.status} · Oyun Modu: ${app.gameMode ? "açık" : "kapalı"}`,
      `Strateji: ${app.strategy} · DNS profili: ${app.dns}`,
      `Siteler (${app.sites.length}): ${app.sites.join(", ") || "—"}`,
      net,
      dns,
      `Metrik: ping ${app.ping}ms · jitter ${app.jitter}ms · kayıp ${app.loss}% · ↓${app.down} ↑${app.up} Mbps`,
      `--- Olay günlüğü (${eventLog.lines.length}) ---`,
    ];
    const body = eventLog.lines.map((l) => `[${l.t}] ${l.level.toUpperCase()} ${l.msg}`).join("\n");
    try {
      await navigator.clipboard.writeText(head.join("\n") + "\n" + body);
      copied = true;
      setTimeout(() => (copied = false), 1500);
    } catch (_) {}
  }
</script>

<header class="head">
  <button class="back" onclick={() => onBack?.()} aria-label={t("onb.back")}>←</button>
  <h1>{t("logs.title")}</h1>
  <button class="btn copy" onclick={copyDiag}>{copied ? t("logs.copied") : t("logs.copy")}</button>
</header>

<div class="bar">
  <Segmented options={filters} bind:value={filter} />
</div>

<div class="card log">
  {#if shown.length === 0}
    <div class="empty">{t("logs.empty")}</div>
  {:else}
    {#each shown as l (l.id)}
      <div class="line">
        <span class="time mono">{l.t}</span>
        <span class="tag tag-{l.level}">{l.level}</span>
        <span class="msg">{l.msg}</span>
      </div>
    {/each}
  {/if}
</div>

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 14px; }
  .head h1 { font-size: 22px; font-weight: 700; }
  .head .copy { margin-left: auto; }
  .back {
    width: 32px; height: 32px; border-radius: var(--radius-sm); cursor: pointer;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--text);
    font-size: 16px; display: grid; place-items: center; transition: background 0.12s;
  }
  .back:hover { background: var(--bg-hover); }

  .bar { margin-bottom: 14px; max-width: 360px; }

  .log { padding: 8px; font-size: 13px; max-height: calc(100vh - 240px); overflow-y: auto; }
  .line {
    display: grid; grid-template-columns: 72px 56px 1fr; gap: 10px; align-items: baseline;
    padding: 6px 10px; border-radius: 6px;
  }
  .line:hover { background: var(--bg-elevated); }
  .time { color: var(--text-dim); font-size: 12px; }
  .tag {
    font-size: 10.5px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.4px;
    text-align: center; padding: 2px 0; border-radius: 4px;
  }
  .tag-info { color: var(--text-muted); background: var(--bg-elevated); }
  .tag-ok { color: var(--green); background: color-mix(in srgb, var(--green) 14%, transparent); }
  .tag-warn { color: var(--amber); background: color-mix(in srgb, var(--amber) 14%, transparent); }
  .tag-error { color: var(--red); background: color-mix(in srgb, var(--red) 14%, transparent); }
  .msg { color: var(--text); line-height: 1.4; }
  .empty { color: var(--text-muted); padding: 16px; text-align: center; }
</style>
