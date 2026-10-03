//! CLI: docx → document model JSON on stdout (default), or a round-trip
//! `write` face — `nanooffice-ooxml <in.docx> <out.docx>` parses the input
//! and serializes the model back to `out.docx`.

use std::process::ExitCode;

fn main() -> ExitCode {
    let Some(path) = std::env::args().nth(1) else {
        eprintln!("usage: nanooffice-ooxml <file.docx> [out.docx]");
        return ExitCode::from(2);
    };
    let out_path = std::env::args().nth(2);
    let bytes = match std::fs::read(&path) {
        Ok(b) => b,
        Err(e) => {
            eprintln!("error: read {path}: {e}");
            return ExitCode::from(1);
        }
    };
    match nanooffice_ooxml::parse_docx(&bytes) {
        Ok(doc) => match out_path {
            None => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).expect("model serialize")
                );
                ExitCode::SUCCESS
            }
            // round-trip face passes the source back in: media parts must
            // survive byte-exact (T-A26)
            Some(out) => match nanooffice_ooxml::write_docx_file(&doc, Some(&bytes), &out) {
                Ok(()) => ExitCode::SUCCESS,
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::from(1)
                }
            },
        },
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::from(1)
        }
    }
}
