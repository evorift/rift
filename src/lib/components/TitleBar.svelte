<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  const win = getCurrentWindow();
  const minimize = () => win.minimize();
  const maximize = () => win.toggleMaximize();
  const close = () => win.close();
</script>

<div class="titlebar" data-tauri-drag-region>
  <div class="brand" data-tauri-drag-region>
    <span class="logo">◈</span>
    <span class="name">Rift</span>
  </div>

  <div class="controls">
    <button class="ctl" onclick={minimize} title="Küçült" aria-label="Küçült">
      <svg width="11" height="11" viewBox="0 0 11 11"><line x1="1" y1="6" x2="10" y2="6" stroke="currentColor" stroke-width="1.2"/></svg>
    </button>
    <button class="ctl" onclick={maximize} title="Büyüt" aria-label="Büyüt">
      <svg width="11" height="11" viewBox="0 0 11 11"><rect x="1.5" y="1.5" width="8" height="8" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.2"/></svg>
    </button>
    <button class="ctl close" onclick={close} title="Kapat" aria-label="Kapat">
      <svg width="11" height="11" viewBox="0 0 11 11"><line x1="1.5" y1="1.5" x2="9.5" y2="9.5" stroke="currentColor" stroke-width="1.2"/><line x1="9.5" y1="1.5" x2="1.5" y2="9.5" stroke="currentColor" stroke-width="1.2"/></svg>
    </button>
  </div>
</div>

<style>
  .titlebar {
    height: var(--titlebar-h);
    display: flex; align-items: center; justify-content: space-between;
    background: var(--bg-base);
    border-bottom: 1px solid var(--border-soft);
    padding-left: 14px;
    flex: 0 0 auto;
  }
  .brand { display: flex; align-items: center; gap: 9px; pointer-events: none; }
  .logo { color: var(--accent); font-size: 16px; }
  .name { font-weight: 700; letter-spacing: .5px; }
  .controls { display: flex; height: 100%; }
  .ctl {
    width: 46px; height: 100%; border: none; background: transparent;
    color: var(--text-muted); cursor: pointer; display: grid; place-items: center;
    transition: background .12s, color .12s;
  }
  .ctl:hover { background: var(--bg-hover); color: var(--text); }
  .ctl.close:hover { background: var(--red); color: #fff; }
</style>
