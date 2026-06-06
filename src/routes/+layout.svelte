<script lang="ts">
  import "$lib/theme.css";
  import TitleBar from "$lib/components/TitleBar.svelte";
  let { children } = $props();
</script>

<div class="app-root">
  <TitleBar />
  <div class="body">
    {@render children()}
  </div>
</div>

<style>
  .app-root {
    display: flex; flex-direction: column;
    height: 100vh; width: 100vw;
    background: var(--bg-base);
    overflow: hidden;
  }
  .body { flex: 1 1 auto; display: flex; min-height: 0; }
</style>
