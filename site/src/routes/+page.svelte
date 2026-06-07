<script>
  import BlackHoleHero from "$lib/BlackHoleHero.svelte";
  import { t, getLang, setLang, LANGS, LANG_LABEL } from "$lib/i18n.svelte.js";
  import { base } from "$app/paths";
  import { slide } from "svelte/transition";

  const BRAND = "evorift";
  const REPO = "https://github.com/evorift/rift";
  const RELEASES = REPO + "/releases";
  const SPONSOR = "https://github.com/sponsors/evorift";

  let openFaq = $state(-1);

  function scrollToGet() {
    document.getElementById("get")?.scrollIntoView({ behavior: "smooth", block: "center" });
  }

  const features = [
    { k: "unblock", ico: "◎" },
    { k: "novpn", ico: "⚡" },
    { k: "safe", ico: "🛡" },
    { k: "light", ico: "❉" },
  ];

  const faqs = [1, 2, 3, 4, 5];
</script>

<svelte:head>
  <title>{BRAND} — {t("meta.title")}</title>
  <meta name="description" content={t("hero.sub")} />
</svelte:head>

{#snippet mark()}
  <img class="mark" src="{base}/logo.png" alt="" aria-hidden="true" />
{/snippet}

<a class="skip" href="#get">{t("cta.download")}</a>

<header class="topbar">
  <a class="brand" href="#top" aria-label={BRAND}>
    {@render mark()}
    <span class="brand-name">{BRAND}</span>
  </a>

  <nav class="top-nav">
    <a href="#features">{t("nav.features")}</a>
    <a href="#how">{t("nav.how")}</a>
    <a href="#faq">{t("nav.faq")}</a>
    <a href={REPO} target="_blank" rel="noopener">{t("nav.github")}</a>
  </nav>

  <div class="langs" role="group" aria-label="language">
    {#each LANGS as l (l)}
      <button class="lang" class:active={getLang() === l} onclick={() => setLang(l)}>{LANG_LABEL[l]}</button>
    {/each}
  </div>
</header>

<main id="top">
  <section class="hero">
    <div class="bh-stage">
      <BlackHoleHero onactivate={scrollToGet} />
    </div>

    <div class="wordmark">{BRAND}</div>

    <span class="chip"><i class="dot"></i>{t("brand.badge")}</span>
    <h1 class="hero-title">{t("hero.title")}</h1>
    <p class="hero-sub">{t("hero.sub")}</p>

    <div class="cta" id="get">
      <a class="btn primary" href={RELEASES} target="_blank" rel="noopener">⬇ {t("cta.download")}</a>
      <a class="btn" href={REPO} target="_blank" rel="noopener">{t("cta.github")}</a>
      <a class="btn sponsor" href={SPONSOR} target="_blank" rel="noopener">♥ {t("cta.sponsor")}</a>
    </div>
    <span class="platform mono">{t("platform")}</span>
  </section>

  <section class="features" id="features">
    <h2 class="sec-title">{t("feat.title")}</h2>
    <div class="grid">
      {#each features as f (f.k)}
        <article class="card">
          <span class="f-ico" aria-hidden="true">{f.ico}</span>
          <h3>{t(`feat.${f.k}.t`)}</h3>
          <p>{t(`feat.${f.k}.d`)}</p>
        </article>
      {/each}
    </div>
  </section>

  <section class="how" id="how">
    <h2 class="sec-title">{t("how.title")}</h2>
    <ol class="steps">
      <li><span class="num">1</span><p>{t("how.1")}</p></li>
      <li><span class="num">2</span><p>{t("how.2")}</p></li>
      <li><span class="num">3</span><p>{t("how.3")}</p></li>
    </ol>
  </section>

  <section class="faq" id="faq">
    <h2 class="sec-title">{t("faq.title")}</h2>
    <div class="faq-list">
      {#each faqs as i (i)}
        <div class="faq-item" class:open={openFaq === i}>
          <button class="faq-q" aria-expanded={openFaq === i} onclick={() => (openFaq = openFaq === i ? -1 : i)}>
            <span>{t(`faq.q${i}`)}</span>
            <span class="chev" aria-hidden="true"></span>
          </button>
          {#if openFaq === i}
            <div class="faq-a" transition:slide={{ duration: 240 }}>
              <p>{t(`faq.a${i}`)}</p>
            </div>
          {/if}
        </div>
      {/each}
    </div>
  </section>
</main>

<footer class="footer" id="footer">
  <div class="foot-top">
    <a class="brand" href="#top" aria-label={BRAND}>
      {@render mark()}
      <span class="brand-name">{BRAND}</span>
    </a>
    <p class="foot-tag">{t("foot.tagline")}</p>
  </div>
  <nav class="foot-links">
    <a href="#features">{t("nav.features")}</a>
    <a href="#how">{t("nav.how")}</a>
    <a href="#faq">{t("nav.faq")}</a>
    <a href={REPO} target="_blank" rel="noopener">GitHub</a>
    <a href={SPONSOR} target="_blank" rel="noopener">♥ {t("cta.sponsor")}</a>
  </nav>
  <p class="ss">{t("foot.smartscreen")}</p>
  <p class="made">{t("foot.made")} · © {BRAND}</p>
</footer>

<style>
  .skip {
    position: absolute;
    left: -9999px;
    top: 0;
    z-index: 100;
    background: var(--accent);
    color: #04140a;
    padding: 10px 16px;
    border-radius: 0 0 8px 0;
    font-weight: 700;
  }
  .skip:focus { left: 0; }

  .topbar {
    position: sticky;
    top: 0;
    z-index: 10;
    display: flex;
    align-items: center;
    gap: 20px;
    padding: 13px clamp(16px, 5vw, 48px);
    background: color-mix(in srgb, var(--bg-base) 78%, transparent);
    backdrop-filter: blur(12px);
    -webkit-backdrop-filter: blur(12px);
    border-bottom: 1px solid var(--border-soft);
  }
  .brand { display: inline-flex; align-items: center; gap: 10px; font-weight: 800; letter-spacing: 0.2px; }
  .mark { width: 32px; height: 32px; flex: none; display: block; object-fit: contain; }
  .brand-name { font-size: 19px; }
  .top-nav { display: flex; gap: 22px; margin-left: 8px; color: var(--text-muted); font-size: 14px; font-weight: 600; }
  .top-nav a:hover { color: var(--text); }
  .langs { margin-left: auto; display: inline-flex; gap: 2px; background: var(--bg-elevated); border: 1px solid var(--border-soft); border-radius: 999px; padding: 3px; }
  .lang { border: none; background: transparent; color: var(--text-dim); font: inherit; font-size: 12px; font-weight: 700; padding: 5px 10px; border-radius: 999px; cursor: pointer; }
  .lang:hover { color: var(--text-muted); }
  .lang.active { background: var(--accent); color: #04140a; }

  main { max-width: var(--maxw); margin: 0 auto; padding: 0 clamp(16px, 5vw, 48px); }

  section { scroll-margin-top: 84px; }

  .hero { display: flex; flex-direction: column; align-items: center; text-align: center; padding: clamp(20px, 5vw, 56px) 0 60px; }
  .bh-stage {
    width: min(440px, 80vw);
    height: min(440px, 80vw);
    margin-bottom: 4px;
    filter: drop-shadow(0 0 60px rgba(57, 230, 107, 0.10));
  }
  .wordmark {
    font-size: clamp(32px, 7vw, 56px);
    font-weight: 800;
    letter-spacing: -0.01em;
    line-height: 1;
    color: #fff;
    margin: 0 0 18px;
  }
  .hero-title {
    font-size: clamp(34px, 6vw, 60px);
    line-height: 1.14;
    font-weight: 800;
    letter-spacing: -0.02em;
    margin: 16px 0 0;
    padding-bottom: 0.12em;
    background: linear-gradient(180deg, #fff 62%, #b9c3b9);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
  .hero-sub { max-width: 600px; margin-top: 16px; color: var(--text-muted); font-size: clamp(15px, 2.2vw, 18px); }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; justify-content: center; margin-top: 30px; scroll-margin-top: 120px; }
  .btn.sponsor { color: var(--accent); }
  .btn.sponsor:hover { border-color: var(--accent); color: var(--accent); }
  .platform { margin-top: 14px; color: var(--text-dim); font-size: 12px; letter-spacing: 0.05em; }

  .sec-title { text-align: center; font-size: clamp(24px, 4vw, 34px); font-weight: 800; letter-spacing: -0.01em; margin-bottom: 32px; }

  .features { padding: 40px 0; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 16px; }
  .card {
    background: var(--bg-surface);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius);
    padding: 24px;
    transition: border-color 0.2s ease, transform 0.2s ease;
  }
  .card:hover { border-color: var(--accent-dim); transform: translateY(-3px); }
  .f-ico { display: inline-flex; font-size: 22px; color: var(--accent); margin-bottom: 12px; filter: drop-shadow(0 0 10px var(--accent-glow)); }
  .card h3 { font-size: 18px; margin-bottom: 8px; }
  .card p { color: var(--text-muted); font-size: 14.5px; }

  .how { padding: 40px 0; }
  .steps { list-style: none; padding: 0; margin: 0; display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 16px; }
  .steps li { display: flex; align-items: flex-start; gap: 14px; background: var(--bg-surface); border: 1px solid var(--border-soft); border-radius: var(--radius); padding: 20px; }
  .num {
    flex: none;
    width: 34px; height: 34px;
    display: grid; place-items: center;
    border-radius: 50%;
    background: var(--bg-elevated);
    border: 1px solid var(--accent-dim);
    color: var(--accent);
    font-weight: 800;
  }
  .steps p { color: var(--text-muted); font-size: 14.5px; padding-top: 4px; }

  .faq { padding: 40px 0 56px; }
  .faq-list { max-width: 760px; margin: 0 auto; display: flex; flex-direction: column; gap: 10px; }
  .faq-item {
    background: var(--bg-surface);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .faq-q {
    width: 100%;
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
    background: transparent;
    border: none;
    color: var(--text);
    font: inherit;
    font-weight: 700;
    font-size: 16px;
    text-align: left;
    padding: 18px 20px;
    cursor: pointer;
  }
  .faq-q:hover { background: var(--bg-elevated); }
  .chev { flex: none; position: relative; width: 16px; height: 16px; }
  .chev::before, .chev::after {
    content: "";
    position: absolute;
    background: var(--accent);
    border-radius: 2px;
    transition: transform 0.25s ease, opacity 0.25s ease;
  }
  .chev::before { top: 7px; left: 0; width: 16px; height: 2px; }
  .chev::after { top: 0; left: 7px; width: 2px; height: 16px; }
  .faq-item.open .chev::after { transform: scaleY(0); opacity: 0; }
  .faq-a { padding: 0 20px; }
  .faq-a p { color: var(--text-muted); font-size: 14.5px; padding: 0 0 20px; max-width: 64ch; }

  .footer {
    max-width: var(--maxw);
    margin: 0 auto;
    padding: 40px clamp(16px, 5vw, 48px) 56px;
    border-top: 1px solid var(--border-soft);
    text-align: center;
    color: var(--text-dim);
    font-size: 13px;
  }
  .foot-top { display: flex; flex-direction: column; align-items: center; gap: 6px; margin-bottom: 18px; }
  .footer .brand-name { font-size: 18px; }
  .foot-tag { color: var(--text-muted); font-size: 14px; }
  .foot-links { display: flex; flex-wrap: wrap; justify-content: center; gap: 18px; margin-bottom: 20px; color: var(--text-muted); font-weight: 600; }
  .foot-links a:hover { color: var(--accent); }
  .footer .ss { max-width: 640px; margin: 0 auto 12px; line-height: 1.5; }
  .made { color: var(--text-dim); }

  @media (max-width: 640px) {
    .top-nav { display: none; }
    .langs { margin-left: auto; }
  }
</style>
