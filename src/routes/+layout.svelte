<script lang="ts">
  import "$lib/theme.css";
  import TitleBar from "$lib/components/TitleBar.svelte";
  import Toaster from "$lib/components/ui/Toaster.svelte";
  import Onboarding from "$lib/components/Onboarding.svelte";
  import BinaryRain from "$lib/components/BinaryRain.svelte";
  import Modal from "$lib/components/ui/Modal.svelte";
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n.svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { openUrl } from "@tauri-apps/plugin-opener";
  import { onMount } from "svelte";
  let { children } = $props();

  // GitHub Sponsor popup — uygulama HER açıldığında gösterilir (kullanıcı isteği).
  // Kolay kapanır: "Şimdi Değil" / Escape / backdrop tıklaması. Buton → tarayıcıda sponsor sayfası.
  const SPONSOR_URL = "https://github.com/sponsors/evorift";
  let showSponsor = $state(false);
  function openSponsor() {
    openUrl(SPONSOR_URL).catch(() => {});
  }

  onMount(() => {
    app.init();
    showSponsor = true; // her açılışta sponsor popup
  });

  // dil değişince (ve açılışta) tray menü etiketlerini güncelle (Faz 4.7 tray i18n)
  $effect(() => {
    app.language; // reaktif bağımlılık
    invoke("set_tray_labels", { show: t("tray.show"), toggle: t("tray.toggle"), quit: t("tray.quit") }).catch(() => {});
  });
</script>

<BinaryRain />

<div class="app-root">
  <TitleBar />
  <div class="body">
    {@render children()}
  </div>
  <Toaster />
  {#if app.showOnboarding}<Onboarding />{/if}
</div>

<!-- GitHub Sponsor popup (her açılışta) — link butonu + kolay kapanır -->
<Modal
  bind:open={showSponsor}
  title="evorift'i Destekle 💜"
  message={"evorift ücretsiz ve açık kaynak. Geliştirmenin devam edebilmesi için GitHub'da sponsor olarak destek olabilirsin — her katkı çok değerli, teşekkürler!"}
  confirmLabel="💜 GitHub'da Sponsor Ol"
  cancelLabel="Şimdi Değil"
  onconfirm={openSponsor}
/>

<style>
  .app-root {
    position: relative; z-index: 1;
    display: flex; flex-direction: column;
    height: 100vh; width: 100vw;
    background: transparent; /* binary yağmuru içeriğin arkasından görünsün; taban siyah body'de */
    overflow: hidden;
  }
  .body { flex: 1 1 auto; display: flex; min-height: 0; }
</style>
