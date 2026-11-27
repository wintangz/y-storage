use std::ffi::c_void;
use std::path::Path;

use windows_sys::Win32::Security::Cryptography::{
    CERT_CONTEXT, CERT_FIND_SUBJECT_CERT, CERT_INFO, CERT_NAME_ATTR_TYPE,
    CERT_NAME_SIMPLE_DISPLAY_TYPE, CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
    CERT_QUERY_FORMAT_FLAG_BINARY, CERT_QUERY_OBJECT_FILE, CMSG_SIGNER_INFO,
    CMSG_SIGNER_INFO_PARAM, CertCloseStore, CertFindCertificateInStore,
    CertFreeCertificateContext, CertGetNameStringW, CryptMsgClose, CryptMsgGetParam,
    CryptQueryObject, HCERTSTORE, PKCS_7_ASN_ENCODING, X509_ASN_ENCODING,
    szOID_ORGANIZATION_NAME,
};

use crate::paths::{from_wide_slice, to_wide_null};

struct StoreGuard(HCERTSTORE);

impl Drop for StoreGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CertCloseStore(self.0, 0);
            }
        }
    }
}

struct MsgGuard(*mut c_void);

impl Drop for MsgGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CryptMsgClose(self.0);
            }
        }
    }
}

struct CertGuard(*const CERT_CONTEXT);

impl Drop for CertGuard {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CertFreeCertificateContext(self.0);
            }
        }
    }
}

fn normalize_org_string(raw: &str) -> Option<String> {
    let cleaned: String = raw
        .chars()
        .map(|c| if c.is_control() { ' ' } else { c })
        .collect();
    let collapsed: Vec<&str> = cleaned.split_whitespace().collect();
    if collapsed.is_empty() {
        None
    } else {
        Some(collapsed.join(" "))
    }
}

unsafe fn query_cert_name(
    cert_ctx: *const CERT_CONTEXT,
    name_type: u32,
    type_para: *const c_void,
) -> Option<String> {
    let needed = unsafe {
        CertGetNameStringW(
            cert_ctx,
            name_type,
            0,
            type_para,
            std::ptr::null_mut(),
            0,
        )
    };
    if needed <= 1 {
        return None;
    }
    let mut buf = vec![0u16; needed as usize];
    let written = unsafe {
        CertGetNameStringW(
            cert_ctx,
            name_type,
            0,
            type_para,
            buf.as_mut_ptr(),
            needed,
        )
    };
    if written <= 1 {
        return None;
    }
    normalize_org_string(&from_wide_slice(&buf))
}

/// Extracts and normalizes the embedded Authenticode signer organization (`O`) for a file,
/// falling back to the certificate's simple display name (`CN`) if `O` is absent.
///
/// Fully offline: decodes the embedded PKCS#7 CMS structure from the file itself with no
/// revocation or network checks. Returns `None` for unsigned or catalog-only files.
#[must_use]
pub fn extract_authenticode_signer_org(path: impl AsRef<Path>) -> Option<String> {
    let path_ref = path.as_ref();
    if !path_ref.is_file() {
        return None;
    }
    let wpath = to_wide_null(&path_ref.to_string_lossy());
    let mut encoding: u32 = 0;
    let mut content_type: u32 = 0;
    let mut format_type: u32 = 0;
    let mut hstore: HCERTSTORE = std::ptr::null_mut();
    let mut hmsg: *mut c_void = std::ptr::null_mut();

    let ok = unsafe {
        CryptQueryObject(
            CERT_QUERY_OBJECT_FILE,
            wpath.as_ptr().cast::<c_void>(),
            CERT_QUERY_CONTENT_FLAG_PKCS7_SIGNED_EMBED,
            CERT_QUERY_FORMAT_FLAG_BINARY,
            0,
            &mut encoding,
            &mut content_type,
            &mut format_type,
            &mut hstore,
            &mut hmsg,
            std::ptr::null_mut(),
        )
    };
    let _store_guard = StoreGuard(hstore);
    let _msg_guard = MsgGuard(hmsg);
    if ok == 0 || hstore.is_null() || hmsg.is_null() {
        return None;
    }

    let mut signer_info_size: u32 = 0;
    let ok_size = unsafe {
        CryptMsgGetParam(
            hmsg,
            CMSG_SIGNER_INFO_PARAM,
            0,
            std::ptr::null_mut(),
            &mut signer_info_size,
        )
    };
    if ok_size == 0 || (signer_info_size as usize) < std::mem::size_of::<CMSG_SIGNER_INFO>() {
        return None;
    }

    let mut signer_buf = vec![0u8; signer_info_size as usize];
    let ok_param = unsafe {
        CryptMsgGetParam(
            hmsg,
            CMSG_SIGNER_INFO_PARAM,
            0,
            signer_buf.as_mut_ptr().cast::<c_void>(),
            &mut signer_info_size,
        )
    };
    if ok_param == 0 {
        return None;
    }

    // SAFETY: signer_buf was sized by CryptMsgGetParam and contains a valid CMSG_SIGNER_INFO header
    // plus its variable-length Issuer/SerialNumber blobs; zero-initialized CERT_INFO takes pointers
    // valid for the lifetime of signer_buf.
    let signer_info = unsafe { &*signer_buf.as_ptr().cast::<CMSG_SIGNER_INFO>() };
    let mut cert_info: CERT_INFO = unsafe { std::mem::zeroed() };
    cert_info.Issuer = signer_info.Issuer;
    cert_info.SerialNumber = signer_info.SerialNumber;

    let cert_ctx = unsafe {
        CertFindCertificateInStore(
            hstore,
            X509_ASN_ENCODING | PKCS_7_ASN_ENCODING,
            0,
            CERT_FIND_SUBJECT_CERT,
            (&cert_info as *const CERT_INFO).cast::<c_void>(),
            std::ptr::null(),
        )
    };
    if cert_ctx.is_null() {
        return None;
    }
    let _cert_guard = CertGuard(cert_ctx);

    unsafe {
        query_cert_name(cert_ctx, CERT_NAME_ATTR_TYPE, szOID_ORGANIZATION_NAME.cast::<c_void>())
            .or_else(|| query_cert_name(cert_ctx, CERT_NAME_SIMPLE_DISPLAY_TYPE, std::ptr::null()))
    }
}
