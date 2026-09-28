//! The data object handed to drop targets: a file-group descriptor for the
//! folders and files, a stream per file on demand, and "copy" as the
//! preferred effect. Every other format (drag image, drop descriptions,
//! what the target reports back) goes to a stock shell data object inside.

use std::mem::{size_of, ManuallyDrop};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};

use windows::core::{implement, Error, Ref, Result, BOOL, HRESULT};
use windows::Win32::Foundation::{
    GlobalFree, DATA_S_SAMEFORMATETC, DV_E_LINDEX, DV_E_TYMED, E_FAIL, E_NOTIMPL, E_OUTOFMEMORY, E_POINTER, OLE_E_ADVISENOTSUPPORTED,
    S_OK,
};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::System::Com::{
    CoTaskMemFree, IAdviseSink, IBindCtx, IDataObject, IDataObject_Impl, IEnumFORMATETC, IEnumSTATDATA, IStream, DATADIR_GET,
    DVASPECT_CONTENT, FORMATETC, STGMEDIUM, STGMEDIUM_0, TYMED, TYMED_HGLOBAL, TYMED_ISTREAM,
};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows::Win32::UI::Shell::{
    IDataObjectAsyncCapability, IDataObjectAsyncCapability_Impl, SHCreateDataObject, SHCreateStdEnumFmtEtc, CFSTR_FILECONTENTS,
    CFSTR_FILEDESCRIPTORW, CFSTR_PREFERREDDROPEFFECT, FD_ATTRIBUTES, FD_FILESIZE, FD_PROGRESSUI, FD_UNICODE, FD_WRITESTIME,
    FILEDESCRIPTORW,
};

use crate::stream::FileStream;
use crate::{filetime, Entry, Source, MAX_PATH_LEN};

/// The stock shell data object. It is only ever used behind the mutex in
/// [`DataObject`], one call at a time.
struct Inner(IDataObject);
unsafe impl Send for Inner {}
unsafe impl Sync for Inner {}

#[implement(IDataObject, IDataObjectAsyncCapability, Agile = false)]
pub(crate) struct DataObject {
    source: Arc<dyn Source>,
    entries: OnceLock<std::result::Result<Arc<Vec<Entry>>, String>>,
    inner: Mutex<Inner>,
    cf_descriptor: u16,
    cf_contents: u16,
    cf_effect: u16,
    async_mode: AtomicBool,
    in_operation: AtomicBool,
}

impl DataObject {
    pub(crate) fn new(source: Arc<dyn Source>) -> Result<Self> {
        let inner: IDataObject = unsafe { SHCreateDataObject(None, None, None::<&IDataObject>)? };
        Ok(Self {
            source,
            entries: OnceLock::new(),
            inner: Mutex::new(Inner(inner)),
            cf_descriptor: clipboard_format(CFSTR_FILEDESCRIPTORW),
            cf_contents: clipboard_format(CFSTR_FILECONTENTS),
            cf_effect: clipboard_format(CFSTR_PREFERREDDROPEFFECT),
            // Ask targets to copy on their own thread, so Explorer stays
            // responsive while a large file streams in.
            async_mode: AtomicBool::new(true),
            in_operation: AtomicBool::new(false),
        })
    }

    fn inner(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// The entries the descriptor lists, fetched from the source once.
    fn entries(&self) -> Result<Arc<Vec<Entry>>> {
        let listed = self.entries.get_or_init(|| {
            self.source.entries().map(|all| {
                Arc::new(all.into_iter().filter(|e| !e.path.is_empty() && e.path.encode_utf16().count() <= MAX_PATH_LEN).collect())
            })
        });
        match listed {
            Ok(entries) => Ok(entries.clone()),
            Err(message) => Err(Error::new(E_FAIL, message.as_str())),
        }
    }

    fn own_tymed(&self, format: u16) -> Option<TYMED> {
        if format == self.cf_descriptor || format == self.cf_effect {
            Some(TYMED_HGLOBAL)
        } else if format == self.cf_contents {
            Some(TYMED_ISTREAM)
        } else {
            None
        }
    }
}

fn clipboard_format(name: windows::core::PCWSTR) -> u16 {
    unsafe { RegisterClipboardFormatW(name) as u16 }
}

fn format(cf: u16, tymed: TYMED) -> FORMATETC {
    FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex: -1, tymed: tymed.0 as u32 }
}

