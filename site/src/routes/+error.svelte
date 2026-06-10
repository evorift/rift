<script>
  import { page } from "$app/state";
</script>

<svelte:head>
  <title>evorift — {page.status}</title>
</svelte:head>

<main class="err">
  <svg class="mark" viewBox="0 0 64 64" aria-hidden="true">
    <g transform="rotate(-15 32 33)">
      <ellipse cx="32" cy="33" rx="24" ry="9" fill="none" stroke="var(--accent)" stroke-width="7" opacity="0.18" />
      <ellipse cx="32" cy="33" rx="24" ry="9" fill="none" stroke="#fff" stroke-width="2.6" opacity="0.95" />
      <circle cx="32" cy="33" r="12" fill="#000" />
      <path d="M8 33 A24 9 0 0 0 56 33" fill="none" stroke="#fff" stroke-width="2.6" opacity="0.95" />
    </g>
  </svg>
  <h1>{page.status}</h1>
  <p>{page.error?.message ?? "Sayfa bulunamadı"}</p>
  <a class="btn primary" href="/">← Ana sayfa</a>
</main>

<style>
  .err {
    min-height: 100vh;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 14px;
    text-align: center;
    padding: 24px;
  }
  .mark { width: 80px; height: 80px; opacity: 0.9; margin-bottom: 8px; }
  .err h1 { font-size: 64px; font-weight: 800; }
  .err p { color: var(--text-muted); margin-bottom: 12px; }
</style>
