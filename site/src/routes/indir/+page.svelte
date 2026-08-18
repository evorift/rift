<script>
  // İndirme sayfası. skill §2: imza durumu, her artefaktın SHA-256'sı, VirusTotal,
  // HackTool açıklaması ve tek resmi kanal ilanı — hiçbiri opsiyonel değil, hiçbiri
  // dipnota gömülmez. Rakamlar docs/LIVE-VERIFICATION.md ve build-031/'den.
  import Shell from "$lib/Shell.svelte";
  import { getLang } from "$lib/i18n.svelte.js";

  const REPO = "https://github.com/evorift/rift";
  const RELEASES = REPO + "/releases";

  // 0.3.1 build-031/ kurulum dosyası: 12.332.683 bayt.
  const VERSION = "0.3.1";
  const SIZE = "12,3 MB";
  const FILE = `evorift_${VERSION}_x64-setup.exe`;

  // Sürüm dondurulduğunda Get-FileHash çıktısıyla doldurulacak.
  const SHA256 = null;
  // Kalıcı VirusTotal analiz bağlantısı geldiğinde doldurulacak.
  const VIRUSTOTAL = null;

  const L = {
    tr: {
      title: "İndir",
      lead: `Windows 10 / 11, 64-bit. Sürüm ${VERSION}, ${SIZE}. Kurulum dosyası — taşınabilir sürüm yok.`,
      get: "Kurulum dosyasını indir",
      via: "İndirme GitHub Releases üzerinden yapılır.",
      channelT: "Tek resmi kanal",
      channelD:
        "evorift yalnız buradan dağıtılır. Başka bir yerden indirdiğin dosya bize ait değildir — adı ve simgesi aynı olsa bile.",
      signT: "Bu sürüm imzasız",
      signD:
        "Kod imzalama sertifikası yıllık ücretli; ücretsiz kalabilmek için almadık. Bunun somut sonucu şu: dosyayı ilk çalıştırdığında Windows SmartScreen mavi bir uyarı ekranı gösterecek. Bu, dosyada bir sorun olduğu anlamına gelmez; Windows'un tanımadığı yayıncıya verdiği standart tepkidir.",
      signSteps: [
        "Mavi ekranda 'Ek bilgi' (More info) yazısına tıkla.",
        "Açılan satırın altında beliren 'Yine de çalıştır' (Run anyway) düğmesine bas.",
        "Kurulum normal şekilde devam eder.",
      ],
      shotCap: "SmartScreen uyarısının göründüğü ekran",
      shotPh: "ekran görüntüsü",
      hashT: "SHA-256",
      hashD:
        "İndirdiğin dosyanın yolda bozulmadığını ya da değiştirilmediğini böyle doğrularsın. PowerShell'de şunu çalıştır ve çıktıyı aşağıdaki değerle karşılaştır — eşleşmiyorsa dosyayı çalıştırma.",
      hashPending:
        "Bu sürümün özeti henüz yayınlanmadı. Doldurulana kadar dosyayı Releases sayfasındaki SHA256SUMS ile doğrula.",
      avT: "Antivirüsün 'HackTool' demesi bekleniyor",
      avD:
        "evorift, ağ paketlerini yakalayıp yeniden yazan WinDivert adlı açık kaynaklı bir Windows sürücüsü kullanır (LGPL, yaygın kullanımda). Bazı antivirüs motorları paket yakalayan HER programı — ne yaptığına bakmaksızın — genel bir 'HackTool' imzasıyla işaretler. Yani birkaç motorun uyarı vermesi beklenen bir durumdur ve önceden söylüyoruz ki sürprizle karşılaşma.",
      vtPending:
        "Bu sürümün kalıcı VirusTotal analiz bağlantısı henüz eklenmedi. O zamana kadar dosyayı virustotal.com'a kendin yükleyip tüm motorların sonucunu görebilirsin.",
      vtLink: "VirusTotal analizi",
      reqT: "Sistem gereksinimleri",
      reqList: [
        "Windows 10 veya 11, 64-bit",
        "Yönetici hakkı (ağ sürücüsünü kurmak için, ilk çalıştırmada bir kez)",
        "İnternet bağlantısı",
      ],
      collectT: "Ne topluyoruz",
      collectNone: "Hiçbir şey. Sunucumuz yok, hesap yok, telemetri yok.",
      collectD:
        "Ayarların ve günlükler yalnız kendi bilgisayarında durur. Günlüklere alan adı yazılmaz; yazılmadığını doğrulayan bir test var. Ayrıntı Gizlilik sayfasında.",
      srcT: "Kaynak kod",
      srcD: "Çekirdek açık kaynak, MIT lisansı. İncelemek istersen:",
      metaDesc: `evorift'i indir: Windows 10/11, ${SIZE} kurulum dosyası. SHA-256, imza durumu, SmartScreen rehberi ve VirusTotal linki bu sayfada.`,
    },
    en: {
      title: "Download",
      lead: `Windows 10 / 11, 64-bit. Version ${VERSION}, ${SIZE}. Installer — there is no portable build.`,
      get: "Download the installer",
      via: "The download is served from GitHub Releases.",
      channelT: "One official channel",
      channelD:
        "evorift is distributed only from here. A file you downloaded anywhere else is not ours — even if the name and icon match.",
      signT: "This build is unsigned",
      signD:
        "A code-signing certificate costs money every year; we did not buy one in order to stay free. The concrete consequence: the first time you run the file, Windows SmartScreen shows a blue warning screen. That does not mean anything is wrong with the file; it is Windows' standard response to a publisher it does not recognise.",
      signSteps: [
        "Click 'More info' on the blue screen.",
        "Press the 'Run anyway' button that appears underneath.",
        "Setup continues normally.",
      ],
      shotCap: "The screen where the SmartScreen warning appears",
      shotPh: "screenshot",
      hashT: "SHA-256",
      hashD:
        "This is how you confirm the file was not corrupted or altered on the way. Run this in PowerShell and compare the output to the value below — if it does not match, do not run it.",
      hashPending:
        "The digest for this build has not been published yet. Until it is, verify the file against the SHA256SUMS on the Releases page.",
      avT: "Your antivirus saying 'HackTool' is expected",
      avD:
        "evorift uses WinDivert, an open-source Windows driver (LGPL, widely used) that captures and rewrites network packets. Some antivirus engines flag ANY program that captures packets — regardless of what it does — with a generic 'HackTool' signature. So a few engines warning about it is expected, and we are telling you up front so it does not come as a surprise.",
      vtPending:
        "A permanent VirusTotal report link for this build has not been added yet. Until then you can upload the file to virustotal.com yourself and see every engine's verdict.",
      vtLink: "VirusTotal report",
      reqT: "System requirements",
      reqList: [
        "Windows 10 or 11, 64-bit",
        "Administrator rights (to install the network driver, once on first run)",
        "An internet connection",
      ],
      collectT: "What we collect",
      collectNone: "Nothing. No servers, no accounts, no telemetry.",
      collectD:
        "Your settings and logs stay on your own machine. Domain names are not written to the logs, and there is a test that proves they aren't. Detail on the Privacy page.",
      srcT: "Source code",
      srcD: "The core is open source under the MIT license. If you want to read it:",
      metaDesc: `Download evorift: Windows 10/11, ${SIZE} installer. SHA-256, signing status, SmartScreen guide and VirusTotal link on this page.`,
    },
  };

  const c = $derived(L[getLang()] ?? L.en);