/// FILEGROUPDESCRIPTORW: a count, then one descriptor per entry.
pub(crate) fn descriptor_bytes(entries: &[Entry]) -> Vec<u8> {
    let mut out = Vec::with_capacity(4 + entries.len() * size_of::<FILEDESCRIPTORW>());
    out.extend_from_slice(&(entries.len() as u32).to_le_bytes());
    for entry in entries {
        let mut d = FILEDESCRIPTORW::default();
        let mut flags = FD_ATTRIBUTES.0 | FD_PROGRESSUI.0 | FD_UNICODE.0;
        if entry.is_dir {
            d.dwFileAttributes = FILE_ATTRIBUTE_DIRECTORY.0;
        } else {
            flags |= FD_FILESIZE.0;
            d.dwFileAttributes = FILE_ATTRIBUTE_NORMAL.0;
            d.nFileSizeHigh = (entry.size >> 32) as u32;
            d.nFileSizeLow = entry.size as u32;
        }
        if let Some(time) = entry.modified.and_then(filetime) {
            flags |= FD_WRITESTIME.0;
            d.ftLastWriteTime = time;
        }
        d.dwFlags = flags as u32;
        let mut name = [0u16; 260];
        for (slot, unit) in name.iter_mut().zip(entry.path.encode_utf16().take(MAX_PATH_LEN)) {
            *slot = unit;
        }
        d.cFileName = name;
        // SAFETY: FILEDESCRIPTORW is plain old data (packed, no padding).
        out.extend_from_slice(unsafe { std::slice::from_raw_parts((&d as *const FILEDESCRIPTORW).cast::<u8>(), size_of::<FILEDESCRIPTORW>()) });
    }
    out
}

fn hglobal_medium(bytes: &[u8]) -> Result<STGMEDIUM> {
    unsafe {
        let handle = GlobalAlloc(GMEM_MOVEABLE, bytes.len().max(1))?;
        let target = GlobalLock(handle).cast::<u8>();
        if target.is_null() {
            let _ = GlobalFree(Some(handle));
            return Err(E_OUTOFMEMORY.into());
        }
        std::ptr::copy_nonoverlapping(bytes.as_ptr(), target, bytes.len());
        // Reports "failure" once the lock count is back to zero; nothing to handle.
        let _ = GlobalUnlock(handle);
        Ok(STGMEDIUM { tymed: TYMED_HGLOBAL.0 as u32, u: STGMEDIUM_0 { hGlobal: handle }, pUnkForRelease: ManuallyDrop::new(None) })
    }
}

impl IDataObject_Impl for DataObject_Impl {
    fn GetData(&self, pformatetcin: *const FORMATETC) -> Result<STGMEDIUM> {
        let wanted = unsafe { pformatetcin.as_ref() }.ok_or_else(|| Error::from(E_POINTER))?;
        let Some(tymed) = self.own_tymed(wanted.cfFormat) else {
            return unsafe { self.inner().0.GetData(pformatetcin) };
        };
        if wanted.tymed & tymed.0 as u32 == 0 {
            return Err(DV_E_TYMED.into());
        }
        if wanted.cfFormat == self.cf_descriptor {
            hglobal_medium(&descriptor_bytes(&self.entries()?))
        } else if wanted.cfFormat == self.cf_effect {
            hglobal_medium(&DROPEFFECT_COPY.0.to_le_bytes())
        } else {
            let entries = self.entries()?;
            let entry = usize::try_from(wanted.lindex)
                .ok()
                .and_then(|i| entries.get(i))
                .filter(|e| !e.is_dir)
                .ok_or_else(|| Error::from(DV_E_LINDEX))?;
            let stream: IStream = FileStream::new(self.source.clone(), entry.clone()).into();
            Ok(STGMEDIUM {
                tymed: TYMED_ISTREAM.0 as u32,
                u: STGMEDIUM_0 { pstm: ManuallyDrop::new(Some(stream)) },
                pUnkForRelease: ManuallyDrop::new(None),
            })
        }
    }

