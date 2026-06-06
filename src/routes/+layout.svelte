<script lang="ts">
  import "$lib/theme.css";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import RiftBackground from "$lib/components/RiftBackground.svelte";
  import { app } from "$lib/state.svelte";
  import { onMount } from "svelte";
  let { children } = $props();
  onMount(() => { app.init(); });
</script>

<RiftBackground />

<div class="app-root">
  <TitleBar />
  <div class="body">
    {@render children()}
  </div>
</div>

<style>
  .app-root {
    position: relative; z-index: 1;
    display: flex; flex-direction: column;
    height: 100vh; width: 100vw;
    background: transparent;   /* rift arka planı görünsün */
    overflow: hidden;
  }
  .body { flex: 1 1 auto; display: flex; min-height: 0; }
</style>
