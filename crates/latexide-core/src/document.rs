use crate::{CoreError, MAX_FILE_BYTES, Result};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct Position { pub line: u32, pub character: u32 }

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextEdit { pub start: Position, pub end: Position, pub text: String }

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LineEnding { Lf, Crlf }

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

pub(crate) struct Document {
    pub id: String,
    pub path: PathBuf,
    pub text: String,
    pub saved_text: String,
    pub revision: u64,
    pub bom: bool,
    pub line_ending: LineEnding,
    pub last_origin: String,
    pub transaction_id: u64,
}

#[derive(Clone)]
pub(crate) struct Patch { pub start: usize, pub before: String, pub after: String }

impl Document {
    pub fn info(&self) -> DocumentInfo {
        DocumentInfo { id: self.id.clone(), revision: self.revision, dirty: self.text != self.saved_text,
            last_origin: self.last_origin.clone(), transaction_id: self.transaction_id }
    }

    pub fn load(id: String, path: PathBuf, bytes: Vec<u8>) -> Result<Self> {
        if bytes.len() > MAX_FILE_BYTES {
            return Err(CoreError::new("file_too_large", "A fájl nagyobb a támogatott 1 MiB határnál."));
        }
        let bom = bytes.starts_with(&[0xef, 0xbb, 0xbf]);
        let raw = std::str::from_utf8(if bom { &bytes[3..] } else { &bytes })
            .map_err(|_| CoreError::new("invalid_utf8", "A fájl nem érvényes UTF-8; nem módosítottuk."))?;
        if raw.contains('\0') { return Err(CoreError::new("invalid_text", "A fájl bináris nullkaraktert tartalmaz.")); }
        let crlf = raw.contains("\r\n");
        let normalized = raw.replace("\r\n", "\n");
        if normalized.contains('\r') || (crlf && raw.replace("\r\n", "").contains('\n')) {
            return Err(CoreError::new("unsupported_line_endings", "Kevert vagy régi CR sorvégek; automatikus átalakítás helyett a fájl változatlan maradt."));
        }
        Ok(Self { id, path, saved_text: normalized.clone(), text: normalized, revision: 1,
            bom, line_ending: if crlf { LineEnding::Crlf } else { LineEnding::Lf },
            last_origin: "disk".into(), transaction_id: 0 })
    }

    pub fn snapshot(&self) -> DocumentSnapshot {
        DocumentSnapshot { id: self.id.clone(), text: self.text.clone(), revision: self.revision,
            dirty: self.text != self.saved_text, bom: self.bom, line_ending: self.line_ending,
            last_origin: self.last_origin.clone(), transaction_id: self.transaction_id }
    }

    pub fn bytes_for(&self, text: &str) -> Vec<u8> {
        let body = match self.line_ending { LineEnding::Lf => text.to_owned(), LineEnding::Crlf => text.replace('\n', "\r\n") };
        let mut bytes = Vec::with_capacity(body.len() + if self.bom { 3 } else { 0 });
        if self.bom { bytes.extend_from_slice(&[0xef, 0xbb, 0xbf]); }
        bytes.extend_from_slice(body.as_bytes());
        bytes
    }

    pub fn byte_offset(&self, position: Position) -> Result<usize> {
        let mut base = 0;
        for (number, line) in self.text.split('\n').enumerate() {
            if number == position.line as usize {
                let mut column = 0;
                for (offset, character) in line.char_indices() {
                    if column == position.character { return Ok(base + offset); }
                    column += character.len_utf16() as u32;
                    if column > position.character { break; }
                }
                if column == position.character { return Ok(base + line.len()); }
                return Err(CoreError::new("invalid_position", "Érvénytelen UTF-16 oszlop vagy félbevágott surrogate pár."));
            }
            base += line.len() + 1;
        }
        Err(CoreError::new("invalid_position", "A megadott sor nem létezik."))
    }

    pub fn prepare_edits(&self, edits: Vec<TextEdit>) -> Result<(String, Vec<Patch>)> {
        if edits.len() > 1000 { return Err(CoreError::new("too_many_edits", "Legfeljebb 1000 csere küldhető egy műveletben.")); }
        let mut ranges = Vec::with_capacity(edits.len());
        for edit in edits {
            let start = self.byte_offset(edit.start)?;
            let end = self.byte_offset(edit.end)?;
            let replacement = edit.text.replace("\r\n", "\n");
            if start > end || replacement.contains(['\r', '\0']) {
                return Err(CoreError::new("invalid_edit", "Érvénytelen tartomány vagy szöveg."));
            }
            ranges.push((start, end, replacement));
        }
        ranges.sort_by_key(|range| range.0);
        for pair in ranges.windows(2) {
            if pair[0].1 > pair[1].0 || pair[0].0 == pair[1].0 {
                return Err(CoreError::new("overlapping_edits", "A cserék átfedik egymást."));
            }
        }
        let resulting_length = ranges.iter().try_fold(self.text.len(), |length, (start, end, text)|
            length.checked_sub(end - start).and_then(|length| length.checked_add(text.len())))
            .ok_or_else(|| CoreError::new("file_too_large", "A módosítás túl nagy."))?;
        if resulting_length > MAX_FILE_BYTES { return Err(CoreError::new("file_too_large", "A módosított fájl meghaladná az 1 MiB határt.")); }
        let mut result = self.text.clone();
        let mut patches = Vec::with_capacity(ranges.len());
        for (start, end, after) in ranges.into_iter().rev() {
            let before = result[start..end].to_owned();
            if before != after {
                result.replace_range(start..end, &after);
                patches.push(Patch { start, before, after });
            }
        }
        if self.bytes_for(&result).len() > MAX_FILE_BYTES { return Err(CoreError::new("file_too_large", "A kódolt fájl meghaladná az 1 MiB határt.")); }
        Ok((result, patches))
    }
}
