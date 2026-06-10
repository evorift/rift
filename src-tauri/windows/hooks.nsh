; evorift NSIS installer hook'ları — son kullanıcıda EvoriftSvc servisini otomatik kur/başlat/kaldır.
; tauri.conf.json bundle.windows.nsis.installerHooks bunu bağlar. installMode=perMachine →
; $INSTDIR = %ProgramFiles%\evorift (yalnız admin yazabilir; WinDivert dll/sys binary-planting'e kapalı).
; binPath TIRNAKLI mutlak yol — boşluklu yolda (Program Files) tırnaksız sc = klasik unquoted-path escalation (P7 A11).
; NOT: NSIS'te '$' özeldir → kabuğa LİTERAL '$' geçirmek için '$$' yazılır (PowerShell $_ → $$_, $false → $$false).

!macro NSIS_HOOK_PREINSTALL
  ; Dosyalari ACMADAN ONCE calisan her seyi durdur ki binary'ler KILITLI kalmasin. Aksi halde
  ; ust-uste / guncelleme kurulumunda "Error opening file for writing: ...\warp\wireguard.exe" gibi
  ; hata cikar (WARP tuneli servisi wireguard.exe'yi, calisan winws/evorift kendi exe'lerini kilitler).
  nsExec::ExecToLog 'sc stop EvoriftSvc'
  nsExec::ExecToLog 'taskkill /f /im winws.exe'
  nsExec::ExecToLog 'taskkill /f /im evorift.exe'
  ; WARP WireGuard tunelini kaldir -> wireguard.exe kilidini birakir (varsa).
  nsExec::ExecToLog '"$INSTDIR\warp\wireguard.exe" /uninstalltunnelservice warp'
  ; WinDivert CEKIRDEK surucusunu durdur -> winws\WinDivert64.sys dosya kilidini birakir.
  ; winws.exe oldurulse bile surucu YUKLU kalir; .sys'i kernel tutar -> aksi halde
  ; "Error opening file for writing: ...\winws\WinDivert64.sys" cikar (uzerine kurulum).
  ; winws.exe oldugu icin acik handle yok -> stop temiz indirir, dosya serbest kalir.
  nsExec::ExecToLog 'sc stop WinDivert'
  nsExec::ExecToLog 'sc stop WinDivert1.4'
  ; kilitlerin (surucu unload + servis durdurma) tam serbest kalmasi icin bekle
  Sleep 3500
!macroend

!macro NSIS_HOOK_POSTINSTALL
  ; varsa eski servisi temizle (idempotent — yeniden kurulum/yükseltme)
  nsExec::ExecToLog 'sc stop EvoriftSvc'
  nsExec::ExecToLog 'sc delete EvoriftSvc'
  ; LocalSystem servisi olarak kur, boot'ta otomatik başlat (her zaman yetkili → bir daha UAC yok)
  nsExec::ExecToLog 'sc create EvoriftSvc binPath= "$INSTDIR\evorift-svc.exe" start= auto DisplayName= "evorift Koruma Servisi"'
  nsExec::ExecToLog 'sc description EvoriftSvc "evorift DPI-bypass + ag koruma motoru (LocalSystem)."'
  ; Çökme kurtarma (§6): servis crash-proof'landı (catch_unwind + poison-safe kilit + job object) →
  ; artık crash-loop riski yok. 3 yeniden başlatma denemesi, 5 sn arayla; sayaç 1 günde (86400 sn) sıfırlanır.
  nsExec::ExecToLog 'sc failure EvoriftSvc reset= 86400 actions= restart/5000/restart/5000/restart/5000'
  nsExec::ExecToLog 'sc start EvoriftSvc'
  ; Windows ile otomatik baslat: kontrol arayuzunu tray'e MINIMIZED ac (tum kullanicilar -> Startup kisayolu).
  ; Bypass servisi zaten boot'ta otomatik (UI olmadan da calisir); bu yalnizca pencereyi tray'e getirir.
  ; CreateShortcut bosluklu yolu (Program Files) sorunsuz isler -> tirnak kacis derdi yok.
  SetShellVarContext all
  CreateShortcut "$SMSTARTUP\evorift.lnk" "$INSTDIR\evorift.exe" "--minimized"
!macroend

!macro NSIS_HOOK_PREUNINSTALL
  ; kaldırırken servisi durdur + sil (yoksa exe kilitli kalır, silinemez)
  nsExec::ExecToLog 'sc stop EvoriftSvc'
  nsExec::ExecToLog 'sc delete EvoriftSvc'
  ; Motor B: WARP WireGuard tünelini kaldır (yetim WireGuardTunnel$$warp servisi kalmasın). §8.6
  nsExec::ExecToLog '"$INSTDIR\warp\wireguard.exe" /uninstalltunnelservice warp'
  ; Sistem değişikliklerini geri al (§8.6): DNS'i otomatiğe (DHCP) döndür + DoH/önbellek temizle.
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-NetAdapter -Physical | ForEach-Object { Set-DnsClientServerAddress -InterfaceIndex $$_.InterfaceIndex -ResetServerAddresses -Confirm:$$false }; Clear-DnsClientCache"'
  ; Per-app QoS hız-limiti politikalarını kaldır (evorift-*).
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-NetQosPolicy -ErrorAction SilentlyContinue | Where-Object { $$_.Name -like ''evorift-*'' } | Remove-NetQosPolicy -Confirm:$$false"'
  ; Per-app firewall engel kurallarını kaldır (evorift-block-*).
  nsExec::ExecToLog 'powershell -NoProfile -Command "Get-NetFirewallRule -ErrorAction SilentlyContinue | Where-Object { $$_.DisplayName -like ''evorift-block-*'' } | Remove-NetFirewallRule"'
  ; Otomatik baslat kisayolunu kaldir.
  SetShellVarContext all
  Delete "$SMSTARTUP\evorift.lnk"
!macroend
