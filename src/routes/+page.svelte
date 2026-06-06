<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
  import BlackHole from "$lib/components/BlackHole.svelte";
  import Connection from "$lib/components/sections/Connection.svelte";
  import Performance from "$lib/components/sections/Performance.svelte";
  import Apps from "$lib/components/sections/Apps.svelte";
  import Logs from "$lib/components/sections/Logs.svelte";
  import Settings from "$lib/components/sections/Settings.svelte";
  import Advanced from "$lib/components/sections/Advanced.svelte";
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n.svelte";

  let active = $state("dashboard");
  const status = $derived(app.status);

  const statusLabel = $derived(
    status === "on" ? t("status.on") : status === "connecting" ? t("status.connecting") : t("status.off")
  );
  const heroTitle = $derived(
    status === "on" ? t("dash.heroOn") : status === "connecting" ? t("dash.heroConnecting") : t("dash.heroOff")
  );
  const heroSub = $derived(status === "on" ? t("dash.subOn") : t("dash.subOff"));
</script>

<NavRail {active} onSelect={(k) => (active = k)} />

<main class="content">
  {#if active === "dashboard"}
    <header class="head">
      <h1>{t("dash.title")}</h1>
      <span class="chip status-{status}"><i class="dot"></i> {statusLabel}</span>
    </header>

    <section class="hero">
      <div class="bh-stage"><BlackHole /></div>
      <h2 class="hero-title">{heroTitle}</h2>
      <p class="hero-sub">{heroSub}</p>
    </section>

    <section class="stats">
      <div class="card stat"><span class="lbl">{t("dash.ping")}</span><span class="val mono">{status === "on" ? app.ping : "—"}<small>ms</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.download")}</span><span class="val mono">{status === "on" ? app.down.toFixed(1) : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.upload")}</span><span class="val mono">{status === "on" ? app.up.toFixed(1) : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.strategy")}</span><span class="val small">{status === "on" ? t("dash.auto") : "—"}</span></div>
    </section>
  {:else if active === "connection"}
    <Connection />
  {:else if active === "performance"}
    <Performance />
  {:else if active === "apps"}
    <Apps />
  {:else if active === "logs"}
    <Logs />
  {:else if active === "settings"}
    <Settings onNavigate={(k) => (active = k)} />
  {:else if active === "advanced"}
    <Advanced onBack={() => (active = "settings")} />
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
  .bh-stage { width: 320px; height: 320px; margin-bottom: 10px; }
  .hero-title { font-size: 26px; font-weight: 800; letter-spacing: .3px; }
  .hero-sub { color: var(--text-muted); max-width: 460px; text-align: center; line-height: 1.5; }

  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 10px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }
</style>
