//! WasenOffice Model Context Protocol (MCP) server & configuration.
//!
//! Exposes native document creation, inspection, and editing tools to external
//! AI assistants (Antigravity 2.0, Claude Desktop, Claude Code CLI, Cursor, Codex, etc.)
//! via JSON-RPC 2.0 over standard I/O (stdio).
//!
//! Running with `--mcp` launches this server directly in headless mode without
//! starting GUI windows, consuming minimal memory and saving AI token budgets.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::BTreeMap;
use std::io::{BufRead, Write};
use std::path::{Path, PathBuf};

use wasenoffice_ooxml::{
    parse_docx, parse_docx_file, write_docx_file, Block, Document, Paragraph, Run, Table, TableCell,
};

/// Get the best path to the WasenOffice binary for external MCP configs.
pub fn resolve_mcp_binary_path() -> String {
    if let Ok(exe) = std::env::current_exe() {
        let s = exe.to_string_lossy().to_string();
        if s.contains("WasenOffice.app") {
            return s;
        }
    }
    let app_path = PathBuf::from("/Applications/WasenOffice.app/Contents/MacOS/wasenoffice");
    if app_path.exists() {
        return app_path.to_string_lossy().to_string();
    }
    std::env::current_exe()
        .map(|p| p.to_string_lossy().to_string())
        .unwrap_or_else(|_| "/Applications/WasenOffice.app/Contents/MacOS/wasenoffice".into())
}

/// JSON snippet for manual client configuration.
pub fn mcp_config_json_snippet() -> String {
    let bin = resolve_mcp_binary_path();
    format!(
        r#"{{
  "mcpServers": {{
    "wasenoffice": {{
      "command": "{bin}",
      "args": ["--mcp"]
    }}
  }}
}}"#
    )
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct McpClientStatus {
    pub claude_code: bool,
    pub codex: bool,
    pub gemini: bool,
    pub antigravity: bool,
    pub opencode: bool,
    pub kiro: bool,
    pub claude_desktop: bool,
}

fn mcp_storage_path() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    PathBuf::from(home)
        .join(".wasenoffice")
        .join("mcp_clients.json")
}

fn load_saved_status() -> McpClientStatus {
    let path = mcp_storage_path();
    if let Ok(data) = std::fs::read_to_string(&path) {
        if let Ok(s) = serde_json::from_str::<McpClientStatus>(&data) {
            return s;
        }
    }
    // Default initial states based on environment detection
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
    let claude_cfg =
        PathBuf::from(&home).join("Library/Application Support/Claude/claude_desktop_config.json");
    let claude_desktop = if claude_cfg.exists() {
        std::fs::read_to_string(&claude_cfg)
            .map(|c| c.contains("\"wasenoffice\""))
            .unwrap_or(false)
    } else {
        false
    };

    let antigravity_dir = PathBuf::from(&home).join(".gemini/antigravity/mcp/wasenoffice");
    let antigravity = antigravity_dir.exists();

    McpClientStatus {
        claude_code: true,
        codex: true,
        gemini: true,
        antigravity,
        opencode: true,
        kiro: true,
        claude_desktop,
    }
}

fn save_status(status: &McpClientStatus) {
    let path = mcp_storage_path();
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string_pretty(status) {
        let _ = std::fs::write(path, json);
    }
}

/// Tauri command: retrieve status of external AI tool integrations.
#[tauri::command]
pub fn mcp_get_status() -> Result<McpClientStatus, String> {
    Ok(load_saved_status())
}

/// Tauri command: return standard JSON configuration snippet.
#[tauri::command]
pub fn mcp_get_config_snippet() -> Result<String, String> {
    Ok(mcp_config_json_snippet())
}

/// Configure Claude Desktop external MCP config.
fn configure_claude_desktop(enable: bool) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let claude_dir = PathBuf::from(home).join("Library/Application Support/Claude");
    let cfg_path = claude_dir.join("claude_desktop_config.json");

    if !claude_dir.exists() {
        std::fs::create_dir_all(&claude_dir).map_err(|e| e.to_string())?;
    }

    let mut root: Value = if cfg_path.exists() {
        let content = std::fs::read_to_string(&cfg_path).unwrap_or_else(|_| "{}".into());
        serde_json::from_str(&content).unwrap_or_else(|_| json!({}))
    } else {
        json!({})
    };

    if !root.is_object() {
        root = json!({});
    }

    let servers = root
        .as_object_mut()
        .unwrap()
        .entry("mcpServers")
        .or_insert_with(|| json!({}));

    if let Some(servers_obj) = servers.as_object_mut() {
        if enable {
            let bin = resolve_mcp_binary_path();
            servers_obj.insert(
                "wasenoffice".into(),
                json!({
                    "command": bin,
                    "args": ["--mcp"],
                    "env": {},
                    "type": "stdio"
                }),
            );
        } else {
            servers_obj.remove("wasenoffice");
        }
    }

    let formatted = serde_json::to_string_pretty(&root).map_err(|e| e.to_string())?;
    std::fs::write(&cfg_path, formatted).map_err(|e| e.to_string())?;
    Ok(())
}

