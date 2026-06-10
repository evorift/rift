<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "$lib/state.svelte";
  import { toasts } from "$lib/toast.svelte";
  import { t } from "$lib/i18n.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";
  import Segmented from "$lib/components/ui/Segmented.svelte";
  import Slider from "$lib/components/ui/Slider.svelte";
  import Tooltip from "$lib/components/ui/Tooltip.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";

  let { onBack }: { onBack?: () => void } = $props();

  // ---- onay modalı ----
  let modal = $state({
    open: false, title: "", message: "", danger: false, confirmLabel: "",
    action: () => {},
  });
  function ask(opts: { title: string; message: string; danger?: boolean; confirmLabel?: string; action: () => void }) {
    modal = { open: true, danger: false, confirmLabel: t("adv.apply"), ...opts };
  }
  // Koruma aktifken 🔴 ayar uyarısı (docs/04 §UI kuralı)
  const liveWarn = $derived(app.status === "on" ? t("adv.liveWarn") : "");

  // Tüm tweak durumu app.tweaks'te (Dashboard'daki Oyun Modu ile paylaşılır).
  const tw = app.tweaks;

  // Tweak'i servise ilet (best-effort) + toast.
  function toggleTweak(key: string, label: string, v: boolean) {
    invoke("set_tweak", { key, value: v ? "on" : "off" }).catch(() => {});
    toasts.success(`${label} ${v ? t("adv.tOn") : t("adv.tOff")}`);
  }
  function setTweakValue(key: string, value: string, toastMsg: string) {
    invoke("set_tweak", { key, value }).catch(() => {});
    toasts.info(toastMsg);
  }

  // congestion seçimi — BBR2 onay ister (en tehlikeli, docs/04)
  let prevCong = "cubic";
  function pickCongestion(v: string) {
    if (v === "bbr2") {
      ask({
        title: t("adv.bbr2Title"),
        message: t("adv.bbr2Msg") + liveWarn,
        danger: true, confirmLabel: t("adv.bbr2Confirm"),
        action: () => {
          prevCong = "bbr2";
          tw.congestion = "bbr2";
          invoke("set_tweak", { key: "congestion", value: "bbr2" }).catch(() => {});
          toasts.warn(t("adv.bbr2Toast"));
        },
      });
      // seçimi geri al; onaylanırsa action yine bbr2 yapar ama segmented görsel olarak prev'e döner
      tw.congestion = prevCong;
      return;
    }
    prevCong = v;
    invoke("set_tweak", { key: "congestion", value: v }).catch(() => {});
    toasts.success(`${t("adv.congToast")} ${v.toUpperCase()}`);
  }

  // offload kapatma onay ister (🔴 NIC oto-restart)
  function toggleOffload(v: boolean) {
    if (!v) {
      ask({
        title: t("adv.offloadOffTitle"),
        message: t("adv.offloadOffMsg") + liveWarn,
        danger: true, confirmLabel: t("adv.offloadOffConfirm"),
        action: () => {
          tw.offload = false;
          invoke("set_tweak", { key: "offload", value: "off" }).catch(() => {});
          toasts.warn(t("adv.offloadOffToast"));
        },
      });
      tw.offload = true; // onaya kadar açık kalsın
    } else {
      invoke("set_tweak", { key: "offload", value: "on" }).catch(() => {});
      toasts.success(t("adv.offloadOnToast"));
    }
  }

  function applyMtu() {
    ask({
      title: t("adv.mtuTitle"),
      message: t("adv.mtuMsg").replace("{n}", String(tw.mtu)) + liveWarn,
      danger: tw.mtu < 1500, confirmLabel: t("adv.apply"),
      action: () => {
        invoke("set_tweak", { key: "mtu", value: String(tw.mtu) }).catch(() => {});
        toasts.success(`MTU = ${tw.mtu}`);
      },
    });
  }
  function resetMtu() {
    tw.mtu = 1500;
    invoke("set_tweak", { key: "mtu", value: "1500" }).catch(() => {}); // GERÇEK uygula (1500 güvenli, onay gerekmez)
    toasts.success(t("adv.mtuResetToast"));
  }

  // ---- Ağ Onar / Sıfırla ----
  type Repair = { id: string; labelKey: string; level: "green" | "amber" | "red"; tipKey: string; confirm?: boolean; reboot?: boolean };
  const repairs: Repair[] = [
    { id: "flushdns", labelKey: "adv.rpFlushdns", level: "green", tipKey: "adv.rpFlushdnsTip" },
    { id: "registerdns", labelKey: "adv.rpRegisterdns", level: "green", tipKey: "adv.rpRegisterdnsTip" },
    { id: "dnscache", labelKey: "adv.rpDnscache", level: "amber", tipKey: "adv.rpDnscacheTip" },
    { id: "renew", labelKey: "adv.rpRenew", level: "amber", tipKey: "adv.rpRenewTip" },
    { id: "winsock", labelKey: "adv.rpWinsock", level: "red", confirm: true, reboot: true, tipKey: "adv.rpWinsockTip" },
    { id: "ipreset", labelKey: "adv.rpIpreset", level: "red", confirm: true, reboot: true, tipKey: "adv.rpIpresetTip" },
  ];

  function runRepair(r: Repair) {
    const label = t(r.labelKey);
    const exec = () => {
      invoke("run_repair", { tool: r.id }).catch(() => {}); // servise ilet (best-effort)
      toasts.success(`${label} ${t("adv.ranSuffix")}`);
    };
    if (r.confirm) {
      ask({
        title: label,
        message: t(r.tipKey) + (r.reboot ? t("adv.continueQ") : "") + liveWarn,
        danger: true, confirmLabel: r.reboot ? t("adv.runReboot") : t("adv.run"),
        action: exec,
      });
    } else {
      exec();
    }
  }

  const congestionOpts = [
    { value: "cubic", label: "CUBIC" },
    { value: "ctcp", label: "CTCP" },
    { value: "bbr2", label: "BBR2 ⚠️" },
  ];
  const autotuningOpts = [
    { value: "normal", label: "Normal" },
    { value: "disabled", label: "Disabled" },
  ];
