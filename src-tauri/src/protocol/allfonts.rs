//! AllFonts.js generator (T-A11, Track A rung A1) — replaces the file the
//! installed ONLYOFFICE app enumerates into its data dir.
//!
//! Clean-room provenance (harness/rules/clean-room.md — the replaced module is
//! the *generation* of AllFonts.js inside the ONLYOFFICE core, whose source was
//! never read):
//! - file layout + selection-bin byte format: `oracle-obs` — byte-exact decode
//!   of the app-generated artifact (clean parse of all 557 records, zero byte
//!   drift; recorded in docs/reference.map.md § AllFonts generator);
//! - consumer contract: sdkjs `common/Drawings/Externals.js` (`__fonts_files`
//!   → `CFontFileLoader`s, `__fonts_infos` → `CFontInfo`, style field = face
//!   index inside the file) and `common/libfont/{map,character}.js` (selection
//!   bin via `CFontSelect::fromStream`, `__fonts_ranges` triplets with a
//!   tolerant early-return when absent) — read under mit-purity: sdkjs is a
//!   consumer, not the module being replaced;
//! - font metadata: OpenType spec (name / OS/2 / head / post / cmap tables).
//!
//! One home for both consumers: filesrv serves the engine a path-rewritten
//! copy of the same string `bridge::save` writes to disk for x2t
//! (`<m_sAllFontsPath>`), so engine indexes and x2t indexes are identical by
//! construction.

use std::collections::{BTreeMap, HashMap};
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// A font face parsed from disk (before the per-(family, slot) selection).
struct Face {
    /// index into the walk-order file list (remapped to the final list later)
    file: u16,
    face: u16,
    family: String,
    /// additional family name from the Windows name table, when it differs
    alt: Option<String>,
    italic: bool,
    bold: bool,
    fixed: bool,
    weight: u16,
    width: u16,
    fam_class: i16,
    /// 1 = glyf (TrueType), 2 = CFF — observed artifact values {1, 2}
    format: i16,
    /// OS/2 metrics scaled to 1000 units per em (truncated toward zero)
    avg_char_width: i16,
    ascent: i16,
    descent: i16,
    line_gap: i16,
    x_height: i16,
    cap_height: i16,
    /// OS/2 fsType (embedding permissions) — the selection-bin `m_usType`
    fs_type: u16,
    panose: [u8; 10],
    unicode_range: [u32; 4],
    codepage_range: [u32; 2],
    /// covered codepoint intervals (cmap format 4/12), for `__fonts_ranges`
    coverage: Vec<(u32, u32)>,
}

/// One enumeration: everything the two AllFonts.js flavors are built from.
pub(crate) struct AllFonts {
    files: Vec<PathBuf>,
    /// family names, sorted case-insensitively — row i of `infos` is family i
    fam_names: Vec<String>,
    /// per family: `[rFile, rFace, iFile, iFace, bFile, bFace, biFile, biFace]`
    /// — file = index into `files`, face = face index inside the file, -1 = none
    infos: Vec<[i64; 8]>,
    /// winning faces in (file, face) order — the selection-bin records
    faces: Vec<Face>,
    /// `(start, end, info_index)` unicode fallback triplets
    ranges: Vec<(u32, u32, u32)>,
}

static GENERATED: OnceLock<Option<AllFonts>> = OnceLock::new();
static WRITTEN: OnceLock<Option<PathBuf>> = OnceLock::new();

/// The enumeration, generated once per run.
pub(crate) fn generated() -> &'static Option<AllFonts> {
    GENERATED.get_or_init(|| {
        let fonts = AllFonts::enumerate();
        if fonts.is_empty() {
            log::warn!(
                "AllFonts: no fonts enumerated from the font roots — editor will run with fallback fonts"
            );
            None
        } else {
            Some(fonts)
        }
    })
}

/// Write the raw AllFonts.js (disk paths) to a stable temp location once per
/// run and return its path — the single copy `<m_sAllFontsPath>` and
/// `<m_sFontDir>` (bridge::save) point at.
pub(crate) fn ensure_written() -> Option<PathBuf> {
    WRITTEN
        .get_or_init(|| {
            let fonts = generated().as_ref()?;
            let dir = std::env::temp_dir().join("nanooffice-allfonts");
            std::fs::create_dir_all(&dir).ok()?;
            let path = dir.join("AllFonts.js");
            std::fs::write(&path, fonts.js_raw()).ok()?;
            log::info!(
                "AllFonts: generated {} files / {} faces / {} families / {} ranges → {}",
                fonts.files.len(),
                fonts.faces.len(),
                fonts.infos.len(),
                fonts.ranges.len(),
                path.display()
            );
            Some(path)
        })
        .clone()
}

