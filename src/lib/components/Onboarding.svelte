<script lang="ts">
  // İlk-çalıştırma onboarding (docs/02 §6). 4 adım, atlanabilir.
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "$lib/state.svelte";
  import { t } from "$lib/i18n.svelte";
  import { fade, fly } from "svelte/transition";

  let step = $state(0);
  const STEPS = 3;

  // gerçek bağlantı testi (lib.rs connectivity_test)
  type Diag = { dns_ok: boolean; reachable: boolean; ms: number };
  let test = $state<"idle" | "running" | "done">("idle");
  let diag = $state<Diag>({ dns_ok: false, reachable: false, ms: 0 });
  async function runTest() {
    test = "running";
    try {
      diag = await invoke<Diag>("connectivity_test");
    } catch (_) {
      diag = { dns_ok: false, reachable: false, ms: 0 };
    }
    test = "done";
  }

  function next() {
    if (step < STEPS - 1) step += 1;
    else finish();
  }
  function back() {
    if (step > 0) step -= 1;
  }
  function finish() {
    app.finishOnboarding();
  }
  function onkey(e: KeyboardEvent) {
    if (e.key === "Escape") finish();
    else if (e.key === "ArrowRight" || e.key === "Enter") next();
    else if (e.key === "ArrowLeft") back();
  }
</script>

<svelte:window onkeydown={onkey} />

<div class="overlay" transition:fade={{ duration: 160 }}>
  <div class="card" role="dialog" aria-modal="true" aria-label={t("onb.welcomeTitle")} transition:fly={{ y: 12, duration: 220 }}>
    <button class="skip" onclick={finish}>{t("onb.skip")}</button>

    <div class="stage">
      {#if step === 0}
        <div class="step" in:fly={{ x: 16, duration: 180 }}>
          <div class="emblem">◈</div>
          <h2>{t("onb.welcomeTitle")}</h2>
          <p>{t("onb.welcomeBody")}</p>
        </div>
      {:else if step === 1}
        <div class="step" in:fly={{ x: 16, duration: 180 }}>
          <div class="emblem">📡</div>
          <h2>{t("onb.testTitle")}</h2>
          <p>{t("onb.testBody")}</p>
          {#if test === "idle"}
            <button class="btn primary big" onclick={runTest}>{t("onb.testRun")}</button>
          {:else if test === "running"}
            <div class="testing"><span class="spinner"></span> {t("onb.testing")}</div>
          {:else}
            <div class="test-ok" in:fade={{ duration: 200 }}>
              <div class="res"><span class="ok" class:bad={!diag.dns_ok}>{diag.dns_ok ? "✓" : "✕"}</span> {t("onb.testDns")}</div>
              <div class="res"><span class="ok" class:bad={!diag.reachable}>{diag.reachable ? "✓" : "✕"}</span> {t("onb.testEngine")}</div>
              <div class="res mono">{diag.reachable ? `~${diag.ms} ms` : "—"}</div>
            </div>
          {/if}
        </div>
      {:else}
        <div class="step" in:fly={{ x: 16, duration: 180 }}>
          <div class="emblem pulse">⏻</div>
          <h2>{t("onb.readyTitle")}</h2>
          <p>{t("onb.readyBody")}</p>
        </div>
      {/if}
    </div>

    <div class="dots" role="progressbar" aria-valuenow={step + 1} aria-valuemax={STEPS}>
      {#each Array(STEPS) as _, i}
        <span class="dot" class:on={i === step}></span>
      {/each}
    </div>

    <div class="nav">
      <button class="btn ghost" onclick={back} disabled={step === 0}>{t("onb.back")}</button>
      <button class="btn primary" onclick={next}>
        {step === STEPS - 1 ? t("onb.start") : t("onb.next")}
      </button>
    </div>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 950;
    display: grid; place-items: center;
    background: rgba(2, 4, 8, 0.72); backdrop-filter: blur(4px);
  }
  .card {
    position: relative;
    width: min(460px, calc(100vw - 40px));
    background: var(--bg-surface); border: 1px solid var(--border);
    border-radius: var(--radius); padding: 28px 26px 22px;
    box-shadow: 0 24px 70px rgba(0, 0, 0, 0.6);
  }
  .skip {
    position: absolute; top: 14px; right: 16px;
    background: none; border: none; color: var(--text-dim); cursor: pointer;
    font-size: 13px; padding: 4px 8px; border-radius: 6px; transition: color 0.12s;
  }
  .skip:hover { color: var(--text); }

  .stage { min-height: 230px; display: grid; }
  .step { text-align: center; display: flex; flex-direction: column; align-items: center; gap: 10px; grid-area: 1 / 1; }
  .emblem {
    width: 64px; height: 64px; border-radius: 18px; display: grid; place-items: center;
    font-size: 30px; margin-bottom: 4px;
    background: var(--bg-elevated); border: 1px solid var(--border); color: var(--accent);
  }
  .emblem.pulse { animation: breathe 2.4s ease-in-out infinite; }
  @keyframes breathe {
    0%, 100% { transform: scale(1); opacity: 0.9; }
    50% { transform: scale(1.06); opacity: 1; }
  }
  h2 { font-size: 20px; font-weight: 800; }
  .step p { color: var(--text-muted); line-height: 1.55; max-width: 380px; }

  .big { padding: 11px 22px; font-size: 15px; margin-top: 6px; }
  .testing { display: flex; align-items: center; gap: 10px; color: var(--text-muted); margin-top: 10px; }
  .spinner {
    width: 16px; height: 16px; border-radius: 50%;
    border: 2px solid var(--border); border-top-color: var(--accent);
    animation: spin 0.7s linear infinite;
  }
  @keyframes spin { to { transform: rotate(360deg); } }
  .test-ok { display: flex; flex-direction: column; gap: 8px; margin-top: 8px; }
  .res { display: flex; align-items: center; gap: 8px; font-size: 14px; }
  .res .ok { color: var(--green); font-weight: 700; }
  .res .ok.bad { color: var(--red); }
  .res.mono { color: var(--accent); justify-content: center; }

  .dots { display: flex; justify-content: center; gap: 7px; margin: 18px 0 16px; }
  .dot { width: 7px; height: 7px; border-radius: 50%; background: var(--border); transition: background 0.18s, transform 0.18s; }
  .dot.on { background: var(--accent); transform: scale(1.25); }

  .nav { display: flex; justify-content: space-between; gap: 10px; }
  .btn.ghost { background: transparent; }
  .btn.ghost:hover { background: var(--bg-elevated); }
  .btn:disabled { opacity: 0.4; cursor: not-allowed; }
</style>
