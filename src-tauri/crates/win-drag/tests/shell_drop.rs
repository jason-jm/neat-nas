//! Drops virtual files onto a real folder through the shell's own drop target
//! (the one Explorer uses) and checks what lands on disk. Runs on Windows only.

#![cfg(windows)]

use std::collections::HashMap;
use std::io::{Cursor, Read};
use std::mem::size_of;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use win_drag::{data_object, Entry, Source};
use windows::core::{HSTRING, PCWSTR};
use windows::Win32::Foundation::POINTL;
use windows::Win32::System::Com::{IDataObject, IStream, DVASPECT_CONTENT, FORMATETC, STATFLAG_DEFAULT, STATSTG, STREAM_SEEK_SET, TYMED_HGLOBAL, TYMED_ISTREAM};
use windows::Win32::System::DataExchange::RegisterClipboardFormatW;
use windows::Win32::System::Memory::{GlobalLock, GlobalSize, GlobalUnlock};
use windows::Win32::System::Ole::{IDropTarget, OleInitialize, ReleaseStgMedium, DROPEFFECT_COPY};
use windows::Win32::System::SystemServices::{MK_LBUTTON, MODIFIERKEYS_FLAGS};
use windows::Win32::UI::Shell::{IShellItem, SHCreateItemFromParsingName, BHID_SFUIObject, CFSTR_FILECONTENTS, CFSTR_FILEDESCRIPTORW, FILEDESCRIPTORW};
use windows::Win32::UI::WindowsAndMessaging::{DispatchMessageW, PeekMessageW, TranslateMessage, MSG, PM_REMOVE};

const BIG: usize = 3 * 1024 * 1024 + 17;

/// Deterministic bytes, so a wrong offset shows up as a mismatch.
fn pattern(len: usize, seed: u32) -> Vec<u8> {
    let mut x = seed | 1;
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x as u8
        })
        .collect()
}

struct Memory {
    entries: Vec<Entry>,
    files: HashMap<String, Vec<u8>>,
    opens: AtomicUsize,
    started: AtomicBool,
    ended: Mutex<Option<bool>>,
}

impl Source for Memory {
    fn entries(&self) -> Result<Vec<Entry>, String> {
        Ok(self.entries.clone())
    }

    fn open(&self, entry: &Entry, offset: u64) -> Result<Box<dyn Read + Send>, String> {
        self.opens.fetch_add(1, Ordering::SeqCst);
        let data = self.files.get(&entry.key).ok_or("no such file")?;
        let start = (offset as usize).min(data.len());
        Ok(Box::new(Cursor::new(data[start..].to_vec())))
    }

    fn copy_started(&self) {
        self.started.store(true, Ordering::SeqCst);
    }

    fn copy_ended(&self, ok: bool) {
        *self.ended.lock().unwrap() = Some(ok);
    }
}

fn modified() -> SystemTime {
    // A whole second, well in the past: easy to compare after the copy.
    UNIX_EPOCH + Duration::from_secs(1_700_000_000)
}

fn sample() -> Arc<Memory> {
    let files: Vec<(&str, Vec<u8>)> = vec![
        ("notes.txt", b"hello from the NAS\r\n".to_vec()),
        ("Album\\Disc 1\\01 Track.flac", pattern(BIG, 7)),
        ("Album\\cover.jpg", pattern(4096, 11)),
        ("Album\\empty.txt", Vec::new()),
        ("读书笔记 ☕.md", "中文内容".as_bytes().to_vec()),
    ];
    let dirs = ["Album", "Album\\Disc 1", "Album\\Empty folder"];
    let mut entries: Vec<Entry> = Vec::new();
    for dir in dirs {
        entries.push(Entry { path: dir.into(), is_dir: true, size: 0, modified: None, key: String::new() });
    }
    for (path, data) in &files {
        entries.push(Entry { path: (*path).into(), is_dir: false, size: data.len() as u64, modified: Some(modified()), key: (*path).into() });
    }
    Arc::new(Memory {
        entries,
        files: files.into_iter().map(|(p, d)| (p.to_string(), d)).collect(),
        opens: AtomicUsize::new(0),
        started: AtomicBool::new(false),
        ended: Mutex::new(None),
    })
}

fn init() {
    unsafe { OleInitialize(None).expect("OleInitialize") };
}

fn cf(name: PCWSTR) -> u16 {
    unsafe { RegisterClipboardFormatW(name) as u16 }
}

fn formatetc(cf: u16, tymed: u32, lindex: i32) -> FORMATETC {
    FORMATETC { cfFormat: cf, ptd: std::ptr::null_mut(), dwAspect: DVASPECT_CONTENT.0, lindex, tymed }
}

