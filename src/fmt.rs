use colored::*;
use std::path::Path;

pub fn banner(title: &str) {
    println!("{}", "─".repeat(60).dimmed());
    println!("  {}", title.bold());
    println!("{}", "─".repeat(60).dimmed());
}

pub fn human_size(bytes: u64) -> String {
    let units = ["B", "KB", "MB", "GB", "TB"];
    let mut size = bytes as f64;
    let mut unit = units[0];
    for u in &units {
        unit = u;
        if size < 1024.0 {
            break;
        }
        size /= 1024.0;
    }
    format!("{:.2} {}", size, unit)
}

pub fn dir_size(path: &Path) -> std::io::Result<u64> {
    let mut total = 0u64;
    for entry in walkdir::WalkDir::new(path)
        .into_iter()
        .filter_map(|e| e.ok())
        .filter(|e| e.file_type().is_file())
    {
        if let Ok(meta) = entry.metadata() {
            total += meta.len();
        }
    }
    Ok(total)
}
