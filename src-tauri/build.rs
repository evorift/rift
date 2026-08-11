fn main() {
    // P0-a (2026-08-13, net3/SOLUTION.md line 213 — net3's build.rs, same pattern):
    // WinDivert needs admin. Embedding `requireAdministrator` means the app self-elevates
    // (UAC) instead of silently failing to apply protection.
    //
    // NOTE — deviates from net3 in one way: net3 has a single binary, so its manifest
    // applies to exactly the process that needs elevation. This package builds two
    // binaries (`evorift` UI, `evorift-svc` service) from one build.rs, and Cargo doesn't
    // give this build script a way to target only one of them — the manifest embeds into
    // both. This is believed harmless: `evorift-svc` runs as a Windows Service under
    // LocalSystem via the Service Control Manager, which does not go through interactive
    // UAC/manifest-driven elevation at all, so an unused `requireAdministrator` marker on
    // that binary should be inert. Not verified live — flagging the assumption rather than
    // asserting it as fact.
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        use embed_manifest::manifest::ExecutionLevel;
        use embed_manifest::{embed_manifest, new_manifest};

        embed_manifest(
            new_manifest("Evorift.App").requested_execution_level(ExecutionLevel::RequireAdministrator),
        )
        .expect("unable to embed manifest");
    }
    tauri_build::build()
}
