<script>
  import BlackHoleHero from "$lib/BlackHoleHero.svelte";
  import BinaryRainHero from "$lib/BinaryRainHero.svelte";
  import { t, getLang, setLang, LANGS, LANG_LABEL } from "$lib/i18n.svelte.js";
  import { base } from "$app/paths";
  import { slide } from "svelte/transition";
  import { onMount } from "svelte";
  import { SITE_URL } from "$lib/seo.js";

  // Kullanıcı gözlemi: "en başta çok hafif aşağıya kayıp başlıyor" — tarayıcının
  // kendi scroll-restoration'ı (reload'da önceki konuma dönme) olası sebep.
  // Manuel'e alıp sayfayı her zaman en tepede başlatıyoruz.
  onMount(() => {
    if ("scrollRestoration" in history) history.scrollRestoration = "manual";
    window.scrollTo(0, 0);
  });

  const BRAND = "evorift";
  const REPO = "https://github.com/evorift/rift";
  const RELEASES = REPO + "/releases";
  const SPONSOR = "https://github.com/sponsors/evorift";
  // Kullanıcı kararı: indirme butonu ikiye bölünür — sol taraf doğrudan .exe'ye
  // gider (tıklar tıklamaz iner), sağ taraf sürüm/hash/imza bilgisinin olduğu
  // Releases sayfasına. Sürüm numarası burada elle yazılı — build-031/tauri.conf.json
  // ile senkron tutulmalı, yeni sürümde bu satır güncellenmeden unutulmasın.
  const DIRECT_EXE = RELEASES + "/download/v0.3.1/evorift_0.3.1_x64-setup.exe";

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
    { k: "vpn",    ico: "⤳", warp: true, wip: true },
  ];

  const measured = [1, 2, 3];
  const faqs = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10];

  // Google'ın rich-result'ları render edip etmemesi opsiyonel; ama yapılandırılmış
  // veri arama motoruna sayfanın ne olduğunu (yazılım, ücretsiz, Windows) net
  // anlatır. Yalnız görünür sayfa metniyle örtüşen alanlar (skill §1 — ölçülmemiş
  // hiçbir şey yazılmaz).
  const jsonLd = {
    "@context": "https://schema.org",
    "@type": "SoftwareApplication",
    name: BRAND,
    url: SITE_URL + "/",
    operatingSystem: "Windows 10, Windows 11",
    applicationCategory: "UtilitiesApplication",
    description:
      "Operatör kaynaklı bağlantı bozulmalarını gideren Windows aracı. Mesajlaşma, sesli görüşme ve oyunlar yeniden açılır. Sunucu yok, trafik evorift'ten geçmez.",
    downloadUrl: RELEASES,
    softwareVersion: "0.3.1",
    offers: { "@type": "Offer", price: "0", priceCurrency: "USD" },
  };
</script>

