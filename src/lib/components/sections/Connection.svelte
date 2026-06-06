<script lang="ts">
  import { app } from "$lib/state.svelte";
  import Segmented from "$lib/components/ui/Segmented.svelte";

  // DPI-bypass strateji profilleri (docs/03 §1). Şimdilik etiketler; motor Faz 3'te bağlanır.
  const strategies = [
    { value: "auto", label: "Otomatik" },
    { value: "c1", label: "Fragmentasyon" },
    { value: "multidisorder", label: "Multidisorder" },
    { value: "fake", label: "Fake TLS" },
  ];

  // Secure DNS sağlayıcıları (docs/03 §2)
  const dnsProviders = [
    { value: "cloudflare", name: "Cloudflare", addr: "1.1.1.1", note: "DoH · hızlı" },
    { value: "quad9", name: "Quad9", addr: "9.9.9.9", note: "DoH · malware filtresi" },
    { value: "adguard", name: "AdGuard", addr: "94.140.14.14", note: "DoH · reklam/tracker" },
    { value: "google", name: "Google", addr: "8.8.8.8", note: "DoH · stabil" },
  ];

  // Tek-tık Quick Fix preset'leri (docs/03 §1 — Free)
  const quickFixes = [
    { id: "discord", label: "Discord", desc: "Ses + metin engelini aç" },
    { id: "roblox", label: "Roblox", desc: "Oyun + giriş erişimi" },
    { id: "youtube", label: "YouTube", desc: "Video throttle bypass" },
  ];

  // Per-domain Site List (docs/03 §1 — Free)
  let sites = $state<string[]>(["discord.com", "roblox.com", "youtube.com"]);
  let newSite = $state("");

  function addSite() {
    const v = newSite.trim().toLowerCase();
    if (v && !sites.includes(v)) sites = [...sites, v];
    newSite = "";
  }
  function removeSite(s: string) {
    sites = sites.filter((x) => x !== s);
  }
</script>

<header class="head"><h1>Bağlantı & Stratejiler</h1></header>

<div class="grid">
  <!-- Strateji seçici -->
  <section class="card">
    <div class="card-head">
      <h3>Bypass Stratejisi</h3>
      <span class="hint">Engel aşma yöntemi. Çalışmazsa diğerini dene.</span>
    </div>
    <Segmented options={strategies} bind:value={app.strategy} />
    <p class="explain">
      <strong>Otomatik</strong>: Rift hattını test edip çalışan stratejiyi kendi seçer (önerilen).
    </p>
  </section>

  <!-- Hızlı düzelt -->
  <section class="card">
    <div class="card-head">
      <h3>Hızlı Düzelt</h3>
      <span class="hint">Tek tıkla uygulamaya özel preset.</span>
    </div>
    <div class="quick">
      {#each quickFixes as q}
        <button class="qf">
          <span class="qf-name">{q.label}</span>
          <span class="qf-desc">{q.desc}</span>
        </button>
      {/each}
    </div>
  </section>

  <!-- Secure DNS -->
  <section class="card span-2">
    <div class="card-head">
      <h3>Güvenli DNS</h3>
      <span class="hint">DNS'i HTTPS üzerinden çözer — ISS göremez/engelleyemez.</span>
    </div>
    <div class="dns">
      {#each dnsProviders as d}
        <button
          class="dns-card"
          class:active={app.dns === d.value}
          onclick={() => (app.dns = d.value)}
        >
          <span class="dns-name">{d.name}</span>
          <span class="dns-addr mono">{d.addr}</span>
          <span class="dns-note">{d.note}</span>
          {#if app.dns === d.value}<span class="dns-check">✓</span>{/if}
        </button>
      {/each}
    </div>
  </section>

  <!-- Site listesi -->
  <section class="card span-2">
    <div class="card-head">
      <h3>Site Listesi</h3>
      <span class="hint">Yalnızca bu siteler bypass'tan etkilenir; gerisi normal kalır.</span>
    </div>

    <form class="add-row" onsubmit={(e) => { e.preventDefault(); addSite(); }}>
      <input class="inp" placeholder="örn. example.com" bind:value={newSite} />
      <button class="btn primary" type="submit">Ekle</button>
    </form>

    {#if sites.length === 0}
      <div class="empty">Liste boş — bir alan adı ekle.</div>
    {:else}
      <ul class="sites">
        {#each sites as s}
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
  .head { margin-bottom: 16px; }
  .head h1 { font-size: 22px; font-weight: 700; }

  .grid { display: grid; grid-template-columns: 1fr 1fr; gap: 16px; align-content: start; }
  .span-2 { grid-column: 1 / -1; }

  .card-head { margin-bottom: 14px; }
  .card-head h3 { font-size: 15px; font-weight: 700; }
  .hint { display: block; color: var(--text-muted); font-size: 12.5px; margin-top: 3px; line-height: 1.45; }

  .explain { margin-top: 12px; color: var(--text-muted); font-size: 13px; line-height: 1.5; }
  .explain strong { color: var(--text); }

  /* quick fix */
  .quick { display: grid; grid-template-columns: 1fr 1fr 1fr; gap: 10px; }
  .qf {
    display: flex; flex-direction: column; gap: 4px; align-items: flex-start;
    padding: 12px; border-radius: var(--radius-sm); cursor: pointer; text-align: left;
    background: var(--bg-elevated); border: 1px solid var(--border);
    transition: transform 0.08s ease, border-color 0.14s ease, background 0.14s ease;
  }
  .qf:hover { border-color: var(--accent-dim); background: var(--bg-hover); }
  .qf:active { transform: scale(0.98); }
  .qf-name { font-weight: 700; font-size: 14px; }
  .qf-desc { color: var(--text-muted); font-size: 12px; line-height: 1.4; }

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
