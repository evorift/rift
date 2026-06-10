<script lang="ts">
  // Animasyonlu toggle — thumb yalnızca transform'da kayar (compositor, jank yok).
  let {
    checked = $bindable(false),
    label = "",
    ariaLabel,
    disabled = false,
    onchange,
  }: {
    checked?: boolean;
    label?: string;
    ariaLabel?: string; // görünür metin olmadan erişilebilir ad (label boşken)
    disabled?: boolean;
    onchange?: (v: boolean) => void;
  } = $props();

  function toggle() {
    if (disabled) return;
    checked = !checked;
    onchange?.(checked);
  }
</script>

<button
  type="button"
  class="toggle"
  class:on={checked}
  {disabled}
  role="switch"
  aria-checked={checked}
  aria-label={ariaLabel ?? label}
  onclick={toggle}
>
  <span class="track"><span class="thumb"></span></span>
  {#if label}<span class="lbl">{label}</span>{/if}
</button>

<style>
  .toggle {
    display: inline-flex; align-items: center; gap: 10px;
    background: none; border: none; cursor: pointer; padding: 2px;
    color: var(--text); font: inherit;
  }
  .toggle:disabled { opacity: 0.45; cursor: not-allowed; }
  .track {
    position: relative; width: 38px; height: 22px; border-radius: 999px;
    background: var(--bg-hover); border: 1px solid var(--border);
    transition: background 0.16s ease, border-color 0.16s ease;
    flex: 0 0 auto;
  }
  .thumb {
    position: absolute; top: 2px; left: 2px; width: 16px; height: 16px;
    border-radius: 50%; background: var(--text-muted);
    transform: translateX(0); transition: transform 0.18s cubic-bezier(.2,.8,.3,1), background 0.16s ease;
    will-change: transform;
  }
  .toggle.on .track { background: color-mix(in srgb, var(--accent) 30%, transparent); border-color: var(--accent-dim); }
  .toggle.on .thumb { transform: translateX(16px); background: var(--accent); }
  .lbl { font-size: 14px; font-weight: 600; }
</style>
