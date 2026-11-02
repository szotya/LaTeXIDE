//! macOS in-memory CoreGraphics parser and CommonCrypto digest adapter.
//!
//! Opens the document, checks every page/MediaBox, decodes and scans page content
//! streams. This is parser-based structural acceptance, not ISO conformance or a
//! rendering/security guarantee for arbitrary nested resources, fonts or images.
use super::{ArtifactChecks, CheckError};
use std::{
    ffi::{CStr, c_char, c_void},
    ptr,
};

type Raw = *const c_void;

#[repr(C)]
#[derive(Clone, Copy)]
struct Point {
    x: f64,
    y: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Size {
    width: f64,
    height: f64,
}
#[repr(C)]
#[derive(Clone, Copy)]
struct Rect {
    origin: Point,
    size: Size,
}

#[link(name = "CoreGraphics", kind = "framework")]
unsafe extern "C" {
    fn CGDataProviderCreateWithData(
        info: *mut c_void,
        data: Raw,
        size: usize,
        release: Option<unsafe extern "C" fn(*mut c_void, Raw, usize)>,
    ) -> Raw;
    fn CGDataProviderRelease(value: Raw);
    fn CGPDFDocumentCreateWithProvider(provider: Raw) -> Raw;
    fn CGPDFDocumentRelease(value: Raw);
    fn CGPDFDocumentIsEncrypted(document: Raw) -> bool;
    fn CGPDFDocumentGetNumberOfPages(document: Raw) -> usize;
    fn CGPDFDocumentGetPage(document: Raw, number: usize) -> Raw;
    fn CGPDFPageGetBoxRect(page: Raw, kind: i32) -> Rect;
    fn CGPDFPageGetDictionary(page: Raw) -> Raw;
    fn CGPDFDictionaryGetName(dict: Raw, key: *const c_char, out: *mut *const c_char) -> bool;
    fn CGPDFDictionaryGetObject(dict: Raw, key: *const c_char, out: *mut Raw) -> bool;
    fn CGPDFDictionaryGetInteger(dict: Raw, key: *const c_char, out: *mut isize) -> bool;
    fn CGPDFObjectGetType(object: Raw) -> i32;
    fn CGPDFObjectGetValue(object: Raw, kind: i32, value: *mut c_void) -> bool;
    fn CGPDFArrayGetCount(array: Raw) -> usize;
    fn CGPDFArrayGetStream(array: Raw, index: usize, out: *mut Raw) -> bool;
    fn CGPDFStreamCopyData(stream: Raw, format: *mut i32) -> Raw;
    fn CGPDFStreamGetDictionary(stream: Raw) -> Raw;
    fn CGPDFContentStreamCreateWithPage(page: Raw) -> Raw;
    fn CGPDFContentStreamRelease(value: Raw);
    fn CGPDFOperatorTableCreate() -> Raw;
    fn CGPDFOperatorTableRelease(value: Raw);
    fn CGPDFScannerCreate(stream: Raw, table: Raw, info: *mut c_void) -> Raw;
    fn CGPDFScannerRelease(value: Raw);
    fn CGPDFScannerScan(scanner: Raw) -> bool;
}
#[link(name = "CoreFoundation", kind = "framework")]
unsafe extern "C" {
    fn CFRelease(value: Raw);
    fn CFDataGetLength(value: Raw) -> isize;
}
#[link(name = "System")]
unsafe extern "C" {
    fn CC_SHA256(bytes: Raw, length: u32, output: *mut u8) -> *mut u8;
}

/// Create/Copy references are released exactly once, including on rejection.
struct Owned {
    value: Raw,
    release: unsafe extern "C" fn(Raw),
}
impl Owned {
    fn new(value: Raw, release: unsafe extern "C" fn(Raw)) -> Result<Self, CheckError> {
        if value.is_null() {
            Err(CheckError::InvalidData)
        } else {
            Ok(Self { value, release })
        }
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: only non-null Create/Copy references enter Owned; it is never cloned.
        unsafe { (self.release)(self.value) }
    }
}

/// Local bounds constrain accepted artifacts, not shared transport/editor quotas.
#[derive(Debug, Clone)]
pub struct CoreGraphicsChecks {
    pub max_pages: usize,
    pub max_page_streams: usize,
    pub max_decoded_stream_bytes: usize,
}
impl Default for CoreGraphicsChecks {
    fn default() -> Self {
        Self {
            max_pages: 4096,
            max_page_streams: 4096,
            max_decoded_stream_bytes: 16 * 1024 * 1024,
        }
    }
}

impl ArtifactChecks for CoreGraphicsChecks {
    fn sha256(&self, bytes: &[u8]) -> Result<String, CheckError> {
        let length = u32::try_from(bytes.len()).map_err(|_| CheckError::InvalidData)?;
        let mut digest = [0u8; 32];
        // SAFETY: the immutable input and writable 32-byte digest live for this call.
        let result = unsafe { CC_SHA256(bytes.as_ptr().cast(), length, digest.as_mut_ptr()) };
        if result.is_null() {
            return Err(CheckError::Unavailable);
        }
        Ok(digest.iter().map(|b| format!("{b:02x}")).collect())
    }

