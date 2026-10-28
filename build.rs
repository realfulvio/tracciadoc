fn main() {
    println!("cargo:rerun-if-changed=assets/ui/TracciaDoc-dark.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        winresource::WindowsResource::new()
            .set_icon("assets/ui/TracciaDoc-dark.ico")
            .set("FileDescription", "TracciaDoc - verifica documenti locali")
            .set("ProductName", "TracciaDoc")
            .set("LegalCopyright", "hcok - MIT")
            .compile()
            .expect("Risorsa icona Windows non compilata");
    }
}
