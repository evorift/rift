<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
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
      <div class="boost-wrap {status}">
        <span class="wave w1"></span>
        <span class="wave w2"></span>
        <span class="wave w3"></span>
        <button class="boost {status}" onclick={toggleBoost} aria-label="Korumayı aç/kapat">
          <span class="aura"></span>
          <span class="glyph">
            {#if status === "on"}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/><path d="m9 12 2 2 4-4"/>
              </svg>
            {:else if status === "connecting"}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
                <path d="M12 3v9"/><path d="M6.4 7a8 8 0 1 0 11.2 0"/>
              </svg>
            {:else}
              <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">
                <path d="M12 22s8-4 8-10V5l-8-3-8 3v7c0 6 8 10 8 10z"/>
              </svg>
            {/if}
          </span>
        </button>
      </div>

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

  /* BOOST — büyük, ortalı, animasyonlu */
  .boost-wrap { position: relative; width: 230px; height: 230px; display: grid; place-items: center; }

  .boost {
    position: relative; width: 170px; height: 170px; border-radius: 50%;
    cursor: pointer; border: 2px solid var(--border);
    background: radial-gradient(circle at 50% 38%, var(--bg-elevated), var(--bg-surface));
    color: var(--text-muted); display: grid; place-items: center;
    transition: color .25s, border-color .25s, box-shadow .35s, transform .1s;
    z-index: 2;
  }
  .boost:hover { color: var(--text); border-color: #2a3346; }
  .boost:active { transform: scale(.97); }
  .glyph svg { width: 64px; height: 64px; }

  /* dönen enerji halkası (kalkan / gizlilik) */
  .aura { position: absolute; inset: -8px; border-radius: 50%; opacity: 0; transition: opacity .4s; pointer-events: none;
    background: conic-gradient(from 0deg, transparent 0deg, var(--accent) 90deg, transparent 200deg);
    -webkit-mask: radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3px));
            mask: radial-gradient(farthest-side, transparent calc(100% - 4px), #000 calc(100% - 3px)); }

  /* connecting */
  .boost.connecting { color: var(--amber); border-color: var(--amber); }
  .boost.connecting .aura { opacity: .9; animation: spin .9s linear infinite;
    background: conic-gradient(from 0deg, transparent 0deg, var(--amber) 70deg, transparent 160deg); }

  /* on — korumalı + özgür */
  .boost.on { color: var(--accent); border-color: var(--accent);
    box-shadow: 0 0 40px -6px var(--accent-glow), inset 0 0 26px -10px var(--accent-glow);
    animation: breathe 2.6s ease-in-out infinite; }
  .boost.on .aura { opacity: 1; animation: spin 5s linear infinite; }

  /* özgürleşme dalgaları — dışa patlayan halkalar */
  .wave { position: absolute; width: 170px; height: 170px; border-radius: 50%;
    border: 2px solid var(--accent); opacity: 0; pointer-events: none; z-index: 1; }
  .boost-wrap.on .wave { animation: freedom 2.8s cubic-bezier(.2,.6,.2,1) infinite; }
  .boost-wrap.on .w2 { animation-delay: .9s; }
  .boost-wrap.on .w3 { animation-delay: 1.8s; }

  @keyframes freedom {
    0%   { transform: scale(.6); opacity: .8; }
    70%  { opacity: .12; }
    100% { transform: scale(1.9); opacity: 0; }
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes breathe {
    0%,100% { box-shadow: 0 0 30px -8px var(--accent-glow), inset 0 0 26px -10px var(--accent-glow); }
    50%     { box-shadow: 0 0 52px -2px var(--accent-glow), inset 0 0 26px -10px var(--accent-glow); }
  }

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
