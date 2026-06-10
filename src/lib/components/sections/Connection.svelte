<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app, toDomain } from "$lib/state.svelte";
  import { toasts } from "$lib/toast.svelte";
  import { eventLog } from "$lib/log.svelte";
  import { t } from "$lib/i18n.svelte";
  import Dropdown from "$lib/components/ui/Dropdown.svelte";

  let { onBack }: { onBack?: () => void } = $props();

  // DPI-bypass strateji profilleri (docs/03 §1). Her birinin açıklaması (ne işe yarar) gösterilir.
  const strategies = [
    { value: "auto", labelKey: "conn.stratAuto", descKey: "conn.stratAutoDesc" },
    { value: "c1", labelKey: "conn.stratFrag", descKey: "conn.stratFragDesc" },
    { value: "multidisorder", labelKey: "conn.stratMulti", descKey: "conn.stratMultiDesc" },
    { value: "fake", labelKey: "conn.stratFake", descKey: "conn.stratFakeDesc" },
  ];
  const strategyOpts = $derived(strategies.map((s) => ({ value: s.value, label: t(s.labelKey) })));
  const stratDesc = $derived(t(strategies.find((s) => s.value === app.strategy)?.descKey ?? "conn.stratAutoDesc"));

  // Secure DNS sağlayıcıları (docs/03 §2)
  const dnsProviders = [
    { value: "cloudflare", name: "Cloudflare", addr: "1.1.1.1", noteKey: "conn.dnsCloudflareNote" },
    { value: "quad9", name: "Quad9", addr: "9.9.9.9", noteKey: "conn.dnsQuad9Note" },
    { value: "adguard", name: "AdGuard", addr: "94.140.14.14", noteKey: "conn.dnsAdguardNote" },
    { value: "google", name: "Google", addr: "8.8.8.8", noteKey: "conn.dnsGoogleNote" },
  ];

  // Per-domain Site List (docs/03 §1 — Free) · app.sites'te kalıcı
  let newSite = $state("");

  function addSite() {
    const v = toDomain(newSite); // tam URL yapıştırılsa bile yalın alan adına indir (https://discord.com/app → discord.com)
    if (!v) {
      if (newSite.trim()) toasts.info(t("conn.invalidDomain"));
      newSite = "";
      return;
    }
    if (!app.sites.includes(v)) {
      app.sites = [...app.sites, v];
      app.syncHostlist();
      toasts.success(`${v} ${t("conn.tAddedSuffix")}`);
    } else {
      toasts.info(`${v} ${t("conn.tExistsSuffix")}`);
    }
    newSite = "";
  }
  function removeSite(s: string) {
    app.sites = app.sites.filter((x) => x !== s);
    app.syncHostlist();
    toasts.info(`${s} ${t("conn.tRemovedSuffix")}`);
  }

  function pickDns(value: string, name: string) {
    app.dns = value;
    invoke("set_dns", { profile: value }).catch(() => {}); // servise ilet (best-effort)
    eventLog.info(`DNS değiştirildi: ${name}`);
    toasts.info(`${t("conn.dnsToast")} ${name}`);
  }

  // DNS sızıntı kontrolü (gerçek, yetkisiz) + otomatiğe (ISS) dön
  type DnsRes = { servers: string[]; secure: boolean; provider: string };
  let dnsCheck = $state<"idle" | "checking" | "done">("idle");
  let dnsRes = $state<DnsRes | null>(null);
  async function checkDns() {
    dnsCheck = "checking";
    try {
      dnsRes = await invoke<DnsRes>("dns_status");
    } catch (_) {
      dnsRes = null;
    }
    dnsCheck = "done";
  }
  function resetDns() {
    app.dns = "auto";
    invoke("set_dns", { profile: "auto" }).catch(() => {});
    toasts.info(t("conn.dnsAutoToast"));
  }
  function pickStrategy(v: string) {
    invoke("set_strategy", { id: v }).catch(() => {}); // servise ilet (best-effort)
    eventLog.info(`Strateji değiştirildi: ${v}`);
    toasts.info(`${t("conn.stratToast")} ${t(strategies.find((s) => s.value === v)?.labelKey ?? "")}`);
  }
</script>

<header class="head">
  <button class="back" onclick={() => onBack?.()} aria-label={t("onb.back")}>←</button>
  <h1>{t("conn.title")}</h1>
</header>

