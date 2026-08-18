<script>
  // Skill §1'in "motor adları sitede hiç geçmez" kuralına TEK, bilinçli istisna:
  // kullanıcı kararı — bu sayfa "goodbyedpi" araması için var, isim burada açıkça
  // geçer. Sitenin geri kalanı (ana sayfa/SSS/gizlilik) kuralca temiz kalır.
  //
  // ÖNEMLİ doğrulama: evorift'in kod tabanında GoodbyeDPI'yi motor seçeneği olarak
  // destekleme planı var ama şu an kullanıcıya KAPALI ve fonksiyonel değil
  // (bkz. CHANGELOG: "GoodbyeDPI/ByeDPI controls... hidden instead of
  // shown-but-non-functional"). Bu yüzden bu sayfa evorift'i GoodbyeDPI'yi
  // "içeren" değil, aynı problemi çözen BAĞIMSIZ bir araç olarak anlatır —
  // skill §8 (uygulamada olmayan özellik gösterilmez) ve §1 (ölçülmemiş
  // iddia yok) burada da geçerli.
  import Shell from "$lib/Shell.svelte";
  import { getLang } from "$lib/i18n.svelte.js";
  import { base } from "$app/paths";

  const REPO = "https://github.com/evorift/rift";
  const RELEASES = REPO + "/releases";

  const L = {
    tr: {
      title: "GoodbyeDPI Alternatifi",
      lead: "GoodbyeDPI kullanıyorsan ya da onu araştırıyorsan: aynı problemi (operatör kaynaklı bağlantı bozulması) farklı bir yoldan çözen bir seçenek daha var. Karşılaştırma dürüst — hangisinin sana uyduğuna sen karar ver.",
      whatT: "GoodbyeDPI nedir",
      whatD:
        "GoodbyeDPI, Windows için geliştirilmiş, komut satırından çalışan açık kaynaklı bir bağlantı düzeltme aracı. Farklı senaryolar için hazır komut dosyaları (.cmd) sunar; hangisinin kendi hattında işe yaradığını deneyerek bulursun. Sürekli çalışması için görev zamanlayıcı ya da servis kurulumunu kendin yaparsın.",
      diffT: "evorift ile fark ne",
      diffList: [
        { t: "Kurulum", d: "GoodbyeDPI: zip indir, doğru komut dosyasını seç, çalıştır, kalıcılık için ek adım gerekir. evorift: kurulum sihirbazı, bittiğinde arka planda otomatik çalışır." },
        { t: "Arayüz", d: "GoodbyeDPI komut satırından yönetilir, grafik arayüzü yok. evorift'te aç/kapat anahtarı ve hazır modlar var." },
        { t: "Kapsam", d: "GoodbyeDPI varsayılan olarak sistem genelindeki 80/443 trafiğine dokunur. evorift yalnızca seçtiğin uygulama/alan adı listesine dokunur, geri kalan trafik doğrudan akar." },
        { t: "Oyun / anti-cheat", d: "evorift, popüler anti-cheat sistemlerini (Vanguard / EAC / BattlEye) tanır ve korumalı bir oyun açıldığında korumayı otomatik duraklatabilir." },
        { t: "Kaldırma", d: "evorift içindeki 'Tüm verileri sil', yaptığı bütün sistem değişikliğini geri alır; ardından Windows'un Uygulamalar listesinden normal şekilde kaldırılır." },
      ],
      conflictT: "İkisini aynı anda çalıştırma",
      conflictD:
        "GoodbyeDPI ve evorift, ikisi de ağ paketlerini WinDivert sürücüsü üzerinden yakalayıp değiştiriyor. Aynı anda ikisini birden çalıştırmak çakışmaya yol açabilir. GoodbyeDPI kuruluysa, evorift'i denemeden önce onu durdurman (görev zamanlayıcısındaki veya servisteki kaydını kapatman) önerilir.",
      faqT: "Sık sorulanlar",
      faqs: [
        {
          q: "evorift, GoodbyeDPI'nin yerine mi geçiyor?",
          a: "Zorunlu değil. İkisi de aynı problem sınıfını çözüyor, farklı şekillerde. Komut satırını ve ince ayarı seviyorsan GoodbyeDPI iyi bir seçim olmaya devam eder. Kurulumdan hemen sonra hazır bir moddan başlamak istiyorsan evorift'i deneyebilirsin.",
        },
        {
          q: "evorift, GoodbyeDPI ile ilişkili mi ya da onun kodunu mu kullanıyor?",
          a: "Hayır. evorift bağımsız bir projedir; GoodbyeDPI'nin bir türevi, resmi alternatifi ya da onunla iş birliği içinde geliştirilen bir araç değildir. Bu sayfa yalnızca aynı problemi çözen iki farklı aracı karşılaştırmak için var.",
        },
        {
          q: "Hangisini seçmeliyim?",
          a: "Komut satırına elini sokmaktan çekinmiyorsan ve parametre ince ayarı önemliyse: GoodbyeDPI. Kurulumu bitirir bitirmez arayüzden yönetmek istiyorsan: evorift.",
        },
      ],
      cta: "evorift'i indir",
      ctaBack: "Ana sayfaya dön",
      metaDesc:
        "GoodbyeDPI'ye dürüst bir karşılaştırma: kurulum sihirbazı, hazır modlar, tek tık aç/kapat. evorift ile GoodbyeDPI arasındaki farkları gör.",
    },
    en: {
      title: "GoodbyeDPI Alternative",
      lead: "If you use GoodbyeDPI, or you're researching it: there's another option that solves the same problem (ISP-level connection breakage) a different way. This comparison is honest — decide for yourself which fits.",
      whatT: "What GoodbyeDPI is",
      whatD:
        "GoodbyeDPI is an open-source, command-line connection-fixing tool for Windows. It ships ready-made scripts (.cmd) for different scenarios; you find the one that works on your line by trying them. Keeping it running persistently (task scheduler or a service) is up to you to set up.",
      diffT: "How evorift differs",
      diffList: [
        { t: "Setup", d: "GoodbyeDPI: download a zip, pick the right script, run it, set up persistence yourself. evorift: an installer wizard, then it runs automatically in the background." },
        { t: "Interface", d: "GoodbyeDPI is managed from the command line, no GUI. evorift has an on/off switch and ready-made modes." },
        { t: "Scope", d: "GoodbyeDPI touches system-wide port 80/443 traffic by default. evorift only touches the app/domain list you pick — everything else flows unchanged." },
        { t: "Games / anti-cheat", d: "evorift is aware of popular anti-cheat systems (Vanguard / EAC / BattlEye) and can auto-pause protection when a protected game launches." },
        { t: "Uninstalling", d: "evorift's 'Delete all data' reverts every system change it made; after that it's removed normally from the Windows apps list." },
      ],
      conflictT: "Running both at once",
      conflictD:
        "GoodbyeDPI and evorift both capture and rewrite network packets through the WinDivert driver. Running both at the same time can conflict. If GoodbyeDPI is installed, stop it (its task-scheduler entry or service) before trying evorift.",
      faqT: "FAQ",
      faqs: [
        {
          q: "Does evorift replace GoodbyeDPI?",
          a: "Not necessarily. Both solve the same class of problem, differently. If you like the command line and fine-tuning parameters, GoodbyeDPI remains a good choice. If you want to start from a ready-made mode right after install, try evorift.",
        },
        {
          q: "Is evorift affiliated with GoodbyeDPI, or does it use its code?",
          a: "No. evorift is an independent project; it is not a fork, official alternative, or collaboration with GoodbyeDPI. This page exists only to compare two different tools that solve the same problem.",
        },
        {
          q: "Which one should I pick?",
          a: "If you don't mind the command line and parameter tuning matters to you: GoodbyeDPI. If you want to manage it from a GUI right after setup: evorift.",
        },
      ],
      cta: "Download evorift",
      ctaBack: "Back to homepage",
      metaDesc:
        "An honest comparison with GoodbyeDPI: installer wizard, ready-made modes, one-click on/off. See how evorift differs from GoodbyeDPI.",
    },
  };

  const c = $derived(L[getLang()] ?? L.en);

  const faqJsonLd = $derived({
    "@context": "https://schema.org",
    "@type": "FAQPage",
    mainEntity: c.faqs.map((f) => ({
      "@type": "Question",
      name: f.q,
      acceptedAnswer: { "@type": "Answer", text: f.a },
    })),
  });
