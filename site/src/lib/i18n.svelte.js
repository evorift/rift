// Hafif, bağımsız i18n — uygulamadaki yapının landing sürümü. Harici kütüphane yok.
// `lang` reaktif ($state) → t() çağrıları markup'ta otomatik güncellenir.
//
// METİN KURALI (evorift-website skill §1): docs/LIVE-VERIFICATION.md'de ölçülmemiş
// hiçbir rakam ya da vaat burada geçmez. Rakam varsa yanında sürümü ve koşulu durur.
// §3: "engel aşma" değil "bağlantı düzeltme" dili. §5: motor adı yok, teknik terim yok.

export const LANGS = /** @type {const} */ (["tr", "en", "es", "ru"]);
export const LANG_LABEL = { tr: "TR", en: "EN", es: "ES", ru: "RU" };

const tr = {
  "brand.badge": "Açık kaynak · Ücretsiz · Sunucusuz",
  "nav.features": "Ne yapar",
  "nav.how": "Nasıl çalışır",
  "nav.measured": "Ölçülen",
  "nav.github": "GitHub",
  "nav.faq": "SSS",
  "nav.download": "İndir",
  "nav.privacy": "Gizlilik",
  "nav.support": "Destek ol",

  "hero.title": "Hiçbir şey seni durduramaz.",
  "hero.sub":
    "Operatör kaynaklı bağlantı bozulmalarını gideren Windows aracı. Mesajlaşma, sesli görüşme ve oyunlar yeniden açılır. Sunucumuz yok — trafiğin bizden geçmez.",
  "cta.download": "Windows için indir",
  "cta.releases": "GitHub Releases",
  "cta.github": "GitHub'da gör",
  "cta.sponsor": "Sponsor ol",
  "platform": "Windows 10 / 11 · 64-bit",

  "feat.title": "Ne yapar?",
  "feat.unblock.t": "Bozulan bağlantıyı onarır",
  "feat.unblock.d":
    "Operatörün bağlantını incelemesinden kaynaklanan bozulmayı giderir; kapanan uygulamalar yeniden açılır. Trafiğin senin cihazından çıkar, bizim hiçbir sunucumuza uğramaz.",
  "feat.smart.t": "Yalnız gerekene dokunur",
  "feat.smart.d":
    "Bozulan bağlantılar düzeltilir, geri kalan her şey doğrudan akar. Gereksiz trafik dolaştırılmaz.",
  "feat.safe.t": "Geri alınabilir",
  "feat.safe.d":
    "Anti-cheat farkındadır (Vanguard / EAC / BattlEye). Yaptığı her ayar geri alınabilir; riskli olan önce sorar.",
  "feat.light.t": "Yerel yazılım",
  "feat.light.d":
    "Hesap yok, abonelik yok, telemetri yok. Pencere kapanınca tepside çalışmaya devam eder.",

  "how.title": "Nasıl çalışır?",
  "how.1": "İndir ve çalıştır — hesap açmadan.",
  "how.2": "Koruma modunu seç.",
  "how.3": "Kullan — kapanan uygulamalar açılır, trafiğinin geri kalanı doğrudan kalır.",

  "how.modes.title": "Dört mod",
  "how.modes.lead":
    "Uygulamadaki adlarla birebir aynı. Hangisinin işe yaradığı hattına göre değişir.",
  "mode.light.t": "Hafif Koruma",
  "mode.light.d":
    "En dar ve en güvenli kapsam: yalnız bilinen birkaç alan adına dokunur, başka hiçbir şeyi değiştirmez.",
  "mode.strong.t": "Güçlü Koruma",
  "mode.strong.d":
    "Geniş kapsam. Ölçülen sonuçların alındığı mod bu — aşağıdaki Ölçülen bölümüne bak.",
  "mode.auto.t": "Otomatik",
  "mode.auto.d":
    "Hattını kendi test edip çalışan ayarı seçmesi hedeflenen mod. Şu anda geliştiriliyor, henüz çalışmıyor.",
  "mode.vpn.t": "VPN",
  "mode.vpn.d":
    "Temiz rota isteyen uygulamalar için Cloudflare WARP üzerinden tünel. Yalnız bu modda tünel kurulur; diğer üç modda trafik doğrudan akar.",
  "mode.wip": "geliştiriliyor",

  "meas.title": "Ölçülen",
  "meas.lead":
    "Her rakamın yanında hangi sürümde, hangi tarihte ve hangi hatta ölçüldüğü yazıyor. Ölçümü olmayan iddia bu sitede yok.",
  "meas.1.v": "400/400",
  "meas.1.l": "başarılı el sıkışma, dört hedef, Güçlü Koruma",
  "meas.1.p": "2026-08-15 · tek ev hattı · sürüm 0.1.7",
  "meas.2.v": "184 ms",
  "meas.2.l": "discord.com ortalama, 100/100 başarılı",
  "meas.2.p": "2026-08-15 · tek ev hattı · sürüm 0.1.7",
  "meas.3.v": "12,3 MB",
  "meas.3.l": "kurulum dosyası boyutu",
  "meas.3.p": "sürüm 0.3.1",
  "meas.caveat":
    "Bu sonuçlar tek hatta, tek oturumda alındı — başka bir operatörde aynısını vereceğini söyleyemeyiz. İndirdiğin 0.3.1 sürümünün ölçüm turu henüz tamamlanmadı; tamamlandığında buradaki rakamlar değişecek.",

  "foot.smartscreen":
    "Sürüm imzasız: ilk açılışta Windows SmartScreen uyarısı çıkar → 'Ek bilgi → Yine de çalıştır'.",
  "foot.official": "evorift yalnız buradan dağıtılır:",
  "foot.official.warn": "Başka bir yerden indirdiğin dosya bize ait değildir.",
  "foot.made": "MIT lisansı altında açık kaynak.",
  "foot.tagline": "Bağlantın düzelsin.",
  "meta.title": "bağlantı sorunlarını çözer",

  "faq.title": "Sık sorulanlar",
  "faq.q1": "Bağlantımı yavaşlatır mı?",
  "faq.a1":
    "Trafiğinin çoğu doğrudan akmaya devam eder — yalnız bozulan bağlantılar düzeltilir. VPN modunu açtığında o moda alınan uygulamalar Cloudflare WARP üzerinden gider; diğer üç modda hiçbir tünel kurulmaz. Gecikme etkisini kendi hattında ölçmedik, o yüzden bir rakam vermiyoruz.",
  "faq.q2": "Oyunlarda / anti-cheat ile güvenli mi?",
  "faq.a2":
    "Anti-cheat farkındadır (Vanguard / EAC / BattlEye) ve korumalı bir oyun açılınca korumayı otomatik duraklatabilir. Prototip anti-cheat ile çalıştı; bunu bir garanti değil, opsiyonel bir güvenlik ağı olarak sun. Her ayar geri alınabilir.",
  "faq.q3": "Neden ücretsiz?",
  "faq.a3":
    "Sunucu işletmiyoruz, dolayısıyla taşıyacak bir maliyet yok. Hesap yok, abonelik yok, reklam yok, veri satışı yok. Açık kaynak (MIT). İstersen GitHub Sponsors üzerinden destekleyebilirsin, zorunlu değil.",
  "faq.q4": "Hangi uygulamalar için çalışıyor?",
  "faq.a4":
    "Mesajlaşma, sesli görüşme, oyunlar ve listene eklediklerin. Discord ve Roblox üzerinde doğrulandı. Hangi alan adlarının etkileneceğini sen seçersin.",
  "faq.q5": "Windows ilk açılışta uyarı veriyor (SmartScreen)?",
  "faq.a5":
    "Sürüm imzasız olduğu için Windows bir uyarı gösterir → 'Ek bilgi → Yine de çalıştır'. Kod imzalama sertifikası yıllık ücretli; ücretsiz kalabilmek için almadık. İndirme sayfasındaki SHA-256 ile dosyanın bozulmadığını doğrulayabilirsin.",
  "faq.q6": "Antivirüs 'HackTool' diyor, virüs mü bu?",
  "faq.a6":
    "Hayır — bu WinDivert sürücüsü yüzünden. WinDivert, ağ paketlerini yakalayıp yeniden yazan, açık kaynaklı ve yaygın kullanılan (LGPL) bir Windows sürücüsü; evorift'in bağlantı düzeltmesi tam olarak bunu kullanır. Bazı antivirüs programları paket yakalayan HER programı — gerçek amacı ne olursa olsun — genel bir imzayla 'HackTool' diye işaretler. Bu bilinen ve beklenen bir yanlış pozitif.",
  "faq.q7": "İndirdiğim dosyanın gerçek olduğunu nasıl doğrularım?",
  "faq.a7":
    "İndirme sayfasında her dosyanın SHA-256 özeti yazılı. PowerShell'de 'Get-FileHash indirdigin-dosya.exe -Algorithm SHA256' çalıştır ve çıktıyı oradaki satırla karşılaştır — eşleşmiyorsa dosyayı çalıştırma. evorift'in tek resmi kaynağı github.com/evorift/rift/releases; başka hiçbir yerden indirme.",
  "faq.q8": "Veri topluyor musunuz?",
  "faq.a8":
    "Hayır. Sunucumuz yok, hesap yok, telemetri yok. Ayarların ve günlükler yalnız kendi bilgisayarında durur. Günlüklere alan adı yazılmaz — yazılmadığını doğrulayan bir test var. Ayrıntı için Gizlilik sayfasına bak.",
  "faq.q9": "Yasal mı?",
  "faq.a9":
    "evorift yerel bir yazılımdır: bilgisayarında çalışır, bizim işlettiğimiz bir sunucu ya da relay yoktur, trafiğin bizden geçmez. Bir hizmet satmıyoruz. Kendi ülkendeki kuralların ne dediğinden sen sorumlusun.",
  "faq.q10": "Nasıl kaldırırım?",
  "faq.a10":
    "Uygulama içindeki 'Tüm verileri sil' ile yaptığı bütün değişiklikleri geri alır, sonra Windows'un Uygulamalar listesinden normal şekilde kaldırılır. Geride sürücü ya da servis bırakmaz.",
};

