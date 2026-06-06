<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
  import { app } from "$lib/state.svelte";

  let active = $state("dashboard");
  const status = $derived(app.status);

  const statusLabel = $derived(
    status === "on" ? "Korumalı" : status === "connecting" ? "Bağlanıyor…" : "Kapalı"
  );
  const heroTitle = $derived(
    status === "on" ? "Rift açık · Gizlisin" : status === "connecting" ? "Rift açılıyor…" : "Rift kapalı"
  );
  const heroSub = $derived(
    status === "on"
      ? "Engeller aşıldı, izin gizlendi — yalnızca seçili siteler etkileniyor, gerisi normal."
      : "Korumasızsın. Rift'i aç: tek tıkla engelleri aş. VPN yok, hız düşmez."
  );

  const sections: Record<string, string> = {
    connection: "Bağlantı & Stratejiler",
    performance: "Performans",
    apps: "Uygulamalar",
    logs: "Günlük",
    settings: "Ayarlar",
  };
</script>

<NavRail {active} onSelect={(k) => (active = k)} />

<main class="content">
  {#if active === "dashboard"}
    <header class="head">
      <h1>Panel</h1>
      <span class="chip status-{status}"><i class="dot"></i> {statusLabel}</span>
    </header>

    <section class="hero">
      <button class="power {status}" onclick={() => app.toggle()} aria-label="Korumayı aç/kapat">
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
          <path d="M12 3v9" /><path d="M6.4 7a8 8 0 1 0 11.2 0" />
        </svg>
      </button>
      <h2 class="hero-title">{heroTitle}</h2>
      <p class="hero-sub">{heroSub}</p>
    </section>

    <section class="stats">
      <div class="card stat"><span class="lbl">Ping</span><span class="val mono">{status === "on" ? "24" : "—"}<small>ms</small></span></div>
      <div class="card stat"><span class="lbl">İndirme</span><span class="val mono">{status === "on" ? "0.0" : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">Yükleme</span><span class="val mono">{status === "on" ? "0.0" : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">Strateji</span><span class="val small">{status === "on" ? "Otomatik" : "—"}</span></div>
    </section>
  {:else}
    <header class="head"><h1>{sections[active]}</h1></header>
    <div class="card placeholder">
      <p>Bu bölüm yapım aşamasında.</p>
      <span class="muted">Yakında: {sections[active]} özellikleri.</span>
    </div>
  {/if}
</main>

<style>
  .content { flex: 1 1 auto; padding: 20px 26px; overflow-y: auto; display: flex; flex-direction: column; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 8px; }
  .head h1 { font-size: 22px; font-weight: 700; }

  .chip { backdrop-filter: blur(6px); }
  .chip .dot { width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
  .status-off { color: var(--text-muted); }
  .status-connecting { color: var(--amber); }
  .status-on { color: var(--green); }

  /* HERO — arka planda rift, ortada güç butonu */
  .hero { flex: 1 1 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; }
  .power {
    width: 92px; height: 92px; border-radius: 50%; cursor: pointer;
    border: 2px solid rgba(190,210,255,.25);
    background: rgba(8,12,20,.35); color: var(--text-muted);
    backdrop-filter: blur(4px);
    display: grid; place-items: center; margin-bottom: 16px;
    transition: color .25s, border-color .25s, box-shadow .35s, transform .1s;
  }
  .power svg { width: 38px; height: 38px; }
  .power:hover { color: #fff; border-color: rgba(190,210,255,.5); }
  .power:active { transform: scale(.96); }
  .power.connecting { color: var(--amber); border-color: var(--amber); }
  .power.on {
    color: var(--accent); border-color: var(--accent);
    box-shadow: 0 0 30px -4px var(--accent-glow);
  }
  .hero-title { font-size: 28px; font-weight: 800; letter-spacing: .3px; text-shadow: 0 2px 18px rgba(0,0,0,.6); }
  .hero-sub { color: var(--text); opacity: .85; max-width: 460px; text-align: center; line-height: 1.5; text-shadow: 0 2px 14px rgba(0,0,0,.7); }

  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 10px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }

  .placeholder { display: flex; flex-direction: column; gap: 6px; }
  .placeholder .muted { color: var(--text-muted); font-size: 13px; }
</style>