</script>

<svelte:head>
  {@html `<script type="application/ld+json">${JSON.stringify(faqJsonLd)}<\/script>`}
</svelte:head>

<Shell title={c.title} lead={c.lead} description={c.metaDesc} path="/goodbyedpi-alternatifi">
  <section class="block">
    <h2>{c.whatT}</h2>
    <p>{c.whatD}</p>
  </section>

  <section class="block">
    <h2>{c.diffT}</h2>
    <dl class="diff">
      {#each c.diffList as item (item.t)}
        <div class="diff-row">
          <dt>{item.t}</dt>
          <dd>{item.d}</dd>
        </div>
      {/each}
    </dl>
  </section>

  <section class="block warn">
    <h2>{c.conflictT}</h2>
    <p>{c.conflictD}</p>
  </section>

  <section class="block get-block">
    <a class="btn primary" href={RELEASES} target="_blank" rel="noopener">⬇ {c.cta}</a>
    <a class="btn" href="{base}/">{c.ctaBack}</a>
  </section>

  <section class="block">
    <h2>{c.faqT}</h2>
    <dl class="faq">
      {#each c.faqs as f (f.q)}
        <div class="faq-row">
          <dt>{f.q}</dt>
          <dd>{f.a}</dd>
        </div>
      {/each}
    </dl>
  </section>
</Shell>

<style>
  .block { margin-top: 40px; }
  .block h2 { font-size: 20px; font-weight: 800; letter-spacing: -0.01em; margin-bottom: 10px; }
  .block p { color: var(--text-muted); line-height: 1.65; margin-bottom: 10px; }

  .diff { display: flex; flex-direction: column; gap: 18px; margin: 0; }
  .diff-row dt { font-weight: 700; margin-bottom: 4px; }
  .diff-row dd { margin: 0; color: var(--text-muted); line-height: 1.6; }

  .warn {
    border: 1px solid var(--border);
    border-left: 2px solid var(--accent-dim);
    border-radius: var(--radius);
    padding: 22px;
    background: var(--bg-surface);
  }

  .get-block { display: flex; gap: 12px; flex-wrap: wrap; }

  .faq { display: flex; flex-direction: column; gap: 18px; margin: 0; }
  .faq-row dt { font-weight: 700; margin-bottom: 4px; }
  .faq-row dd { margin: 0; color: var(--text-muted); line-height: 1.6; }
</style>