/// Configure Antigravity 2.0 tool definitions directory.
fn configure_antigravity(enable: bool) -> Result<(), String> {
    let home = std::env::var("HOME").map_err(|e| e.to_string())?;
    let mcp_dir = PathBuf::from(home).join(".gemini/antigravity/mcp/wasenoffice");

    if enable {
        std::fs::create_dir_all(&mcp_dir).map_err(|e| e.to_string())?;

        let instructions = "# WasenOffice MCP Server\n\n\
            Use WasenOffice MCP tools to directly inspect, read, create, and edit office documents \
            (DOCX, XLSX, PPTX, PDF) with high token efficiency without generating ad-hoc python scripts.\n\n\
            Available tools:\n\
            - `read_document`: read formatted text, paragraphs, headings, and tables as markdown\n\
            - `create_document`: create new valid DOCX, XLSX, or PPTX documents\n\
            - `edit_document`: append paragraphs, headings, tables, or replace text\n\
            - `inspect_document`: inspect document outline, word count, and structure\n";
        let _ = std::fs::write(mcp_dir.join("instructions.md"), instructions);

        let read_schema = json!({
            "name": "read_document",
            "description": "Read and extract structured content (headings, paragraphs, tables) from a document without python scripts.",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": { "type": "string", "description": "Absolute path to the document file" },
                    "max_paragraphs": { "type": "integer", "description": "Optional maximum number of paragraphs to return" }
                },
                "required": ["filePath"]
            }
        });
        let _ = std::fs::write(
            mcp_dir.join("read_document.json"),
            serde_json::to_string_pretty(&read_schema).unwrap(),
        );

        let create_schema = json!({
            "name": "create_document",
            "description": "Create a new valid document file (DOCX, XLSX, PPTX) with optional title and content.",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": { "type": "string", "description": "Destination file path" },
                    "format": { "type": "string", "enum": ["docx", "xlsx", "pptx"], "description": "Document format" },
                    "title": { "type": "string", "description": "Optional title for the document" },
                    "content": { "type": "string", "description": "Initial text content" }
                },
                "required": ["filePath"]
            }
        });
        let _ = std::fs::write(
            mcp_dir.join("create_document.json"),
            serde_json::to_string_pretty(&create_schema).unwrap(),
        );

        let edit_schema = json!({
            "name": "edit_document",
            "description": "Edit or append content to a DOCX document (append paragraphs, headings, tables, or replace text).",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": { "type": "string", "description": "Path to document file to modify" },
                    "action": {
                        "type": "string",
                        "enum": ["append_paragraph", "append_heading", "append_table", "replace_text"],
                        "description": "Operation to perform"
                    },
                    "text": { "type": "string", "description": "Text content" },
                    "heading_level": { "type": "integer", "description": "Heading level 1-6" },
                    "table_data": {
                        "type": "array",
                        "items": { "type": "array", "items": { "type": "string" } },
                        "description": "2D array of rows and cell strings"
                    },
                    "search_text": { "type": "string", "description": "Text to search when action is replace_text" },
                    "replace_text": { "type": "string", "description": "Replacement text" }
                },
                "required": ["filePath", "action"]
            }
        });
        let _ = std::fs::write(
            mcp_dir.join("edit_document.json"),
            serde_json::to_string_pretty(&edit_schema).unwrap(),
        );

        let inspect_schema = json!({
            "name": "inspect_document",
            "description": "Get document outline, paragraph/table counts, styles, and page layout metadata.",
            "parameters": {
                "type": "object",
                "properties": {
                    "filePath": { "type": "string", "description": "Path to document file" }
                },
                "required": ["filePath"]
            }
        });
        let _ = std::fs::write(
            mcp_dir.join("inspect_document.json"),
            serde_json::to_string_pretty(&inspect_schema).unwrap(),
        );
    } else if mcp_dir.exists() {
        let _ = std::fs::remove_dir_all(&mcp_dir);
    }

    Ok(())
}