const en = {
  "brand.badge": "Open source · Free · No servers",
  "nav.features": "What it does",
  "nav.how": "How it works",
  "nav.measured": "Measured",
  "nav.github": "GitHub",
  "nav.faq": "FAQ",
  "nav.download": "Download",
  "nav.privacy": "Privacy",
  "nav.support": "Support",

  "hero.title": "Nothing can stop you.",
  "hero.sub":
    "A Windows tool that repairs connections broken by your provider. Messaging, voice and games start working again. We run no servers — your traffic never passes through us.",
  "cta.download": "Download for Windows",
  "cta.releases": "GitHub Releases",
  "cta.github": "View on GitHub",
  "cta.sponsor": "Sponsor",
  "platform": "Windows 10 / 11 · 64-bit",

  "feat.title": "What it does",
  "feat.unblock.t": "Repairs a broken connection",
  "feat.unblock.d":
    "Fixes the breakage caused by your provider inspecting your connection, so apps that stopped working start again. Your traffic leaves your own machine and touches none of our servers.",
  "feat.smart.t": "Touches only what needs it",
  "feat.smart.d":
    "Broken connections get repaired, everything else flows direct. No detour for traffic that doesn't need one.",
  "feat.safe.t": "Reversible",
  "feat.safe.d":
    "Anti-cheat aware (Vanguard / EAC / BattlEye). Every setting it changes can be undone; the risky ones ask first.",
  "feat.light.t": "Local software",
  "feat.light.d":
    "No account, no subscription, no telemetry. Keeps running in the tray when you close the window.",

  "how.title": "How it works",
  "how.1": "Download and run — no account needed.",
  "how.2": "Pick a protection mode.",
  "how.3": "Use it — apps that stopped working open up, the rest of your traffic stays direct.",

  "how.modes.title": "Four modes",
  "how.modes.lead":
    "Named exactly as they are in the app. Which one works depends on your line.",
  "mode.light.t": "Hafif Koruma",
  "mode.light.d":
    "The narrowest, safest scope: touches only a few known domains and changes nothing else.",
  "mode.strong.t": "Güçlü Koruma",
  "mode.strong.d":
    "Wide scope. This is the mode the measured results below were taken in.",
  "mode.auto.t": "Otomatik",
  "mode.auto.d":
    "Intended to test your line and pick the setting that works. Currently in development — it does not work yet.",
  "mode.vpn.t": "VPN",
  "mode.vpn.d":
    "A tunnel through Cloudflare WARP for apps that need a clean route. Only this mode builds a tunnel; in the other three your traffic goes direct.",
  "mode.wip": "in development",

  "meas.title": "Measured",
  "meas.lead":
    "Every number here carries the version, the date and the line it was measured on. Nothing unmeasured is claimed on this site.",
  "meas.1.v": "400/400",
  "meas.1.l": "successful handshakes, four targets, Güçlü Koruma",
  "meas.1.p": "2026-08-15 · one home line · version 0.1.7",
  "meas.2.v": "184 ms",
  "meas.2.l": "discord.com average, 100/100 successful",
  "meas.2.p": "2026-08-15 · one home line · version 0.1.7",
  "meas.3.v": "12.3 MB",
  "meas.3.l": "installer size",
  "meas.3.p": "version 0.3.1",
  "meas.caveat":
    "These results come from one line in one session — we cannot tell you they will repeat on another provider. The measurement pass for the 0.3.1 build you download has not been completed yet; these numbers will change when it is.",

  "foot.smartscreen":
    "Unsigned build: Windows SmartScreen will warn on first launch → 'More info → Run anyway'.",
  "foot.official": "evorift is distributed only from here:",
  "foot.official.warn": "A file you downloaded anywhere else is not ours.",
  "foot.made": "Open source under the MIT license.",
  "foot.tagline": "Get your connection working.",
  "meta.title": "fixes connection problems",

  "faq.title": "Frequently asked",
  "faq.q1": "Will it slow my connection down?",
  "faq.a1":
    "Most of your traffic keeps flowing direct — only broken connections get repaired. When you turn on VPN mode, the apps you put in it go through Cloudflare WARP; the other three modes build no tunnel at all. We have not measured the latency effect on your line, so we are not giving you a number.",
  "faq.q2": "Is it safe with games / anti-cheat?",
  "faq.a2":
    "It is anti-cheat aware (Vanguard / EAC / BattlEye) and can pause protection automatically when a protected game starts. It worked with a prototype anti-cheat; treat it as an optional safety net, not a guarantee. Every setting is reversible.",
  "faq.q3": "Why is it free?",
  "faq.a3":
    "We run no servers, so there is no cost to pass on. No accounts, no subscriptions, no ads, no data selling. Open source (MIT). You can support it through GitHub Sponsors if you want to — it is not required.",
  "faq.q4": "Which apps does it work for?",
  "faq.a4":
    "Messaging, voice, games, and whatever you add to the list. Verified on Discord and Roblox. You choose which domains are affected.",
  "faq.q5": "Windows warns me on first launch (SmartScreen)?",
  "faq.a5":
    "The build is unsigned, so Windows shows a warning → 'More info → Run anyway'. A code-signing certificate costs money every year; we did not buy one in order to stay free. You can verify the file against the SHA-256 on the download page.",
  "faq.q6": "My antivirus says 'HackTool' — is this a virus?",
  "faq.a6":
    "No — that is the WinDivert driver. WinDivert is an open-source, widely used (LGPL) Windows driver that captures and rewrites network packets, and evorift's connection repair is built on exactly that. Some antivirus engines flag ANY program that captures packets — whatever it actually does — with a generic 'HackTool' signature. This is a known, expected false positive.",
  "faq.q7": "How do I verify the file I downloaded is genuine?",
  "faq.a7":
    "The download page lists the SHA-256 of every file. Run 'Get-FileHash your-download.exe -Algorithm SHA256' in PowerShell and compare it to the line there — if it does not match, do not run it. evorift's only official source is github.com/evorift/rift/releases; never download it anywhere else.",
  "faq.q8": "Do you collect data?",
  "faq.a8":
    "No. No servers, no accounts, no telemetry. Your settings and logs stay on your own machine. Domain names are not written to the logs — there is a test that proves they aren't. See the Privacy page for detail.",
  "faq.q9": "Is it legal?",
  "faq.a9":
    "evorift is local software: it runs on your machine, there is no server or relay we operate, and your traffic does not pass through us. We are not selling a service. What your own country's rules say is your responsibility.",
  "faq.q10": "How do I uninstall it?",
  "faq.a10":
    "'Tüm verileri sil' inside the app reverts every change it made, then you remove it normally from the Windows apps list. It leaves no driver or service behind.",
};

