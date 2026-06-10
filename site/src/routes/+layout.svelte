<script>
  import "../app.css";
  import { onMount } from "svelte";
  import { initLang } from "$lib/i18n.svelte.js";

  let { children } = $props();

  onMount(() => initLang());
</script>

{@render children()}