impl AllFonts {
    fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The AllFonts.js text with RAW disk paths (the x2t flavor). The engine
    /// flavor is produced by `rewrite_font_paths` in protocol/mod.rs, which
    /// rewrites the quoted file paths to loopback URLs and leaves the
    /// base64-embedded selection-bin paths alone — same as with the
    /// app-generated file before this task.
    pub(crate) fn js_raw(&self) -> String {
        let mut js = String::with_capacity(512 * 1024);
        js.push('\u{feff}');
        js.push_str("window[\"__all_fonts_js_version__\"] = 2;\n\n");
        js.push_str("window[\"__fonts_files\"] = [\n");
        for (i, f) in self.files.iter().enumerate() {
            js.push_str(&format!(
                "\"{}\"{}\n",
                f.to_string_lossy(),
                if i + 1 < self.files.len() { "," } else { "" }
            ));
        }
        js.push_str("];\n\n");
        js.push_str("window[\"__fonts_infos\"] = [\n");
        for (i, (name, slots)) in self.fam_names.iter().zip(&self.infos).enumerate() {
            let comma = if i + 1 < self.infos.len() { "," } else { "" };
            let vals = slots
                .iter()
                .map(|v| v.to_string())
                .collect::<Vec<_>>()
                .join(",");
            js.push_str(&format!("[\"{}\",{vals}]{comma}\n", js_escape_name(name)));
        }
        js.push_str("];\n\n");
        js.push_str("window[\"__fonts_ranges\"] = [\n");
        for (i, (s, e, f)) in self.ranges.iter().enumerate() {
            js.push_str(&format!(
                "{s},{e},{f}{}",
                if i + 1 < self.ranges.len() { "," } else { "" }
            ));
        }
        js.push_str("\n];\n\n");
        js.push_str(&format!(
            "window[\"g_fonts_selection_bin\"] = \"{}\";\n",
            base64(&self.selection_bin())
        ));
        js
    }

    /// Selection binary: `count` then per-face records in the v2 layout the
    /// engine parses (`CFontSelect::fromStream`); recordLen-prefixed so the
    /// consumer can skip over anything it does not know.
    fn selection_bin(&self) -> Vec<u8> {
        let mut out = Vec::with_capacity(96 * self.faces.len() + 4);
        out.extend_from_slice(&(self.faces.len() as u32).to_le_bytes());
        for f in &self.faces {
            let mut rec = Vec::with_capacity(96);
            let name = f.family.as_bytes();
            rec.extend_from_slice(&(name.len() as u32).to_le_bytes());
            rec.extend_from_slice(name);
            match &f.alt {
                Some(a) => {
                    let a = a.as_bytes();
                    rec.extend_from_slice(&1u32.to_le_bytes());
                    rec.extend_from_slice(&(a.len() as u32).to_le_bytes());
                    rec.extend_from_slice(a);
                }
                None => rec.extend_from_slice(&0u32.to_le_bytes()),
            }
            let path = self.files[f.file as usize].to_string_lossy();
            rec.extend_from_slice(&(path.len() as u32).to_le_bytes());
            rec.extend_from_slice(path.as_bytes());
            rec.extend_from_slice(&(u32::from(f.face)).to_le_bytes());
            rec.extend_from_slice(&(u32::from(f.italic)).to_le_bytes());
            rec.extend_from_slice(&(u32::from(f.bold)).to_le_bytes());
            rec.extend_from_slice(&(u32::from(f.fixed)).to_le_bytes());
            rec.extend_from_slice(&10u32.to_le_bytes());
            rec.extend_from_slice(&f.panose);
            for v in f.unicode_range {
                rec.extend_from_slice(&v.to_le_bytes());
            }
            for v in f.codepage_range {
                rec.extend_from_slice(&v.to_le_bytes());
            }
            rec.extend_from_slice(&f.weight.to_le_bytes());
            rec.extend_from_slice(&f.width.to_le_bytes());
            rec.extend_from_slice(&f.fam_class.to_le_bytes());
            rec.extend_from_slice(&f.format.to_le_bytes());
            for v in [
                f.avg_char_width,
                f.ascent,
                f.descent,
                f.line_gap,
                f.x_height,
                f.cap_height,
            ] {
                rec.extend_from_slice(&v.to_le_bytes());
            }
            rec.extend_from_slice(&f.fs_type.to_le_bytes());
            // recordLen counts from its own field (verified: artifact decode
            // with `pos = record_start + recordLen` walks the blob exactly)
            out.extend_from_slice(&((rec.len() + 4) as u32).to_le_bytes());
            out.extend_from_slice(&rec);
        }
        out
    }

    // -- enumeration --------------------------------------------------------