    fn validate_pdf(&self, pdf: &[u8]) -> Result<(), CheckError> {
        if !pdf.starts_with(b"%PDF-") || !pdf.trim_ascii_end().ends_with(b"%%EOF") {
            return Err(CheckError::InvalidData);
        }
        // SAFETY: provider borrows pdf only during this function. Reverse drop order
        // releases scanners/streams, then document, then provider before pdf can expire.
        // Borrowed pages/dictionaries/objects remain owned by the live document.
        unsafe {
            let provider = Owned::new(
                CGDataProviderCreateWithData(ptr::null_mut(), pdf.as_ptr().cast(), pdf.len(), None),
                CGDataProviderRelease,
            )?;
            let document = Owned::new(
                CGPDFDocumentCreateWithProvider(provider.value),
                CGPDFDocumentRelease,
            )?;
            if CGPDFDocumentIsEncrypted(document.value) {
                return Err(CheckError::InvalidData);
            }
            let count = CGPDFDocumentGetNumberOfPages(document.value);
            if count == 0 || count > self.max_pages {
                return Err(CheckError::InvalidData);
            }
            for number in 1..=count {
                let page = CGPDFDocumentGetPage(document.value, number);
                if page.is_null() {
                    return Err(CheckError::InvalidData);
                }
                let rect = CGPDFPageGetBoxRect(page, 0);
                if !rect.origin.x.is_finite()
                    || !rect.origin.y.is_finite()
                    || !rect.size.width.is_finite()
                    || !rect.size.height.is_finite()
                    || rect.size.width <= 0.0
                    || rect.size.height <= 0.0
                {
                    return Err(CheckError::InvalidData);
                }
                let dict = CGPDFPageGetDictionary(page);
                let mut kind = ptr::null();
                if dict.is_null()
                    || !CGPDFDictionaryGetName(dict, c"Type".as_ptr(), &mut kind)
                    || kind.is_null()
                    || CStr::from_ptr(kind).to_bytes() != b"Page"
                {
                    return Err(CheckError::InvalidData);
                }
                let mut contents = ptr::null();
                if CGPDFDictionaryGetObject(dict, c"Contents".as_ptr(), &mut contents) {
                    let kind = CGPDFObjectGetType(contents);
                    if kind != 1 {
                        let mut value = ptr::null();
                        if !matches!(kind, 7 | 9)
                            || !CGPDFObjectGetValue(contents, kind, (&mut value as *mut Raw).cast())
                        {
                            return Err(CheckError::InvalidData);
                        }
                        if kind == 9 {
                            self.decode_stream(value)?;
                        } else {
                            let count = CGPDFArrayGetCount(value);
                            if count > self.max_page_streams {
                                return Err(CheckError::InvalidData);
                            }
                            for index in 0..count {
                                let mut stream = ptr::null();
                                if !CGPDFArrayGetStream(value, index, &mut stream) {
                                    return Err(CheckError::InvalidData);
                                }
                                self.decode_stream(stream)?;
                            }
                        }
                    }
                }
                let content = Owned::new(
                    CGPDFContentStreamCreateWithPage(page),
                    CGPDFContentStreamRelease,
                )?;
                // A null table is permitted by the SDK declaration but crashed Scan
                // on our positive fixture. Supply a live empty table for every scan.
                let table = Owned::new(CGPDFOperatorTableCreate(), CGPDFOperatorTableRelease)?;
                let scanner = Owned::new(
                    CGPDFScannerCreate(content.value, table.value, ptr::null_mut()),
                    CGPDFScannerRelease,
                )?;
                if !CGPDFScannerScan(scanner.value) {
                    return Err(CheckError::InvalidData);
                }
            }
        }
        Ok(())
    }
}

impl CoreGraphicsChecks {
    fn decode_stream(&self, stream: Raw) -> Result<(), CheckError> {
        // SAFETY: caller passes a stream resolved by CoreGraphics in the live document.
        unsafe {
            let mut format = 0;
            let decoded = Owned::new(CGPDFStreamCopyData(stream, &mut format), CFRelease)?;
            let size = CFDataGetLength(decoded.value);
            if format != 0 || size < 0 || size as usize > self.max_decoded_stream_bytes {
                return Err(CheckError::InvalidData);
            }
            // CoreGraphics recovered our corrupt Flate stream as an empty CFData,
            // without failing Scan. Reject a non-empty stream decoded to zero bytes.
            // This intentionally also rejects valid compressed-empty streams: this
            // conservative first-slice policy is NOT complete PDF conformance.
            let mut encoded_length = 0isize;
            if !CGPDFDictionaryGetInteger(
                CGPDFStreamGetDictionary(stream),
                c"Length".as_ptr(),
                &mut encoded_length,
            ) || encoded_length < 0
                || (encoded_length > 0 && size == 0)
            {
                return Err(CheckError::InvalidData);
            }
        }
        Ok(())
    }
}
