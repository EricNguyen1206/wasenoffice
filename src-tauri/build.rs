fn main() {
    #[cfg(target_os = "macos")]
    {
        cc::Build::new()
            .file("src/touchbar_macos.m")
            .flag("-fobjc-arc")
            .compile("wasen_touchbar");
    }
    tauri_build::build();
}
