fn main() {
    let config = slint_build::CompilerConfiguration::new().with_style("fluent".into());
    slint_build::compile_with_config("ui/app.slint", config).expect("Slint build failed");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico")
            .set("ProductName", "YT Downloader")
            .set("FileDescription", "YT Downloader")
            .set("CompanyName", "Gab8bit")
            .set("LegalCopyright", "© Gab8bit")
            .set_manifest_file("assets/app.manifest");
        res.compile().expect("Windows resource compile failed");
    }
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=assets/icon.ico");
}