</script>

<header class="head">
  <button class="back" onclick={() => onBack?.()} aria-label={t("onb.back")}>←</button>
  <h1>{t("adv.title")}</h1>
  <span class="exp-tag">{t("adv.expBadge")}</span>
</header>

<!-- Deneysel uyarı bandı — riskler net listelenir; eski "may kill your computer" satırını değiştirir.
     Kullanıcı buraya tıklayıp gelirse ne olabileceğini bilmeli; geri kalan sayfa zaten satır bazında
     ⚠️/🔴 rozetlerle çalışıyor. -->
<div class="exp-banner" role="note">
  <div class="exp-banner-head">
    <span class="exp-banner-icon">⚠️</span>
    <h2 class="exp-banner-title">{t("adv.expTitle")}</h2>
  </div>
  <p class="exp-banner-body">{t("adv.expBody")}</p>
</div>

<p class="warn-note">{t("adv.warnNote")}</p>

<!-- 🟢 GÜVENLİ -->
<section class="card group">
  <h3>{t("adv.grpSafe")}</h3>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.nagle")} <Tooltip text={t("adv.nagleTip")} level="green" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.nagle} onchange={(v) => toggleTweak("nagle", "Nagle", v)} label="Nagle" /></div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.heuristics")} <Tooltip text={t("adv.heuristicsTip")} level="green" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.heuristics} onchange={(v) => toggleTweak("heuristics", "Heuristics", v)} label="Heuristics" /></div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.throttle")} <Tooltip text={t("adv.throttleTip")} level="green" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.throttleIdx} onchange={(v) => toggleTweak("throttleIdx", t("adv.tnThrottle"), v)} label={t("adv.tnThrottle")} /></div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.nicPower")} <Tooltip text={t("adv.nicPowerTip")} level="green" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.nicPower} onchange={(v) => toggleTweak("nicPower", t("adv.tnNicPower"), v)} label={t("adv.tnNicPower")} /></div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.highPerf")} <Tooltip text={t("adv.highPerfTip")} level="green" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.highPerf} onchange={(v) => toggleTweak("highPerf", t("adv.tnHighPerf"), v)} label={t("adv.tnHighPerf")} /></div>
  </div>

  <div class="row col">
    <div class="row-text">
      <span class="name">{t("adv.autotuning")} <Tooltip text={t("adv.autotuningTip")} level="green" /></span>
    </div>
    <div class="seg-wrap"><Segmented options={autotuningOpts} bind:value={tw.autotuning} onchange={(v) => setTweakValue("autotuning", v, `Autotuning: ${v}`)} /></div>
  </div>

  <div class="row col">
    <div class="row-text">
      <span class="name">{t("adv.congestion")} <Tooltip text={t("adv.congestionTip")} level="break" /></span>
    </div>
    <div class="seg-wrap"><Segmented options={congestionOpts} bind:value={tw.congestion} onchange={pickCongestion} /></div>
  </div>
