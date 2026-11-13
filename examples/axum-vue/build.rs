//! Table des pages (routage par fichiers), depuis `web/src/pages/`.

fn main() {
    if let Err(e) = moltn_pages::build::scan_project() {
        panic!("{e}");
    }
}
