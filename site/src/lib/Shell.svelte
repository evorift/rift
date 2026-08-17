<script>
  // Alt sayfaların ortak kabuğu: üst bar + dil seçici + alt bilgi.
  // Ana sayfa kendi hero düzenini taşıdığı için bunu kullanmaz.
  import { t, getLang, setLang, LANGS, LANG_LABEL } from "$lib/i18n.svelte.js";
  import { base } from "$app/paths";

  let { title, lead = "", children } = $props();

  const BRAND = "evorift";
  const REPO = "https://github.com/evorift/rift";
  const RELEASES = REPO + "/releases";
</script>

<svelte:head>
  <title>{BRAND} — {title}</title>
</svelte:head>

<header class="topbar">
  <a class="brand" href="{base}/" aria-label={BRAND}>
    <img class="mark" src="{base}/logo.png" alt="" aria-hidden="true" />
    <span class="brand-name">{BRAND}</span>
  </a>

  <nav class="top-nav">
    <a href="{base}/indir">{t("nav.download")}</a>
    <a href="{base}/gizlilik">{t("nav.privacy")}</a>
    <a href="{base}/destek">{t("nav.support")}</a>
    <a href={REPO} target="_blank" rel="noopener">{t("nav.github")}</a>
  </nav>

  <div class="langs" role="group" aria-label="language">
    {#each LANGS as l (l)}
      <button class="lang" class:active={getLang() === l} onclick={() => setLang(l)}>{LANG_LABEL[l]}</button>
    {/each}
  </div>
</header>

<main class="doc">
  <h1>{title}</h1>
  {#if lead}<p class="lead">{lead}</p>{/if}
  {@render children()}
</main>

<footer class="footer">
  <p class="ss">{t("foot.smartscreen")}</p>
  <p class="ss">
    {t("foot.official")}
    <a href={RELEASES} target="_blank" rel="noopener">github.com/evorift/rift/releases</a>.
    {t("foot.official.warn")}
  </p>
  <p class="made">{t("foot.made")} · © {BRAND}</p>
</footer>

<style>
  .topbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 13px clamp(16px, 5vw, 48px);
    background: color-mix(in srgb, var(--bg-base) 78%, transparent);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-soft);
  }
  .brand { display: inline-flex; align-items: center; gap: 10px; font-weight: 800; }
  .mark { width: 32px; height: 32px; flex: none; display: block; object-fit: contain; }
  .brand-name { font-size: 19px; }
  .top-nav { display: flex; gap: 22px; color: var(--text-muted); font-size: 14px; font-weight: 600; }
  .top-nav a:hover { color: var(--text); }
  .langs { margin-left: auto; display: inline-flex; gap: 2px; background: var(--bg-elevated); border: 1px solid var(--border-soft); border-radius: 999px; padding: 3px; }
  .lang { border: none; background: transparent; color: var(--text-dim); font: inherit; font-size: 12px; font-weight: 700; padding: 5px 10px; border-radius: 999px; cursor: pointer; }
  .lang:hover { color: var(--text-muted); }
  .lang.active { background: var(--accent); color: #04140a; }

  .doc {
    max-width: 760px;
    margin: 0 auto;
    padding: clamp(28px, 6vw, 56px) clamp(16px, 5vw, 48px) 64px;
  }
  .doc h1 { font-size: clamp(28px, 5vw, 42px); font-weight: 800; letter-spacing: -0.02em; }
  .lead { margin-top: 14px; color: var(--text-muted); font-size: 16px; }

  .footer {
    max-width: var(--maxw);
    margin: 0 auto;
    padding: 32px clamp(16px, 5vw, 48px) 56px;
    border-top: 1px solid var(--border-soft);
    text-align: center;
    color: var(--text-dim);
    font-size: 13px;
  }
  .footer .ss { max-width: 640px; margin: 0 auto 12px; line-height: 1.5; }
  .footer a { color: var(--accent); }

  @media (max-width: 640px) {
    .top-nav { display: none; }
  }
</style>
