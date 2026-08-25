use std::env;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=wrapper.h");
    println!("cargo:rerun-if-changed=libxaac");

    let bundled = env::var_os("CARGO_FEATURE_BUNDLED").is_some();
    let prefer_static = env::var_os("CARGO_FEATURE_STATIC").is_some();
    let prefer_dynamic = env::var_os("CARGO_FEATURE_DYNAMIC").is_some();

    assert!(
        !(prefer_static && prefer_dynamic),
        "`static` and `dynamic` features are mutually exclusive"
    );

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("missing manifest dir"));
    let source_dir = manifest_dir.join("libxaac");
    let out_dir = PathBuf::from(env::var("OUT_DIR").expect("missing OUT_DIR"));

    if bundled {
        let processor = cmake_processor();
        let build_dir = out_dir.join("cmake-build");

        if build_dir.exists() {
            std::fs::remove_dir_all(&build_dir)
                .unwrap_or_else(|err| panic!("failed to clean {}: {err}", build_dir.display()));
        }

        run(Command::new("cmake")
            .arg("-S")
            .arg(&source_dir)
            .arg("-B")
            .arg(&build_dir)
            .arg(format!("-DCMAKE_SYSTEM_PROCESSOR={processor}"))
            .arg("-DCMAKE_POSITION_INDEPENDENT_CODE=ON"));
        let config_type = if env::var("PROFILE").unwrap_or_default() == "release" {
            "Release"
        } else {
            "Debug"
        };

        run(Command::new("cmake")
            .arg("--build")
            .arg(&build_dir)
            .arg("--config")
            .arg(config_type)
            .arg("--target")
            .arg("libxaacenc")
            .arg("libxaacdec"));

        let find_lib = |base_name: &str| -> (PathBuf, String) {
            let candidates = [
                format!("lib{base_name}.a"),
                format!("{base_name}.a"),
                format!("lib{base_name}.lib"),
                format!("{base_name}.lib"),
            ];
            for candidate in &candidates {
                if let Some(p) = find_file(&build_dir, candidate) {
                    let stem = p.file_stem().unwrap().to_str().unwrap().to_string();
                    return (p, stem);
                }
            }
            panic!(
                "failed to locate library for {base_name} under {}",
                build_dir.display()
            );
        };

        let (enc_path, enc_stem) = find_lib("xaacenc");
        let (dec_path, dec_stem) = find_lib("xaacdec");

        let enc_dir = enc_path
            .parent()
            .expect("static library missing parent directory");
        let dec_dir = dec_path
            .parent()
            .expect("static library missing parent directory");

        println!("cargo:rustc-link-search=native={}", enc_dir.display());
        if dec_dir != enc_dir {
            println!("cargo:rustc-link-search=native={}", dec_dir.display());
        }

        let is_msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
        let (enc_link, dec_link) = if is_msvc {
            (enc_stem, dec_stem)
        } else {
            (
                enc_stem
                    .strip_prefix("lib")
                    .unwrap_or(&enc_stem)
                    .to_string(),
                dec_stem
                    .strip_prefix("lib")
                    .unwrap_or(&dec_stem)
                    .to_string(),
            )
        };

        let link_kind = if bundled || prefer_static {
            "static"
        } else {
            "dylib"
        };

        println!("cargo:rustc-link-lib={link_kind}={enc_link}");
        println!("cargo:rustc-link-lib={link_kind}={dec_link}");
    }

    if bundled && prefer_dynamic {
        println!(
            "cargo:warning=`dynamic` requested with `bundled`, but vendored libxaac only builds static libraries; using static linking"
        );
    }

    if !bundled {
        let link_kind = if prefer_static { "static" } else { "dylib" };

        println!("cargo:rustc-link-lib={link_kind}=xaacenc");
        println!("cargo:rustc-link-lib={link_kind}=xaacdec");
    }

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap_or_default();
    if target_os != "windows" {
        println!("cargo:rustc-link-lib=m");
    }

    let bindings = bindgen::Builder::default()
        .header(manifest_dir.join("wrapper.h").display().to_string())
        .clang_arg(format!("-I{}", manifest_dir.display()))
        .clang_arg(format!("-I{}", source_dir.join("common").display()))
        .clang_arg(format!("-I{}", source_dir.join("decoder").display()))
        .clang_arg(format!(
            "-I{}",
            source_dir.join("decoder/drc_src").display()
        ))
        .clang_arg(format!("-I{}", source_dir.join("encoder").display()))
        .clang_arg(format!(
            "-I{}",
            source_dir.join("encoder/drc_src").display()
        ))
        .allowlist_function("ixheaace_(get_lib_id_strings|create|process|delete)")
        .allowlist_function("ixheaacd_(get_lib_id_strings|dec_api|dec_main)")
        .allowlist_function("ia_drc_dec_api")
        .allowlist_type("ixheaace_.*")
        .allowlist_type("ia_(mem_info_struct|lib_info_struct)")
        .allowlist_var("(IA|IXHEAACE|AOT|DEFAULT_MEM_ALIGN_8).*")
        .parse_callbacks(Box::new(bindgen::CargoCallbacks::new()))
        .generate()
        .expect("failed to generate bindings");

    bindings
        .write_to_file(out_dir.join("bindings.rs"))
        .expect("failed to write bindings");
}

fn cmake_processor() -> &'static str {
    match env::var("CARGO_CFG_TARGET_ARCH").as_deref() {
        Ok("x86_64") => "x86_64",
        Ok("x86") => "i686",
        Ok("aarch64") => "aarch64",
        Ok("arm") => "aarch32",
        Ok(other) => panic!("unsupported target arch: {other}"),
        Err(_) => panic!("missing CARGO_CFG_TARGET_ARCH"),
    }
}

fn find_file(dir: &Path, file_name: &str) -> Option<PathBuf> {
    if !dir.exists() {
        return None;
    }

    let entries = std::fs::read_dir(dir).ok()?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if let Some(found) = find_file(&path, file_name) {
                return Some(found);
            }
        } else if path.file_name().and_then(|name| name.to_str()) == Some(file_name) {
            return Some(path);
        }
    }

    None
}

fn run(command: &mut Command) {
    let status = command.status().expect("failed to spawn command");
    assert!(status.success(), "command failed with status {status}");
}