const es = {
  "brand.badge": "Código abierto · Gratis · Sin servidores",
  "nav.features": "Qué hace",
  "nav.how": "Cómo funciona",
  "nav.measured": "Medido",
  "nav.github": "GitHub",
  "nav.faq": "FAQ",
  "nav.download": "Descargar",
  "nav.privacy": "Privacidad",
  "nav.support": "Apoyar",

  "hero.title": "Nada puede detenerte.",
  "hero.sub":
    "Una herramienta de Windows que repara conexiones dañadas por tu proveedor. La mensajería, la voz y los juegos vuelven a funcionar. No tenemos servidores: tu tráfico no pasa por nosotros.",
  "cta.download": "Descargar para Windows",
  "cta.releases": "GitHub Releases",
  "cta.github": "Ver en GitHub",
  "cta.sponsor": "Patrocinar",
  "platform": "Windows 10 / 11 · 64 bits",

  "feat.title": "Qué hace",
  "feat.unblock.t": "Repara una conexión dañada",
  "feat.unblock.d":
    "Corrige el daño que causa tu proveedor al inspeccionar tu conexión, así las apps que dejaron de funcionar arrancan de nuevo. Tu tráfico sale de tu propia máquina y no toca ningún servidor nuestro.",
  "feat.smart.t": "Solo toca lo necesario",
  "feat.smart.d":
    "Las conexiones dañadas se reparan, el resto va directo. Sin desvíos para el tráfico que no los necesita.",
  "feat.safe.t": "Reversible",
  "feat.safe.d":
    "Consciente del anti-cheat (Vanguard / EAC / BattlEye). Cada ajuste que hace se puede deshacer; los riesgosos preguntan primero.",
  "feat.light.t": "Software local",
  "feat.light.d":
    "Sin cuenta, sin suscripción, sin telemetría. Sigue en la bandeja cuando cierras la ventana.",

  "how.title": "Cómo funciona",
  "how.1": "Descarga y ejecuta — sin crear una cuenta.",
  "how.2": "Elige un modo de protección.",
  "how.3": "Úsalo — las apps que dejaron de funcionar se abren, el resto del tráfico sigue directo.",

  "how.modes.title": "Cuatro modos",
  "how.modes.lead":
    "Con los mismos nombres que en la aplicación. Cuál funciona depende de tu línea.",
  "mode.light.t": "Hafif Koruma",
  "mode.light.d":
    "El alcance más estrecho y seguro: solo toca unos pocos dominios conocidos y no cambia nada más.",
  "mode.strong.t": "Güçlü Koruma",
  "mode.strong.d": "Alcance amplio. Es el modo en el que se tomaron los resultados medidos.",
  "mode.auto.t": "Otomatik",
  "mode.auto.d":
    "Pensado para probar tu línea y elegir el ajuste que funcione. En desarrollo — todavía no funciona.",
  "mode.vpn.t": "VPN",
  "mode.vpn.d":
    "Un túnel por Cloudflare WARP para las apps que necesitan una ruta limpia. Solo este modo crea un túnel; en los otros tres el tráfico va directo.",

  "meas.title": "Medido",
  "meas.lead":
    "Cada número indica la versión, la fecha y la línea en la que se midió. En este sitio no se afirma nada sin medición.",
  "meas.1.v": "400/400",
  "meas.1.l": "saludos correctos, cuatro objetivos, Güçlü Koruma",
  "meas.1.p": "2026-08-15 · una línea doméstica · versión 0.1.7",
  "meas.2.v": "184 ms",
  "meas.2.l": "media de discord.com, 100/100 correctos",
  "meas.2.p": "2026-08-15 · una línea doméstica · versión 0.1.7",
  "meas.3.v": "12,3 MB",
  "meas.3.l": "tamaño del instalador",
  "meas.3.p": "versión 0.3.1",
  "meas.caveat":
    "Estos resultados vienen de una sola línea en una sola sesión — no podemos decirte que se repetirán con otro proveedor. La ronda de medición de la versión 0.3.1 que descargas aún no se ha completado; estos números cambiarán cuando lo esté.",

  "foot.smartscreen":
    "Build sin firmar: Windows SmartScreen avisará al primer inicio → 'Más información → Ejecutar de todos modos'.",
  "foot.official": "evorift se distribuye solo desde aquí:",
  "foot.official.warn": "Un archivo descargado en cualquier otro sitio no es nuestro.",
  "foot.made": "Código abierto bajo licencia MIT.",
  "foot.tagline": "Que tu conexión funcione.",
  "meta.title": "arregla problemas de conexión",
};