    fn enumerate() -> AllFonts {
        let mut all_files: Vec<PathBuf> = Vec::new();
        let mut all_faces: Vec<Face> = Vec::new();
        for (disk_prefix, _) in super::FONT_ROOTS {
            for path in walk(Path::new(disk_prefix)) {
                let file_idx = all_files.len() as u16;
                let mut parsed = parse_file(&path, file_idx);
                if parsed.is_empty() {
                    continue;
                }
                all_files.push(path);
                all_faces.append(&mut parsed);
            }
        }

        // winner per (family, slot): minimum |weight - 400|, ties go to the
        // later face. (The app's exact tie-break differs on ~2% of faces —
        // recorded as a deviation in reference.map § AllFonts generator.)
        let mut winners: HashMap<(String, u8), (u32, usize)> = HashMap::new();
        for (i, f) in all_faces.iter().enumerate() {
            let slot = u8::from(f.bold) * 2 + u8::from(f.italic);
            let dist = (i32::from(f.weight) - 400).unsigned_abs();
            let key = (f.family.clone(), slot);
            match winners.get_mut(&key) {
                Some((d, _)) if *d < dist => {}
                Some(e) => *e = (dist, i),
                None => {
                    winners.insert(key, (dist, i));
                }
            }
        }
        let mut winner_idx: Vec<usize> = winners.into_values().map(|(_, i)| i).collect();
        winner_idx.sort_by_key(|&i| (all_faces[i].file, all_faces[i].face));

        // keep only files hosting a winning face; remap their indexes
        let mut remap: HashMap<u16, u16> = HashMap::new();
        let mut files = Vec::new();
        for &i in &winner_idx {
            let old = all_faces[i].file;
            remap.entry(old).or_insert_with(|| {
                files.push(all_files[old as usize].clone());
                (files.len() - 1) as u16
            });
        }

        // families sorted case-insensitively (the app's infos order)
        let mut fam_names: Vec<String> = winner_idx
            .iter()
            .map(|&i| all_faces[i].family.clone())
            .collect();
        fam_names.sort_by_key(|a| a.to_lowercase());
        fam_names.dedup();

        let mut infos = vec![[-1i64; 8]; fam_names.len()];
        let mut fam_cov: Vec<Vec<(u32, u32)>> = vec![Vec::new(); fam_names.len()];
        let mut faces = Vec::with_capacity(winner_idx.len());
        for &i in &winner_idx {
            let f = &all_faces[i];
            let fam = fam_names
                .iter()
                .position(|n| n == &f.family)
                .expect("family came from this list");
            let slot = usize::from(f.bold) * 2 + usize::from(f.italic);
            infos[fam][slot * 2] = i64::from(remap[&f.file]);
            infos[fam][slot * 2 + 1] = i64::from(f.face);
            fam_cov[fam].extend(f.coverage.iter().copied());
            faces.push(Face {
                file: remap[&f.file],
                face: f.face,
                family: f.family.clone(),
                alt: f.alt.clone(),
                italic: f.italic,
                bold: f.bold,
                fixed: f.fixed,
                weight: f.weight,
                width: f.width,
                fam_class: f.fam_class,
                format: f.format,
                avg_char_width: f.avg_char_width,
                ascent: f.ascent,
                descent: f.descent,
                line_gap: f.line_gap,
                x_height: f.x_height,
                cap_height: f.cap_height,
                fs_type: f.fs_type,
                panose: f.panose,
                unicode_range: f.unicode_range,
                codepage_range: f.codepage_range,
                coverage: Vec::new(),
            });
        }

        let mut fonts = AllFonts {
            files,
            fam_names,
            infos,
            faces,
            ranges: Vec::new(),
        };
        fonts.ranges = build_ranges(&fam_cov);
        fonts
    }
}

/// `__fonts_ranges`: per codepoint interval, the family whose coverage
/// interval spanning it is the widest (general-purpose fonts win, like the
/// app's Arial-first latin choices), ties to the first family. Neighbouring
/// intervals resolving to the same family are merged.
fn build_ranges(fam_cov: &[Vec<(u32, u32)>]) -> Vec<(u32, u32, u32)> {
    let merged: Vec<Vec<(u32, u32)>> = fam_cov
        .iter()
        .map(|cov| {
            let mut v = cov.clone();
            v.sort_unstable();
            let mut out: Vec<(u32, u32)> = Vec::with_capacity(v.len());
            for (s, e) in v {
                match out.last_mut() {
                    Some(last) if s <= last.1.saturating_add(1) => last.1 = last.1.max(e),
                    _ => out.push((s, e)),
                }
            }
            out
        })
        .collect();

    let mut points: Vec<u32> = merged
        .iter()
        .flat_map(|iv| iv.iter().flat_map(|(s, e)| [*s, e.saturating_add(1)]))
        .collect();
    points.sort_unstable();
    points.dedup();

    let mut out: Vec<(u32, u32, u32)> = Vec::new();
    for pair in points.windows(2) {
        let (a, b) = (pair[0], pair[1].saturating_sub(1));
        if a > b {
            continue;
        }
        let mut best: Option<(u64, usize)> = None;
        for (fi, cov) in merged.iter().enumerate() {
            let idx = cov.partition_point(|(s, _)| *s <= a);
            if idx == 0 {
                continue;
            }
            let (s, e) = cov[idx - 1];
            if e >= b {
                let len = u64::from(e) - u64::from(s);
                match best {
                    Some((l, _)) if l >= len => {}
                    _ => best = Some((len, fi)),
                }
            }
        }
        if let Some((_, fi)) = best {
            match out.last_mut() {
                Some(last) if last.2 == fi as u32 && last.1 + 1 == a => last.1 = b,
                _ => out.push((a, b, fi as u32)),
            }
        }
    }
    out
}

/// Depth-first walk of a font root: font files sorted by name first, then
/// subdirectories.
fn walk(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut dirs = Vec::new();
    let mut font_files = Vec::new();
    for e in entries.flatten() {
        let p = e.path();
        if p.is_dir() {
            dirs.push(p);
        } else if p
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|ext| matches!(ext.to_ascii_lowercase().as_str(), "ttf" | "ttc" | "otf"))
        {
            font_files.push(p);
        }
    }
    font_files.sort();
    dirs.sort();
    let mut out = font_files;
    for d in dirs {
        out.extend(walk(&d));
    }
    out
}

