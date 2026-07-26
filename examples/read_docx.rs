fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("usage: read_docx <path> [char_limit]");
    match wps_mcp_rust::docs::docx::read_text(&path) {
        Ok(r) => {
            let t = r.text;
            let chars = t.chars().count();
            eprintln!("OK chars={} bytes={}", chars, t.len());
            let limit: usize = std::env::args()
                .nth(2)
                .and_then(|s| s.parse().ok())
                .unwrap_or(30000);
            let preview: String = t.chars().take(limit).collect();
            print!("{}", preview);
            if chars > limit {
                eprintln!("\n[truncated: {} more chars]", chars - limit);
            }
        }
        Err(e) => {
            eprintln!("ERR: {e}");
            std::process::exit(1);
        }
    }
}