/// Tauri command: toggle an external AI tool integration.
#[tauri::command]
pub fn mcp_toggle_client(client_id: String, enabled: bool) -> Result<McpClientStatus, String> {
    let mut status = load_saved_status();
    match client_id.as_str() {
        "claude_desktop" => {
            configure_claude_desktop(enabled)?;
            status.claude_desktop = enabled;
        }
        "antigravity" => {
            configure_antigravity(enabled)?;
            status.antigravity = enabled;
        }
        "claude_code" => {
            status.claude_code = enabled;
        }
        "codex" => {
            status.codex = enabled;
        }
        "gemini" => {
            status.gemini = enabled;
        }
        "opencode" => {
            status.opencode = enabled;
        }
        "kiro" => {
            status.kiro = enabled;
        }
        _ => return Err(format!("Unknown client id {client_id}")),
    }

    save_status(&status);
    Ok(status)
}

// ---------------------------------------------------------------------------
// Standard MCP JSON-RPC 2.0 Engine over Stdio
// ---------------------------------------------------------------------------

fn tool_list() -> Value {
    json!({
        "tools": [
            {
                "name": "create_document",
                "description": "Create a new office document (DOCX, XLSX, PPTX) at the given path with optional title and content, saving tokens over generating python scripts.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute target file path (e.g. /path/to/report.docx)" },
                        "format": { "type": "string", "enum": ["docx", "xlsx", "pptx"], "description": "Document format" },
                        "title": { "type": "string", "description": "Optional title for the document (formatted as Heading 1)" },
                        "content": { "type": "string", "description": "Initial text content (paragraphs separated by double newlines)" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "read_document",
                "description": "Read and extract formatted text, paragraphs, headings, and tables from a DOCX or office document as clean markdown.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Absolute path to the document file" },
                        "max_paragraphs": { "type": "integer", "description": "Optional maximum number of paragraphs to extract" }
                    },
                    "required": ["path"]
                }
            },
            {
                "name": "edit_document",
                "description": "Edit or append content to a DOCX document (append paragraphs, headings, bullet points, tables, or replace text).",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to document file to modify" },
                        "action": {
                            "type": "string",
                            "enum": ["append_paragraph", "append_heading", "append_bullet", "append_table", "replace_text"],
                            "description": "Operation to perform"
                        },
                        "text": { "type": "string", "description": "Text content to add or append" },
                        "heading_level": { "type": "integer", "description": "Heading level 1-6 when action is append_heading" },
                        "table_data": {
                            "type": "array",
                            "items": { "type": "array", "items": { "type": "string" } },
                            "description": "2D array of rows and cell strings when action is append_table"
                        },
                        "search_text": { "type": "string", "description": "Text to search when action is replace_text" },
                        "replace_text": { "type": "string", "description": "Replacement text when action is replace_text" }
                    },
                    "required": ["path", "action"]
                }
            },
            {
                "name": "inspect_document",
                "description": "Get high-level document outline, paragraph and table count, word count, styles, and page layout dimensions.",
                "inputSchema": {
                    "type": "object",
                    "properties": {
                        "path": { "type": "string", "description": "Path to document file" }
                    },
                    "required": ["path"]
                }
            }
        ]
    })
}

