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

  const features = [
    { k: "unblock", ico: "◎" },
    { k: "smart", ico: "⚡" },
    { k: "safe", ico: "🛡" },
    { k: "light", ico: "❉" },
  ];

  // Uygulamadaki mod adlarıyla birebir aynı (skill §5). Mor yalnız VPN kartında
  // görünür; Otomatik henüz çalışmadığı için dürüst bir durum notu taşır.
  const modes = [
    { k: "light",  ico: "◔" },
    { k: "strong", ico: "◉" },
    { k: "auto",   ico: "◌", wip: true },
    { k: "vpn",    ico: "⤳", warp: true },
  ];

  const measured = [1, 2, 3];
  const faqs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];
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
    <a href="#measured">{t("nav.measured")}</a>
    <a href="#faq">{t("nav.faq")}</a>
    <a href="{base}/indir">{t("nav.download")}</a>
  </nav>

  <div class="langs" role="group" aria-label="language">
    {#each LANGS as l (l)}
      <button class="lang" class:active={getLang() === l} onclick={() => setLang(l)}>{LANG_LABEL[l]}</button>
    {/each}
  </div>
</header>

<section class="hero" id="top">
  <div class="hero-sticky">
    <div class="bh-stage"><BlackHoleHero /></div>

    <div class="hero-copy">
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
    </div>

    <span class="scroll-hint" aria-hidden="true"></span>
  </div>
</section>

<main>
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

    <h3 class="tech-title">{t("how.modes.title")}</h3>
    <p class="tech-lead">{t("how.modes.lead")}</p>
    <div class="tech-grid">
      {#each modes as m (m.k)}
        <article class="tech-card" class:warp={m.warp}>
          <span class="t-ico" aria-hidden="true">{m.ico}</span>
          <h4>
            {t(`mode.${m.k}.t`)}
            {#if m.wip}<span class="wip mono">{t("mode.wip")}</span>{/if}
          </h4>
          <p>{t(`mode.${m.k}.d`)}</p>
        </article>
      {/each}
    </div>
  </section>

  <!-- Ölçülen. Sitenin signature'ı: her rakam kendi kaydını yanında taşır. -->
  <section class="measured" id="measured">
    <h2 class="sec-title">{t("meas.title")}</h2>
    <p class="meas-lead">{t("meas.lead")}</p>
    <div class="meas-grid">
      {#each measured as i (i)}
        <article class="meas-card">
          <span class="meas-v mono">{t(`meas.${i}.v`)}</span>
          <span class="meas-l">{t(`meas.${i}.l`)}</span>
          <span class="meas-p mono">{t(`meas.${i}.p`)}</span>
        </article>
      {/each}
    </div>
    <p class="meas-caveat">{t("meas.caveat")}</p>
  </section>

  <section class="shots" id="shots">
    <div class="shot-grid">
      {#each [1, 2, 3] as i (i)}
        <figure class="shot">
          <div class="shot-ph mono" aria-hidden="true">{t("shot.ph")} {i}</div>
          <figcaption>{t(`shot.${i}`)}</figcaption>
        </figure>
      {/each}
    </div>
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
    <a href="{base}/indir">{t("nav.download")}</a>
    <a href="#faq">{t("nav.faq")}</a>
    <a href="{base}/gizlilik">{t("nav.privacy")}</a>
    <a href="{base}/destek">{t("nav.support")}</a>
    <a href={REPO} target="_blank" rel="noopener">GitHub</a>
  </nav>
  <p class="ss">{t("foot.smartscreen")}</p>
  <p class="ss">
    {t("foot.official")}
    <a href={RELEASES} target="_blank" rel="noopener">github.com/evorift/rift/releases</a>.
    {t("foot.official.warn")}
  </p>
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

  /* Kaydırma sahnesi: 220vh boyunca sticky kalır, delik shader içinde küçülür.
     DOM tarafında layout işi yok — tek kaydırma bağımlı şey --p ile opacity. */
  .hero { position: relative; height: 220vh; }
  .hero-sticky {
    position: sticky;
    top: 0;
    height: 100vh;
    display: grid;
    place-items: center;
    overflow: hidden;
  }
  .bh-stage { position: absolute; inset: 0; }
  .hero-copy {
    position: relative;
    z-index: 1;
    display: flex;
    flex-direction: column;
    align-items: center;
    text-align: center;
    max-width: 760px;
    padding: 0 clamp(16px, 5vw, 48px);
  }
  .scroll-hint {
    position: absolute;
    bottom: 28px;
    left: 50%;
    width: 20px;
    height: 20px;
    margin-left: -10px;
    border-right: 2px solid var(--accent-dim);
    border-bottom: 2px solid var(--accent-dim);
    transform: rotate(45deg);
    opacity: calc(1 - var(--p) * 3);
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

  .tech-title { text-align: center; font-size: clamp(18px, 2.6vw, 22px); font-weight: 800; letter-spacing: -0.01em; margin: 44px 0 6px; }
  .tech-lead { text-align: center; max-width: 620px; margin: 0 auto 26px; color: var(--text-muted); font-size: 14.5px; }
  .tech-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(240px, 1fr)); gap: 16px; }
  .tech-card {
    background: var(--bg-surface);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius);
    padding: 22px;
    transition: border-color 0.2s ease;
  }
  .tech-card:hover { border-color: var(--accent-dim); }
  .t-ico { display: inline-flex; font-size: 20px; color: var(--accent); margin-bottom: 10px; filter: drop-shadow(0 0 10px var(--accent-glow)); }
  .tech-card h4 { font-size: 16px; margin-bottom: 8px; }
  .tech-card p { color: var(--text-muted); font-size: 14px; line-height: 1.55; }

  /* Mor YALNIZ burada — WARP/VPN bağlamı dışında sitenin hiçbir yerinde yok (skill §4). */
  .tech-card.warp { --warp: #a78bfa; border-color: color-mix(in srgb, var(--warp) 26%, transparent); }
  .tech-card.warp .t-ico { color: var(--warp); filter: drop-shadow(0 0 10px color-mix(in srgb, var(--warp) 40%, transparent)); }
  .tech-card.warp:hover { border-color: var(--warp); }
  .wip {
    margin-left: 8px;
    padding: 2px 8px;
    border: 1px solid var(--border);
    border-radius: 999px;
    color: var(--text-dim);
    font-size: 11px;
    font-weight: 600;
    vertical-align: middle;
  }

  /* Ölçülen — sitenin signature'ı: her rakam kendi kaydını yanında taşır. */
  .measured { padding: 40px 0; }
  .meas-lead { max-width: 640px; margin: -14px auto 26px; text-align: center; color: var(--text-muted); font-size: 14.5px; }
  .meas-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(220px, 1fr)); gap: 16px; }
  .meas-card {
    display: flex;
    flex-direction: column;
    gap: 6px;
    background: var(--bg-surface);
    border: 1px solid var(--border-soft);
    border-left: 2px solid var(--accent-dim);
    border-radius: var(--radius);
    padding: 22px;
  }
  .meas-v { font-size: 30px; font-weight: 700; letter-spacing: -0.02em; line-height: 1.1; color: var(--accent); }
  .meas-l { font-size: 14.5px; }
  .meas-p { color: var(--text-dim); font-size: 11.5px; letter-spacing: 0.02em; }
  .meas-caveat {
    max-width: 720px;
    margin: 22px auto 0;
    padding-top: 18px;
    border-top: 1px solid var(--border-soft);
    text-align: center;
    color: var(--text-muted);
    font-size: 13.5px;
    line-height: 1.6;
  }

  /* Ekran görüntüsü yerleri — gerçek görseller gelince .shot-ph kalkar. */
  .shots { padding: 8px 0 40px; }
  .shot-grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(260px, 1fr)); gap: 16px; }
  .shot { margin: 0; }
  .shot-ph {
    display: grid;
    place-items: center;
    aspect-ratio: 1000 / 680;
    border: 1px dashed var(--border);
    border-radius: var(--radius);
    background: var(--bg-surface);
    color: var(--text-dim);
    font-size: 12px;
    letter-spacing: 0.04em;
  }
  .shot figcaption { margin-top: 10px; color: var(--text-muted); font-size: 13.5px; }

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

  /* Mobilde ve reduced-motion'da sahne statik çizilir. Sabit duran bir şeyi
     220vh boyunca kaydırtmanın anlamı yok — hero normal akışa döner. */
  @media (max-width: 767px) {
    .hero { height: auto; }
    .hero-sticky { position: static; height: auto; min-height: 100svh; padding: 40px 0 56px; }
    .scroll-hint { display: none; }
  }
  @media (prefers-reduced-motion: reduce) {
    .hero { height: auto; }
    .hero-sticky { position: static; height: auto; min-height: 100svh; padding: 40px 0 56px; }
    .scroll-hint { display: none; }
  }
</style>
