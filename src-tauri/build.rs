fn main() {
    // P0-a (2026-08-13, net3/SOLUTION.md line 213 — net3's build.rs, same underlying reason):
    // WinDivert needs admin. Embedding `requireAdministrator` means the app self-elevates
    // (UAC) instead of silently failing to apply protection.
    //
    // CORRECTED same day: the first attempt used a second, independent manifest-embed pass
    // (the `embed-manifest` crate, matching net3's build.rs literally). That failed to LINK
    // ("CVT1100: duplicate resource, type:MANIFEST") — tauri_build::build() already embeds
    // its own manifest (a bundled windows-app-manifest.xml, via its own resource-compile
    // step), and a second independent embed collides with it at the same resource slot.
    // `cargo check` / `cargo test --lib` / `cargo clippy` do not perform this final link
    // step, so none of them caught it — only `cargo build` does. This was an incomplete
    // verification claim on the first pass, not a case where all checks genuinely passed.
    //
    // Fixed by using tauri_build's own manifest hook (WindowsAttributes::app_manifest)
    // instead of a second embed pass, supplying a manifest that keeps Tauri's default
    // Common-Controls dependency (see tauri-build's windows-app-manifest.xml) and adds
    // requireAdministrator on top of it.
    //
    // Same caveat as before: this package builds two binaries (`evorift` UI, `evorift-svc`
    // service) from one build.rs; Cargo gives no way to scope this to only the UI binary,
    // so both get it. Believed harmless for evorift-svc (runs as a Windows Service under
    // LocalSystem via the Service Control Manager, which doesn't go through interactive
    // UAC/manifest-driven elevation at all) but not verified live.
    let windows = tauri_build::WindowsAttributes::new().app_manifest(
        r#"<assembly xmlns="urn:schemas-microsoft-com:asm.v1" manifestVersion="1.0">
  <dependency>
    <dependentAssembly>
      <assemblyIdentity
        type="win32"
        name="Microsoft.Windows.Common-Controls"
        version="6.0.0.0"
        processorArchitecture="*"
        publicKeyToken="6595b64144ccf1df"
        language="*"
      />
    </dependentAssembly>
  </dependency>
  <trustInfo xmlns="urn:schemas-microsoft-com:asm.v3">
    <security>
      <requestedPrivileges>
        <requestedExecutionLevel level="requireAdministrator" uiAccess="false" />
      </requestedPrivileges>
    </security>
  </trustInfo>
</assembly>"#,
    );
    tauri_build::try_build(tauri_build::Attributes::new().windows_attributes(windows))
        .expect("failed to run tauri-build");
}
