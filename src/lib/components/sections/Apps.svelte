<script lang="ts">
  import { t } from "$lib/i18n.svelte";
  import { app, type AppMode, type AppRow } from "$lib/state.svelte";

  // Gerçek çalışan ağ-uygulamaları (Faz P1.3, list_apps); boşsa kalıcı liste korunur.
  const rows = $derived(app.apps);

  const initials = (n: string) => n.slice(0, 1).toUpperCase();

  /// Uzun uygulama isimlerini 25 karakterde kırp + tek ellipsis ekle (toplam ≤ 25 görünür kalsın
  /// → liste satırları daima aynı yükseklikte, kayma yok). 25 ASCII'de net okunur, Türkçe'de bile
  /// kelime sınırını yakalamadan kesmek sorun değil (gerçek ad title attr'ında tam görünür).
  const truncName = (n: string) => (n.length > 25 ? n.slice(0, 24) + "…" : n);

  /// Mod açıklama blokları (sayfa üstündeki hero) + alttaki segment radyo başlıkları.
  const MODES: { value: AppMode; key: string; tip: string; descKey: string }[] = [
    { value: "off",  key: "apps.modeOff",  tip: "apps.modeOffTip",  descKey: "apps.modeOffDesc"  },
    { value: "dpi",  key: "apps.modeDpi",  tip: "apps.modeDpiTip",  descKey: "apps.modeDpiDesc"  },
    { value: "warp", key: "apps.modeWarp", tip: "apps.modeWarpTip", descKey: "apps.modeWarpDesc" },
  ];
</script>

<header class="head">
  <h1>{t("apps.title")}</h1>
  <div class="head-right">
    <span class="muted">{rows.length} {t("apps.detectedSuffix")}</span>
    <button class="btn" onclick={() => app.refreshApps()}>↻ {t("apps.refresh")}</button>
  </div>
</header>

