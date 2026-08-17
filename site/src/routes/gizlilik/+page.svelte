<script>
  import Shell from "$lib/Shell.svelte";
  import { getLang } from "$lib/i18n.svelte.js";

  const L = {
    tr: {
      title: "Gizlilik",
      lead: "Kısa sürüm: hiçbir şey toplamıyoruz, çünkü toplayacak bir yerimiz yok.",
      noServerT: "Sunucumuz yok",
      noServerD:
        "evorift senin bilgisayarında çalışan yerel bir programdır. Bizim işlettiğimiz bir sunucu, relay ya da hesap sistemi yoktur. Trafiğin bize uğramaz — teknik olarak toplayamayız, sadece söz vermiş olmuyoruz.",
      collectT: "Toplanmayanlar",
      collectList: [
        "Hesap, e-posta, telefon — kayıt yok.",
        "Gezdiğin siteler, ziyaret geçmişi, alan adları.",
        "IP adresin.",
        "Kullanım istatistiği, çökme raporu, analitik.",
        "Reklam kimliği, çerez, üçüncü taraf takip kodu.",
      ],
      localT: "Bilgisayarında ne duruyor",
      localList: [
        "Ayarların: seçtiğin mod, uygulama listesi, tercihler.",
        "Yerel günlükler: neyin çalışıp neyin çalışmadığını görebilmen için.",
      ],
      localD:
        "Bunlar senin diskinde durur ve hiçbir yere gönderilmez. Günlüklere alan adı yazılmaz — istek atılan site adları maskelenir, ve bunu doğrulayan bir test var.",
      wipeT: "Nasıl silinir",
      wipeD:
        "Uygulama içindeki 'Tüm verileri sil' hem ayarları hem günlükleri kaldırır ve yapılan sistem değişikliklerini geri alır. Sonrasında Windows'un Uygulamalar listesinden normal şekilde kaldırabilirsin; geride sürücü veya servis kalmaz.",
      siteT: "Bu site",
      siteD:
        "Bu sayfada analitik, çerez, harici yazı tipi, reklam ya da sohbet aracı yok. Hiçbir üçüncü taraf script'i yüklenmiyor. Ziyaretini kimse saymıyor.",
    },
    en: {
      title: "Privacy",
      lead: "Short version: we collect nothing, because we have nowhere to collect it to.",
      noServerT: "We run no servers",
      noServerD:
        "evorift is local software that runs on your machine. There is no server, relay or account system that we operate. Your traffic never reaches us — we technically cannot collect it, rather than merely promising not to.",
      collectT: "Not collected",
      collectList: [
        "Account, email, phone — there is no sign-up.",
        "The sites you visit, browsing history, domain names.",
        "Your IP address.",
        "Usage statistics, crash reports, analytics.",
        "Advertising IDs, cookies, third-party tracking code.",
      ],
      localT: "What stays on your machine",
      localList: [
        "Your settings: the mode you picked, your app list, preferences.",
        "Local logs, so you can see what worked and what didn't.",
      ],
      localD:
        "These live on your own disk and are never sent anywhere. Domain names are not written to the logs — requested host names are masked, and there is a test that proves it.",
      wipeT: "How to delete it",
      wipeD:
        "'Tüm verileri sil' inside the app removes both settings and logs and reverts the system changes it made. After that you can uninstall normally from the Windows apps list; no driver or service is left behind.",
      siteT: "This website",
      siteD:
        "This page has no analytics, cookies, external fonts, ads or chat widget. No third-party script is loaded. Nobody is counting your visit.",
    },
  };

  const c = $derived(L[getLang()] ?? L.en);
</script>

<Shell title={c.title} lead={c.lead}>
  <section class="block">
    <h2>{c.noServerT}</h2>
    <p>{c.noServerD}</p>
  </section>

  <section class="block">
    <h2>{c.collectT}</h2>
    <ul class="list no">
      {#each c.collectList as i (i)}<li>{i}</li>{/each}
    </ul>
  </section>

  <section class="block">
    <h2>{c.localT}</h2>
    <ul class="list">
      {#each c.localList as i (i)}<li>{i}</li>{/each}
    </ul>
    <p>{c.localD}</p>
  </section>

  <section class="block">
    <h2>{c.wipeT}</h2>
    <p>{c.wipeD}</p>
  </section>

  <section class="block">
    <h2>{c.siteT}</h2>
    <p>{c.siteD}</p>
  </section>
</Shell>

<style>
  .block { margin-top: 36px; }
  .block h2 { font-size: 20px; font-weight: 800; letter-spacing: -0.01em; margin-bottom: 10px; }
  .block p { color: var(--text-muted); line-height: 1.65; margin-bottom: 10px; }
  .list { margin: 0 0 10px; padding-left: 20px; color: var(--text-muted); line-height: 1.8; }
  .list.no li::marker { color: var(--accent); }
</style>
