<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
  import ParticleEmblem from "$lib/components/ParticleEmblem.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { onMount } from "svelte";

  let active = $state("dashboard");
  type Status = "off" | "connecting" | "on";
  let status = $state<Status>("off");

  const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

  type EngineStatus = { running: boolean; strategy: string };

  onMount(async () => {
    try {
      const s = await invoke<EngineStatus>("protection_status");
      status = s.running ? "on" : "off";
    } catch (_) {}
  });

  async function toggleBoost() {
    if (status === "connecting") return;
    if (status === "on") {
      try { await invoke("stop_protection"); } catch (_) {}
      status = "off";
      return;
    }
    status = "connecting";
    try {
      // backend + minimum animasyon süresi birlikte
      await Promise.all([invoke("start_protection"), sleep(800)]);
      status = "on";
    } catch (_) {
      status = "off"; // TODO: hata toast'u
    }
  }

  const statusLabel = $derived(
    status === "on" ? "Korumalı" : status === "connecting" ? "Bağlanıyor…" : "Kapalı"
  );
  const heroTitle = $derived(
    status === "on" ? "Özgürsün." : status === "connecting" ? "Bağlanıyor…" : "Korumayı başlat"
  );
  const heroSub = $derived(
    status === "on"
      ? "Gizlisin ve engeller aşıldı — yalnızca seçili siteler etkileniyor, gerisi normal."
      : "Tek tıkla engelleri aş. VPN yok, hız düşmez."
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
      <ParticleEmblem {status} onToggle={toggleBoost} />

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

  .chip .dot { width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
  .status-off { color: var(--text-muted); }
  .status-connecting { color: var(--amber); }
  .status-on { color: var(--green); }

  /* HERO — ortalı */
  .hero { flex: 1 1 auto; display: flex; flex-direction: column; align-items: center; justify-content: center; gap: 4px; min-height: 320px; }
  .hero-title { font-size: 30px; font-weight: 800; letter-spacing: .3px; margin-top: 26px; }
  .hero-sub { color: var(--text-muted); max-width: 460px; text-align: center; line-height: 1.5; }

  /* stat kartları */
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 10px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }

  .placeholder { display: flex; flex-direction: column; gap: 6px; }
  .placeholder .muted { color: var(--text-muted); font-size: 13px; }
</style>