<!-- 3 koruma modu açıklaması — sayfa üstündeki kalıcı hero. -->
<div class="modes-hero">
  {#each MODES as m (m.value)}
    <div class="mode-block mode-{m.value}">
      <span class="mode-badge">{t(m.key)}</span>
      <span class="mode-desc">{t(m.descKey)}</span>
    </div>
  {/each}
</div>

<!-- Dürüst açıklama: DPI atlatma sistem-geneli; mod seçimi WARP'ı ve domain hint'lerini etkiler. -->
<div class="scope-note">
  <span class="scope-icon">ⓘ</span>
  <span>{t("apps.scopeNote")}</span>
</div>

{#if rows.length === 0}
  <div class="card empty-state">
    <div class="es-icon">🎮</div>
    <h3>{t("apps.emptyTitle")}</h3>
    <p>{t("apps.emptyBody")}</p>
  </div>
{:else}
  <div class="card list">
    <div class="row head-row">
      <span class="col-name">{t("apps.colApp")}</span>
      <span class="col-mode">{t("apps.colMode")}</span>
    </div>
    {#each rows as r (r.id)}
      <div class="row">
        <div class="col-name app">
          <span class="ico">{initials(r.name)}</span>
          <span class="meta">
            <span class="name" title={r.name}>{truncName(r.name)}</span>
            <span class="kind">
              {truncName(r.kind ?? "")}
              {#if r.mode !== "off" && r.domains && r.domains.length > 0}
                <span class="dchip" title={r.domains.join("\n")}>· {r.domains.length} {t("apps.domainsMasked")}</span>
              {/if}
            </span>
          </span>
        </div>
        <div class="col-mode">
          <div class="seg" role="radiogroup" aria-label="{r.name} koruma modu">
            {#each MODES as m (m.value)}
              <button
                type="button"
                role="radio"
                aria-checked={r.mode === m.value}
                class="seg-btn"
                class:active={r.mode === m.value}
                class:warp={m.value === "warp"}
                class:dpi={m.value === "dpi"}
                class:off={m.value === "off"}
                title={t(m.tip)}
                onclick={() => app.setAppMode(r as AppRow, m.value)}
              >
                {t(m.key)}
              </button>
            {/each}
          </div>
        </div>
      </div>
    {/each}
  </div>

{/if}

<style>
  .head { display: flex; align-items: center; justify-content: space-between; margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }
  .head-right { display: flex; align-items: center; gap: 10px; }
  .muted { color: var(--text-muted); font-size: 13px; }

  /* Üstteki 3 koruma modu açıklama bloğu — segment renkleriyle eşleşir. */
  .modes-hero {
    display: grid; grid-template-columns: repeat(3, 1fr); gap: 10px;
    margin-bottom: 18px;
  }
  .mode-block {
    background: var(--bg-elevated); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 12px 14px;
    display: flex; flex-direction: column; gap: 6px;
  }
  .mode-badge {
    align-self: flex-start; font-size: 11.5px; font-weight: 700; letter-spacing: 0.4px;
    text-transform: uppercase;
    padding: 3px 9px; border-radius: 999px;
    color: var(--bg);
  }
  .mode-off  .mode-badge { background: var(--text-muted); }
  .mode-dpi  .mode-badge { background: var(--accent); }
  .mode-warp .mode-badge { background: #7c5cff; }
  .mode-off  { border-color: color-mix(in srgb, var(--text-muted) 25%, var(--border)); }
  .mode-dpi  { border-color: color-mix(in srgb, var(--accent) 30%, var(--border)); }
  .mode-warp { border-color: color-mix(in srgb, #7c5cff 30%, var(--border)); }
  .mode-desc { color: var(--text); font-size: 12.5px; line-height: 1.5; }

  /* Dürüst kapsam notu — mod butonlarının altında, ne yaptığını net açıklar. */
  .scope-note {
    display: flex; gap: 10px; align-items: flex-start;
    color: var(--text-muted); font-size: 12.5px; line-height: 1.55;
    padding: 10px 14px; margin-bottom: 16px;
    background: var(--bg-elevated); border: 1px solid var(--border-soft);
    border-left: 3px solid var(--accent);
    border-radius: var(--radius-sm);
  }
  .scope-icon { color: var(--accent); font-weight: 700; font-size: 14px; line-height: 1.4; flex: 0 0 auto; }

  .empty-state { display: flex; flex-direction: column; align-items: center; gap: 8px; text-align: center; padding: 48px 20px; }
  .es-icon { font-size: 34px; opacity: 0.6; }
  .empty-state h3 { font-size: 16px; font-weight: 700; }
  .empty-state p { color: var(--text-muted); max-width: 340px; line-height: 1.5; }

  .list { padding: 6px 0; }
  .row {
    display: grid; grid-template-columns: 1fr 320px; align-items: center;
    padding: 10px 18px; gap: 10px;
  }
  .row:not(.head-row):not(:last-child) { border-bottom: 1px solid var(--border-soft); }
  .head-row { color: var(--text-muted); font-size: 11.5px; font-weight: 600; text-transform: uppercase; letter-spacing: 0.4px; padding-bottom: 8px; }
  .col-mode { display: flex; justify-content: flex-end; }

  /* 3-segment mod seçici: off | dpi | warp. Aktif segment vurgulanır (renk segmente göre değişir). */
  .seg {
    display: inline-flex; gap: 0;
    background: var(--bg-elevated); border: 1px solid var(--border);
    border-radius: 999px; padding: 3px;
  }
  .seg-btn {
    appearance: none; background: transparent; border: 0; color: var(--text-muted);
    padding: 6px 12px; font-size: 12px; font-weight: 600; line-height: 1;
    border-radius: 999px; cursor: pointer; transition: background 120ms, color 120ms;
  }
  .seg-btn:hover:not(.active) { color: var(--text); }
  .seg-btn.active { color: var(--bg); }
  .seg-btn.active.off  { background: var(--text-muted); }
  .seg-btn.active.dpi  { background: var(--accent); }
  .seg-btn.active.warp { background: #7c5cff; }

  .app { display: flex; align-items: center; gap: 12px; }
  .ico {
    width: 34px; height: 34px; flex: 0 0 auto; border-radius: 8px;
    display: grid; place-items: center; font-weight: 700; font-size: 15px;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--accent);
  }
  .meta { display: flex; flex-direction: column; min-width: 0; }
  .name { font-weight: 600; font-size: 14px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .kind { color: var(--text-muted); font-size: 12px; white-space: nowrap; overflow: hidden; text-overflow: ellipsis; }
  .dchip { color: var(--accent); font-weight: 600; cursor: help; }
</style>