</script>

<Shell title={c.title} lead={c.lead} description={c.metaDesc} path="/indir">
  <section class="block get-block">
    <a class="btn primary big" href={RELEASES} target="_blank" rel="noopener">⬇ {c.get}</a>
    <p class="file mono">{FILE} · {SIZE}</p>
    <p class="muted">{c.via}</p>
  </section>

  <section class="block">
    <h2>{c.channelT}</h2>
    <p>{c.channelD}</p>
    <p class="mono chan">github.com/evorift/rift/releases</p>
  </section>

  <section class="block warn">
    <h2>{c.signT}</h2>
    <p>{c.signD}</p>
    <ol class="steps">
      {#each c.signSteps as s (s)}<li>{s}</li>{/each}
    </ol>
    <figure class="shot">
      <div class="shot-ph mono" aria-hidden="true">SmartScreen · {c.shotPh}</div>
      <figcaption>{c.shotCap}</figcaption>
    </figure>
  </section>

  <section class="block">
    <h2>{c.hashT}</h2>
    <p>{c.hashD}</p>
    <pre class="code mono">Get-FileHash {FILE} -Algorithm SHA256</pre>
    {#if SHA256}
      <p class="hash mono">{SHA256}</p>
    {:else}
      <p class="pending">{c.hashPending}</p>
    {/if}
  </section>

  <section class="block">
    <h2>{c.avT}</h2>
    <p>{c.avD}</p>
    {#if VIRUSTOTAL}
      <p><a class="btn" href={VIRUSTOTAL} target="_blank" rel="noopener">{c.vtLink}</a></p>
    {:else}
      <p class="pending">{c.vtPending}</p>
    {/if}
  </section>

  <section class="block">
    <h2>{c.reqT}</h2>
    <ul class="list">
      {#each c.reqList as r (r)}<li>{r}</li>{/each}
    </ul>
  </section>

  <section class="block">
    <h2>{c.collectT}</h2>
    <p class="none">{c.collectNone}</p>
    <p>{c.collectD}</p>
  </section>

  <section class="block">
    <h2>{c.srcT}</h2>
    <p>{c.srcD} <a href={REPO} target="_blank" rel="noopener">github.com/evorift/rift</a></p>
  </section>
</Shell>

<style>
  .block { margin-top: 40px; }
  .block h2 { font-size: 20px; font-weight: 800; letter-spacing: -0.01em; margin-bottom: 10px; }
  .block p { color: var(--text-muted); line-height: 1.65; margin-bottom: 10px; }

  .get-block { margin-top: 30px; }
  .big { padding: 16px 28px; font-size: 16px; }
  .file { margin-top: 12px; color: var(--text); font-size: 13.5px; }
  .muted { color: var(--text-dim); font-size: 13px; }

  .chan { color: var(--accent); font-size: 14px; }

  /* İmza uyarısı gizlenmez, vurgulanır — kullanıcının kendi keşfetmesi daha kötü. */
  .warn {
    border: 1px solid var(--border);
    border-left: 2px solid var(--accent-dim);
    border-radius: var(--radius);
    padding: 22px;
    background: var(--bg-surface);
  }

  .steps { margin: 0 0 16px; padding-left: 20px; color: var(--text-muted); line-height: 1.8; }
  .list { margin: 0; padding-left: 20px; color: var(--text-muted); line-height: 1.8; }

  .code {
    margin: 0 0 10px;
    padding: 13px 16px;
    background: var(--bg-elevated);
    border: 1px solid var(--border-soft);
    border-radius: var(--radius-sm);
    color: var(--text);
    font-size: 13px;
    overflow-x: auto;
  }
  .hash { word-break: break-all; color: var(--accent); font-size: 13px; }

  /* Eksik olan gizlenmiyor, eksik olduğu yazıyor. */
  .pending {
    padding: 12px 14px;
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
    color: var(--text-dim);
    font-size: 13.5px;
  }

  .none { color: var(--accent) !important; font-weight: 600; }

  .shot { margin: 0; }
  .shot-ph {
    display: grid;
    place-items: center;
    aspect-ratio: 16 / 10;
    border: 1px dashed var(--border);
    border-radius: var(--radius-sm);
    background: var(--bg-elevated);
    color: var(--text-dim);
    font-size: 12px;
  }
  .shot figcaption { margin-top: 8px; color: var(--text-dim); font-size: 12.5px; }
</style>