fn execute_create(args: &Value) -> Result<String, String> {
    let path_str = args
        .get("path")
        .or_else(|| args.get("filePath"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;

    let path = Path::new(path_str);
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|s| s.to_ascii_lowercase())
        .or_else(|| {
            args.get("format")
                .and_then(|f| f.as_str())
                .map(|s| s.to_ascii_lowercase())
        })
        .unwrap_or_else(|| "docx".into());

    let title = args.get("title").and_then(|v| v.as_str());
    let content = args.get("content").and_then(|v| v.as_str());

    if ext == "xlsx" || ext == "pptx" {
        if let Some(bytes) = crate::untitled::template_bytes(&ext) {
            std::fs::write(path, bytes)
                .map_err(|e| format!("Failed to create {ext} document: {e}"))?;
            return Ok(format!(
                "Created blank {ext} document at {} ({} bytes)",
                path.display(),
                bytes.len()
            ));
        }
    }

    // DOCX creation with OOXML
    let mut body = Vec::new();

    if let Some(t) = title {
        body.push(Block::Paragraph(Paragraph {
            style: Some("Heading1".into()),
            runs: vec![Run {
                text: t.to_string(),
                bold: Some(true),
                size: Some(36), // 18pt
                ..Default::default()
            }],
            ..Default::default()
        }));
    }

    if let Some(c) = content {
        for para in c.split("\n\n") {
            let trimmed = para.trim();
            if !trimmed.is_empty() {
                body.push(Block::Paragraph(Paragraph {
                    runs: vec![Run {
                        text: trimmed.to_string(),
                        ..Default::default()
                    }],
                    ..Default::default()
                }));
            }
        }
    }

    if body.is_empty() {
        body.push(Block::Paragraph(Paragraph::default()));
    }

    let doc = Document {
        body,
        headers: Vec::new(),
        footers: Vec::new(),
        styles: BTreeMap::new(),
        numbering: BTreeMap::new(),
        doc_defaults: None,
        page: None,
        page_mar: None,
    };

    let template = crate::untitled::template_bytes("docx");
    write_docx_file(&doc, template, path).map_err(|e| format!("write_docx_file error: {e:?}"))?;

    let size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    Ok(format!(
        "Successfully created document at {} ({size} bytes)",
        path.display()
    ))
}

fn execute_read(args: &Value) -> Result<String, String> {
    let path_str = args
        .get("path")
        .or_else(|| args.get("filePath"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("File not found: {}", path.display()));
    }

    let max_paras = args
        .get("max_paragraphs")
        .and_then(|v| v.as_u64())
        .unwrap_or(500) as usize;

    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "docx" {
        let doc = parse_docx_file(path).map_err(|e| format!("parse_docx error: {e:?}"))?;

        let mut out = String::new();
        out.push_str(&format!(
            "# Document: {}\n\n",
            path.file_name().unwrap_or_default().to_string_lossy()
        ));

        let mut para_count = 0;
        let mut table_count = 0;

        for block in &doc.body {
            match block {
                Block::Paragraph(p) => {
                    para_count += 1;
                    if para_count > max_paras {
                        out.push_str("\n... [Remaining paragraphs truncated for brevity] ...\n");
                        break;
                    }

                    let style_name = p.style.as_deref().unwrap_or("");
                    let heading_prefix = if style_name.starts_with("Heading1") {
                        "# "
                    } else if style_name.starts_with("Heading2") {
                        "## "
                    } else if style_name.starts_with("Heading3") {
                        "### "
                    } else {
                        ""
                    };

                    out.push_str(heading_prefix);
                    for r in &p.runs {
                        let text = &r.text;
                        if r.bold == Some(true) && !heading_prefix.is_empty() {
                            out.push_str(text);
                        } else if r.bold == Some(true) {
                            out.push_str(&format!("**{text}**"));
                        } else if r.italic == Some(true) {
                            out.push_str(&format!("*{text}*"));
                        } else {
                            out.push_str(text);
                        }
                    }
                    out.push_str("\n\n");
                }
                Block::Table(tbl) => {
                    table_count += 1;
                    out.push_str(&format!("**[Table {table_count}]**\n\n"));
                    for (row_idx, row) in tbl.rows.iter().enumerate() {
                        let mut cells_text = Vec::new();
                        for cell in row {
                            let mut cell_content = String::new();
                            for blk in &cell.blocks {
                                if let Block::Paragraph(cp) = blk {
                                    for cr in &cp.runs {
                                        cell_content.push_str(&cr.text);
                                    }
                                }
                            }
                            cells_text.push(cell_content.trim().replace('|', "\\|"));
                        }
                        out.push_str("| ");
                        out.push_str(&cells_text.join(" | "));
                        out.push_str(" |\n");

                        if row_idx == 0 {
                            out.push_str("| ");
                            let dividers: Vec<String> =
                                cells_text.iter().map(|_| "---".to_string()).collect();
                            out.push_str(&dividers.join(" | "));
                            out.push_str(" |\n");
                        }
                    }
                    out.push('\n');
                }
            }
        }

        out.push_str(&format!(
            "\n---\n*Extracted {} paragraphs, {} tables.*",
            para_count, table_count
        ));
        Ok(out)
    } else {
        // Fallback simple read
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Failed to read text file: {e}"))?;
        Ok(content)
    }
}

fn execute_edit(args: &Value) -> Result<String, String> {
    let path_str = args
        .get("path")
        .or_else(|| args.get("filePath"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    let action = args
        .get("action")
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'action'".to_string())?;

    let bytes = std::fs::read(path).map_err(|e| format!("Read error: {e}"))?;
    let mut doc = parse_docx(&bytes).map_err(|e| format!("parse_docx error: {e:?}"))?;

    let result_msg = match action {
        "append_paragraph" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            doc.body.push(Block::Paragraph(Paragraph {
                runs: vec![Run {
                    text: text.to_string(),
                    ..Default::default()
                }],
                ..Default::default()
            }));
            format!(
                "Appended paragraph ({} chars) to {}",
                text.len(),
                path.display()
            )
        }
        "append_heading" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            let lvl = args
                .get("heading_level")
                .and_then(|v| v.as_u64())
                .unwrap_or(1);
            doc.body.push(Block::Paragraph(Paragraph {
                style: Some(format!("Heading{lvl}")),
                runs: vec![Run {
                    text: text.to_string(),
                    bold: Some(true),
                    size: Some(match lvl {
                        1 => 36,
                        2 => 30,
                        3 => 26,
                        _ => 24,
                    }),
                    ..Default::default()
                }],
                ..Default::default()
            }));
            format!("Appended Heading {lvl} ('{text}') to {}", path.display())
        }
        "append_bullet" => {
            let text = args.get("text").and_then(|v| v.as_str()).unwrap_or("");
            doc.body.push(Block::Paragraph(Paragraph {
                runs: vec![
                    Run {
                        text: "• ".to_string(),
                        bold: Some(true),
                        ..Default::default()
                    },
                    Run {
                        text: text.to_string(),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }));
            format!("Appended bullet item to {}", path.display())
        }
        "append_table" => {
            let rows_val = args
                .get("table_data")
                .and_then(|v| v.as_array())
                .ok_or_else(|| "append_table requires 'table_data' 2D array".to_string())?;

            let mut table_rows = Vec::new();
            for r in rows_val {
                if let Some(cells) = r.as_array() {
                    let mut row_cells = Vec::new();
                    for c in cells {
                        let text = c.as_str().unwrap_or("").to_string();
                        row_cells.push(TableCell {
                            blocks: vec![Block::Paragraph(Paragraph {
                                runs: vec![Run {
                                    text,
                                    ..Default::default()
                                }],
                                ..Default::default()
                            })],
                        });
                    }
                    table_rows.push(row_cells);
                }
            }

            let num_rows = table_rows.len();
            let num_cols = table_rows.first().map(|r| r.len()).unwrap_or(0);
            doc.body.push(Block::Table(Table {
                grid: vec![None; num_cols],
                rows: table_rows,
            }));
            format!(
                "Appended table ({num_rows}x{num_cols}) to {}",
                path.display()
            )
        }
        "replace_text" => {
            let search = args
                .get("search_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let replace = args
                .get("replace_text")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if search.is_empty() {
                return Err("replace_text requires non-empty 'search_text'".into());
            }

            let mut replace_count = 0;
            for blk in &mut doc.body {
                match blk {
                    Block::Paragraph(p) => {
                        for r in &mut p.runs {
                            if r.text.contains(search) {
                                r.text = r.text.replace(search, replace);
                                replace_count += 1;
                            }
                        }
                    }
                    Block::Table(t) => {
                        for row in &mut t.rows {
                            for cell in row {
                                for cblk in &mut cell.blocks {
                                    if let Block::Paragraph(cp) = cblk {
                                        for cr in &mut cp.runs {
                                            if cr.text.contains(search) {
                                                cr.text = cr.text.replace(search, replace);
                                                replace_count += 1;
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
            format!(
                "Replaced '{search}' with '{replace}' ({replace_count} run occurrences) in {}",
                path.display()
            )
        }
        _ => return Err(format!("Unsupported action '{action}'")),
    };

    write_docx_file(&doc, Some(&bytes), path)
        .map_err(|e| format!("write_docx_file error: {e:?}"))?;

    Ok(result_msg)
}

fn execute_inspect(args: &Value) -> Result<String, String> {
    let path_str = args
        .get("path")
        .or_else(|| args.get("filePath"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| "Missing required parameter 'path'".to_string())?;

    let path = Path::new(path_str);
    if !path.exists() {
        return Err(format!("File does not exist: {}", path.display()));
    }

    let meta = std::fs::metadata(path).map_err(|e| e.to_string())?;
    let doc = parse_docx_file(path).map_err(|e| format!("parse_docx error: {e:?}"))?;

    let mut para_count = 0;
    let mut table_count = 0;
    let mut word_count = 0;

    for blk in &doc.body {
        match blk {
            Block::Paragraph(p) => {
                para_count += 1;
                for r in &p.runs {
                    word_count += r.text.split_whitespace().count();
                }
            }
            Block::Table(t) => {
                table_count += 1;
                for row in &t.rows {
                    for cell in row {
                        for cb in &cell.blocks {
                            if let Block::Paragraph(cp) = cb {
                                for cr in &cp.runs {
                                    word_count += cr.text.split_whitespace().count();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    let page_info = if let Some(pg) = &doc.page {
        format!(
            "w: {:?} twips, h: {:?} twips, orient: {:?}",
            pg.w, pg.h, pg.orient
        )
    } else {
        "Default".into()
    };

    let report = json!({
        "file": path.display().to_string(),
        "fileSizeBytes": meta.len(),
        "paragraphs": para_count,
        "tables": table_count,
        "estimatedWords": word_count,
        "headersCount": doc.headers.len(),
        "footersCount": doc.footers.len(),
        "pageSetup": page_info,
        "stylesCount": doc.styles.len()
    });

    Ok(serde_json::to_string_pretty(&report).unwrap())
}

/// Run standard Model Context Protocol (MCP) server over stdin/stdout.
pub fn run_mcp_server() {
    eprintln!("[wasenoffice-mcp] Starting stdio server (protocol: 2024-11-05)...");

    let stdin = std::io::stdin();
    let stdout = std::io::stdout();
    let mut reader = stdin.lock();
    let mut line = String::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }

                let req: Value = match serde_json::from_str(trimmed) {
                    Ok(v) => v,
                    Err(e) => {
                        eprintln!("[wasenoffice-mcp] JSON parse error: {e}");
                        continue;
                    }
                };

                let id = req.get("id").cloned();
                let method = req.get("method").and_then(|m| m.as_str()).unwrap_or("");

                match method {
                    "initialize" => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {
                                "protocolVersion": "2024-11-05",
                                "capabilities": {
                                    "tools": {}
                                },
                                "serverInfo": {
                                    "name": "wasenoffice",
                                    "version": "0.1.0"
                                }
                            }
                        });
                        send_response(&stdout, &resp);
                    }
                    "notifications/initialized" => {
                        // Client acknowledgment notification: no response needed
                    }
                    "ping" => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": {}
                        });
                        send_response(&stdout, &resp);
                    }
                    "tools/list" => {
                        let resp = json!({
                            "jsonrpc": "2.0",
                            "id": id,
                            "result": tool_list()
                        });
                        send_response(&stdout, &resp);
                    }
                    "tools/call" => {
                        let params = req.get("params").cloned().unwrap_or(json!({}));
                        let name = params.get("name").and_then(|n| n.as_str()).unwrap_or("");
                        let arguments = params.get("arguments").cloned().unwrap_or(json!({}));

                        let tool_result = match name {
                            "create_document" => execute_create(&arguments),
                            "read_document" => execute_read(&arguments),
                            "edit_document" => execute_edit(&arguments),
                            "inspect_document" => execute_inspect(&arguments),
                            _ => Err(format!("Unknown tool '{name}'")),
                        };

                        let resp = match tool_result {
                            Ok(text) => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": text
                                        }
                                    ]
                                }
                            }),
                            Err(err) => json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "result": {
                                    "content": [
                                        {
                                            "type": "text",
                                            "text": format!("Error: {err}")
                                        }
                                    ],
                                    "isError": true
                                }
                            }),
                        };
                        send_response(&stdout, &resp);
                    }
                    _ => {
                        if id.is_some() {
                            let resp = json!({
                                "jsonrpc": "2.0",
                                "id": id,
                                "error": {
                                    "code": -32601,
                                    "message": format!("Method '{method}' not found")
                                }
                            });
                            send_response(&stdout, &resp);
                        }
                    }
                }
            }
            Err(e) => {
                eprintln!("[wasenoffice-mcp] Stdin read error: {e}");
                break;
            }
        }
    }

    eprintln!("[wasenoffice-mcp] Exiting stdio server.");
}

fn send_response(stdout: &std::io::Stdout, val: &Value) {
    if let Ok(json) = serde_json::to_string(val) {
        let mut handle = stdout.lock();
        let _ = writeln!(handle, "{json}");
        let _ = handle.flush();
    }
}