fn descriptors(data: &IDataObject) -> Vec<FILEDESCRIPTORW> {
    let mut medium = unsafe { data.GetData(&formatetc(cf(CFSTR_FILEDESCRIPTORW), TYMED_HGLOBAL.0 as u32, -1)) }.expect("descriptor");
    let out = unsafe {
        let h = medium.u.hGlobal;
        let size = GlobalSize(h);
        let p = GlobalLock(h).cast::<u8>();
        let bytes = std::slice::from_raw_parts(p, size);
        let count = u32::from_le_bytes(bytes[..4].try_into().unwrap()) as usize;
        assert!(size >= 4 + count * size_of::<FILEDESCRIPTORW>());
        let list = (0..count)
            .map(|i| std::ptr::read_unaligned(bytes.as_ptr().add(4 + i * size_of::<FILEDESCRIPTORW>()).cast::<FILEDESCRIPTORW>()))
            .collect();
        let _ = GlobalUnlock(h);
        list
    };
    unsafe { ReleaseStgMedium(&mut medium) };
    out
}

fn name_of(d: &FILEDESCRIPTORW) -> String {
    let name = d.cFileName;
    let end = name.iter().position(|&c| c == 0).unwrap_or(name.len());
    String::from_utf16_lossy(&name[..end])
}

fn read_all(stream: &IStream, chunk: usize) -> Vec<u8> {
    let mut out = Vec::new();
    let mut buf = vec![0u8; chunk];
    loop {
        let mut got = 0u32;
        let hr = unsafe { stream.Read(buf.as_mut_ptr().cast(), buf.len() as u32, Some(&mut got)) };
        assert!(hr.is_ok(), "Read failed: {hr:?}");
        if got == 0 {
            return out;
        }
        out.extend_from_slice(&buf[..got as usize]);
    }
}

#[test]
fn descriptor_lists_folders_files_sizes_and_times() {
    init();
    let source = sample();
    let data = data_object(source.clone()).expect("data object");
    let list = descriptors(&data);
    assert_eq!(list.len(), source.entries.len());
    for (d, e) in list.iter().zip(&source.entries) {
        assert_eq!(name_of(d), e.path);
        let attributes = d.dwFileAttributes;
        assert_eq!(attributes & 0x10 != 0, e.is_dir, "directory attribute of {}", e.path);
        if !e.is_dir {
            let size = (u64::from(d.nFileSizeHigh) << 32) | u64::from(d.nFileSizeLow);
            assert_eq!(size, e.size, "size of {}", e.path);
        }
    }
    // Nothing is read until a stream is asked for.
    assert_eq!(source.opens.load(Ordering::SeqCst), 0);
}

#[test]
fn file_contents_stream_reads_seeks_and_stats() {
    init();
    let source = sample();
    let data = data_object(source.clone()).expect("data object");
    let index = source.entries.iter().position(|e| e.path.ends_with("01 Track.flac")).unwrap() as i32;
    let mut medium = unsafe { data.GetData(&formatetc(cf(CFSTR_FILECONTENTS), TYMED_ISTREAM.0 as u32, index)) }.expect("contents");
    let stream = unsafe { (*medium.u.pstm).clone() }.expect("stream");

    let expected = &source.files["Album\\Disc 1\\01 Track.flac"];
    assert_eq!(read_all(&stream, 65_531), *expected);

    let mut pos = 0u64;
    unsafe { stream.Seek(1_000_003, STREAM_SEEK_SET, Some(&mut pos)) }.expect("seek");
    assert_eq!(pos, 1_000_003);
    let mut buf = vec![0u8; 4096];
    let mut got = 0u32;
    let _ = unsafe { stream.Read(buf.as_mut_ptr().cast(), 4096, Some(&mut got)) };
    assert_eq!(&buf[..got as usize], &expected[1_000_003..1_000_003 + 4096]);

    let mut stat = STATSTG::default();
    unsafe { stream.Stat(&mut stat, STATFLAG_DEFAULT) }.expect("stat");
    assert_eq!(stat.cbSize, BIG as u64);
    let name = unsafe { stat.pwcsName.to_string() }.unwrap();
    assert_eq!(name, "01 Track.flac");
    unsafe { windows::Win32::System::Com::CoTaskMemFree(Some(stat.pwcsName.0 as _)) };

    drop(stream);
    unsafe { ReleaseStgMedium(&mut medium) };

    // A folder has no contents.
    let folder = source.entries.iter().position(|e| e.is_dir).unwrap() as i32;
    assert!(unsafe { data.GetData(&formatetc(cf(CFSTR_FILECONTENTS), TYMED_ISTREAM.0 as u32, folder)) }.is_err());
}

