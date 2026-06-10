<script lang="ts">
  import NavRail from "$lib/components/NavRail.svelte";
  import BlackHole from "$lib/components/BlackHole.svelte";
  import Connection from "$lib/components/sections/Connection.svelte";
  import Performance from "$lib/components/sections/Performance.svelte";
  import Apps from "$lib/components/sections/Apps.svelte";
  import Limits from "$lib/components/sections/Limits.svelte";
  import Logs from "$lib/components/sections/Logs.svelte";
  import Settings from "$lib/components/sections/Settings.svelte";
  import Advanced from "$lib/components/sections/Advanced.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import { app } from "$lib/state.svelte";
  import { toasts } from "$lib/toast.svelte";
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

  // aktif profil çipleri
  const STRAT_KEY: Record<string, string> = {
    auto: "conn.stratAuto", c1: "conn.stratFrag", multidisorder: "conn.stratMulti", fake: "conn.stratFake",
  };
  const DNS_NAME: Record<string, string> = {
    cloudflare: "Cloudflare", quad9: "Quad9", adguard: "AdGuard", google: "Google",
  };
  const stratLabel = $derived(t(STRAT_KEY[app.strategy] ?? "conn.stratAuto"));
  const dnsName = $derived(DNS_NAME[app.dns] ?? app.dns);

  // otomatik oyun modu + modallar
  let previewOpen = $state(false);
  let repairOpen = $state(false);

  // Oyun Modu = kalıcı AÇ/KAPA switch (state.gameMode kalıcı; PC kapanıp açılsa bile açık kalır).
  // Hız-sınırlama (ağır-uygulama popup'ı) KALDIRILDI — Hız Sınırı özelliği şimdilik gizli (WinDivert
  // inbound throttle canlı test edilene dek). Bu, eski popup'tan gelen takılmayı da tamamen ortadan kaldırır.
  async function toggleGameMode(v: boolean) {
    if (!v) {
      app.exitGameMode();
      toasts.info(t("gm.exited"));
      return;
    }
    await app.applyGameMode();
    toasts.success(t("gm.applied"));
  }
  function doRepair() {
    app.repairNetwork();
    toasts.success(t("repair.done"));
  }
</script>

<NavRail {active} onSelect={(k) => (active = k)} />

<main class="content">
  {#if active === "dashboard"}
    <header class="head">
      <h1>{t("dash.title")}</h1>
      <span class="chip status-{status}"><i class="dot"></i> {statusLabel}</span>
    </header>

    <!-- Yönetici bandı KALDIRILDI: servis modelinde (EvoriftSvc LocalSystem) app'in elevated olmasına
         gerek yok — tüm yetkili işler (tweak/DNS/limit/firewall) serviste çalışır. Banner yanıltıcıydı. -->

    <section class="hero">
      <div class="bh-stage"><BlackHole /></div>
      <h2 class="hero-title">{heroTitle}</h2>
      <p class="hero-sub">{heroSub}</p>

      <div class="gamemode" class:on={app.gameMode}>
        <span class="gm-ico">🎮</span>
        <span class="gm-text">
          <span class="gm-title">{app.gameMode ? t("gm.active") : t("gm.button")}</span>
          <span class="gm-sub">{app.gameMode ? t("gm.activeSub") : t("gm.sub")}</span>
        </span>
        <Toggle checked={app.gameMode} onchange={toggleGameMode} ariaLabel={t("gm.button")} />
      </div>

      <button class="link-btn" onclick={() => (previewOpen = true)}>{t("gm.preview")}</button>

      <div class="profile">
        <span class="p-lbl">{t("gm.profile")}:</span>
        <span class="chip"><i class="dot"></i>{t("dash.strategy")}: {stratLabel}</span>
        <span class="chip"><i class="dot"></i>{t("gm.dnsChip")}: {dnsName}</span>
      </div>
    </section>

    <section class="stats">
      <div class="card stat"><span class="lbl">{t("dash.ping")}</span><span class="val mono">{status === "on" ? app.ping : "—"}<small>ms</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.download")}</span><span class="val mono">{status === "on" ? app.down.toFixed(1) : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.upload")}</span><span class="val mono">{status === "on" ? app.up.toFixed(1) : "—"}<small>Mbps</small></span></div>
      <div class="card stat"><span class="lbl">{t("dash.strategy")}</span><span class="val small">{status === "on" ? stratLabel : "—"}</span></div>
    </section>

    <div class="repair-row">
      <span class="repair-lbl">{t("repair.sub")}</span>
      <button class="btn" onclick={() => (repairOpen = true)}>🛠 {t("repair.button")}</button>
    </div>
  {:else if active === "connection"}
    <Connection onBack={() => (active = "settings")} />
  {:else if active === "performance"}
    <Performance onBack={() => (active = "settings")} />
  {:else if active === "apps"}
    <Apps />
  {:else if active === "limits"}
    <Limits />
  {:else if active === "logs"}
    <Logs onBack={() => (active = "settings")} />
  {:else if active === "settings"}
    <Settings onNavigate={(k) => (active = k)} />
  {:else if active === "advanced"}
    <Advanced onBack={() => (active = "settings")} />
  {/if}
