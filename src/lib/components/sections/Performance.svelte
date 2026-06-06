<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Sparkline from "$lib/components/ui/Sparkline.svelte";

  const live = $derived(app.status === "on");

  // grafik renk seçimi (loss yüksekse kırmızıya kayar)
  const graphColor = $derived(app.loss > 1 ? "var(--red)" : "var(--accent)");

  const fmt = (v: number, on = live) => (on ? String(v) : "—");
</script>

<header class="head">
  <h1>Performans</h1>
  <span class="chip live-{live ? 'on' : 'off'}">
    <i class="dot"></i>{live ? "Canlı" : "Durdu"}
  </span>
</header>

{#if !live}
  <div class="card empty-state">
    <div class="es-icon">⏻</div>
    <h3>Ölçüm için korumayı aç</h3>
    <p>Panel'den kara deliği açtığında ping, jitter ve kayıp canlı izlenir.</p>
  </div>
{:else}
  <section class="card graph-card">
    <div class="graph-head">
      <div>
        <span class="g-lbl">Gecikme (ping)</span>
        <div class="g-now"><span class="mono">{app.ping}</span><small>ms</small></div>
      </div>
      <div class="g-meta mono">
        <span>min {Math.min(...app.pingHistory)}</span>
        <span>max {Math.max(...app.pingHistory)}</span>
      </div>
    </div>
    <Sparkline data={app.pingHistory} color={graphColor} height={120} />
  </section>

  <section class="stats">
    <div class="card stat">
      <span class="lbl">Ping</span>
      <span class="val mono">{fmt(app.ping)}<small>ms</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">Jitter</span>
      <span class="val mono">{fmt(app.jitter)}<small>ms</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">Paket kaybı</span>
      <span class="val mono" class:warn={app.loss > 1}>{fmt(app.loss)}<small>%</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">İndirme</span>
      <span class="val mono">{fmt(app.down)}<small>Mbps</small></span>
    </div>
    <div class="card stat">
      <span class="lbl">Yükleme</span>
      <span class="val mono">{fmt(app.up)}<small>Mbps</small></span>
    </div>
  </section>

  <p class="note">
    Değerler 1 Hz'de güncellenir; pencere arka plana alınınca ölçüm durur (idle ~sıfır).
    Servis IPC bağlanınca bu sayılar gerçek trafiğe geçecek.
  </p>
{/if}

<style>
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }
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
