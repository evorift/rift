<script lang="ts">
  import { app } from "$lib/state.svelte";
  import { toasts } from "$lib/toast.svelte";
  import { t } from "$lib/i18n.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Segmented from "$lib/components/ui/Segmented.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";

  let { onNavigate }: { onNavigate?: (k: string) => void } = $props();

  const langs = [
    { value: "tr", label: "Türkçe" },
    { value: "en", label: "English" },
    { value: "es", label: "Español" },
    { value: "ru", label: "Русский" },
  ];

  // reduce-motion'ı kök elemana yansıt (CSS @media zaten OS tercihini de yakalar)
  function applyMotion(v: boolean) {
    document.documentElement.classList.toggle("reduce-motion", v);
  }

  // Varsayılana sıfırla (onaylı) — UI + SERVİS (gerçek sistem ayarları da sıfırlanır, state.resetToDefaults)
  let resetOpen = $state(false);
  function resetDefaults() {
    app.resetToDefaults();
    applyMotion(false);
    toasts.success(t("set.reset"));
  }
</script>

<header class="head"><h1>{t("set.title")}</h1></header>

<section class="card group">
  <h3>{t("set.startup")}</h3>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("set.autostart")}</span>
      <span class="opt-desc">{t("set.autostartDesc")}</span>
    </div>
    <Toggle bind:checked={app.autostart} onchange={(v) => app.setAutostart(v)} label={t("set.autostart")} />
  </div>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("set.tray")}</span>
      <span class="opt-desc">{t("set.trayDesc")}</span>
    </div>
    <Toggle bind:checked={app.startMinimized} onchange={(v) => app.setStartMinimized(v)} label={t("set.tray")} />
  </div>
</section>

<section class="card group">
  <h3>{t("set.appearance")}</h3>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("set.reduceMotion")}</span>
      <span class="opt-desc">{t("set.reduceMotionDesc")}</span>
    </div>
    <Toggle bind:checked={app.reduceMotion} onchange={applyMotion} label={t("set.reduceMotion")} />
  </div>
  <div class="opt col">
    <div class="opt-text">
      <span class="opt-name">{t("set.language")}</span>
      <span class="opt-desc">{t("set.languageDesc")}</span>
    </div>
    <div class="seg-wrap"><Segmented options={langs} bind:value={app.language} /></div>
  </div>
</section>

<section class="card group">
  <h3>{t("set.pages")}</h3>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("nav.connection")}</span>
      <span class="opt-desc">{t("set.connDesc")}</span>
    </div>
    <button class="btn" onclick={() => onNavigate?.("connection")}>{t("set.open")}</button>
  </div>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("nav.performance")}</span>
      <span class="opt-desc">{t("set.perfDesc")}</span>
    </div>
    <button class="btn" onclick={() => onNavigate?.("performance")}>{t("set.open")}</button>
  </div>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("nav.logs")}</span>
      <span class="opt-desc">{t("set.logsDesc")}</span>
    </div>
    <button class="btn" onclick={() => onNavigate?.("logs")}>{t("set.open")}</button>
  </div>
</section>

<section class="card group">
  <h3>{t("set.advanced")}</h3>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">
        {t("set.advNet")}
        <span class="exp-badge" title={t("adv.expBadgeTip")}>{t("adv.expBadge")}</span>
      </span>
      <span class="opt-desc">{t("set.advNetDesc")}</span>
    </div>
    <button class="btn" onclick={() => onNavigate?.("advanced")}>{t("set.open")}</button>
  </div>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("set.replay")}</span>
      <span class="opt-desc">{t("set.replayDesc")}</span>
    </div>
    <button class="btn" onclick={() => app.replayOnboarding()}>{t("set.show")}</button>
  </div>
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("set.reset")}</span>
      <span class="opt-desc">{t("set.resetDesc")}</span>
    </div>
    <button class="btn destructive-ghost" onclick={() => (resetOpen = true)}>{t("set.resetBtn")}</button>
  </div>
</section>

<section class="card group">
  <h3>{t("set.about")}</h3>
  <div class="about">
    <div><span class="k">{t("set.version")}</span><span class="v mono">v0.1.0</span></div>
    <div><span class="k">{t("set.engine")}</span><span class="v">WinDivert · DPI bypass</span></div>
    <div><span class="k">{t("set.license")}</span><span class="v">MIT</span></div>
  </div>
  <p class="note">{t("set.aboutNote")}</p>
</section>

<Modal
  bind:open={resetOpen}
  title={t("set.reset")}
  message={t("set.resetDesc")}
  danger
  confirmLabel={t("set.resetBtn")}
  onconfirm={resetDefaults}
/>

<style>
  .head { margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }

  .group { margin-bottom: 16px; }
  .group h3 { font-size: 13px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px; color: var(--text-muted); margin-bottom: 14px; }

  .opt {
    display: flex; align-items: center; justify-content: space-between; gap: 18px;
    padding: 12px 0;
  }
  .opt:not(:last-child) { border-bottom: 1px solid var(--border-soft); }
  .opt.col { flex-direction: column; align-items: stretch; gap: 12px; }
  .opt-text { display: flex; flex-direction: column; gap: 3px; }
  .opt-name { font-weight: 600; font-size: 14px; display: inline-flex; align-items: center; gap: 8px; }
  .opt-desc { color: var(--text-muted); font-size: 12.5px; line-height: 1.45; }

  /* Deneysel rozet — Gelişmiş ağ ayarları girişinin yanında küçük kırmızı badge. Hover'da tip. */
  .exp-badge {
    font-size: 10.5px; font-weight: 800; letter-spacing: 0.6px; line-height: 1;
    padding: 3px 7px; border-radius: 999px; user-select: none; cursor: help;
    color: var(--red);
    background: color-mix(in srgb, var(--red) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--red) 35%, transparent);
  }
  .seg-wrap { max-width: 420px; }

  .about { display: flex; flex-direction: column; gap: 10px; }
  .about > div { display: flex; justify-content: space-between; font-size: 13.5px; }
  .about .k { color: var(--text-muted); }
  .about .v { font-weight: 600; }
  .note { margin-top: 14px; color: var(--text-dim); font-size: 12px; line-height: 1.5; }

  .destructive-ghost { color: var(--red); }
  .destructive-ghost:hover { background: color-mix(in srgb, var(--red) 14%, transparent); }
</style>
