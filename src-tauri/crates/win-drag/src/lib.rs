//! Drag files that are not on this computer yet ("virtual files") out of a
//! window into Explorer, the desktop or any other drop target, on Windows.
//!
//! The data object offers `FileGroupDescriptorW` and `FileContents`: the drop
//! target first learns the folders, names and sizes, then pulls each file's
//! bytes through [`Source::open`] while it copies, with its own progress,
//! conflict and error dialogs. Nothing is fetched before the drop, and a
//! cancelled drag costs nothing.
//!
//! The data object and its streams live in the process's multithreaded
//! apartment. A drop target in another process (Explorer) therefore calls
//! them on COM worker threads, so a slow network read never blocks the
//! window's UI thread.

#![cfg(windows)]

mod data;
mod image;
mod stream;

use std::io::Read;
use std::mem::ManuallyDrop;
use std::sync::{Arc, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

use windows::core::{implement, Error, Interface, Result, BOOL, HRESULT};
use windows::Win32::Foundation::{DRAGDROP_S_CANCEL, DRAGDROP_S_DROP, DRAGDROP_S_USEDEFAULTCURSORS, E_UNEXPECTED, FILETIME, S_OK};
use windows::Win32::System::Com::Marshal::CoMarshalInterThreadInterfaceInStream;
use windows::Win32::System::Com::StructuredStorage::CoGetInterfaceAndReleaseStream;
use windows::Win32::System::Com::{CoIncrementMTAUsage, CoInitializeEx, CoUninitialize, IDataObject, IStream, COINIT_MULTITHREADED};
use windows::Win32::System::Ole::{DoDragDrop, IDropSource, IDropSource_Impl, OleInitialize, DROPEFFECT, DROPEFFECT_COPY, DROPEFFECT_NONE};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};

/// Longest relative path, in UTF-16 units, that a file descriptor can hold.
/// Entries with longer paths are left out of the drag.
pub const MAX_PATH_LEN: usize = 259;

/// One folder or file to create at the drop location.
#[derive(Debug, Clone)]
pub struct Entry {
    /// Path relative to the drop folder, components separated by `\`.
    pub path: String,
    pub is_dir: bool,
    /// Size in bytes (files only).
    pub size: u64,
    /// Last modification time, given to the copy when known.
    pub modified: Option<SystemTime>,
    /// The source's own reference for this entry, handed back to [`Source::open`].
    pub key: String,
}

/// Where the dragged items come from.
pub trait Source: Send + Sync + 'static {
    /// Every folder and file to create, each folder before its contents.
    /// Called once, the first time a drop target asks, on a COM worker
    /// thread; it may block while it lists folders.
    fn entries(&self) -> std::result::Result<Vec<Entry>, String>;

    /// A reader for the file `entry`, starting `offset` bytes in. Called on a
    /// COM worker thread whenever the drop target starts reading or jumps to
    /// another position. End of file is a read that returns 0.
    fn open(&self, entry: &Entry, offset: u64) -> std::result::Result<Box<dyn Read + Send>, String>;

    /// The drop target began copying in the background.
    fn copy_started(&self) {}

    /// The background copy ended; `ok` is false when it failed or was cancelled.
    fn copy_ended(&self, _ok: bool) {}
}

/// An item as it appears under the pointer while dragging.
#[derive(Debug, Clone)]
pub struct Preview {
    pub name: String,
    pub is_dir: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Outcome {
    /// Dropped on a target that accepted a copy. An asynchronous target keeps
    /// copying after this; see [`Source::copy_ended`].
    Dropped,
    /// Cancelled, or dropped where nothing accepted it.
    Cancelled,
}

/// COM pointers moved to a helper thread only to be marshalled there.
struct SendCom<T>(T);
unsafe impl<T> Send for SendCom<T> {}

/// Build the data object for `source`. The object lives in the process's
/// multithreaded apartment; the returned interface is a proxy for the calling
/// thread, and whoever it is handed to (Explorer included) reaches the object
/// on COM worker threads.
pub fn data_object(source: Arc<dyn Source>) -> Result<IDataObject> {
    // Keep the multithreaded apartment alive for the life of the process, so
    // the objects marshalled into it outlive the helper thread below.
    static MTA: OnceLock<()> = OnceLock::new();
    MTA.get_or_init(|| {
        let _ = unsafe { CoIncrementMTAUsage() };
    });

    let packet = std::thread::spawn(move || -> Result<SendCom<IStream>> {
        unsafe { CoInitializeEx(None, COINIT_MULTITHREADED).ok()? };
        let marshalled = data::DataObject::new(source).and_then(|object| {
            let object: IDataObject = object.into();
            unsafe { CoMarshalInterThreadInterfaceInStream(&IDataObject::IID, &object) }
        });
        unsafe { CoUninitialize() };
        marshalled.map(SendCom)
    })
    .join()
    .map_err(|_| Error::from(E_UNEXPECTED))??;

    // CoGetInterfaceAndReleaseStream releases the stream itself, even when it
    // fails, so our reference must not be released a second time.
    let packet = ManuallyDrop::new(packet.0);
    unsafe { CoGetInterfaceAndReleaseStream(&*packet) }
}

/// Drag `source` out of the window. Call on the window's UI thread while the
/// left mouse button is still down; returns when the items are dropped or the
/// drag is cancelled.
pub fn drag(source: Arc<dyn Source>, preview: &[Preview]) -> Result<Outcome> {
    // Normally done already by the window (it accepts file drops).
    unsafe { OleInitialize(None)? };
    let data = data_object(source)?;
    // Best effort: without an image Windows still shows the copy cursor.
    let _ = image::attach(&data, preview);
    let drop_source: IDropSource = DropSource.into();
    let mut effect = DROPEFFECT_NONE;
    let hr = unsafe { DoDragDrop(&data, &drop_source, DROPEFFECT_COPY, &mut effect) };
    if hr == DRAGDROP_S_DROP && effect.0 & DROPEFFECT_COPY.0 != 0 {
        Ok(Outcome::Dropped)
    } else if hr == DRAGDROP_S_DROP || hr == DRAGDROP_S_CANCEL {
        Ok(Outcome::Cancelled)
    } else {
        Err(Error::from(hr))
    }
}

/// Drops when the left button is released; Escape cancels.
#[implement(IDropSource)]
struct DropSource;

impl IDropSource_Impl for DropSource_Impl {
    fn QueryContinueDrag(&self, escape: BOOL, keys: MODIFIERKEYS_FLAGS) -> HRESULT {
        if escape.as_bool() {
            DRAGDROP_S_CANCEL
        } else if keys.0 & MK_LBUTTON.0 == 0 {
            DRAGDROP_S_DROP
        } else {
            S_OK
        }
    }

    fn GiveFeedback(&self, _effect: DROPEFFECT) -> HRESULT {
        DRAGDROP_S_USEDEFAULTCURSORS
    }
}

/// Windows file time (100 ns ticks since 1601) for `time`, if it is after 1970.
pub(crate) fn filetime(time: SystemTime) -> Option<FILETIME> {
    let since_unix = time.duration_since(UNIX_EPOCH).ok()?;
    let ticks = since_unix.as_nanos() / 100 + 116_444_736_000_000_000;
    let ticks = u64::try_from(ticks).ok()?;
    Some(FILETIME { dwLowDateTime: ticks as u32, dwHighDateTime: (ticks >> 32) as u32 })
}
