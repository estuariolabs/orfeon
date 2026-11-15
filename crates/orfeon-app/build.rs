use std::{env, fs, path::PathBuf, process::Command};

fn main() {
    if env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }

    let icon = PathBuf::from(env::var_os("CARGO_MANIFEST_DIR").unwrap())
        .join("../../assets/Orfeon.ico");
    let out = PathBuf::from(env::var_os("OUT_DIR").unwrap());
    let script = out.join("orfeon.rc");
    let icon_path = icon.to_str().expect("ruta Unicode del ícono").replace('\\', "/");
    fs::write(&script, format!("1 ICON \"{icon_path}\"\n")).expect("escribir el recurso de Orfeón");
    println!("cargo:rerun-if-changed={}", icon.display());

    let msvc = env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("msvc");
    let output = if msvc { out.join("orfeon.res") } else { out.join("orfeon.o") };
    let status = if msvc {
        let mut command = Command::new(msvc_rc());
        command.arg("/nologo").arg(format!("/fo{}", output.display())).arg(&script).status()
    } else {
        println!("cargo:rerun-if-env-changed=WINDRES");
        let compiler = env::var_os("WINDRES").unwrap_or_else(|| {
            let arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
            let arch = if arch == "x86" { "i686" } else { &arch };
            format!("{arch}-w64-mingw32-windres").into()
        });
        Command::new(compiler).arg(&script).arg(&output).status()
    };
    assert!(status.expect("ejecutar el compilador de recursos de Windows").success(), "compilar el ícono de Orfeón");
    println!("cargo:rustc-link-arg-bin=orfeon-app={}", output.display());
}

fn msvc_rc() -> PathBuf {
    // Cargo puede encontrar el enlazador con Visual Studio fuera del PATH.
    let sdk = env::var_os("ProgramFiles(x86)").map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\Program Files (x86)"))
        .join("Windows Kits").join("10").join("bin");
    let host = if env::var("HOST").unwrap().starts_with("aarch64") { "arm64" } else { "x64" };
    let mut versions: Vec<_> = fs::read_dir(sdk).into_iter().flatten()
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with("10."))
        .collect();
    versions.sort_unstable_by_key(|entry| entry.file_name());
    versions.into_iter().rev()
        .map(|entry| entry.path().join(host).join("rc.exe"))
        .find(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from("rc.exe"))
}
