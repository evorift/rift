<script lang="ts">
  // Hız Sınırı — per-app indirme/yükleme limiti (QoS, docs/03 §4) + Solo Mode.
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "$lib/state.svelte";
  import { toasts } from "$lib/toast.svelte";
  import { t } from "$lib/i18n.svelte";
  import Toggle from "$lib/components/ui/Toggle.svelte";

  const rows = $derived(app.apps);
  const initials = (n: string) => n.slice(0, 1).toUpperCase();

  function clampN(v: number): number {
    if (!Number.isFinite(v) || v < 0) return 0;
    return Math.min(1_000_000, Math.round(v)); // kbps (0 = sınırsız)
  }
  // Debounce: number input her tuşta/spinner adımında oninput tetikler. Borusuz gönderim
  // saniyede onlarca PowerShell QoS komutu çalıştırıyordu → kullanıcı yazmayı bırakınca tek gönder.
  const timers: Record<string, ReturnType<typeof setTimeout>> = {};
  function sendLimit(r: { id: string; down: number; up: number; path?: string }) {
    clearTimeout(timers[r.id]);
    timers[r.id] = setTimeout(() => {
      r.down = clampN(r.down);
      r.up = clampN(r.up);
      invoke("set_limit", { id: r.id, path: r.path ?? "", down: r.down, up: r.up }).catch(() => {});
    }, 400);
  }
  function onSolo(v: boolean) {
    app.setSolo(v); // gerçek: yoğun uygulamaların upload'ını kıs / geri yükle (set_limit)
    toasts.info(v ? t("limits.soloOn") : t("limits.soloOff"));
  }
</script>

<header class="head"><h1>{t("limits.title")}</h1></header>

<section class="card solo">
  <div class="opt">
    <div class="opt-text">
      <span class="opt-name">{t("limits.solo")}</span>
      <span class="opt-desc">{t("limits.soloDesc")}</span>
    </div>
    <Toggle bind:checked={app.soloMode} onchange={onSolo} label={t("limits.solo")} />
  </div>
</section>

<div class="card list">
  <div class="row head-row">
    <span class="col-name">{t("apps.colApp")}</span>
    <span class="col-lim">{t("limits.down")}</span>
    <span class="col-lim">{t("limits.up")}</span>
  </div>
  {#each rows as r (r.id)}
    <div class="row">
      <div class="col-name app">
        <span class="ico" class:heavy={r.heavy}>{initials(r.name)}</span>
        <span class="meta">
          <span class="name">{r.name}
            {#if r.heavy}<span class="tag-heavy">{t("limits.heavy")}</span>{/if}
          </span>
          <span class="kind">{r.kind}</span>
        </span>
      </div>
      <div class="col-lim">
        <input class="num" type="number" min="0" max="1000000" bind:value={r.down} oninput={() => sendLimit(r)} />
        <span class="unit">{r.down ? "kbps" : t("limits.unlimited")}</span>
      </div>
      <div class="col-lim">
        <input class="num" type="number" min="0" max="1000000" bind:value={r.up} oninput={() => sendLimit(r)} />
        <span class="unit">{r.up ? "kbps" : t("limits.unlimited")}</span>
      </div>
    </div>
  {/each}
</div>

<p class="note">{t("limits.note")}</p>

<style>
  .head { margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }

  .solo { margin-bottom: 16px; }
  .opt { display: flex; align-items: center; justify-content: space-between; gap: 18px; }
  .opt-text { display: flex; flex-direction: column; gap: 3px; }
  .opt-name { font-weight: 600; font-size: 14px; }
  .opt-desc { color: var(--text-muted); font-size: 12.5px; line-height: 1.45; }

  .list { padding: 6px 0; }
  .row { display: grid; grid-template-columns: 1fr 160px 160px; align-items: center; padding: 10px 18px; gap: 10px; }
  .row:not(.head-row):not(:last-child) { border-bottom: 1px solid var(--border-soft); }
  .head-row { color: var(--text-muted); font-size: 11.5px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; padding-bottom: 8px; }

  .app { display: flex; align-items: center; gap: 12px; }
  .ico {
    width: 34px; height: 34px; flex: 0 0 auto; border-radius: 8px;
    display: grid; place-items: center; font-weight: 700; font-size: 15px;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--accent);
  }
  .ico.heavy { color: var(--amber); border-color: color-mix(in srgb, var(--amber) 45%, transparent); }
  .meta { display: flex; flex-direction: column; }
  .name { font-weight: 600; font-size: 14px; display: inline-flex; align-items: center; gap: 7px; }
  .tag-heavy { font-size: 10px; font-weight: 700; color: var(--amber); background: color-mix(in srgb, var(--amber) 14%, transparent); padding: 1px 6px; border-radius: 4px; }
  .kind { color: var(--text-muted); font-size: 12px; }

  .col-lim { display: flex; align-items: center; gap: 8px; }
  .num {
    width: 100px; padding: 7px 9px; border-radius: var(--radius-sm);
    background: var(--bg-base); border: 1px solid var(--border); color: var(--text);
    font: inherit; font-size: 13px; font-variant-numeric: tabular-nums; outline: none;
  }
  .num:focus { border-color: var(--accent); }
  .unit { color: var(--text-muted); font-size: 12px; }

  .note { margin-top: 14px; color: var(--text-dim); font-size: 12px; line-height: 1.5; }
</style>