<div class="grid">
  <!-- Strateji seçici (dropdown + açıklama) -->
  <section class="card span-2">
    <div class="card-head">
      <h3>{t("conn.stratTitle")}</h3>
      <span class="hint">{t("conn.stratHint")}</span>
    </div>
    <div class="strat-row">
      <div class="strat-dd"><Dropdown options={strategyOpts} bind:value={app.strategy} onchange={pickStrategy} ariaLabel={t("conn.stratTitle")} /></div>
    </div>
    <p class="explain">{stratDesc}</p>
  </section>

  <!-- Secure DNS -->
  <section class="card span-2">
    <div class="card-head">
      <h3>{t("conn.dnsTitle")}</h3>
      <span class="hint">{t("conn.dnsHint")}</span>
    </div>
    <div class="dns">
      {#each dnsProviders as d}
        <button
          class="dns-card"
          class:active={app.dns === d.value}
          onclick={() => pickDns(d.value, d.name)}
        >
          <span class="dns-name">{d.name}</span>
          <span class="dns-addr mono">{d.addr}</span>
          <span class="dns-note">{t(d.noteKey)}</span>
          {#if app.dns === d.value}<span class="dns-check">✓</span>{/if}
        </button>
      {/each}
    </div>

    <div class="dns-actions">
      <button class="btn" onclick={checkDns}>
        {dnsCheck === "checking" ? t("conn.dnsChecking") : t("conn.dnsCheck")}
      </button>
      <button class="btn" onclick={resetDns}>{t("conn.dnsAuto")}</button>
      {#if dnsCheck === "done" && dnsRes}
        <span class="dns-result" class:ok={dnsRes.secure} class:leak={!dnsRes.secure}>
          <i class="dot"></i>{dnsRes.secure ? `${t("conn.dnsSecure")}: ${dnsRes.provider}` : t("conn.dnsLeak")}
        </span>
        {#if dnsRes.servers.length}
          <span class="dns-srv mono">{t("conn.dnsServers")}: {dnsRes.servers.join(", ")}</span>
        {/if}
      {/if}
    </div>
  </section>

  <!-- Site listesi -->
  <section class="card span-2">
    <div class="card-head">
      <h3>{t("conn.siteTitle")}</h3>
      <span class="hint">{t("conn.siteHint")}</span>
    </div>

    <form class="add-row" onsubmit={(e) => { e.preventDefault(); addSite(); }}>
      <input class="inp" placeholder={t("conn.addPlaceholder")} bind:value={newSite} />
      <button class="btn primary" type="submit">{t("conn.add")}</button>
    </form>

    {#if app.sites.length === 0}
      <div class="empty">{t("conn.empty")}</div>
    {:else}
      <ul class="sites">
        {#each app.sites as s (s)}
          <li>
            <span class="mono">{s}</span>
            <button class="x" onclick={() => removeSite(s)} aria-label="{s} kaldır">✕</button>
          </li>
        {/each}
      </ul>
    {/if}
  </section>
</div>

<style>
  .head { display: flex; align-items: center; gap: 12px; margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }
  .back {
    width: 32px; height: 32px; border-radius: var(--radius-sm); cursor: pointer;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--text);
    font-size: 16px; display: grid; place-items: center; transition: background 0.12s;
  }
  .back:hover { background: var(--bg-hover); }

  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-content: start; }
  .span-2 { grid-column: 1 / -1; }

  .card-head { margin-bottom: 14px; }
  .card-head h3 { font-size: 15px; font-weight: 700; }
  .hint { display: block; color: var(--text-muted); font-size: 12.5px; margin-top: 3px; line-height: 1.45; }

  .explain { margin-top: 12px; color: var(--text-muted); font-size: 13px; line-height: 1.5; }

  /* strateji dropdown */
  .strat-dd { max-width: 320px; }

  /* dns */
  .dns { display: grid; grid-template-columns: repeat(4, 1fr); gap: 12px; }
  .dns-card {
    position: relative; display: flex; flex-direction: column; gap: 5px; align-items: flex-start;
    padding: 14px; border-radius: var(--radius-sm); cursor: pointer; text-align: left;
    background: var(--bg-elevated); border: 1px solid var(--border);
    transition: border-color 0.14s ease, background 0.14s ease;
  }
  .dns-card:hover { border-color: var(--accent-dim); }
  .dns-card.active { border-color: var(--accent); background: color-mix(in srgb, var(--accent) 8%, var(--bg-elevated)); }
  .dns-name { font-weight: 700; font-size: 14px; }
  .dns-addr { font-size: 13px; color: var(--accent); }
  .dns-note { font-size: 11.5px; color: var(--text-muted); }
  .dns-check { position: absolute; top: 10px; right: 12px; color: var(--accent); font-weight: 700; }

  .dns-actions { display: flex; align-items: center; flex-wrap: wrap; gap: 10px; margin-top: 14px; }
  .dns-result { display: inline-flex; align-items: center; gap: 6px; font-size: 12.5px; font-weight: 600; padding: 4px 10px; border-radius: 999px; }
  .dns-result .dot { width: 7px; height: 7px; border-radius: 50%; background: currentColor; }
  .dns-result.ok { color: var(--green); background: color-mix(in srgb, var(--green) 12%, transparent); }
  .dns-result.leak { color: var(--amber); background: color-mix(in srgb, var(--amber) 14%, transparent); }
  .dns-srv { font-size: 12px; color: var(--text-muted); }

  /* site list */
  .add-row { display: flex; gap: 10px; margin-bottom: 14px; }
  .inp {
    flex: 1 1 auto; padding: 9px 12px; border-radius: var(--radius-sm);
    background: var(--bg-base); border: 1px solid var(--border); color: var(--text);
    font: inherit; font-size: 14px; outline: none;
  }
  .inp:focus { border-color: var(--accent); }
  .sites { list-style: none; margin: 0; padding: 0; display: flex; flex-direction: column; gap: 6px; }
  .sites li {
    display: flex; align-items: center; justify-content: space-between;
    padding: 9px 12px; border-radius: var(--radius-sm);
    background: var(--bg-elevated); border: 1px solid var(--border-soft);
  }
  .sites .mono { font-size: 13.5px; }
  .x {
    background: none; border: none; color: var(--text-dim); cursor: pointer;
    font-size: 13px; padding: 2px 6px; border-radius: 4px; transition: color 0.12s, background 0.12s;
  }
  .x:hover { color: var(--red); background: var(--bg-hover); }
  .empty { color: var(--text-muted); font-size: 13px; padding: 8px 0; }
</style>