</main>

<!-- Oyun Modu önizleme (ne açılır?) -->
<Modal
  bind:open={previewOpen}
  title={t("gm.previewTitle")}
  message={t("gm.previewBody")}
  confirmLabel={t("common.ok")}
  hideCancel
/>

<!-- Kullanıcı-dostu Ağı Onar -->
<Modal
  bind:open={repairOpen}
  title={t("repair.title")}
  message={t("repair.body")}
  confirmLabel={t("repair.button")}
  onconfirm={doRepair}
/>

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

  /* OTOMATİK OYUN MODU butonu */
  .gamemode {
    display: flex; align-items: center; gap: 14px;
    width: 100%; max-width: 460px; margin-top: 18px;
    padding: 14px 18px; border-radius: var(--radius); cursor: pointer; text-align: left;
    background: var(--bg-surface); border: 1px solid var(--border);
    transition: transform .08s ease, border-color .15s ease, background .15s ease;
  }
  .gamemode:hover { border-color: var(--accent-dim); background: var(--bg-elevated); }
  .gamemode:active { transform: scale(.99); }
  .gamemode.on {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, var(--bg-surface));
    box-shadow: 0 0 0 1px var(--accent-dim), 0 8px 28px var(--accent-glow);
  }
  .gm-ico { font-size: 22px; flex: 0 0 auto; }
  .gm-text { display: flex; flex-direction: column; gap: 2px; flex: 1 1 auto; min-width: 0; }
  .gm-title { font-weight: 800; font-size: 15px; }
  .gamemode.on .gm-title { color: var(--accent); }
  .gm-sub { color: var(--text-muted); font-size: 12.5px; line-height: 1.4; }

  /* aktif profil çipleri */
  .profile { display: flex; align-items: center; gap: 8px; margin-top: 12px; flex-wrap: wrap; justify-content: center; }
  .profile .p-lbl { color: var(--text-dim); font-size: 12px; }
  .profile .chip { background: var(--bg-elevated); }
  .profile .chip .dot { width: 6px; height: 6px; border-radius: 50%; background: var(--accent); }

  .link-btn {
    margin-top: 10px; background: none; border: none; cursor: pointer;
    color: var(--text-muted); font-size: 12.5px; text-decoration: underline; padding: 2px 6px;
  }
  .link-btn:hover { color: var(--accent); }

  .repair-row {
    display: flex; align-items: center; justify-content: center; gap: 12px;
    margin-top: 14px; color: var(--text-dim); font-size: 12.5px;
  }

  .stats { display: grid; grid-template-columns: repeat(4, 1fr); gap: 14px; margin-top: 10px; }
  .stat { display: flex; flex-direction: column; gap: 8px; }
  .stat .lbl { color: var(--text-muted); font-size: 12px; font-weight: 600; text-transform: uppercase; letter-spacing: .4px; }
  .stat .val { font-size: 28px; font-weight: 700; }
  .stat .val small { font-size: 13px; font-weight: 600; color: var(--text-muted); margin-left: 5px; }
  .stat .val.small { font-size: 18px; }
</style>
