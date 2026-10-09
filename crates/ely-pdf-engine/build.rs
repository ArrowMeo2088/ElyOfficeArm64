//! Builds the MuPDF shim into the worker binary only.

use std::path::PathBuf;

fn main() {
    let root = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("set by cargo")).join("../..");
    let mupdf = std::env::var_os("ELY_MUPDF_DIR").map_or_else(|| root.join("engines/mupdf-build"), PathBuf::from);
    // One stamp for client and worker; stale workers refused.
    let mut stamp: u64 = 0xcbf2_9ce4_8422_2325;
    for source in [
        "shim/ely_mupdf.c",
        "shim/ely_pages.c",
        "shim/ely_insert.c",
        "shim/ely_text.c",
        "shim/ely_write.c",
        "shim/ely_place.c",
        "shim/ely_annot.c",
        "shim/ely_form.c",
        "shim/ely_ocr.c",
        "shim/ely_outline.c",
        "shim/ely_meta.c",
        "shim/ely_redact.c",
        "shim/ely_alt.c",
        "shim/ely_clean.c",
        "shim/ely_secure.c",
        "shim/ely_sign.c",
        "shim/ely_output.c",
        "shim/ely_shim.h",
        "src/proto.rs",
        "src/bin/ely-pdf-worker/main.rs",
        "src/bin/ely-pdf-worker/ffi.rs",
        "src/bin/ely-pdf-worker/edit.rs",
        "src/bin/ely-pdf-worker/content.rs",
        "src/edit.rs",
        "src/content.rs",
        "src/marks.rs",
        "src/bin/ely-pdf-worker/marks.rs",
        "src/bin/ely-pdf-worker/ocr.rs",
        "src/bin/ely-pdf-worker/structure.rs",
        "src/bin/ely-pdf-worker/redact.rs",
        "src/redact.rs",
        "src/structure.rs",
        "src/ocr.rs",
        "src/bin/ely-pdf-worker/secure.rs",
        "src/secure.rs",
        "src/bin/ely-pdf-worker/sign.rs",
        "src/sign.rs",
        "src/bin/ely-pdf-worker/output.rs",
        "src/output.rs",
        "../ely-ipc/src/lib.rs",
    ] {
        println!("cargo:rerun-if-changed={source}");
        for byte in std::fs::read(source).expect("a source file") {
            stamp = (stamp ^ u64::from(byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    println!("cargo:rustc-env=ELY_PDF_BUILD={stamp:016x}");
    println!("cargo:rerun-if-env-changed=ELY_MUPDF_DIR");
    for lib in ["libmupdf.a", "libmupdf-third.a", "libmupdf-pkcs7.a"] {
        println!("cargo:rerun-if-changed={}", mupdf.join(lib).display());
        if !mupdf.join(lib).is_file() {
            panic!("no {lib} in {} (run scripts/build-mupdf.sh or set ELY_MUPDF_DIR)", mupdf.display());
        }
    }
    cc::Build::new()
        .file("shim/ely_mupdf.c")
        .file("shim/ely_pages.c")
        .file("shim/ely_insert.c")
        .file("shim/ely_text.c")
        .file("shim/ely_write.c")
        .file("shim/ely_place.c")
        .file("shim/ely_annot.c")
        .file("shim/ely_form.c")
        .file("shim/ely_ocr.c")
        .file("shim/ely_outline.c")
        .file("shim/ely_meta.c")
        .file("shim/ely_redact.c")
        .file("shim/ely_alt.c")
        .file("shim/ely_clean.c")
        .file("shim/ely_secure.c")
        .file("shim/ely_sign.c")
        .file("shim/ely_output.c")
        .include(root.join("reference/mupdf/include"))
        .include(openssl("includedir"))
        .warnings(true)
        .cargo_metadata(false)
        .compile("ely_mupdf");
    let out = PathBuf::from(std::env::var("OUT_DIR").expect("set by cargo"));
    let bin = "ely-pdf-worker";
    println!("cargo:rustc-link-arg-bin={bin}={}", out.join("libely_mupdf.a").display());
    println!("cargo:rustc-link-arg-bin={bin}={}", mupdf.join("libmupdf-pkcs7.a").display());
    println!("cargo:rustc-link-arg-bin={bin}={}", mupdf.join("libmupdf.a").display());
    println!("cargo:rustc-link-arg-bin={bin}={}", mupdf.join("libmupdf-third.a").display());
    // Signing and checking need OpenSSL, linked in whole.
    println!("cargo:rustc-link-arg-bin={bin}={}", PathBuf::from(openssl("libdir")).join("libcrypto.a").display());
    println!("cargo:rustc-link-arg-bin={bin}=-lm");
    // Tesseract inside MuPDF is C++.
    let cxx = if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") { "-lc++" } else { "-lstdc++" };
    println!("cargo:rustc-link-arg-bin={bin}={cxx}");
}

/// An OpenSSL directory, from pkg-config.
fn openssl(variable: &str) -> String {
    let out = std::process::Command::new("pkg-config")
        .args([&format!("--variable={variable}"), "libcrypto"])
        .output()
        .expect("pkg-config, to find OpenSSL");
    assert!(out.status.success(), "pkg-config knows no libcrypto");
    String::from_utf8(out.stdout).expect("a UTF-8 path").trim().to_string()
}