/// All faces of one font file (TTCs contribute every face), in face order.
fn parse_file(path: &Path, file_idx: u16) -> Vec<Face> {
    let Ok(mut f) = std::fs::File::open(path) else {
        return Vec::new();
    };
    let mut tag = [0u8; 4];
    if f.read_exact(&mut tag).is_err() {
        return Vec::new();
    }
    let face_offsets: Vec<u64> = if &tag == b"ttcf" {
        let mut hdr = [0u8; 8];
        if f.read_exact(&mut hdr).is_err() {
            return Vec::new();
        }
        let n = u32::from_be_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
        let mut offs = Vec::with_capacity(n);
        for i in 0..n {
            let mut b = [0u8; 4];
            if f.seek(SeekFrom::Start(12 + 4 * i as u64)).is_err() || f.read_exact(&mut b).is_err()
            {
                return Vec::new();
            }
            offs.push(u32::from_be_bytes(b) as u64);
        }
        offs
    } else {
        vec![0]
    };

    let mut faces = Vec::new();
    for (face_idx, &off) in face_offsets.iter().enumerate() {
        if let Some(face) = parse_face(&mut f, off, face_idx as u16, file_idx) {
            faces.push(face);
        }
    }
    faces
}

/// Parse one face: table directory + the tables the selection records need.
fn parse_face(f: &mut std::fs::File, off: u64, face_idx: u16, file_idx: u16) -> Option<Face> {
    // sfnt header: version @0, numTables @4; entries from @12
    let mut hdr = [0u8; 12];
    f.seek(SeekFrom::Start(off + 4)).ok()?;
    f.read_exact(&mut hdr).ok()?;
    let num_tables = u16::from_be_bytes([hdr[0], hdr[1]]) as usize;

    let mut tables: BTreeMap<[u8; 4], (u64, u32)> = BTreeMap::new();
    for i in 0..num_tables {
        let mut e = [0u8; 16];
        f.seek(SeekFrom::Start(off + 12 + 16 * i as u64)).ok()?;
        f.read_exact(&mut e).ok()?;
        tables.insert(
            [e[0], e[1], e[2], e[3]],
            (
                u32::from_be_bytes([e[8], e[9], e[10], e[11]]) as u64,
                u32::from_be_bytes([e[12], e[13], e[14], e[15]]),
            ),
        );
    }
    let read_table = |f: &mut std::fs::File, tag: &[u8; 4]| -> Option<Vec<u8>> {
        let (o, len) = *tables.get(tag)?;
        let mut buf = vec![0u8; len as usize];
        f.seek(SeekFrom::Start(o)).ok()?;
        f.read_exact(&mut buf).ok()?;
        Some(buf)
    };

    let head = read_table(f, b"head")?;
    let upm = u64::from(u16::from_be_bytes([head[18], head[19]]));
    let mac_style = u16::from_be_bytes([head[44], head[45]]);
    let name = read_table(f, b"name")?;
    let names = parse_name_table(&name);

    // primary family name (verified 557/557 against the artifact): first
    // Windows-English nameID 1, else the Mac platform one, else any Windows
    // one, else the first of all; hidden families (leading '.') are skipped.
    let n1: Vec<&NameRec> = names.iter().filter(|n| n.0 == 1).collect();
    let primary = n1
        .iter()
        .find(|n| n.1 == 3 && n.2 == 1 && n.3 == 0x409)
        .or_else(|| n1.iter().find(|n| n.1 == 1 && n.2 == 0))
        .or_else(|| n1.iter().find(|n| n.1 == 3 && n.2 == 1))
        .or_else(|| n1.first())
        .map(|n| n.4.clone())?;
    if primary.starts_with('.') {
        return None;
    }
    let alt = n1
        .iter()
        .find(|n| n.1 == 3 && n.2 == 1 && n.4 != primary)
        .map(|n| n.4.clone());

    let os2 = read_table(f, b"OS/2");
    let be16 = |o: usize| -> u16 {
        os2.as_ref()
            .and_then(|t| t.get(o..o + 2))
            .map(|b| u16::from_be_bytes([b[0], b[1]]))
            .unwrap_or(0)
    };
    let be16i = |o: usize| -> i16 {
        os2.as_ref()
            .and_then(|t| t.get(o..o + 2))
            .map(|b| i16::from_be_bytes([b[0], b[1]]))
            .unwrap_or(0)
    };
    let be32 = |o: usize| -> u32 {
        os2.as_ref()
            .and_then(|t| t.get(o..o + 4))
            .map(|b| u32::from_be_bytes([b[0], b[1], b[2], b[3]]))
            .unwrap_or(0)
    };

    // style bits: OS/2 fsSelection when present, else head.macStyle (fonts
    // without OS/2 — e.g. Courier.ttc — the artifact fills all four slots
    // from macStyle)
    let (bold, italic) = match &os2 {
        Some(t) if t.len() >= 64 => (be16(62) & 0x20 != 0, be16(62) & 0x01 != 0),
        _ => (mac_style & 0x01 != 0, mac_style & 0x02 != 0),
    };
    let fixed = read_table(f, b"post")
        .filter(|p| p.len() >= 16)
        .is_some_and(|p| i32::from_be_bytes([p[12], p[13], p[14], p[15]]) != 0);

    let scale = |v: i16| -> i16 {
        if upm == 0 {
            return v;
        }
        ((i64::from(v) * 1000) / upm as i64) as i16
    };
    let empty10 = [0u8; 10];
    let (avg_char_width, ascent, descent, line_gap, x_height, cap_height) = if os2.is_some() {
        (
            scale(be16i(2)),
            scale(be16i(68)),
            scale(be16i(70)),
            scale(be16i(72)),
            scale(be16i(86)),
            scale(be16i(88)),
        )
    } else {
        (0, 0, 0, 0, 0, 0)
    };
    let panose: [u8; 10] = os2
        .as_ref()
        .and_then(|t| t.get(32..42))
        .and_then(|s| <[u8; 10]>::try_from(s).ok())
        .unwrap_or(empty10);

    let coverage = read_table(f, b"cmap")
        .map(|c| cmap_coverage(&c))
        .unwrap_or_default();

    Some(Face {
        file: file_idx,
        face: face_idx,
        family: primary,
        alt,
        italic,
        bold,
        fixed,
        weight: be16(4),
        width: be16(6),
        fam_class: be16i(30),
        format: if tables.contains_key(b"CFF ") { 2 } else { 1 },
        avg_char_width,
        ascent,
        descent,
        line_gap,
        x_height,
        cap_height,
        fs_type: be16(8),
        panose,
        unicode_range: [be32(42), be32(46), be32(50), be32(54)],
        codepage_range: [be32(78), be32(82)],
        coverage,
    })
}

