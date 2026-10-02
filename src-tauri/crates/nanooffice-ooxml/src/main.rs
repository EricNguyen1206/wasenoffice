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
    match nanooffice_ooxml::parse_docx_file(&path) {
        Ok(doc) => match out_path {
            None => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&doc).expect("model serialize")
                );
                ExitCode::SUCCESS
            }
            Some(out) => match nanooffice_ooxml::write_docx_file(&doc, &out) {
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
