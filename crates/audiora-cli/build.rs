//! Windows file properties (Details tab): the executable says it is Audiora by Roujan Digital Agency.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../app/src-tauri/icons/icon.ico")
            .set("CompanyName", "Roujan Digital Agency")
            .set("ProductName", "Audiora")
            .set("FileDescription", "Audiora command-line tools")
            .set("LegalCopyright", "© Roujan Digital Agency");
        if let Err(e) = res.compile() {
            println!("cargo:warning=file properties not embedded: {e}");
        }
    }
}