<svelte:head>
  <title>{BRAND} — {t("meta.title")}</title>
  <meta name="description" content={t("hero.sub")} />
  <link rel="canonical" href={SITE_URL + "/"} />
  {@html `<script type="application/ld+json">${JSON.stringify(jsonLd)}<\/script>`}
  <!-- bh-settled class'ı yalnız JS animate() döngüsünden gelir. JS kapalıysa hiç
       eklenmez ve CTA/açıklama kalıcı görünmez kalırdı — içerik animasyona bağlı
       olmamalı (skill §6). Bu, tek gerçek "JS yoksa da çalışır" güvencesi. -->
  <noscript>
    <style>
      .hero-sub, .cta, .platform { opacity: 1 !important; pointer-events: auto !important; }
    </style>
  </noscript>
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
    <BinaryRainHero />

    <!-- Kullanıcı kararı: "evorift", başlık ve rozet AYRI konumlanmasın — hepsi
         tek hero-copy grubunda, birlikte hareket etsin. Önceki sürüm wordmark'ı
         ayrı bir katmanda tam merkeze sabitliyordu; grup dağınık/kopuk görünüyordu.
         Rozet (chip) kullanıcı kararıyla kaldırıldı. -->
    <div class="hero-copy">
      <div class="wordmark">{BRAND}</div>
      <h1 class="hero-title">{t("hero.title")}</h1>
      <p class="hero-sub">{t("hero.sub")}</p>

      <div class="cta" id="get">
        <div class="split-btn">
          <a class="btn primary split-main" href={DIRECT_EXE} download target="_blank" rel="noopener">
            ⬇ {t("cta.download")}
          </a>
          <a class="btn primary split-side" href={RELEASES} target="_blank" rel="noopener" aria-label={t("cta.releases")}>
            {t("cta.releases")}
          </a>
        </div>
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

  /* Kaydırma sahnesi. Kullanıcı kararı: 220vh'de yerine oturduktan (P_SHRINK)
     sonra "What it does"a kadar ~78vh tamamen boş/donmuş kayıyordu — hissedilir
     bir "boşluk" bırakıyordu. 150vh'e indirildi: aynı P_SHRINK oranı (0.35) artık
     çok daha kısa bir donmuş kuyruk bırakıyor (~33vh yerine ~78vh). */
  .hero { position: relative; height: 150vh; }
  .hero-sticky {
    position: sticky;
    top: 0;
    height: 100vh;
    overflow: hidden;
    /* mix-blend-mode aşağıda yalnız kendi canvas'larına karşı hesaplansın diye —
       yoksa blend sayfanın geri kalanına da sızar. */
    isolation: isolate;
  }
  .bh-stage { position: absolute; inset: 0; }
  /* Kullanıcı kararı (basitleştirme, son tur): önceki sürümler metni deliğin
     merkeziyle hizalamaya ve --p'ye göre delikle senkron kaydırmaya çalışıyordu
     — bu çok sayıda hataya yol açtı (stacking context, containing block, yanlış
     ölçülmüş viewport, vs.) VE kullanıcı ARTIK bunu istemiyor. Net istek: metin
     grubu deliğin ALTINDA dursun, SABİT — kaydırmayla hiç hareket etmesi
     gerekmiyor ("en baştaki konumunda kalabilir"). top:62% gözle seçilmiş bir
     sabit — deliğin dinlenme boyutundaki (ölçülmüş, bkz. BlackHoleHero.svelte
     GAP_VH/settledExtraDownPx) tipik alanının altına düşecek şekilde. */
  .hero-copy {
    position: absolute;
    /* ÖLÇÜLDÜ ve ÇELİŞKİ BULUNDU: içerik yüksekliği (başlıktan platform'a)
       ~364px; 800px'lik sticky alana kırpılmadan sığması için top en fazla
       ~%54 olabilir. Ama delik başlangıç (en büyük) halinde görsel olarak
       ~816px'e kadar iniyor — "tamamen deliğin altında" olmak için top en az
       %90+ olması gerekirdi, ki bu da içeriği .hero-sticky'nin overflow:hidden
       sınırının altına iter (kırpılır — daha önce düzeltilen hatayı geri
       getirir). İkisi birden sağlanamıyor. %48 seçildi: kırpma YOK (doğrulandı),
       ama delik en büyük halindeyken üstteki metinle (evorift/başlık) kısmi
       çakışma kalıyor — deliğin kendi küçülme animasyonuyla hızla açılıyor. */
    top: 48%;
    left: 50%;
    transform: translateX(-50%);
    /* z-index KASITLI OLARAK yok (auto kalsın). Bir sayı vermek position:absolute
       ile birleşince kendi stacking context'ini oluşturur — o zaman içindeki
       mix-blend-mode:difference (hero-sub/platform için hâlâ kullanılıyor) kendi
       (boş) mikro-bağlamına karşı hesaplanır, delik/yağmura karşı DEĞİL. */
    width: 100%;
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
  /* Kullanıcı kararı: metin kara deliğin beyaz kısmına gelince siyah, siyah
     kısmına gelince beyaz olsun. mix-blend-mode:difference BURADA sadece bir
     GÜVENLİ VARSAYILAN / no-JS yedeği — WebGL canvas'a karşı güvenilmez çalıştığı
     kanıtlandı (tarayıcı canvas'ı ayrı katmana alıyor, blend göremiyor; metin
     beyaz halkanın üzerinde beyaz kalıp kayboluyordu). Gerçek çözüm artık
     BlackHoleHero.svelte'deki 2D canvas overlay: deliğin karesini kopyalayıp
     üstüne "difference" modunda metni ÇİZİYOR, piksel piksel garantili. O sistem
     hazır olunca aşağıdaki kural DOM metnini görünmez yapıp yerini canvas'a
     bırakır; JS/WebGL hiç çalışmazsa metin burada beyaz kalır (içerik
     animasyona bağlı olmamalı — skill §6). */
  .wordmark,
  .hero-title,
  .hero-sub,
  .platform {
    color: #fff;
    mix-blend-mode: difference;
  }
  /* Yalnız wordmark ve hero-title deliğe değiyor; canvas overlay hazır olunca
     bunların DOM rengi görünmez olur — çizimi artık overlay yapıyor, ikisi
     üst üste binmesin. hero-sub/platform CSS blend'de kalıyor (deliğe değmiyor). */
  :root.text-overlay-ready .wordmark,
  :root.text-overlay-ready .hero-title {
    color: transparent;
  }
  .wordmark {
    font-size: clamp(32px, 7vw, 56px);
    font-weight: 800;
    letter-spacing: -0.01em;
    line-height: 1;
    margin: 0 0 18px; /* hero-copy'nin flex akışına geri döndü, rozetten boşluk gerekiyor */
  }
  .hero-title {
    font-size: clamp(34px, 6vw, 60px);
    line-height: 1.14;
    font-weight: 800;
    letter-spacing: -0.02em;
    /* Eskiden rozetten (chip) boşluk için 16px üst margin taşıyordu; rozet
       kaldırıldı, artık doğrudan wordmark'ı takip ediyor — wordmark'ın kendi
       alt margin'i (18px) yeterli, ikisini toplayıp fazla boşluk bırakmayalım. */
    margin: 0;
    padding-bottom: 0.12em;
  }
  .hero-sub { max-width: 600px; margin-top: 16px; font-size: clamp(15px, 2.2vw, 18px); }
  .cta { display: flex; flex-wrap: wrap; gap: 12px; justify-content: center; margin-top: 30px; scroll-margin-top: 120px; }

  /* İndirme butonu ikiye bölünmüş: sol .exe'yi doğrudan indirir, sağ Releases
     sayfasına (hash/imza bilgisi için) götürür. Tek buton gibi bitişik dururlar. */
  .split-btn { display: flex; }
  .split-main {
    border-top-right-radius: 0;
    border-bottom-right-radius: 0;
  }
  .split-side {
    border-top-left-radius: 0;
    border-bottom-left-radius: 0;
    border-left: 1px solid rgba(4, 20, 10, 0.35);
    padding-left: 14px;
    padding-right: 14px;
    font-size: 13px;
  }

  .btn.sponsor { color: var(--accent); }
  .btn.sponsor:hover { border-color: var(--accent); color: var(--accent); }
  .platform { margin-top: 14px; font-size: 12px; letter-spacing: 0.05em; }

  /* Kullanıcı kararı (2026-08-18): ilk ekranda yalnız wordmark + rozet + başlık.
     Geri kalanı (açıklama, CTA, platform satırı) delik yerine oturunca (bkz.
     BlackHoleHero.svelte'deki bh-settled class'ı) belirir — indirmeye ulaşmak
     için kasıtlı olarak biraz daha kaydırma gerektirir. */
  .hero-sub,
  .cta,
  .platform {
    opacity: 0;
    pointer-events: none;
    transition: opacity 0.5s ease;
  }
  :root.bh-settled .hero-sub { opacity: 0.85; }
  :root.bh-settled .platform { opacity: 0.55; }
  :root.bh-settled .hero-sub,
  :root.bh-settled .cta,
  :root.bh-settled .platform {
    pointer-events: auto;
  }
  :root.bh-settled .cta { opacity: 1; }

  .sec-title { text-align: center; font-size: clamp(24px, 4vw, 34px); font-weight: 800; letter-spacing: -0.01em; margin-bottom: 32px; }

  .features { padding: 40px 0; }
  /* 4 kart: auto-fit wrap ile bazı genişliklerde 3+1'e bölünüp simetriyi bozuyordu.
     Sabit yan yana + taşarsa yatay kaydırma (mobilde beklenen, skill kapsamında). */
  .grid {
    display: flex;
    flex-wrap: nowrap;
    overflow-x: auto;
    gap: 16px;
    scroll-snap-type: x proximity;
    padding-bottom: 4px;
  }
  .card {
    flex: 1 1 0;
    min-width: 220px;
    scroll-snap-align: start;
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
  .tech-grid {
    display: flex;
    flex-wrap: nowrap;
    overflow-x: auto;
    gap: 16px;
    scroll-snap-type: x proximity;
    padding-bottom: 4px;
  }
  .tech-card {
    flex: 1 1 0;
    min-width: 220px;
    scroll-snap-align: start;
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

  /* Mobilde sticky-pinned kaydırma sahnesinin anlamı yok (dar ekranda 220vh'lik
     ölü kaydırma kötü UX) — hero normal akışa döner, ama animate() yine çalışır.
     Reduced-motion'da ise animate() hiç çalışmaz (bkz. BlackHoleHero.svelte). */
  @media (max-width: 767px), (prefers-reduced-motion: reduce) {
    .hero { height: auto; }
    /* static DEĞİL relative: wordmark-anchor/hero-copy position:absolute ile buna
       göre konumlanıyor — static containing block oluşturmaz, çocuklar tüm sayfaya
       göre konumlanıp koparlardı. relative sticky-pinning'i aynı şekilde iptal eder. */
    .hero-sticky { position: relative; height: auto; min-height: 100svh; padding: 40px 0 56px; }
    .scroll-hint { display: none; }
    /* Masaüstündeki "wordmark ekranın tam ortasında, delikle çakışık" kurgusu
       sticky-pinned bağlama özgü. Burada hero-sticky height:auto olduğu için
       position:absolute çocuklar normal akışa katkı vermez — .hero-copy taşarsa
       overflow:hidden onu kırpardı. İkisini de normal akışa döndürüyoruz. */
    .wordmark-anchor, .hero-copy {
      position: static;
      transform: none;
      width: auto;
    }
    .wordmark-anchor { margin-bottom: 18px; }
  }

  /* Reduced-motion'da animate() hiç çalışmadığı için bh-settled class'ı asla
     eklenmez — içerik animasyona bağlı olmamalı kuralı (skill §6) gereği CTA/
     açıklama burada zorla görünür kalır. */
  @media (prefers-reduced-motion: reduce) {
    .hero-sub, .cta, .platform { opacity: 1; pointer-events: auto; }
  }
</style>
