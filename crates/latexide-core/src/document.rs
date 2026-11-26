use crate::{CoreError, MAX_FILE_BYTES, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position {
    pub line: u32,
    pub character: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit {
    pub start: Position,
    pub end: Position,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnding {
    Lf,
    Crlf,
}

// Shared normal encoder for retained documents and read-only rename preflight.
pub(crate) fn encoded_bytes(text: &str, line_ending: LineEnding, bom: bool) -> Vec<u8> {
        let body = match line_ending {
            LineEnding::Lf => text.to_owned(),
            LineEnding::Crlf => text.replace('\n', "\r\n"),
        };
        let mut bytes = Vec::with_capacity(body.len() + if bom { 3 } else { 0 });
        if bom {
            bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]);
        }
        bytes.extend_from_slice(body.as_bytes());
        bytes
    }

pub(crate) fn validate_encoded_size(text: &str, line_ending: LineEnding, bom: bool) -> Result<()> {
    if encoded_bytes(text, line_ending, bom).len() > MAX_FILE_BYTES {
        return Err(CoreError::new("file_too_large", "A kódolt fájl meghaladná az 1 MiB határt."));
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentInfo {
    pub id: String,
    pub revision: u64,
    pub dirty: bool,
    pub last_origin: String,
    pub transaction_id: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DocumentSnapshot {
    pub id: String,
    pub text: String,
    pub revision: u64,
    pub dirty: bool,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub last_origin: String,
    pub transaction_id: u64,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Document {
    pub id: String,
    pub path: PathBuf,
    pub text: String,
    pub saved_text: String,
    pub disk_identity: Option<(u64, u64)>,
    pub revision: u64,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub last_origin: String,
    pub transaction_id: u64,
}

#[derive(Clone, PartialEq)]
pub(crate) struct Patch {
    pub start: usize,
    pub before: String,
    pub after: String,
}

#[derive(Clone, Copy)]
enum OffsetFailure { MissingLine, InvalidColumn }
impl OffsetFailure {
    fn error(self) -> CoreError {
        match self {
            Self::MissingLine => CoreError::new("invalid_position", "A megadott sor nem létezik."),
            Self::InvalidColumn => CoreError::new("invalid_position", "Érvénytelen UTF-16 oszlop vagy félbevágott surrogate pár."),
        }
    }
}

impl Document {
    pub fn info(&self) -> DocumentInfo {
        DocumentInfo {
            id: self.id.clone(),
            revision: self.revision,
            dirty: self.text != self.saved_text,
            last_origin: self.last_origin.clone(),
            transaction_id: self.transaction_id,
        }
    }

    pub fn load(id: String, path: PathBuf, bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(CoreError::new(
                "file_too_large",
                "A fájl nagyobb a támogatott 1 MiB határnál.",
            ));
        }
        let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        let raw = std::str::from_utf8(if bom { &bytes[3..] } else { &bytes }).map_err(|_| {
            CoreError::new(
                "invalid_utf8",
                "A fájl nem érvényes UTF-8; nem módosítottuk.",
            )
        })?;
        if raw.contains('\0') {
            return Err(CoreError::new(
                "invalid_text",
                "A fájl bináris nullkaraktert tartalmaz.",
            ));
        }
        let crlf = raw.contains("\r\n");
        let normalized = raw.replace("\r\n", "\n");
        if normalized.contains('\r') || (crlf && raw.replace("\r\n", "").contains('\n')) {
            return Err(CoreError::new(
                "unsupported_line_endings",
                "Kevert vagy régi CR sorvégek; automatikus átalakítás helyett a fájl változatlan maradt.",
            ));
        }
        let disk_identity = file_identity(&path)?;
        Ok(Self {
            id,
            path,
            disk_identity,
            saved_text: normalized.clone(),
            text: normalized,
            revision: 1,
            bom,
            line_ending: if crlf {
                LineEnding::Crlf
            } else {
                LineEnding::Lf
            },
            last_origin: "disk".into(),
            transaction_id: 0,
        })
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot {
            id: self.id.clone(),
            text: self.text.clone(),
            revision: self.revision,
            dirty: self.text != self.saved_text,
            bom: self.bom,
            line_ending: self.line_ending,
            last_origin: self.last_origin.clone(),
            transaction_id: self.transaction_id,
        }
    }

    pub fn bytes_for(&self, text: &str) -> Vec<u8> {
        encoded_bytes(text, self.line_ending, self.bom)
    }

    pub fn byte_offset(&self, position: Position) -> Result<usize> {
        let mut base = 0;
        for (number, line) in self.text.split('\n').enumerate() {
            if number == position.line as usize {
                let mut column = 0;
                for (offset, character) in line.char_indices() {
                    if column == position.character {
                        return Ok(base + offset);
                    }
                    column += character.len_utf16() as u32;
                    if column > position.character {
                        break;
                    }
                }
                if column == position.character {
                    return Ok(base + line.len());
                }
                return Err(CoreError::new(
                    "invalid_position",
                    "Érvénytelen UTF-16 oszlop vagy félbevágott surrogate pár.",
                ));
            }
            base += line.len() + 1;
        }
        Err(CoreError::new(
            "invalid_position",
            "A megadott sor nem létezik.",
        ))
    }

    // Resolve the original coordinates once. Store failures by input position
    // so preparation preserves its start/end/replacement error precedence.
    fn edit_offsets(&self, edits: &[TextEdit]) -> Vec<std::result::Result<usize, OffsetFailure>> {
        let mut points: Vec<_> = edits.iter().flat_map(|edit| [edit.start, edit.end]).enumerate().collect();
        points.sort_unstable_by_key(|(_, position)| (position.line, position.character));
        let mut offsets = vec![Err(OffsetFailure::MissingLine); points.len()];
        let mut next = points.into_iter().peekable();
        let mut base = 0;
        for (number, line) in self.text.split('\n').enumerate() {
            if next.peek().is_none() { break; }
            let mut characters = line.char_indices();
            let (mut column, mut byte) = (0, 0);
            while next.peek().is_some_and(|(_, position)| position.line as usize == number) {
                let (index, position) = next.next().unwrap();
                while column < position.character {
                    let Some((offset, character)) = characters.next() else { break; };
                    column += character.len_utf16() as u32;
                    byte = offset + character.len_utf8();
                }
                offsets[index] = if column == position.character { Ok(base + byte) }
                    else { Err(OffsetFailure::InvalidColumn) };
            }
            base += line.len() + 1;
        }
        offsets
    }

    pub fn prepare_edits(&self, edits: Vec<TextEdit>) -> Result<(String, Vec<Patch>)> {
        if edits.len() > 1000 {
            return Err(CoreError::new(
                "too_many_edits",
                "Legfeljebb 1000 csere küldhető egy műveletben.",
            ));
        }
        let offsets = self.edit_offsets(&edits);
        let mut ranges = Vec::with_capacity(edits.len());
        for (index, edit) in edits.into_iter().enumerate() {
            let start = offsets[2 * index].map_err(OffsetFailure::error)?;
            let end = offsets[2 * index + 1].map_err(OffsetFailure::error)?;
            let replacement = edit.text.replace("\r\n", "\n");
            if start > end || replacement.contains(['\r', '\0']) {
                return Err(CoreError::new(
                    "invalid_edit",
                    "Érvénytelen tartomány vagy szöveg.",
                ));
            }
            ranges.push((start, end, replacement));
        }
        ranges.sort_by_key(|range| range.0);
        for pair in ranges.windows(2) {
            if pair[0].1 > pair[1].0 || pair[0].0 == pair[1].0 {
                return Err(CoreError::new(
                    "overlapping_edits",
                    "A cserék átfedik egymást.",
                ));
            }
        }
        let resulting_length = ranges
            .iter()
            .try_fold(self.text.len(), |length, (start, end, text)| {
                length
                    .checked_sub(end - start)
                    .and_then(|length| length.checked_add(text.len()))
            })
            .ok_or_else(|| CoreError::new("file_too_large", "A módosítás túl nagy."))?;
        if resulting_length > MAX_FILE_BYTES {
            return Err(CoreError::new(
                "file_too_large",
                "A módosított fájl meghaladná az 1 MiB határt.",
            ));
        }
        let mut result = self.text.clone();
        let mut patches = Vec::with_capacity(ranges.len());
        for (start, end, after) in ranges.into_iter().rev() {
            let before = result[start..end].to_owned();
            if before != after {
                result.replace_range(start..end, &after);
                patches.push(Patch {
                    start,
                    before,
                    after,
                });
            }
        }
        validate_encoded_size(&result, self.line_ending, self.bom)?;
        Ok((result, patches))
    }
}

pub(crate) fn file_identity(path: &std::path::Path) -> std::io::Result<Option<(u64, u64)>> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let m = std::fs::metadata(path)?;
        Ok(Some((m.dev(), m.ino())))
    }
    #[cfg(not(unix))]
    {
        let _ = path;
        Ok(None)
    }
}

#[cfg(test)]
mod edit_position_tests {
    use super::*;
    #[test]
    fn batch_positions_match_individual_offsets_and_error_order() {
        use std::io::Write;
        let stamp = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("latexide-document-positions-{}-{stamp:x}", std::process::id()));
        let mut builder = std::fs::DirBuilder::new();
        #[cfg(unix)] { use std::os::unix::fs::DirBuilderExt; builder.mode(0o700); }
        builder.create(&root).expect("fresh exclusive Document fixture");
        let mut file_index = 0;
        let mut load = |source: &str| {
            let path = root.join(format!("case-{file_index}.tex")); file_index += 1;
            let mut options = std::fs::OpenOptions::new(); options.write(true).create_new(true);
            #[cfg(unix)] { use std::os::unix::fs::OpenOptionsExt; options.mode(0o600); }
            let mut file = options.open(&path).unwrap(); file.write_all(source.as_bytes()).unwrap(); drop(file);
            Document::load(format!("case-{file_index}.tex"), path, source.as_bytes().to_vec()).unwrap()
        };
        for source in ["", "a\n", "😀ab\ncd𝒜\n", "\n\n", "\u{feff}x😀\né𝒜"] {
            let document = load(source);
            let mut points: Vec<_> = (0..6).flat_map(|line| (0..11).map(move |character| Position { line, character })).collect();
            points.extend([Position { line: 0, character: u32::MAX }, Position { line: u32::MAX, character: 0 }]);
            points.reverse();
            let edits: Vec<_> = points.iter().map(|&position| TextEdit { start: position, end: position, text: "x".into() }).collect();
            let offsets = document.edit_offsets(&edits);
            for (index, position) in points.into_iter().enumerate() {
                let expected = document.byte_offset(position).map_err(|error| (error.code, error.message));
                for slot in [2 * index, 2 * index + 1] {
                    assert_eq!(offsets[slot].map_err(OffsetFailure::error).map_err(|error| (error.code, error.message)), expected);
                }
            }
        }
        let document = load("😀ab\ncd𝒜\n");
        let edit = |line, start, end, text: &str| TextEdit { start: Position { line, character: start }, end: Position { line, character: end }, text: text.into() };
        assert_eq!(document.prepare_edits(vec![edit(1, 0, 1, "é"), edit(0, 2, 3, "x")]).unwrap().0, "😀xb\néd𝒜\n");
        for first in [edit(0, 3, 2, "x"), edit(0, 0, 0, "\r")] {
            let error = document.prepare_edits(vec![first, edit(999, 0, 0, "x")]).err().expect("invalid edit must be refused");
            assert_eq!(error.code, "invalid_edit"); assert_eq!(error.message, "Érvénytelen tartomány vagy szöveg.");
        }
        let error = document.prepare_edits(vec![edit(0, 1, 2, "x"), edit(999, 0, 0, "x")]).err().expect("invalid edit must be refused");
        assert_eq!(error.code, "invalid_position");
        assert_eq!(error.message, "Érvénytelen UTF-16 oszlop vagy félbevágott surrogate pár.");
        println!("DOCUMENT_POSITION_FIXTURE={}", root.display());
    }
}
