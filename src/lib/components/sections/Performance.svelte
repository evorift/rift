<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n.svelte";
  import Sparkline from "$lib/components/ui/Sparkline.svelte";

  let { onBack }: { onBack?: () => void } = $props();

  const live = $derived(app.status === "on");

  // grafik renk seçimi (loss yüksekse kırmızıya kayar)
  const graphColor = $derived(app.loss > 1 ? "var(--red)" : "var(--accent)");

  const fmt = (v: number, on = live) => (on ? String(v) : "—");
</script>

<header class="head">
  <button class="back" onclick={() => onBack?.()} aria-label={t("onb.back")}>←</button>
  <h1>{t("perf.title")}</h1>
  <span class="chip live-{live ? 'on' : 'off'}">
    <i class="dot"></i>{live ? t("perf.live") : t("perf.stopped")}
  </span>
</header>

{#if !live}
  <div class="card empty-state">
    <div class="es-icon">⏻</div>
    <h3>{t("perf.emptyTitle")}</h3>
    <p>{t("perf.emptyBody")}</p>
  </div>
{:else}
  <section class="card graph-card">
    <div class="graph-head">
      <div>
        <span class="g-lbl">{t("perf.latency")}</span>
        <div class="g-now"><span class="mono">{app.ping}</span><small>ms</small></div>
      </div>
      <div class="g-meta mono">
        <span>{t("perf.min")} {app.pingHistory.length ? Math.min(...app.pingHistory) : "—"}</span>
        <span>{t("perf.max")} {app.pingHistory.length ? Math.max(...app.pingHistory) : "—"}</span>
      </div>
    </div>
    <Sparkline data={app.pingHistory} color={graphColor} height={120} />
  </section>

  <section class="stats">
    <div class="card stat">
      <span class="lbl">{t("dash.ping")}</span>
      <span class="val mono">{fmt(app.ping)}<small>ms</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">{t("perf.jitter")}</span>
      <span class="val mono">{fmt(app.jitter)}<small>ms</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">{t("perf.loss")}</span>
      <span class="val mono" class:warn={app.loss > 1}>{fmt(app.loss)}<small>%</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">{t("dash.download")}</span>
      <span class="val mono">{fmt(app.down)}<small>Mbps</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">{t("dash.upload")}</span>
      <span class="val mono">{fmt(app.up)}<small>Mbps</small></span>
    </div>
  </section>

  <p class="note">{t("perf.note")}</p>
{/if}

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }
  .head .chip { margin-left: auto; }
  .back {
    width: 32px; height: 32px; border-radius: var(--radius-sm); cursor: pointer;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--text);
    font-size: 16px; display: grid; place-items: center; transition: background 0.12s;
  }
  .back:hover { background: var(--bg-hover); }
  .chip .dot { width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
  .live-on { color: var(--green); }
  .live-off { color: var(--text-muted); }

  .empty-state { display: flex; flex-direction: column; align-items: center; gap: 8px; text-align: center; padding: 48px 20px; }
  .es-icon { font-size: 34px; opacity: 0.5; }
  .empty-state h3 { font-size: 16px; font-weight: 700; }
  .empty-state p { color: var(--text-muted); max-width: 360px; line-height: 1.5; }

  .graph-card { margin-bottom: 16px; }
  .graph-head { display: flex; align-items: flex-start; justify-content: space-between; margin-bottom: 10px; }
  .g-lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; }
  .g-now { display: flex; align-items: baseline; gap: 4px; }
  .g-now .mono { font-size: 30px; font-weight: 700; }
  .g-now small { font-size: 13px; color: var(--text-muted); }
  .g-meta { display: flex; gap: 14px; color: var(--text-muted); font-size: 12px; }

  .stats { display: grid; grid-template-columns: repeat(5, 1fr); gap: 12px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 11.5px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; }
  .stat .val { font-size: 24px; font-weight: 700; }
  .stat .val small { font-size: 12px; font-weight: 600; color: var(--text-muted); margin-left: 4px; }
  .stat .val.warn { color: var(--amber); }

  .note { margin-top: 16px; color: var(--text-dim); font-size: 12px; line-height: 1.5; }
</style>
