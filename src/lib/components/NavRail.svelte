<script lang="ts">
  let { active, onSelect }: { active: string; onSelect: (k: string) => void } = $props();

  // Lucide-tarzı ikonlar (stroke, 24x24)
  const items = [
    { k: "dashboard",   label: "Panel",       icon: `<path d="M3 13h8V3H3zM13 21h8V8h-8zM3 21h8v-6H3zM13 3v3h8V3z"/>` },
    { k: "connection",  label: "Bağlantı",    icon: `<path d="M13 2 3 14h7l-1 8 10-12h-7z"/>` },
    { k: "performance", label: "Performans",  icon: `<path d="M3 12h4l3 8 4-16 3 8h4"/>` },
    { k: "apps",        label: "Uygulamalar", icon: `<path d="M4 4h6v6H4zM14 4h6v6h-6zM4 14h6v6H4zM14 14h6v6h-6z"/>` },
    { k: "logs",        label: "Günlük",      icon: `<path d="M8 6h13M8 12h13M8 18h13M3 6h.01M3 12h.01M3 18h.01"/>` },
    { k: "settings",    label: "Ayarlar",     icon: `<circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 1 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-2.82 1.17V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 1 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.6 15H4a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 6 9.4l-.06-.06a2 2 0 1 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 12 4.6V4a2 2 0 0 1 4 0v.09A1.65 1.65 0 0 0 18 6l.06-.06a2 2 0 1 1 2.83 2.83L20.83 9A1.65 1.65 0 0 0 20.4 11H21a2 2 0 0 1 0 4z"/>` },
  ];
</script>

<nav class="rail">
  <div class="items">
    {#each items as it}
      <button class="item" class:active={active === it.k} onclick={() => onSelect(it.k)}>
        <svg viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{@html it.icon}</svg>
        <span>{it.label}</span>
      </button>
    {/each}
  </div>
  <div class="foot mono">v0.1.0</div>
</nav>

<style>
  .rail {
    width: var(--rail-w); flex: 0 0 auto;
    background: var(--bg-base);
    border-right: 1px solid var(--border-soft);
    display: flex; flex-direction: column; justify-content: space-between;
    padding: 12px 10px;
  }
  .items { display: flex; flex-direction: column; gap: 4px; }
  .item {
    display: flex; align-items: center; gap: 12px;
    padding: 10px 12px; border: none; border-radius: var(--radius-sm);
    background: transparent; color: var(--text-muted);
    font-size: 14px; font-weight: 600; cursor: pointer; text-align: left;
    position: relative; transition: background .14s, color .14s;
  }
  .item:hover { background: var(--bg-surface); color: var(--text); }
  .item.active { background: var(--bg-elevated); color: var(--text); }
  .item.active::before {
    content: ""; position: absolute; left: 0; top: 8px; bottom: 8px; width: 3px;
    background: var(--accent); border-radius: 0 3px 3px 0;
  }
  .item svg { width: 19px; height: 19px; flex: 0 0 auto; }
  .item.active svg { color: var(--accent); }
  .foot { color: var(--text-dim); font-size: 11px; padding: 6px 12px; }
</style>