/// `name` table records as `(nameID, platformID, encodingID, languageID, text)`.
type NameRec = (u16, u16, u16, u16, String);

fn parse_name_table(t: &[u8]) -> Vec<NameRec> {
    if t.len() < 6 {
        return Vec::new();
    }
    let count = u16::from_be_bytes([t[2], t[3]]) as usize;
    let storage = u16::from_be_bytes([t[4], t[5]]) as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let o = 6 + 12 * i;
        if o + 12 > t.len() {
            break;
        }
        let (pid, eid, lid, nid) = (
            u16::from_be_bytes([t[o], t[o + 1]]),
            u16::from_be_bytes([t[o + 2], t[o + 3]]),
            u16::from_be_bytes([t[o + 4], t[o + 5]]),
            u16::from_be_bytes([t[o + 6], t[o + 7]]),
        );
        let (len, soff) = (
            u16::from_be_bytes([t[o + 8], t[o + 9]]) as usize,
            u16::from_be_bytes([t[o + 10], t[o + 11]]) as usize,
        );
        let start = storage.saturating_add(soff);
        if start + len > t.len() {
            continue;
        }
        let raw = &t[start..start + len];
        let text = if pid == 0 || pid == 3 {
            utf16be_to_string(raw)
        } else {
            // Mac platform names: ASCII is exact; the rare high-byte legacy
            // name decodes via Latin-1 (deviation noted in reference.map)
            raw.iter().map(|&b| b as char).collect()
        };
        out.push((nid, pid, eid, lid, text));
    }
    out
}

fn utf16be_to_string(bytes: &[u8]) -> String {
    bytes
        .chunks_exact(2)
        .map(|c| char::from_u32(u32::from(u16::from_be_bytes([c[0], c[1]]))).unwrap_or('\u{fffd}'))
        .collect()
}

/// Covered codepoint intervals from a `cmap` table: the best subtable
/// (format 12 preferred, then format 4).
fn cmap_coverage(t: &[u8]) -> Vec<(u32, u32)> {
    if t.len() < 4 {
        return Vec::new();
    }
    let n = u16::from_be_bytes([t[2], t[3]]) as usize;
    let mut best: Option<(u8, Vec<(u32, u32)>)> = None;
    for i in 0..n {
        let o = 4 + 8 * i;
        if o + 8 > t.len() {
            break;
        }
        let (pid, eid) = (
            u16::from_be_bytes([t[o], t[o + 1]]),
            u16::from_be_bytes([t[o + 2], t[o + 3]]),
        );
        let off = u32::from_be_bytes([t[o + 4], t[o + 5], t[o + 6], t[o + 7]]) as usize;
        if off >= t.len() {
            continue;
        }
        let fmt = u16::from_be_bytes([t[off], t[off + 1]]);
        let score = match (pid, eid, fmt) {
            (3, 10, 12) => 4,
            (0, _, 12) => 3,
            (3, 1, 4) => 2,
            (0, _, 4) => 1,
            _ => 0,
        };
        if score == 0 || best.as_ref().is_some_and(|(s, _)| *s >= score) {
            continue;
        }
        let cov = match fmt {
            4 => cmap4_coverage(&t[off..]),
            12 => cmap12_coverage(&t[off..]),
            _ => Vec::new(),
        };
        best = Some((score, cov));
    }
    best.map(|(_, v)| v).unwrap_or_default()
}

fn cmap4_coverage(t: &[u8]) -> Vec<(u32, u32)> {
    if t.len() < 14 {
        return Vec::new();
    }
    let seg = u16::from_be_bytes([t[6], t[7]]) as usize / 2;
    let end_at = 14;
    let start_at = end_at + 2 * seg + 2;
    let delta_at = start_at + 2 * seg;
    if delta_at + 2 * seg > t.len() {
        return Vec::new();
    }
    let mut out = Vec::with_capacity(seg);
    for i in 0..seg {
        let end = u16::from_be_bytes([t[end_at + 2 * i], t[end_at + 2 * i + 1]]);
        let start = u16::from_be_bytes([t[start_at + 2 * i], t[start_at + 2 * i + 1]]);
        let delta = i16::from_be_bytes([t[delta_at + 2 * i], t[delta_at + 2 * i + 1]]);
        if end == 0xFFFF && start == 0xFFFF {
            continue;
        }
        // segment-level approximation: unmapped only when the whole segment
        // maps to glyph 0 through the delta (idRangeOffset segments assumed
        // mapped) — ranges quality, not correctness
        let maps_zero = |c: u16| ((i32::from(c) + i32::from(delta)) & 0xFFFF) as u16 == 0;
        if maps_zero(start) && maps_zero(end) {
            continue;
        }
        out.push((u32::from(start), u32::from(end)));
    }
    out
}

