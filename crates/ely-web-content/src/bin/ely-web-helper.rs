//! CEF's helper processes inside an app bundle.

fn main() {
    let Some(code) = ely_web_content::subprocess() else {
        eprintln!("ely-web-helper: started without --type; CEF starts it");
        std::process::exit(2);
    };
    std::process::exit(code);
}
