<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
  import RiftMark from "$lib/components/RiftMark.svelte";
  import { app } from "$lib/state.svelte";

  let active = $state("dashboard");
  const status = $derived(app.status);

  const statusLabel = $derived(
    status === "on" ? "Korumalı" : status === "connecting" ? "Bağlanıyor…" : "Kapalı"
  );
  const heroTitle = $derived(
    status === "on" ? "Korumalı · Gizlisin" : status === "connecting" ? "Bağlanıyor…" : "Korumasız"
  );
  const heroSub = $derived(
    status === "on"
      ? "Engeller aşıldı, izin gizlendi — yalnızca seçili siteler etkileniyor, gerisi normal."
      : "Kara deliği aç: tek tıkla engelleri aş. VPN yok, hız düşmez."
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
      <div class="bh-stage"><RiftMark /></div>
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

  /* HERO — kara delik butonu */
  .hero { flex: 1 1 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 6px; }
  .bh-stage { width: 188px; height: 188px; margin-bottom: 18px; }
  .hero-title { font-size: 26px; font-weight: 800; letter-spacing: .3px; }
  .hero-sub { color: var(--text-muted); max-width: 460px; text-align: center; line-height: 1.5; }

  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 10px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }

  .placeholder { display: flex; flex-direction: column; gap: 6px; }
  .placeholder .muted { color: var(--text-muted); font-size: 13px; }
</style>
