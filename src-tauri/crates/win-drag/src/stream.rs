//! The stream a drop target reads one file from. It opens the source lazily
//! at the current position and reopens it after a seek or an error.

use std::ffi::c_void;
use std::io::{ErrorKind, Read};
use std::sync::{Arc, Mutex, MutexGuard};

use windows::core::{implement, Error, Ref, Result, HRESULT, PWSTR};
use windows::Win32::Foundation::{ERROR_UNEXP_NET_ERR, E_NOTIMPL, STG_E_ACCESSDENIED, STG_E_INVALIDFUNCTION, STG_E_INVALIDPOINTER, S_OK};
use windows::Win32::System::Com::{
    CoTaskMemAlloc, ISequentialStream_Impl, IStream, IStream_Impl, LOCKTYPE, STATFLAG, STATFLAG_NONAME, STATSTG, STGC, STGM_READ, STGTY_STREAM,
    STREAM_SEEK, STREAM_SEEK_CUR, STREAM_SEEK_END, STREAM_SEEK_SET,
};

use crate::{filetime, Entry, Source};

/// The error a drop target reports when the source fails mid-copy.
fn source_failed() -> HRESULT {
    HRESULT::from_win32(ERROR_UNEXP_NET_ERR.0)
}

struct Reader {
    at: u64,
    inner: Box<dyn Read + Send>,
}

struct State {
    pos: u64,
    reader: Option<Reader>,
}

#[implement(IStream, Agile = false)]
pub(crate) struct FileStream {
    source: Arc<dyn Source>,
    entry: Entry,
    state: Mutex<State>,
}

impl FileStream {
    pub(crate) fn new(source: Arc<dyn Source>, entry: Entry) -> Self {
        Self { source, entry, state: Mutex::new(State { pos: 0, reader: None }) }
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|p| p.into_inner())
    }

    /// Fill `buf` from the current position; fewer bytes only at end of file.
    fn read_into(&self, buf: &mut [u8]) -> std::result::Result<usize, HRESULT> {
        let mut state = self.state();
        let pos = state.pos;
        if state.reader.as_ref().map_or(true, |r| r.at != pos) {
            state.reader = None;
            match self.source.open(&self.entry, pos) {
                Ok(inner) => state.reader = Some(Reader { at: pos, inner }),
                Err(_) => return Err(source_failed()),
            }
        }
        let reader = state.reader.as_mut().expect("reader was just opened");
        let mut filled = 0;
        let mut failed = false;
        while filled < buf.len() {
            match reader.inner.read(&mut buf[filled..]) {
                Ok(0) => break,
                Ok(n) => filled += n,
                Err(e) if e.kind() == ErrorKind::Interrupted => continue,
                Err(_) => {
                    failed = true;
                    break;
                }
            }
        }
        reader.at += filled as u64;
        state.pos += filled as u64;
        if failed {
            // Report what arrived; the next read reopens and surfaces the error.
            state.reader = None;
            if filled == 0 {
                return Err(source_failed());
            }
        }
        Ok(filled)
    }
}

impl ISequentialStream_Impl for FileStream_Impl {
    fn Read(&self, pv: *mut c_void, cb: u32, pcbread: *mut u32) -> HRESULT {
        if pv.is_null() {
            return STG_E_INVALIDPOINTER;
        }
        let buf = unsafe { std::slice::from_raw_parts_mut(pv.cast::<u8>(), cb as usize) };
        let (read, hr) = match self.read_into(buf) {
            Ok(n) => (n, S_OK),
            Err(hr) => (0, hr),
        };
        if let Some(out) = unsafe { pcbread.as_mut() } {
            *out = read as u32;
        }
        hr
    }

    fn Write(&self, _: *const c_void, _: u32, pcbwritten: *mut u32) -> HRESULT {
        if let Some(out) = unsafe { pcbwritten.as_mut() } {
            *out = 0;
        }
        STG_E_ACCESSDENIED
    }
}

impl IStream_Impl for FileStream_Impl {
    fn Seek(&self, dlibmove: i64, dworigin: STREAM_SEEK, plibnewposition: *mut u64) -> Result<()> {
        let mut state = self.state();
        let base = match dworigin {
            STREAM_SEEK_SET => 0i128,
            STREAM_SEEK_CUR => state.pos as i128,
            STREAM_SEEK_END => self.entry.size as i128,
            _ => return Err(STG_E_INVALIDFUNCTION.into()),
        };
        let target = base + dlibmove as i128;
        let target = u64::try_from(target).map_err(|_| Error::from(STG_E_INVALIDFUNCTION))?;
        state.pos = target;
        if let Some(out) = unsafe { plibnewposition.as_mut() } {
            *out = target;
        }
        Ok(())
    }

    fn SetSize(&self, _: u64) -> Result<()> {
        Err(STG_E_ACCESSDENIED.into())
    }

    fn CopyTo(&self, pstm: Ref<'_, IStream>, cb: u64, pcbread: *mut u64, pcbwritten: *mut u64) -> Result<()> {
        let target = pstm.ok()?;
        let mut buf = vec![0u8; 1 << 20];
        let (mut read, mut written) = (0u64, 0u64);
        let result = loop {
            if read >= cb {
                break Ok(());
            }
            let want = (cb - read).min(buf.len() as u64) as usize;
            let n = match self.read_into(&mut buf[..want]) {
                Ok(0) => break Ok(()),
                Ok(n) => n,
                Err(hr) => break Err(Error::from(hr)),
            };
            read += n as u64;
            let mut done = 0u32;
            let hr = unsafe { target.Write(buf.as_ptr().cast(), n as u32, Some(&mut done)) };
            written += u64::from(done);
            if hr.is_err() {
                break Err(Error::from(hr));
            }
        };
        if let Some(out) = unsafe { pcbread.as_mut() } {
            *out = read;
        }
        if let Some(out) = unsafe { pcbwritten.as_mut() } {
            *out = written;
        }
        result
    }

    fn Commit(&self, _: &STGC) -> Result<()> {
        Ok(())
    }

    fn Revert(&self) -> Result<()> {
        Ok(())
    }

    fn LockRegion(&self, _: u64, _: u64, _: &LOCKTYPE) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }

    fn UnlockRegion(&self, _: u64, _: u64, _: u32) -> Result<()> {
        Err(STG_E_INVALIDFUNCTION.into())
    }

    fn Stat(&self, pstatstg: *mut STATSTG, grfstatflag: &STATFLAG) -> Result<()> {
        let out = unsafe { pstatstg.as_mut() }.ok_or_else(|| Error::from(STG_E_INVALIDPOINTER))?;
        *out = STATSTG::default();
        out.r#type = STGTY_STREAM.0 as u32;
        out.cbSize = self.entry.size;
        out.grfMode = STGM_READ;
        if let Some(time) = self.entry.modified.and_then(filetime) {
            out.mtime = time;
        }
        if grfstatflag.0 & STATFLAG_NONAME.0 == 0 {
            let name = self.entry.path.rsplit('\\').next().unwrap_or(&self.entry.path);
            let wide: Vec<u16> = name.encode_utf16().chain(Some(0)).collect();
            let copy = unsafe { CoTaskMemAlloc(wide.len() * 2) }.cast::<u16>();
            if !copy.is_null() {
                unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), copy, wide.len()) };
                out.pwcsName = PWSTR(copy);
            }
        }
        Ok(())
    }

    fn Clone(&self) -> Result<IStream> {
        Err(E_NOTIMPL.into())
    }
}
