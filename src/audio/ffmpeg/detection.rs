use std::env;
use std::path::PathBuf;

pub(super) fn detect_binary(name: &str) -> Option<PathBuf> {
    let executable = executable_name(name);
    let env_key = format!("{}_PATH", name.to_ascii_uppercase());

    env::var_os(&env_key)
        .map(PathBuf::from)
        .filter(|path| path.is_file())
        .or_else(|| search_path(&executable))
        .or_else(|| search_common_locations(&executable))
        .or_else(|| search_windows_winget(&executable))
}

fn executable_name(name: &str) -> String {
    if cfg!(windows) {
        format!("{name}.exe")
    } else {
        name.to_owned()
    }
}

fn search_path(executable: &str) -> Option<PathBuf> {
    env::var_os("PATH").and_then(|paths| {
        env::split_paths(&paths)
            .map(|entry| entry.join(executable)) // sentinel-disable path-join-sin-canonicalize — executable es el nombre del binario (ffmpeg/ffprobe), no una ruta
            .find(|candidate| candidate.is_file())
    })
}

fn search_common_locations(executable: &str) -> Option<PathBuf> {
    let candidates = if cfg!(windows) {
        windows_candidates(executable)
    } else {
        vec![
            PathBuf::from(format!("/usr/bin/{executable}")),
            PathBuf::from(format!("/usr/local/bin/{executable}")),
            PathBuf::from(format!("/snap/bin/{executable}")),
            PathBuf::from(format!("/opt/homebrew/bin/{executable}")),
        ]
    };

    candidates.into_iter().find(|candidate| candidate.is_file())
}

fn windows_candidates(executable: &str) -> Vec<PathBuf> {
    let local_app_data = env::var_os("LOCALAPPDATA").map(PathBuf::from).or_else(|| {
        env::var_os("USERPROFILE")
            .map(PathBuf::from)
            .map(|path| path.join("AppData").join("Local"))
    });
    let user_profile = env::var_os("USERPROFILE").map(PathBuf::from);
    let mut candidates = vec![
        PathBuf::from(format!(r"C:\ffmpeg\bin\{executable}")),
        PathBuf::from(format!(r"C:\Program Files\ffmpeg\bin\{executable}")),
        PathBuf::from(format!(r"C:\Program Files (x86)\ffmpeg\bin\{executable}")),
        PathBuf::from(format!(r"C:\tools\ffmpeg\bin\{executable}")),
    ];

    if let Some(local_app_data) = local_app_data {
        candidates.push(local_app_data.join("ffmpeg").join("bin").join(executable)); // sentinel-disable path-join-sin-canonicalize — executable es el nombre del binario (ffmpeg/ffprobe), no una ruta
    }

    if let Some(user_profile) = user_profile {
        candidates.push(user_profile.join("ffmpeg").join("bin").join(executable)); // sentinel-disable path-join-sin-canonicalize — executable es el nombre del binario (ffmpeg/ffprobe), no una ruta
        candidates.push(user_profile.join("scoop").join("shims").join(executable)); // sentinel-disable path-join-sin-canonicalize — executable es el nombre del binario (ffmpeg/ffprobe), no una ruta
    }

    candidates
}

fn search_windows_winget(executable: &str) -> Option<PathBuf> {
    if !cfg!(windows) {
        return None;
    }

    let packages_root = env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .map(|path| path.join("Microsoft").join("WinGet").join("Packages"))?;

    let mut matches = Vec::new();

    for vendor_entry in std::fs::read_dir(packages_root).ok()? {
        let vendor_entry = vendor_entry.ok()?;
        let vendor_name = vendor_entry.file_name().to_string_lossy().to_string();
        if !vendor_name.starts_with("Gyan.FFmpeg") {
            continue;
        }

        for package_entry in std::fs::read_dir(vendor_entry.path()).ok()? {
            let package_entry = package_entry.ok()?;
            let candidate = package_entry.path().join("bin").join(executable); // sentinel-disable path-join-sin-canonicalize — executable es el nombre del binario (ffmpeg/ffprobe), no una ruta
            if candidate.is_file() {
                matches.push(candidate);
            }
        }
    }

    matches.sort();
    matches.pop()
}
