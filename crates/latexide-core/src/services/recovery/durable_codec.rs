//! Byte format only. All integers little-endian; full header+payload SHA-256.
use super::*;
const MAGIC: &[u8; 8] = b"LXRECOV\0";
const HEADER: usize = 17;
struct Frame<'a> {
    kind: u8,
    body: &'a [u8],
    next: usize,
    digest: String,
}
fn frame(kind: u8, body: Vec<u8>) -> Result<Vec<u8>> {
    if body.len() > MAX_STORAGE_BYTES {
        return Err(DurableError::Quota);
    }
    let mut b = MAGIC.to_vec();
    b.extend(PRIVATE_VERSION.to_le_bytes());
    b.push(kind);
    b.extend((body.len() as u32).to_le_bytes());
    b.extend(body);
    let digest = raw_digest(&b);
    b.extend(digest.as_bytes());
    Ok(b)
}
fn parse(bytes: &[u8], at: usize) -> Result<Frame<'_>> {
    let h = bytes
        .get(at..at.checked_add(HEADER).ok_or(DurableError::Quota)?)
        .ok_or(DurableError::Corrupt("short header"))?;
    if &h[..8] != MAGIC {
        return Err(DurableError::Corrupt("magic"));
    }
    let version = u32::from_le_bytes(h[8..12].try_into().unwrap());
    if version != PRIVATE_VERSION {
        return Err(DurableError::UnsupportedVersion(version));
    }
    let n = u32::from_le_bytes(h[13..17].try_into().unwrap()) as usize;
    if n > MAX_STORAGE_BYTES {
        return Err(DurableError::Quota);
    }
    let end = at
        .checked_add(HEADER)
        .and_then(|v| v.checked_add(n))
        .ok_or(DurableError::Quota)?;
    let next = end.checked_add(64).ok_or(DurableError::Quota)?;
    let body = bytes
        .get(at + HEADER..end)
        .ok_or(DurableError::Corrupt("short payload"))?;
    let hash = bytes
        .get(end..next)
        .ok_or(DurableError::Corrupt("short checksum"))?;
    if raw_digest(&bytes[at..end]).as_bytes() != hash {
        return Err(DurableError::Corrupt("frame checksum"));
    }
    Ok(Frame {
        kind: h[12],
        body,
        next,
        digest: String::from_utf8(hash.to_vec()).map_err(|_| DurableError::Corrupt("hash utf8"))?,
    })
}
pub(super) fn frame_digest(bytes: &[u8]) -> Result<String> {
    let f = parse(bytes, 0)?;
    if f.next != bytes.len() {
        return Err(DurableError::Corrupt("frame trailing bytes"));
    }
    Ok(f.digest)
}
struct Put(Vec<u8>);
impl Put {
    fn u8(&mut self, v: u8) {
        self.0.push(v)
    }
    fn u32(&mut self, v: u32) {
        self.0.extend(v.to_le_bytes())
    }
    fn u64(&mut self, v: u64) {
        self.0.extend(v.to_le_bytes())
    }
    fn bytes(&mut self, v: &[u8]) {
        self.u32(v.len() as u32);
        self.0.extend(v)
    }
    fn string(&mut self, v: &str) {
        self.bytes(v.as_bytes())
    }
    fn scope(&mut self, s: &StoreScope) {
        self.string(&s.project_id);
        self.string(&s.session_id);
        self.string(&s.nonce)
    }
    fn enc(&mut self, e: Encoding) {
        self.u8(u8::from(e.bom));
        self.u8(u8::from(e.crlf))
    }
}
struct Get<'a> {
    b: &'a [u8],
    at: usize,
}
impl<'a> Get<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8]> {
        let end = self.at.checked_add(n).ok_or(DurableError::Quota)?;
        let x = self
            .b
            .get(self.at..end)
            .ok_or(DurableError::Corrupt("payload field"))?;
        self.at = end;
        Ok(x)
    }
    fn u8(&mut self) -> Result<u8> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn bytes(&mut self, max: usize) -> Result<Vec<u8>> {
        let n = self.u32()? as usize;
        if n > max {
            return Err(DurableError::Quota);
        }
        Ok(self.take(n)?.to_vec())
    }
    fn string(&mut self, max: usize) -> Result<String> {
        String::from_utf8(self.bytes(max)?).map_err(|_| DurableError::Corrupt("string utf8"))
    }
    fn boolean(&mut self) -> Result<bool> {
        match self.u8()? {
            0 => Ok(false),
            1 => Ok(true),
            _ => Err(DurableError::Corrupt("boolean")),
        }
    }
    fn enc(&mut self) -> Result<Encoding> {
        Ok(Encoding {
            bom: self.boolean()?,
            crlf: self.boolean()?,
        })
    }
    fn scope(&mut self) -> Result<StoreScope> {
        let s = StoreScope {
            project_id: self.string(128)?,
            session_id: self.string(128)?,
            nonce: self.string(64)?,
        };
        validate_scope(&s)?;
        Ok(s)
    }
    fn end(&self) -> Result<()> {
        if self.at != self.b.len() {
            Err(DurableError::Corrupt("payload trailing bytes"))
        } else {
            Ok(())
        }
    }
}
fn get_frame(bytes: &[u8], at: usize, kind: u8) -> Result<(Get<'_>, usize, String)> {
    let f = parse(bytes, at)?;
    if f.kind != kind {
        return Err(DurableError::Corrupt("frame kind"));
    }
    Ok((Get { b: f.body, at: 0 }, f.next, f.digest))
}
pub(super) fn checkpoint(
    s: &StoreScope,
    g: u64,
    cp: &Checkpoint,
    enc: &[Encoding],
) -> Result<Vec<u8>> {
    validate_scope(s)?;
    validate_checkpoint(cp, enc)?;
    let mut b = Put(Vec::new());
    b.scope(s);
    b.u64(g);
    b.u32(cp.schema);
    b.u64(cp.sequence);
    b.u32(cp.documents.len() as u32);
    for (d, e) in cp.documents.iter().zip(enc) {
        b.string(&d.document_id);
        b.u64(d.revision);
        b.u8(u8::from(d.dirty));
        b.enc(*e);
        b.bytes(&d.utf8)
    }
    frame(1, b.0)
}
pub(super) fn decode_checkpoint(
    bytes: &[u8],
) -> Result<(StoreScope, u64, Checkpoint, Vec<Encoding>)> {
    let (mut b, next, _) = get_frame(bytes, 0, 1)?;
    if next != bytes.len() {
        return Err(DurableError::Corrupt("checkpoint trailing frame"));
    }
    let s = b.scope()?;
    let g = b.u64()?;
    if g == 0 {
        return Err(DurableError::Corrupt("generation zero"));
    }
    let schema = b.u32()?;
    let sequence = b.u64()?;
    let n = b.u32()? as usize;
    if n > 10 {
        return Err(DurableError::Quota);
    }
    let mut documents = Vec::new();
    let mut enc = Vec::new();
    for _ in 0..n {
        let document_id = b.string(4096)?;
        let revision = b.u64()?;
        let dirty = b.boolean()?;
        enc.push(b.enc()?);
        let utf8 = b.bytes(latexide_core::MAX_FILE_BYTES)?;
        documents.push(latexide_contracts::recovery::RecoveredDocument {
            document_id,
            revision,
            utf8,
            dirty,
        });
    }
    b.end()?;
    Ok((
        s,
        g,
        Checkpoint {
            schema,
            sequence,
            documents,
        },
        enc,
    ))
}
pub(super) fn head(s: &StoreScope, g: u64, cp_hash: &str, sequence: u64) -> Result<Vec<u8>> {
    let mut b = Put(Vec::new());
    b.scope(s);
    b.u64(g);
    b.string(cp_hash);
    b.u64(sequence);
    frame(2, b.0)
}
pub(super) fn decode_head(bytes: &[u8]) -> Result<(StoreScope, u64, String, u64, usize, String)> {
    let (mut b, n, digest) = get_frame(bytes, 0, 2)?;
    let s = b.scope()?;
    let g = b.u64()?;
    let hash = b.string(64)?;
    let sequence = b.u64()?;
    b.end()?;
    Ok((s, g, hash, sequence, n, digest))
}
pub(super) fn entry(
    s: &StoreScope,
    g: u64,
    prev: &str,
    e: &JournalEntry,
    enc: Encoding,
) -> Result<Vec<u8>> {
    let mut b = Put(Vec::new());
    b.scope(s);
    b.u64(g);
    b.string(prev);
    b.u64(e.sequence);
    b.string(&e.document_id);
    b.u64(e.revision);
    b.enc(enc);
    b.bytes(&e.utf8);
    b.string(&e.checksum);
    frame(3, b.0)
}
pub(super) fn decode_entry(
    bytes: &[u8],
    at: usize,
) -> Result<(
    StoreScope,
    u64,
    String,
    JournalEntry,
    Encoding,
    usize,
    String,
)> {
    let (mut b, n, digest) = get_frame(bytes, at, 3)?;
    let s = b.scope()?;
    let g = b.u64()?;
    let prev = b.string(64)?;
    let sequence = b.u64()?;
    let document_id = b.string(4096)?;
    let revision = b.u64()?;
    let enc = b.enc()?;
    let utf8 = b.bytes(latexide_core::MAX_FILE_BYTES)?;
    let checksum = b.string(64)?;
    b.end()?;
    Ok((
        s,
        g,
        prev,
        JournalEntry {
            sequence,
            document_id,
            revision,
            utf8,
            checksum,
        },
        enc,
        n,
        digest,
    ))
}
pub(super) fn validate_ack(a: &AckReceipt) -> Result<()> {
    validate_scope(&a.scope)?;
    if a.generation == 0
        || a.journal_bytes == 0
        || a.journal_bytes > MAX_STORAGE_BYTES as u64
        || ![
            &a.checkpoint_sha256,
            &a.journal_sha256,
            &a.last_frame_sha256,
        ]
        .iter()
        .all(|s| is_sha256(s))
    {
        return Err(DurableError::Corrupt("ack receipt shape"));
    }
    Ok(())
}
pub(super) fn receipt(a: &AckReceipt) -> Result<Vec<u8>> {
    validate_ack(a)?;
    let mut b = Put(Vec::new());
    b.scope(&a.scope);
    b.u64(a.generation);
    b.u64(a.sequence);
    b.string(&a.checkpoint_sha256);
    b.u64(a.journal_bytes);
    b.string(&a.journal_sha256);
    b.string(&a.last_frame_sha256);
    frame(4, b.0)
}
pub(super) fn decode_receipt(bytes: &[u8]) -> Result<AckReceipt> {
    let (mut b, n, _) = get_frame(bytes, 0, 4)?;
    if n != bytes.len() {
        return Err(DurableError::Corrupt("receipt trailing frame"));
    }
    let a = AckReceipt {
        scope: b.scope()?,
        generation: b.u64()?,
        sequence: b.u64()?,
        checkpoint_sha256: b.string(64)?,
        journal_bytes: b.u64()?,
        journal_sha256: b.string(64)?,
        last_frame_sha256: b.string(64)?,
    };
    b.end()?;
    validate_ack(&a)?;
    Ok(a)
}

pub(super) fn entry_count(bytes: &[u8]) -> Result<usize> {
    let mut at = 0;
    let mut count = 0;
    while at < bytes.len() {
        let f = parse(bytes, at)?;
        if at == 0 {
            if f.kind != 2 {
                return Err(DurableError::Corrupt("journal head kind"));
            }
        } else {
            if f.kind != 3 {
                return Err(DurableError::Corrupt("journal entry kind"));
            }
            count += 1;
        }
        at = f.next;
    }
    Ok(count)
}
