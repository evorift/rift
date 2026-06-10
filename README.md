<div align="center">

# evorift

**Engelleri aş. VPN'siz.**

Discord, Roblox ve oyunları tek tıkla açan hafif bir Windows uygulaması. İnternet
sağlayıcının DPI sansürünü cerrahi paket teknikleriyle (WinDivert) aşar — hızın
düşmez, yalnızca senin seçtiğin sitelerin trafiğine dokunur, sesin/UDP'n çalışmaya
devam eder.

*VPN değil · hız kaybı yok · IP'n değişmez · hesap yok*

[![Sürüm](https://img.shields.io/github/v/release/evorift/rift?label=s%C3%BCr%C3%BCm)](https://github.com/evorift/rift/releases)
[![Lisans](https://img.shields.io/badge/lisans-MIT-green)](LICENSE)
[![Platform](https://img.shields.io/badge/platform-Windows%2010%20%2F%2011%20x64-blue)](https://github.com/evorift/rift/releases)

</div>

---

## evorift nedir?

evorift, internet sağlayıcının **DPI (Derin Paket İncelemesi)** ile uyguladığı
engelleri aşan ücretsiz bir Windows uygulamasıdır. **VPN değildir** — trafiğini
başka bir sunucuya yönlendirmez, IP adresini değiştirmez. Yalnızca engeli aşmak
için gereken paketlere dokunur; geri kalan her şey normal hızında akar.

**Neyi açar:**

- **Discord** (sesli sohbet ve görüntülü dahil)
- **Roblox** ve oyun launcher'ları
- **YouTube** ve listene eklediğin diğer alan adları

Hangi sitelerin etkileneceğini sen seçersin. Diğer her şey dokunulmadan, tam hızda kalır.

---

## İndir

Son sürümü **[GitHub Releases](https://github.com/evorift/rift/releases)** sayfasından indir:

| Dosya | Açıklama |
|-------|----------|
| `evorift_0.1.0_x64-setup.exe` | Kurulum sihirbazı (önerilen) |
| `evorift-portable-0.1.0-x64.zip` | Taşınabilir sürüm — kurulum gerektirmez |
| `SHA256SUMS` | İndirme doğrulaması için kontrol toplamları |

> **Sürüm:** Windows 10 / 11, 64-bit.

### Windows SmartScreen / antivirüs uyarısı

Ücretsiz kalabilmek için sürüm imzasızdır (pahalı bir EV sertifikası kullanılmaz).
Bu yüzden ilk açılışta Windows **SmartScreen** bir uyarı gösterebilir. Endişelenme:

1. **Ek bilgi** (*More info*) bağlantısına tıkla.
2. **Yine de çalıştır** (*Run anyway*) düğmesine bas.

İmzasız olduğu ve ağ paketlerine düşük seviyede dokunduğu için bazı antivirüs
programları evorift'i yanlışlıkla işaretleyebilir (false positive). İndirmeni
aşağıdaki **SHA256** adımıyla doğrularsan dosyanın değişmediğinden emin olabilirsin.

### İndirmeni doğrula (önerilen)

İndirdiğin dosyanın bozulmadığından emin olmak için PowerShell'de hash'ini hesapla
ve `SHA256SUMS` içindeki değerle karşılaştır:

```powershell
Get-FileHash .\evorift_0.1.0_x64-setup.exe -Algorithm SHA256
```

Çıkan değer `SHA256SUMS` dosyasındaki satırla aynı olmalı.

---

## Kullanım

1. **Kur veya zip'i çıkar.**
2. **Yönetici olarak çalıştır** — evorift.exe'ye sağ tıkla → *Yönetici olarak çalıştır*.
   (DPI motoru WinDivert sürücüsünü kullandığı için yönetici yetkisi gerekir.)
3. **Kara deliğe bas** — Boost / Bağlan düğmesiyle koruma tek tıkla açılır.
4. **Oyna.** Discord, Roblox ve oyunlar açılır; ping'in ve hızın korunur.

Arka planda küçük bir servis kurulur ve bilgisayar açıldığında otomatik çalışır;
böylece korumayı her seferinde elle başlatman gerekmez. Pencereyi kapatınca uygulama
sistem tepsisinde çalışmaya devam eder.

---

## Sık Sorulan Sorular

**Bu bir VPN mi?**
Hayır. Trafiğini bir sunucuya yönlendirmez, IP'ni değiştirmez, hız sınırı koymaz.
Yalnızca DPI engelini aşmak için gereken paketlere dokunur; gerisi tam hızında akar.

**Oyunlarda / anti-cheat ile güvenli mi?**
evorift oyun belleğine dokunmaz, hiçbir sürece enjeksiyon yapmaz; yalnızca ağ
paketlerini işler. Yine de **Vanguard (Valorant), EAC ve BattlEye** gibi çekirdek
seviyesinde çalışan anti-cheat sistemleri, WinDivert sürücüsü etkinken hassas
davranabilir. **Tavsiye:** anti-cheat korumalı bir oyunu açmadan önce korumayı
kapat. Kapatma tek tıkla yapılır ve tüm değişiklikler geri alınabilir.

**Antivirüsüm uyardı, güvenli mi?**
evorift imzasızdır ve ağ paketlerine düşük seviyede dokunur; bu iki özellik
birleşince bazı antivirüs programları yanlış alarm (false positive) verebilir.
Yapman gereken, dosyayı **GitHub Releases**'ten indirmek ve aşağıdaki **SHA256**
doğrulamasını yapmaktır — değer eşleşiyorsa dosya orijinaldir.

**Neden yönetici yetkisi istiyor?**
Paketleri ağ seviyesinde işleyebilmek için WinDivert sürücüsünü yüklemesi gerekir;
bu da Windows'ta yönetici yetkisi gerektirir.

**Verilerimi topluyor mu?**
Hayır. Telemetri yok, hesap yok, sunucuya veri gönderimi yok. Her şey kendi
bilgisayarında çalışır.

**Hangi internet sağlayıcılarında / sitelerinde çalışır?**
DPI tabanlı engelleme uygulayan sağlayıcılarda çalışır. Discord (ses dahil), Roblox,
oyun launcher'ları ve YouTube için tasarlanmıştır; engellenen başka alan adlarını da
kendi listene ekleyebilirsin.

**Ücretsiz mi?**
Evet, tamamen ücretsiz. Abonelik yok, ücretli sürüm yok.

---

## Diller

Arayüz dört dilde gelir: **Türkçe · English · Español · Русский**

---

## Destek

evorift işine yaradıysa geliştirmeyi desteklemeyi düşünebilirsin. Bağışlar tamamen
isteğe bağlıdır — evorift her zaman ücretsiz kalacak.

- **[GitHub Sponsors](https://github.com/sponsors/evorift)**
- Ko-fi

Hata bildirimi ve öneriler için **[GitHub Issues](https://github.com/evorift/rift/issues)** sayfasını kullanabilirsin.

---

## Lisans

Uygulama kodu **[MIT](LICENSE)** lisansı altındadır. Üçüncü taraf bileşenler için
bkz. **[THIRD-PARTY.md](THIRD-PARTY.md)** — WinDivert ayrı bir DLL olarak gelir ve
LGPLv3 lisanslıdır.

---

<details>
<summary><b>English summary</b></summary>

**evorift — Break through the block. No VPN.**

evorift is a free, lightweight Windows app that bypasses ISP **DPI** censorship on
**Discord, Roblox and YouTube** using surgical packet techniques (WinDivert). It is
**not a VPN** — it does not route your traffic through any server or change your IP.
It only touches the packets needed to bypass the block; everything else runs at full
speed, including Discord voice (UDP/QUIC).

- **Platform:** Windows 10 / 11, 64-bit. Requires **Administrator** (WinDivert driver).
- **Download:** [GitHub Releases](https://github.com/evorift/rift/releases) — NSIS
  installer or portable zip. Verify with `SHA256SUMS`
  (`Get-FileHash file -Algorithm SHA256`).
- **First launch:** the build is unsigned (free, no EV cert), so Windows SmartScreen
  may warn → click **More info → Run anyway**. Some antivirus may false-positive for
  the same reason — verify with `SHA256SUMS`.
- **Usage:** install → run as Administrator → press Boost/Connect. A small background
  service installs and starts at boot automatically.
- **Privacy:** no telemetry, no account. **Anti-cheat note:** evorift does not inject
  into games (network packets only), but it is safest to turn protection off before
  launching kernel-anti-cheat titles (Vanguard / EAC / BattlEye). One click to toggle.
- **Languages:** Turkish · English · Spanish · Russian.
- **Support:** [GitHub Sponsors](https://github.com/sponsors/evorift) / Ko-fi (optional).
- **License:** app code is [MIT](LICENSE); WinDivert ships as a separate LGPLv3 DLL
  (see [THIRD-PARTY.md](THIRD-PARTY.md)).

</details>