    fn GetDataHere(&self, pformatetc: *const FORMATETC, pmedium: *mut STGMEDIUM) -> Result<()> {
        unsafe { self.inner().0.GetDataHere(pformatetc, pmedium) }
    }

    fn QueryGetData(&self, pformatetc: *const FORMATETC) -> HRESULT {
        let Some(wanted) = (unsafe { pformatetc.as_ref() }) else { return E_POINTER };
        match self.own_tymed(wanted.cfFormat) {
            Some(tymed) if wanted.tymed & tymed.0 as u32 != 0 => S_OK,
            Some(_) => DV_E_TYMED,
            None => unsafe { self.inner().0.QueryGetData(pformatetc) },
        }
    }

    fn GetCanonicalFormatEtc(&self, pformatectin: *const FORMATETC, pformatetcout: *mut FORMATETC) -> HRESULT {
        let (Some(input), Some(output)) = (unsafe { pformatectin.as_ref() }, unsafe { pformatetcout.as_mut() }) else {
            return E_POINTER;
        };
        *output = *input;
        output.ptd = std::ptr::null_mut();
        DATA_S_SAMEFORMATETC
    }

    fn SetData(&self, pformatetc: *const FORMATETC, pmedium: *const STGMEDIUM, frelease: BOOL) -> Result<()> {
        unsafe { self.inner().0.SetData(pformatetc, pmedium, frelease.as_bool()) }
    }

    fn EnumFormatEtc(&self, dwdirection: u32) -> Result<IEnumFORMATETC> {
        if dwdirection != DATADIR_GET.0 as u32 {
            return Err(E_NOTIMPL.into());
        }
        let own = [self.cf_descriptor, self.cf_contents, self.cf_effect];
        let mut formats = vec![
            format(self.cf_descriptor, TYMED_HGLOBAL),
            format(self.cf_contents, TYMED_ISTREAM),
            format(self.cf_effect, TYMED_HGLOBAL),
        ];
        if let Ok(others) = unsafe { self.inner().0.EnumFormatEtc(dwdirection) } {
            loop {
                let mut one = [FORMATETC::default()];
                let mut fetched = 0u32;
                if unsafe { others.Next(&mut one, Some(&mut fetched)) } != S_OK || fetched == 0 {
                    break;
                }
                if !one[0].ptd.is_null() {
                    unsafe { CoTaskMemFree(Some(one[0].ptd.cast())) };
                    one[0].ptd = std::ptr::null_mut();
                }
                if !own.contains(&one[0].cfFormat) {
                    formats.push(one[0]);
                }
            }
        }
        unsafe { SHCreateStdEnumFmtEtc(&formats) }
    }

    fn DAdvise(&self, _: *const FORMATETC, _: u32, _: Ref<'_, IAdviseSink>) -> Result<u32> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn DUnadvise(&self, _: u32) -> Result<()> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }

    fn EnumDAdvise(&self) -> Result<IEnumSTATDATA> {
        Err(OLE_E_ADVISENOTSUPPORTED.into())
    }
}

impl IDataObjectAsyncCapability_Impl for DataObject_Impl {
    fn SetAsyncMode(&self, fdoopasync: BOOL) -> Result<()> {
        self.async_mode.store(fdoopasync.as_bool(), Ordering::SeqCst);
        Ok(())
    }

    fn GetAsyncMode(&self) -> Result<BOOL> {
        Ok(self.async_mode.load(Ordering::SeqCst).into())
    }

    fn StartOperation(&self, _: Ref<'_, IBindCtx>) -> Result<()> {
        self.in_operation.store(true, Ordering::SeqCst);
        self.source.copy_started();
        Ok(())
    }

    fn InOperation(&self) -> Result<BOOL> {
        Ok(self.in_operation.load(Ordering::SeqCst).into())
    }

    fn EndOperation(&self, hresult: HRESULT, _: Ref<'_, IBindCtx>, dweffects: u32) -> Result<()> {
        self.in_operation.store(false, Ordering::SeqCst);
        self.source.copy_ended(hresult.is_ok() && dweffects != DROPEFFECT_NONE.0);
        Ok(())
    }
}
