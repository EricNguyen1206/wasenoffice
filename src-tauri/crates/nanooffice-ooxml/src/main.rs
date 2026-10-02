//! CLI: docx → document model JSON on stdout — the slice's runnable face.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: nanooffice-ooxml <file.docx>");
        return ExitCode::from(2);
    };
    match nanooffice_ooxml::parse_docx_file(&path) {
        Ok(doc) => {
            println!(
                "{}",
                serde_json::to_string_pretty(&doc).expect("model serialize")
            );
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