fn scratch_dir(name: &str) -> PathBuf {
    let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
    let dir = std::env::temp_dir().join(format!("win-drag-{name}-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Pump this thread's messages until `done` holds or `timeout` passes.
fn pump_until(timeout: Duration, mut done: impl FnMut() -> bool) -> bool {
    let start = Instant::now();
    loop {
        unsafe {
            let mut msg = MSG::default();
            while PeekMessageW(&mut msg, None, 0, 0, PM_REMOVE).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
        }
        if done() {
            return true;
        }
        if start.elapsed() > timeout {
            return false;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn expect_tree(root: &Path, source: &Memory) -> Result<(), String> {
    for e in &source.entries {
        let path = root.join(&e.path);
        if e.is_dir {
            if !path.is_dir() {
                return Err(format!("missing folder {}", e.path));
            }
            continue;
        }
        let bytes = std::fs::read(&path).map_err(|err| format!("{}: {err}", e.path))?;
        if bytes != source.files[&e.key] {
            return Err(format!("{}: {} bytes differ from the {} expected", e.path, bytes.len(), source.files[&e.key].len()));
        }
    }
    Ok(())
}

#[test]
fn shell_folder_drop_copies_the_whole_tree() {
    init();
    let source = sample();
    let data = data_object(source.clone()).expect("data object");
    let dir = scratch_dir("drop");

    let folder: IShellItem = unsafe { SHCreateItemFromParsingName(&HSTRING::from(dir.as_os_str()), None) }.expect("shell item");
    let target: IDropTarget = unsafe { folder.BindToHandler(None, &BHID_SFUIObject) }.expect("drop target");
    let point = POINTL { x: 0, y: 0 };
    let mut effect = DROPEFFECT_COPY;
    unsafe { target.DragEnter(&data, MK_LBUTTON, point, &mut effect) }.expect("DragEnter");
    assert!(effect.0 & DROPEFFECT_COPY.0 != 0, "the folder refused a copy: {effect:?}");
    effect = DROPEFFECT_COPY;
    unsafe { target.DragOver(MK_LBUTTON, point, &mut effect) }.expect("DragOver");
    effect = DROPEFFECT_COPY;
    unsafe { target.Drop(&data, MODIFIERKEYS_FLAGS(0), point, &mut effect) }.expect("Drop");

    // An asynchronous copy runs on the shell's thread; wait for it to end.
    let finished = pump_until(Duration::from_secs(120), || {
        let ended = source.ended.lock().unwrap().is_some();
        (!source.started.load(Ordering::SeqCst) || ended) && expect_tree(&dir, &source).is_ok()
    });
    let result = expect_tree(&dir, &source);
    let _ = std::fs::remove_dir_all(&dir);
    assert!(finished, "copy did not finish: {result:?}, started={}, ended={:?}", source.started.load(Ordering::SeqCst), source.ended.lock().unwrap());
    result.unwrap();
    if source.started.load(Ordering::SeqCst) {
        assert_eq!(*source.ended.lock().unwrap(), Some(true), "the shell reported a failed copy");
    }
    println!("async copy: {}, streams opened: {}", source.started.load(Ordering::SeqCst), source.opens.load(Ordering::SeqCst));
}

#[test]
fn copied_files_keep_their_modification_time() {
    init();
    let source = sample();
    let data = data_object(source.clone()).expect("data object");
    let dir = scratch_dir("mtime");
    let folder: IShellItem = unsafe { SHCreateItemFromParsingName(&HSTRING::from(dir.as_os_str()), None) }.expect("shell item");
    let target: IDropTarget = unsafe { folder.BindToHandler(None, &BHID_SFUIObject) }.expect("drop target");
    let mut effect = DROPEFFECT_COPY;
    unsafe { target.DragEnter(&data, MK_LBUTTON, POINTL::default(), &mut effect) }.expect("DragEnter");
    effect = DROPEFFECT_COPY;
    unsafe { target.Drop(&data, MODIFIERKEYS_FLAGS(0), POINTL::default(), &mut effect) }.expect("Drop");
    let note = dir.join("notes.txt");
    let ok = pump_until(Duration::from_secs(60), || {
        (!source.started.load(Ordering::SeqCst) || source.ended.lock().unwrap().is_some()) && expect_tree(&dir, &source).is_ok()
    });
    let stamp = std::fs::metadata(&note).and_then(|m| m.modified());
    let _ = std::fs::remove_dir_all(&dir);
    assert!(ok, "copy did not finish");
    let stamp = stamp.expect("modified time");
    let delta = stamp.duration_since(modified()).unwrap_or_else(|e| e.duration());
    assert!(delta < Duration::from_secs(2), "modified time is {stamp:?}, expected {:?}", modified());
}
