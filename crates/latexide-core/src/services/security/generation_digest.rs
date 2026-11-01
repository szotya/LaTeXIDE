//! SDK CommonCrypto SHA-256, never a metadata or noncryptographic hash fallback.
use super::GenerationError;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Digest(pub(super) [u8; 32]);
impl Digest {
    pub fn of(bytes: &[u8]) -> Result<Self, GenerationError> {
        #[cfg(target_os = "macos")]
        {
            // CommonDigest.h: CC_LONG is uint32_t; CC_SHA256 returns md.
            unsafe extern "C" {
                fn CC_SHA256(data: *const std::ffi::c_void, len: u32, md: *mut u8) -> *mut u8;
            }
            let length = u32::try_from(bytes.len()).map_err(|_| GenerationError::Quota)?;
            let mut digest = [0; 32];
            if unsafe { CC_SHA256(bytes.as_ptr().cast(), length, digest.as_mut_ptr()) }
                != digest.as_mut_ptr()
            {
                return Err(GenerationError::IsolationUnavailable);
            }
            Ok(Self(digest))
        }
        #[cfg(not(target_os = "macos"))]
        {
            let _ = bytes;
            Err(GenerationError::IsolationUnavailable)
        }
    }
    pub fn hex(self) -> String {
        self.0.iter().map(|byte| format!("{byte:02x}")).collect()
    }
}