</section>

<!-- 🟡 ANLIK KESİNTİ -->
<section class="card group">
  <h3>{t("adv.grpBlink")}</h3>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.rss")} <Tooltip text={t("adv.rssTip")} level="amber" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.rss} onchange={(v) => toggleTweak("rss", "RSS", v)} label="RSS" /></div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.rsc")} <Tooltip text={t("adv.rscTip")} level="amber" /></span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.rsc} onchange={(v) => toggleTweak("rsc", "RSC", v)} label="RSC" /></div>
  </div>
</section>

<!-- 🔴/⚠️ RİSKLİ -->
<section class="card group danger-group">
  <h3>{t("adv.grpRisky")}</h3>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.offload")} <Tooltip text={t("adv.offloadTip")} level="red" /></span>
      <span class="sub">{t("adv.offloadSub")}</span>
    </div>
    <div class="row-ctl"><Toggle bind:checked={tw.offload} onchange={toggleOffload} label={t("adv.offload")} /></div>
  </div>

  <div class="row col">
    <div class="row-text">
      <span class="name">{t("adv.mtu")} <Tooltip text={t("adv.mtuTip")} level="break" /></span>
    </div>
    <div class="mtu">
      <div class="mtu-slider"><Slider bind:value={tw.mtu} min={1280} max={1500} step={2} label="MTU" /></div>
      <button class="btn" onclick={resetMtu}>{t("adv.resetMtu")}</button>
      <button class="btn primary" onclick={applyMtu}>{t("adv.apply")}</button>
    </div>
  </div>

  <div class="row">
    <div class="row-text">
      <span class="name">{t("adv.adapter")} <Tooltip text={t("adv.adapterTip")} level="red" /></span>
    </div>
    <div class="row-ctl">
      <button class="btn destructive" onclick={() => ask({
        title: t("adv.adapterTitle"),
        message: t("adv.adapterMsg") + liveWarn,
        danger: true, confirmLabel: t("adv.adapterConfirm"),
        action: () => {
          invoke("run_repair", { tool: "adapter" }).catch(() => {}); // GERÇEK: Restart-NetAdapter (servise ilet)
          toasts.warn(t("adv.adapterToast"));
        },
      })}>{t("adv.run")}</button>
    </div>
  </div>
</section>

