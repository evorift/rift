//! Sistem/ağ tweak'leri (docs/04). `value`: bool tweak'lerde "on"/"off"; autotuning normal/disabled;
//! congestion cubic/ctcp/bbr2; mtu sayı. Beyaz-liste ipc::validate_tweak ile zaten doğrulandı.
//! Enumerasyon gereken (per-adapter/GUID) tweak'ler tek `powershell -Command` içinde çalışır.

use crate::sys::{run_os, svec};

pub fn run_tweak(key: &str, value: &str) -> Result<(), String> {
    let on = value == "on";
    let ps_nagle_on = r#"Get-NetAdapter | ForEach-Object { $p = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($_.InterfaceGuid)"; if (Test-Path $p) { Set-ItemProperty -Path $p -Name TcpAckFrequency -Value 1 -Type DWord; Set-ItemProperty -Path $p -Name TCPNoDelay -Value 1 -Type DWord } }"#;
    let ps_nagle_off = r#"Get-NetAdapter | ForEach-Object { $p = "HKLM:\SYSTEM\CurrentControlSet\Services\Tcpip\Parameters\Interfaces\$($_.InterfaceGuid)"; if (Test-Path $p) { Remove-ItemProperty -Path $p -Name TcpAckFrequency -ErrorAction SilentlyContinue; Remove-ItemProperty -Path $p -Name TCPNoDelay -ErrorAction SilentlyContinue } }"#;
    let ps_nicpower = |v: i32| -> String {
        format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter | ForEach-Object {{ Set-NetAdapterAdvancedProperty -Name $_.Name -RegistryKeyword '*PowerSavingMode' -RegistryValue {v}; Set-NetAdapterAdvancedProperty -Name $_.Name -RegistryKeyword '*EEE' -RegistryValue {v} }}")
    };
    let ps_rss = |verb: &str| format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter -Physical | ForEach-Object {{ {verb}-NetAdapterRss -Name $_.Name -Confirm:$false }}");
    let ps_offload = |verb: &str| format!("$ErrorActionPreference='SilentlyContinue'; Get-NetAdapter -Physical | Where-Object {{ $_.Status -eq 'Up' }} | ForEach-Object {{ {verb}-NetAdapterLso -Name $_.Name -IPv4 -IPv6; {verb}-NetAdapterChecksumOffload -Name $_.Name }}");
    let ps_mtu = format!("Get-NetAdapter -Physical | Where-Object {{ $_.Status -eq 'Up' }} | ForEach-Object {{ netsh interface ipv4 set subinterface \"$($_.Name)\" mtu={value} store=persistent }}");

    let cmds: Vec<(&str, Vec<String>)> = match key {
        "heuristics" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "heuristics", if on { "disabled" } else { "enabled" }]),
        )],
        "throttleIdx" => vec![(
            "reg",
            svec(&[
                "add", r"HKLM\SYSTEM\CurrentControlSet\Services\Multimedia\SystemProfile",
                "/v", "NetworkThrottlingIndex", "/t", "REG_DWORD",
                "/d", if on { "4294967295" } else { "10" }, "/f",
            ]),
        )],
        "highPerf" => vec![(
            "powercfg",
            svec(&[
                "/setactive",
                if on { "8c5e7fda-e8bf-4a96-9a85-a6e23a8c635c" } else { "381b4222-f694-41f0-9685-ff5bb260df2e" },
            ]),
        )],
        "autotuning" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "global", &format!("autotuninglevel={value}")]),
        )],
        "congestion" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "supplemental", "custom", &format!("congestionprovider={value}")]),
        )],
        "rsc" => vec![(
            "netsh",
            svec(&["int", "tcp", "set", "global", if on { "rsc=enabled" } else { "rsc=disabled" }]),
        )],
        "nagle" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", if on { ps_nagle_on } else { ps_nagle_off }]),
        )],
        "nicPower" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_nicpower(if on { 0 } else { 1 })]),
        )],
        "rss" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_rss(if on { "Enable" } else { "Disable" })]),
        )],
        "offload" => vec![(
            "powershell",
            svec(&["-NoProfile", "-Command", &ps_offload(if on { "Enable" } else { "Disable" })]),
        )],
        "mtu" => vec![("powershell", svec(&["-NoProfile", "-Command", &ps_mtu]))],
        other => return Err(format!("bilinmeyen tweak: {other}")),
    };

    for (prog, args) in &cmds {
        let argrefs: Vec<&str> = args.iter().map(String::as_str).collect();
        run_os(prog, &argrefs)?;
    }
    Ok(())
}
