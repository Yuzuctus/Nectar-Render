//! Sous Windows : icône et informations de l'exécutable.

fn main() {
    println!("cargo:rerun-if-changed=../../assets/icon/nectar-render.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../assets/icon/nectar-render.ico");
        res.set("ProductName", "Nectar Render");
        res.set("FileDescription", "Nectar Render, atelier de mise en page");
        res.set("CompanyName", "Yuzuctus");
        if let Err(error) = res.compile() {
            println!("cargo:warning=icône Windows non intégrée : {error}");
        }
    }
}