<!-- AĞ ONAR -->
<section class="card group">
  <h3>{t("adv.grpRepair")}</h3>
  <p class="repair-note">{t("adv.repairNote")}</p>
  <div class="repair">
    {#each repairs as r (r.id)}
      <div class="rep-row">
        <div class="rep-text">
          <span class="name">{t(r.labelKey)} <Tooltip text={t(r.tipKey)} level={r.level} /></span>
        </div>
        <div class="rep-ctl">
          <button class="btn {r.level === 'red' ? 'destructive' : ''}" onclick={() => runRepair(r)}>{t("adv.run")}</button>
        </div>
      </div>
    {/each}
  </div>
</section>

<Modal
  bind:open={modal.open}
  title={modal.title}
  message={modal.message}
  danger={modal.danger}
  confirmLabel={modal.confirmLabel}
  onconfirm={() => modal.action()}
/>

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 12px; }
  .head h1 { font-size: 22px; font-weight: 700; }
  .back {
    width: 32px; height: 32px; border-radius: var(--radius-sm); cursor: pointer;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--text);
    font-size: 16px; display: grid; place-items: center; transition: background 0.12s;
  }
  .back:hover { background: var(--bg-hover); }

  /* Başlık yanındaki küçük deneysel etiketi (Settings → giriş rozetiyle aynı stilde). */
  .exp-tag {
    font-size: 10.5px; font-weight: 800; letter-spacing: 0.6px;
    padding: 3px 8px; border-radius: 999px; user-select: none;
    color: var(--red);
    background: color-mix(in srgb, var(--red) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--red) 35%, transparent);
  }

  /* Üst uyarı bandı — eski "may kill your computer" satırının yerini alır; riskleri net listeler. */
  .exp-banner {
    background: color-mix(in srgb, var(--red) 10%, var(--bg-surface));
    border: 1px solid color-mix(in srgb, var(--red) 35%, transparent);
    border-left: 3px solid var(--red);
    border-radius: var(--radius-sm);
    padding: 14px 16px;
    margin-bottom: 14px;
  }
  .exp-banner-head { display: flex; align-items: center; gap: 10px; margin-bottom: 6px; }
  .exp-banner-icon { font-size: 18px; line-height: 1; }
  .exp-banner-title { font-size: 16px; font-weight: 800; color: var(--red); letter-spacing: 0.2px; }
  .exp-banner-body { color: var(--text); font-size: 13.5px; line-height: 1.55; white-space: pre-line; }

  .warn-note {
    color: var(--text-muted); font-size: 13px; line-height: 1.55; margin-bottom: 16px;
    padding: 12px 14px; border-radius: var(--radius-sm);
    background: color-mix(in srgb, var(--amber) 8%, var(--bg-surface));
    border: 1px solid color-mix(in srgb, var(--amber) 25%, transparent);
  }

  .group { margin-bottom: 16px; }
  .group h3 {
    display: flex; align-items: center; gap: 10px;
    font-size: 13px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.5px;
    color: var(--text-muted); margin-bottom: 6px;
  }
  .danger-group { border-color: color-mix(in srgb, var(--red) 25%, var(--border)); }

  .row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 12px 0; }
  .row:not(:last-child) { border-bottom: 1px solid var(--border-soft); }
  .row.col { flex-direction: column; align-items: stretch; gap: 12px; }
  .row-text { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .name { display: inline-flex; align-items: center; gap: 7px; font-weight: 600; font-size: 14px; }
  .sub { color: var(--text-muted); font-size: 12px; }
  .row-ctl { display: flex; align-items: center; gap: 12px; flex: 0 0 auto; }
  .seg-wrap { max-width: 100%; }

  .mtu { display: flex; align-items: flex-end; gap: 12px; }
  .mtu-slider { flex: 1 1 auto; }

  .repair-note { color: var(--text-muted); font-size: 12.5px; margin-bottom: 12px; }
  .repair { display: flex; flex-direction: column; gap: 2px; }
  .rep-row { display: flex; align-items: center; justify-content: space-between; gap: 16px; padding: 9px 0; }
  .rep-row:not(:last-child) { border-bottom: 1px solid var(--border-soft); }
  .rep-text { min-width: 0; }
  .rep-ctl { display: flex; align-items: center; gap: 12px; flex: 0 0 auto; }
</style>
