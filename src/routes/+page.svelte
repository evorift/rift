<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";

  let active = $state("dashboard");
  type Status = "off" | "connecting" | "on";
  let status = $state<Status>("off");

  function toggleBoost() {
    if (status === "on") { status = "off"; return; }
    if (status === "connecting") return;
    status = "connecting";
    // TODO: gerçek motor (servis) çağrısı buraya gelecek
    setTimeout(() => { status = "on"; }, 900);
  }

  const statusLabel = $derived(
    status === "on" ? "Korumalı" : status === "connecting" ? "Bağlanıyor…" : "Kapalı"
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
      <span class="chip status-{status}">
        <i class="dot"></i> {statusLabel}
      </span>
    </header>

    <section class="hero card">
      <button
        class="boost {status}"
        onclick={toggleBoost}
        aria-label="Korumayı aç/kapat"
      >
        <span class="ring"></span>
        <span class="power">
          <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round">
            <path d="M12 3v9" /><path d="M6.4 7a8 8 0 1 0 11.2 0" />
          </svg>
        </span>
      </button>
      <div class="hero-text">
        <h2>{status === "on" ? "Discord, Roblox & oyunlar açık" : status === "connecting" ? "Bağlanıyor…" : "Korumayı başlat"}</h2>
        <p>{status === "on" ? "Yalnızca seçili siteler etkileniyor — gerisi normal." : "Tek tıkla engelleri aş. VPN yok, hız düşmez."}</p>
      </div>
    </section>

    <section class="stats">
      <div class="card stat">
        <span class="lbl">Ping</span>
        <span class="val mono">{status === "on" ? "24" : "—"}<small>ms</small></span>
      </div>
      <div class="card stat">
        <span class="lbl">İndirme</span>
        <span class="val mono">{status === "on" ? "0.0" : "—"}<small>Mbps</small></span>
      </div>
      <div class="card stat">
        <span class="lbl">Yükleme</span>
        <span class="val mono">{status === "on" ? "0.0" : "—"}<small>Mbps</small></span>
      </div>
      <div class="card stat">
        <span class="lbl">Strateji</span>
        <span class="val small">{status === "on" ? "Otomatik" : "—"}</span>
      </div>
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
  .content { flex: 1 1 auto; padding: 22px 26px; overflow-y: auto; }
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 18px; }
  .head h1 { font-size: 22px; font-weight: 700; }

  /* durum chip */
  .chip .dot { width: 8px; height: 8px; border-radius: 50%; background: currentColor; }
  .status-off { color: var(--text-muted); }
  .status-connecting { color: var(--amber); }
  .status-on { color: var(--green); }

  /* hero */
  .hero { display: flex; align-items: center; gap: 26px; margin-bottom: 18px; }
  .hero-text h2 { font-size: 18px; font-weight: 700; margin-bottom: 6px; }
  .hero-text p { color: var(--text-muted); }

  /* boost butonu */
  .boost {
    position: relative; width: 104px; height: 104px; flex: 0 0 auto;
    border-radius: 50%; cursor: pointer;
    border: 2px solid var(--border);
    background: var(--bg-elevated); color: var(--text-muted);
    display: grid; place-items: center;
    transition: color .2s, border-color .2s, background .2s, box-shadow .3s;
  }
  .boost:active { transform: scale(.97); }
  .boost .power svg { width: 42px; height: 42px; }
  .boost .ring { position: absolute; inset: -2px; border-radius: 50%; pointer-events: none; }

  .boost.connecting { color: var(--amber); border-color: var(--amber); }
  .boost.connecting .ring {
    border: 2px solid transparent; border-top-color: var(--amber);
    animation: spin .8s linear infinite;
  }
  .boost.on {
    color: #04222a; background: var(--accent); border-color: var(--accent);
    box-shadow: 0 0 0 0 var(--accent-glow);
    animation: breathe 2.4s ease-in-out infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  @keyframes breathe {
    0%,100% { box-shadow: 0 0 0 0 var(--accent-glow); }
    50%     { box-shadow: 0 0 0 14px transparent; }
  }

  /* stat kartları */
  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }

  .placeholder { display: flex; flex-direction: column; gap: 6px; }
  .placeholder .muted { color: var(--text-muted); font-size: 13px; }
</style>