const ru = {
  "brand.badge": "Открытый код · Бесплатно · Без серверов",
  "nav.features": "Что делает",
  "nav.how": "Как работает",
  "nav.measured": "Измерено",
  "nav.github": "GitHub",
  "nav.faq": "ЧаВо",
  "nav.download": "Скачать",
  "nav.privacy": "Приватность",
  "nav.support": "Поддержать",

  "hero.title": "Ничто не сможет тебя остановить.",
  "hero.sub":
    "Программа для Windows, которая исправляет соединение, испорченное провайдером. Мессенджеры, голосовые вызовы и игры снова работают. У нас нет серверов — ваш трафик через нас не идёт.",
  "cta.download": "Скачать для Windows",
  "cta.releases": "GitHub Releases",
  "cta.github": "Открыть на GitHub",
  "cta.sponsor": "Спонсировать",
  "platform": "Windows 10 / 11 · 64-бит",

  "feat.title": "Что делает",
  "feat.unblock.t": "Исправляет испорченное соединение",
  "feat.unblock.d":
    "Устраняет поломку, возникающую из-за того, что провайдер разбирает ваше соединение, и приложения снова начинают работать. Трафик уходит с вашей машины и не касается наших серверов.",
  "feat.smart.t": "Трогает только нужное",
  "feat.smart.d":
    "Испорченные соединения исправляются, остальное идёт напрямую. Лишний трафик никуда не заворачивается.",
  "feat.safe.t": "Обратимо",
  "feat.safe.d":
    "Учитывает анти-чит (Vanguard / EAC / BattlEye). Любую сделанную настройку можно откатить; рискованные спрашивают заранее.",
  "feat.light.t": "Локальная программа",
  "feat.light.d":
    "Без аккаунта, без подписки, без телеметрии. При закрытии окна продолжает работать в трее.",

  "how.title": "Как работает",
  "how.1": "Скачайте и запустите — без регистрации.",
  "how.2": "Выберите режим защиты.",
  "how.3": "Пользуйтесь — переставшие работать приложения открываются, остальной трафик идёт напрямую.",

  "how.modes.title": "Четыре режима",
  "how.modes.lead":
    "Названия те же, что в приложении. Какой сработает — зависит от вашей линии.",
  "mode.light.t": "Hafif Koruma",
  "mode.light.d":
    "Самый узкий и безопасный охват: затрагивает лишь несколько известных доменов и больше ничего не меняет.",
  "mode.strong.t": "Güçlü Koruma",
  "mode.strong.d": "Широкий охват. Именно в этом режиме получены измерения ниже.",
  "mode.auto.t": "Otomatik",
  "mode.auto.d":
    "Задуман так, чтобы сам проверил линию и выбрал работающую настройку. В разработке — пока не работает.",
  "mode.vpn.t": "VPN",
  "mode.vpn.d":
    "Туннель через Cloudflare WARP для приложений, которым нужен чистый маршрут. Туннель создаётся только в этом режиме; в остальных трёх трафик идёт напрямую.",

  "meas.title": "Измерено",
  "meas.lead":
    "У каждого числа указаны версия, дата и линия, на которой оно измерено. Ничего неизмеренного на этом сайте не утверждается.",
  "meas.1.v": "400/400",
  "meas.1.l": "успешных рукопожатий, четыре цели, Güçlü Koruma",
  "meas.1.p": "2026-08-15 · одна домашняя линия · версия 0.1.7",
  "meas.2.v": "184 мс",
  "meas.2.l": "среднее по discord.com, 100/100 успешно",
  "meas.2.p": "2026-08-15 · одна домашняя линия · версия 0.1.7",
  "meas.3.v": "12,3 МБ",
  "meas.3.l": "размер установщика",
  "meas.3.p": "версия 0.3.1",
  "meas.caveat":
    "Эти результаты получены на одной линии за одну сессию — мы не можем обещать, что они повторятся у другого провайдера. Измерительный прогон для версии 0.3.1, которую вы скачиваете, ещё не завершён; когда он завершится, числа изменятся.",

  "foot.smartscreen":
    "Сборка без подписи: при первом запуске Windows SmartScreen предупредит → 'Подробнее → Выполнить в любом случае'.",
  "foot.official": "evorift распространяется только отсюда:",
  "foot.official.warn": "Файл, скачанный где-либо ещё, не наш.",
  "foot.made": "Открытый код под лицензией MIT.",
  "foot.tagline": "Пусть соединение работает.",
  "meta.title": "решает проблемы с соединением",
};