fn cmap12_coverage(t: &[u8]) -> Vec<(u32, u32)> {
    if t.len() < 16 {
        return Vec::new();
    }
    let n = u32::from_be_bytes([t[12], t[13], t[14], t[15]]) as usize;
    let mut out = Vec::with_capacity(n.min(4096));
    for i in 0..n {
        let o = 16 + 12 * i;
        if o + 12 > t.len() {
            break;
        }
        let s = u32::from_be_bytes([t[o], t[o + 1], t[o + 2], t[o + 3]]);
        let e = u32::from_be_bytes([t[o + 4], t[o + 5], t[o + 6], t[o + 7]]);
        let g = u32::from_be_bytes([t[o + 8], t[o + 9], t[o + 10], t[o + 11]]);
        if g == 0 && s == e {
            continue;
        }
        out.push((s, e));
    }
    out
}

fn js_escape_name(s: &str) -> String {
    s.replace('\\', "\\\\").replace('"', "\\\"")
}

fn base64(data: &[u8]) -> String {
    const T: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let n = ((chunk[0] as u32) << 16)
            | ((u32::from(*chunk.get(1).unwrap_or(&0))) << 8)
            | u32::from(*chunk.get(2).unwrap_or(&0));
        out.push(T[(n >> 18 & 63) as usize] as char);
        out.push(T[(n >> 12 & 63) as usize] as char);
        out.push(if chunk.len() > 1 {
            T[(n >> 6 & 63) as usize] as char
        } else {
            '='
        });
        out.push(if chunk.len() > 2 {
            T[(n & 63) as usize] as char
        } else {
            '='
        });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU32, Ordering};

    #[test]
    fn base64_rfc_vectors() {
        assert_eq!(base64(b""), "");
        assert_eq!(base64(b"f"), "Zg==");
        assert_eq!(base64(b"fo"), "Zm8=");
        assert_eq!(base64(b"foo"), "Zm9v");
        assert_eq!(base64(b"foob"), "Zm9vYg==");
        assert_eq!(base64(b"fooba"), "Zm9vYmE=");
        assert_eq!(base64(b"foobar"), "Zm9vYmFy");
    }

    #[test]
    fn scale_truncates_toward_zero() {
        // TNR: typo ascender 1420 / 2048 em → 693; descender -442 → -215
        assert_eq!((1420i64 * 1000 / 2048) as i16, 693);
        assert_eq!((-442i64 * 1000 / 2048) as i16, -215);
    }

    /// Minimal in-memory sfnt face: name (Windows-English family), OS/2 v4
    /// with known metrics, cmap format 4 with one mapped segment.
    fn write_synthetic_font() -> PathBuf {
        static N: AtomicU32 = AtomicU32::new(0);
        let mut t: BTreeMap<[u8; 4], Vec<u8>> = BTreeMap::new();

        // head: unitsPerEm 2048 @18, macStyle 0 @44
        let mut head = vec![0u8; 54];
        head[18..20].copy_from_slice(&2048u16.to_be_bytes());
        t.insert(*b"head", head);

        // OS/2 v4: avgCharWidth 820, weight 700, width 5, fsType 8,
        // familyClass 261, panose, unicode ranges, typo metrics, codepages,
        // xHeight 916, capHeight 1356, fsSelection BOLD (0x20)
        let mut os2 = vec![0u8; 96];
        os2[0..2].copy_from_slice(&4u16.to_be_bytes());
        os2[2..4].copy_from_slice(&820i16.to_be_bytes());
        os2[4..6].copy_from_slice(&700u16.to_be_bytes());
        os2[6..8].copy_from_slice(&5u16.to_be_bytes());
        os2[8..10].copy_from_slice(&8u16.to_be_bytes());
        os2[30..32].copy_from_slice(&261i16.to_be_bytes());
        os2[32..42].copy_from_slice(&[2, 2, 6, 3, 5, 4, 5, 2, 3, 4]);
        os2[42..46].copy_from_slice(&0xe000_2aefu32.to_be_bytes());
        os2[46..50].copy_from_slice(&0xc000_7841u32.to_be_bytes());
        os2[50..54].copy_from_slice(&9u32.to_be_bytes());
        os2[54..58].copy_from_slice(&0u32.to_be_bytes());
        os2[62..64].copy_from_slice(&0x0020u16.to_be_bytes());
        os2[64..66].copy_from_slice(&32u16.to_be_bytes());
        os2[66..68].copy_from_slice(&0xFFFFu16.to_be_bytes());
        os2[68..70].copy_from_slice(&1420i16.to_be_bytes());
        os2[70..72].copy_from_slice(&(-442i16).to_be_bytes());
        os2[72..74].copy_from_slice(&307i16.to_be_bytes());
        os2[78..82].copy_from_slice(&0x4000_01ffu32.to_be_bytes());
        os2[82..86].copy_from_slice(&0xffff_0000u32.to_be_bytes());
        os2[86..88].copy_from_slice(&916i16.to_be_bytes());
        os2[88..90].copy_from_slice(&1356i16.to_be_bytes());
        t.insert(*b"OS/2", os2);

        // name: Windows-English nameID 1 "Test Font" + nameID 2 "Bold"
        let fam: Vec<u8> = "Test Font"
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect();
        let sub: Vec<u8> = "Bold"
            .encode_utf16()
            .flat_map(|u| u.to_be_bytes())
            .collect();
        let mut storage: Vec<u8> = Vec::new();
        let mut recs: Vec<[u8; 12]> = Vec::new();
        for (nid, data) in [(1u16, &fam), (2u16, &sub)] {
            let mut r = [0u8; 12];
            r[0..2].copy_from_slice(&3u16.to_be_bytes());
            r[2..4].copy_from_slice(&1u16.to_be_bytes());
            r[4..6].copy_from_slice(&0x409u16.to_be_bytes());
            r[6..8].copy_from_slice(&nid.to_be_bytes());
            r[8..10].copy_from_slice(&(data.len() as u16).to_be_bytes());
            r[10..12].copy_from_slice(&(storage.len() as u16).to_be_bytes());
            recs.push(r);
            storage.extend_from_slice(data);
        }
        let mut name = Vec::new();
        name.extend_from_slice(&0u16.to_be_bytes());
        name.extend_from_slice(&(recs.len() as u16).to_be_bytes());
        // string storage begins right after the header + record array
        name.extend_from_slice(&(6 + 12 * recs.len() as u16).to_be_bytes());
        for r in recs {
            name.extend_from_slice(&r);
        }
        name.extend_from_slice(&storage);
        t.insert(*b"name", name);

        // cmap: format 4, segCount 2 (one real segment + the 0xFFFF
        // terminator), segment 0 = 0x41..0x5A mapping via delta +1
        let mut c4 = vec![0u8; 32];
        c4[0..2].copy_from_slice(&4u16.to_be_bytes());
        c4[2..4].copy_from_slice(&32u16.to_be_bytes());
        c4[6..8].copy_from_slice(&4u16.to_be_bytes()); // segCountX2
        c4[14..16].copy_from_slice(&0x5Au16.to_be_bytes()); // endCode[0]
        c4[16..18].copy_from_slice(&0xFFFFu16.to_be_bytes()); // endCode[1]
        c4[18..20].copy_from_slice(&0u16.to_be_bytes()); // reservedPad
        c4[20..22].copy_from_slice(&0x41u16.to_be_bytes()); // startCode[0]
        c4[22..24].copy_from_slice(&0xFFFFu16.to_be_bytes());
        c4[24..26].copy_from_slice(&1i16.to_be_bytes()); // idDelta[0]
        c4[26..28].copy_from_slice(&0i16.to_be_bytes()); // idDelta[1]
        let mut cmap = Vec::new();
        cmap.extend_from_slice(&0u16.to_be_bytes());
        cmap.extend_from_slice(&1u16.to_be_bytes());
        cmap.extend_from_slice(&3u16.to_be_bytes());
        cmap.extend_from_slice(&1u16.to_be_bytes());
        cmap.extend_from_slice(&12u32.to_be_bytes());
        cmap.extend_from_slice(&c4);
        t.insert(*b"cmap", cmap);

        // assemble the sfnt file
        let num = t.len() as u16;
        let dir_len = 12 + 16 * num as usize;
        let mut dir: Vec<u8> = Vec::new();
        dir.extend_from_slice(&1u32.to_be_bytes());
        dir.extend_from_slice(&num.to_be_bytes());
        dir.extend_from_slice(&[0u8; 6]);
        let mut body: Vec<u8> = Vec::new();
        for (tag, data) in t {
            let off = dir_len + body.len();
            dir.extend_from_slice(&tag);
            dir.extend_from_slice(&0u32.to_be_bytes());
            dir.extend_from_slice(&(off as u32).to_be_bytes());
            dir.extend_from_slice(&(data.len() as u32).to_be_bytes());
            body.extend_from_slice(&data);
            while body.len() % 4 != 0 {
                body.push(0);
            }
        }
        dir.extend_from_slice(&body);

        // unique per call — tests sharing the file run in parallel threads
        let path = std::env::temp_dir().join(format!(
            "nanooffice-test-synthetic-{}.ttf",
            N.fetch_add(1, Ordering::SeqCst)
        ));
        std::fs::write(&path, &dir).expect("write synthetic font");
        path
    }

    fn parse_synthetic() -> Face {
        let path = write_synthetic_font();
        let faces = parse_file(&path, 0);
        let _ = std::fs::remove_file(&path);
        assert_eq!(faces.len(), 1, "synthetic face parses");
        faces.into_iter().next().expect("one face")
    }

    #[test]
    fn synthetic_face_parses() {
        let f = parse_synthetic();
        assert_eq!(f.family, "Test Font");
        assert!(f.alt.is_none());
        assert!(f.bold && !f.italic);
        assert_eq!(f.weight, 700);
        assert_eq!(f.width, 5);
        assert_eq!(f.fam_class, 261);
        assert_eq!(f.format, 1);
        assert_eq!(f.fs_type, 8);
        assert_eq!(f.ascent, 693); // 1420/2048*1000
        assert_eq!(f.descent, -215);
        assert_eq!(f.line_gap, 149); // 307/2048*1000
        assert_eq!(f.x_height, 447); // 916/2048*1000
        assert_eq!(f.cap_height, 662); // 1356/2048*1000
        assert_eq!(f.avg_char_width, 400); // 820/2048*1000
        assert_eq!(f.unicode_range[0], 0xe000_2aef);
        assert_eq!(f.codepage_range[0], 0x4000_01ff);
        assert_eq!(f.coverage, vec![(0x41, 0x5A)]);
    }

    fn sample_fonts() -> AllFonts {
        let mut f = parse_synthetic();
        f.file = 0;
        AllFonts {
            files: vec![PathBuf::from("/tmp/Test Font.ttf")],
            fam_names: vec!["Test Font".to_string()],
            infos: vec![[0, 0, -1, -1, -1, -1, -1, -1]],
            faces: vec![f],
            ranges: vec![(0x41, 0x5A, 0)],
        }
    }

    #[test]
    fn selection_bin_roundtrip() {
        let fonts = sample_fonts();
        let bin = fonts.selection_bin();
        let mut pos = 0usize;
        let i32_at = |p: usize| i32::from_le_bytes(bin[p..p + 4].try_into().unwrap());
        let count = i32_at(pos);
        pos += 4;
        assert_eq!(count, 1);
        let start = pos;
        let rec_len = i32_at(pos) as usize;
        pos += 4;
        let nl = i32_at(pos) as usize;
        pos += 4;
        let name = String::from_utf8(bin[pos..pos + nl].to_vec()).unwrap();
        pos += nl;
        assert_eq!(name, "Test Font");
        let nn = i32_at(pos);
        pos += 4;
        assert_eq!(nn, 0);
        let pl = i32_at(pos) as usize;
        pos += 4;
        let path = String::from_utf8(bin[pos..pos + pl].to_vec()).unwrap();
        pos += pl;
        assert_eq!(path, "/tmp/Test Font.ttf");
        let idx = i32_at(pos);
        pos += 4;
        assert_eq!(idx, 0);
        // jump via recordLen — the whole point of the prefix
        pos = start + rec_len;
        assert_eq!(pos, bin.len(), "record consumed exactly");
    }

    /// One-off comparison aid (run with `cargo test dump_generated -- --ignored
    /// --nocapture`): dumps the generated enumeration for a diff against the
    /// app-generated artifact (see docs/diagnostics.md T-A11). No assertions —
    /// the diff itself is the oracle-obs evidence.
    #[test]
    #[ignore]
    fn dump_generated_allfonts() {
        let Some(fonts) = generated().as_ref() else {
            eprintln!("no fonts enumerated on this machine");
            return;
        };
        let mut out = String::new();
        for f in &fonts.faces {
            out.push_str(&format!(
                "FACE\t{}\t{}\t{}\t{}\t{}{}\t{}\t{}\n",
                fonts.files[f.file as usize].display(),
                f.face,
                f.family,
                f.alt.as_deref().unwrap_or(""),
                u8::from(f.italic),
                u8::from(f.bold),
                f.weight,
                f.format
            ));
        }
        for (name, slots) in fonts.fam_names.iter().zip(&fonts.infos) {
            out.push_str(&format!(
                "FAMILY\t{name}\t{}\n",
                slots
                    .iter()
                    .map(|v| v.to_string())
                    .collect::<Vec<_>>()
                    .join("\t")
            ));
        }
        std::fs::write("/tmp/nanooffice-allfonts-dump.txt", out).unwrap();
        eprintln!(
            "dump: {} files / {} faces / {} families / {} ranges → /tmp/nanooffice-allfonts-dump.txt",
            fonts.files.len(),
            fonts.faces.len(),
            fonts.infos.len(),
            fonts.ranges.len()
        );
    }

    #[test]
    fn ranges_pick_widest_family() {
        // A covers 10..50, B covers 30..90 → B's span is wider, so B wins
        // the overlap 30..50 and keeps 51..90; A keeps 10..29
        let fams = vec![vec![(10, 50)], vec![(30, 90)]];
        let r = build_ranges(&fams);
        assert_eq!(r, vec![(10, 29, 0), (30, 90, 1)]);
    }

    #[test]
    fn ranges_merge_same_family_neighbours() {
        let fams = vec![vec![(10, 20), (22, 30), (31, 40)]];
        let r = build_ranges(&fams);
        assert_eq!(r, vec![(10, 20, 0), (22, 40, 0)]);
    }

    #[test]
    fn js_has_expected_globals() {
        let fonts = sample_fonts();
        let js = fonts.js_raw();
        assert!(js.contains("window[\"__all_fonts_js_version__\"] = 2;"));
        assert!(js.contains("window[\"__fonts_files\"] = ["));
        assert!(js.contains("\"/tmp/Test Font.ttf\""));
        assert!(js.contains("window[\"__fonts_infos\"] = ["));
        assert!(js.contains("[\"Test Font\",0,0,-1,-1,-1,-1,-1,-1]"));
        assert!(js.contains("window[\"__fonts_ranges\"] = ["));
        assert!(js.contains("65,90,0"));
        assert!(js.contains("window[\"g_fonts_selection_bin\"] = \""));
        assert!(js.ends_with(";\n"));
    }
}
