// Shared offline fixtures. CI supplies uv and an existing host interpreter;
// these fixtures never download Python or packages.
use std::fs::{self, File};
use std::io::Write;
use std::path::Path;
use std::process::Command;

pub fn host_python() -> String {
    for name in ["python", "python3"] {
        if let Ok(output) = Command::new(name)
            .args(["-I", "-c", "import sys; print(sys.executable)"])
            .output()
        {
            if output.status.success() {
                return String::from_utf8(output.stdout).unwrap().trim().to_owned();
            }
        }
    }
    panic!("Python 3.10+ must be on PATH for offline runtime fixtures")
}

pub fn wheel(path: &Path, version: &str, requires_python: &str, dependencies: &str) {
    let mut writer = zip::ZipWriter::new(File::create(path).unwrap());
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let info = format!("yt_dlp-{version}.dist-info");
    let files = [
        ("yt_dlp/__init__.py".to_owned(), String::new()),
        ("yt_dlp/version.py".to_owned(), format!("__version__ = '{version}'\n")),
        (format!("{info}/METADATA"), format!("Metadata-Version: 2.3\nName: yt-dlp\nVersion: {version}\nRequires-Python: {requires_python}\n{dependencies}\n")),
        (format!("{info}/WHEEL"), "Wheel-Version: 1.0\nGenerator: fixture\nRoot-Is-Purelib: true\nTag: py3-none-any\n".to_owned()),
    ];
    let mut record = String::new();
    for (name, body) in files {
        writer.start_file(&name, options).unwrap();
        writer.write_all(body.as_bytes()).unwrap();
        record.push_str(&format!("{name},,\n"));
    }
    let name = format!("{info}/RECORD");
    record.push_str(&format!("{name},,\n"));
    writer.start_file(name, options).unwrap();
    writer.write_all(record.as_bytes()).unwrap();
    writer.finish().unwrap();
}

pub fn file_url(path: &Path) -> String {
    format!(
        "file:///{}",
        path.to_string_lossy()
            .replace('\\', "/")
            .trim_start_matches('/')
    )
}

pub fn hash(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    Sha256::digest(fs::read(path).unwrap())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