const dict = { tr, en, es, ru };

let lang = $state("tr");

export function getLang() {
  return lang;
}

export function setLang(l) {
  if (!LANGS.includes(l)) return;
  lang = l;
  if (typeof document !== "undefined") document.documentElement.lang = l;
  try {
    localStorage.setItem("rift-lang", l);
  } catch {}
}

/** Kayıtlı tercih → bilgisayarın dili → Türkçe. */
export function initLang() {
  try {
    const saved = localStorage.getItem("rift-lang");
    if (saved && LANGS.includes(saved)) return setLang(saved);
  } catch {}
  const prefs =
    typeof navigator !== "undefined"
      ? navigator.languages?.length
        ? navigator.languages
        : [navigator.language || "tr"]
      : ["tr"];
  for (const p of prefs) {
    const code = String(p).slice(0, 2).toLowerCase();
    if (LANGS.includes(code)) return setLang(code);
  }
  setLang("tr");
}

/**
 * Geri düşüş zinciri: seçili dil → İngilizce → Türkçe → anahtar.
 * ES/RU sözlükleri kasten kısa: çevirisi yazılmamış bir bölümü uydurmak yerine
 * İngilizcesini göstermek doğru davranış (skill §5 — çeviri kokan metin yasak).
 */
export function t(key) {
  return dict[lang]?.[key] ?? dict.en[key] ?? dict.tr[key] ?? key;
}
