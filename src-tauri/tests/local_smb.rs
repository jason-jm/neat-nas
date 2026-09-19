//! End-to-end checks against a real SMB server (the local Samba started by
//! `scripts/dev-smb.sh`, or any NAS). Skipped unless these are set:
//!   NEATNAS_TEST_SMB_ADDR=127.0.0.1:1445
//!   NEATNAS_TEST_SMB_USER=...  NEATNAS_TEST_SMB_PASS=...
//! Assumes a writable share "Public" containing readme.txt and a folder
//! "文档", and a share "Media" with Movies/sample-40mb.mkv.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use neatnas_lib::smb::{self, ConnectionParams, Session};
use neatnas_lib::thumbs;
use neatnas_lib::transfer::{run_transfer, DownloadItem, TransferKind, TransferProgress, TransferStatus};

fn params() -> Option<ConnectionParams> {
    let addr = std::env::var("NEATNAS_TEST_SMB_ADDR").ok()?;
    let (host, port) = addr.rsplit_once(':')?;
    Some(ConnectionParams {
        host: host.to_string(),
        port: port.parse().ok()?,
        username: std::env::var("NEATNAS_TEST_SMB_USER").ok()?,
        password: std::env::var("NEATNAS_TEST_SMB_PASS").ok()?,
        domain: String::new(),
    })
}

fn temp_dir(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("neatnas-test-{tag}-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn download_of(share: &str, path: &str, is_dir: bool, dest: &Path) -> TransferProgress {
    let name = path.rsplit('/').next().unwrap_or(path).to_string();
    let item = DownloadItem { path: path.to_string(), name, is_dir };
    TransferProgress::queued(uuid::Uuid::new_v4().to_string(), "server".into(), share.into(), &item, dest)
}

fn upload_of(share: &str, local: &Path, remote_dir: &str) -> TransferProgress {
    TransferProgress::queued_upload(uuid::Uuid::new_v4().to_string(), "server".into(), share.into(), local, remote_dir)
}

type Events = Arc<Mutex<Vec<TransferProgress>>>;

async fn run(p: &ConnectionParams, progress: TransferProgress, cancel: Arc<AtomicBool>, events: Events) -> TransferProgress {
    run_transfer(p.clone(), progress, cancel, move |x| events.lock().unwrap().push(x.clone())).await
}

fn pseudo_random(len: usize, seed: u64) -> Vec<u8> {
    let mut x = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
    (0..len)
        .map(|_| {
            x ^= x << 13;
            x ^= x >> 7;
            x ^= x << 17;
            (x >> 24) as u8
        })
        .collect()
}

async fn count_files(session: &mut Session, share: &str, path: &str) -> u32 {
    let mut total = 0;
    for e in session.list_dir(share, path).await.expect("list for count") {
        if e.is_dir {
            total += Box::pin(count_files(session, share, &e.path)).await;
        } else {
            total += 1;
        }
    }
    total
}

async fn remove_remote(session: &mut Session, share: &str, path: &str) {
    // Best-effort recursive delete so reruns start clean.
    if let Ok(entries) = session.list_dir(share, path).await {
        for e in entries {
            if e.is_dir {
                Box::pin(remove_remote(session, share, &e.path)).await;
            } else {
                let tree = session.tree(share).await.unwrap();
                let mut t = tree;
                let _ = session.client.delete_file(&mut t, &e.path).await;
            }
        }
        let mut t = session.tree(share).await.unwrap();
        let _ = session.client.delete_directory(&mut t, path).await;
    } else {
        let mut t = session.tree(share).await.unwrap();
        let _ = session.client.delete_file(&mut t, path).await;
    }
}

#[tokio::test]
async fn lists_shares_and_directories() {
    let Some(p) = params() else {
        eprintln!("skipped: NEATNAS_TEST_SMB_* not set");
        return;
    };
    let mut session = Session::new(smb::connect(&p).await.expect("connect"));
    assert!(!session.dialect().is_empty(), "dialect should be negotiated");

    let shares = session.list_shares().await.expect("list_shares");
    let names: Vec<&str> = shares.iter().map(|s| s.name.as_str()).collect();
    assert!(names.contains(&"Public"), "shares: {names:?}");
    assert!(!names.iter().any(|n| n.ends_with('$')), "admin shares must be filtered: {names:?}");

    let root = session.list_dir("Public", "").await.expect("list root");
    assert!(root.iter().all(|e| e.name != "." && e.name != ".."), "dot entries must be filtered");
    assert!(root.iter().any(|e| e.name == "readme.txt" && !e.is_dir && e.size == 15));
    assert!(root.iter().any(|e| e.name == "文档" && e.is_dir));
    let first_file = root.iter().position(|e| !e.is_dir).unwrap_or(root.len());
    assert!(root[..first_file].iter().all(|e| e.is_dir));

    let sub = session.list_dir("Public", "文档").await.expect("list unicode dir");
    let entry = sub.iter().find(|e| e.name == "说明.txt").expect("unicode file");
    assert_eq!(entry.path, "文档/说明.txt");
    assert!(entry.modified.is_some());

    let err = session.list_dir("Public", "does-not-exist").await.unwrap_err();
    assert_eq!(err.code, "not_found", "{err}");

    // Random-access reads through the reader used by previews.
    let reader = session.open_reader("Public", "readme.txt").await.expect("open reader");
    assert_eq!(reader.size(), 15);
    assert_eq!(reader.read_at(6, 4).await.unwrap(), b"from");
    assert!(reader.read_at(100, 10).await.unwrap().is_empty());
    reader.close().await.unwrap();

    let bad = ConnectionParams {
        password: "definitely-wrong".into(),
        ..p.clone()
    };
    let err = match smb::connect(&bad).await {
        Ok(_) => panic!("wrong password was accepted"),
        Err(e) => e,
    };
    assert_eq!(err.code, "auth_failed", "{err}");
}

#[tokio::test]
async fn downloads_with_resume() {
    let Some(p) = params() else {
        eprintln!("skipped: NEATNAS_TEST_SMB_* not set");
        return;
    };
    let dest = temp_dir("dl");
    let no_cancel = || Arc::new(AtomicBool::new(false));

    // Single file.
    let events: Events = Default::default();
    let done = run(&p, download_of("Public", "readme.txt", false, &dest), no_cancel(), events.clone()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert_eq!(done.kind, TransferKind::Download);
    assert_eq!(std::fs::read_to_string(dest.join("readme.txt")).unwrap(), "hello from NAS\n");
    assert!(!dest.join("readme.txt.part").exists(), ".part must be renamed away");
    {
        let ev = events.lock().unwrap();
        assert_eq!(ev.first().unwrap().status, TransferStatus::Scanning);
        assert_eq!(ev.last().unwrap().status, TransferStatus::Done);
        assert_eq!(ev.last().unwrap().bytes_done, 15);
        assert_eq!(ev.last().unwrap().bytes_total, 15);
    }

    // Same file again must not overwrite: gets " (1)" suffix.
    let again = run(&p, download_of("Public", "readme.txt", false, &dest), no_cancel(), Default::default()).await;
    assert_eq!(again.status, TransferStatus::Done);
    assert!(again.local_path.ends_with("readme (1).txt"), "{}", again.local_path);

    // Whole directory, recursively, with non-ASCII names. The expected file
    // count comes from the share itself so extra sample files don't break it.
    let mut session = Session::new(smb::connect(&p).await.expect("connect"));
    let expected_files = count_files(&mut session, "Public", "文档").await;
    let done = run(&p, download_of("Public", "文档", true, &dest), no_cancel(), Default::default()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert!(expected_files >= 3, "sample folder should have at least 3 files, found {expected_files}");
    assert_eq!(done.files_total, expected_files);
    assert_eq!(done.files_done, expected_files);
    assert!(dest.join("文档/项目资料/计划.md").is_file());
    assert_eq!(std::fs::read_to_string(dest.join("文档/说明.txt")).unwrap(), "中文内容测试\n");

    // Large file: progress is monotonic, speed is reported, size matches.
    let last = Arc::new(Mutex::new((0u64, 0u64)));
    let l2 = last.clone();
    let done = run_transfer(
        p.clone(),
        download_of("Media", "Movies/sample-40mb.mkv", false, &dest),
        no_cancel(),
        move |x| {
            let mut prev = l2.lock().unwrap();
            assert!(x.bytes_done >= prev.0, "progress went backwards");
            prev.0 = x.bytes_done;
            prev.1 = prev.1.max(x.speed_bps);
        },
    )
    .await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert_eq!(done.bytes_total, 40_000_000);
    let full = std::fs::read(dest.join("sample-40mb.mkv")).unwrap();
    assert_eq!(full.len(), 40_000_000);
    assert!(last.lock().unwrap().1 > 0, "speed should be reported while running");
    assert_eq!(done.speed_bps, 0, "speed is zeroed once finished");

    // Cancellation keeps the partial file so it can be resumed later.
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    let cancelled = run_transfer(
        p.clone(),
        download_of("Media", "Movies/sample-40mb.mkv", false, &dest),
        cancel,
        move |x| {
            if x.status == TransferStatus::Running && x.bytes_done > 4_000_000 {
                c2.store(true, Ordering::Relaxed);
            }
        },
    )
    .await;
    assert_eq!(cancelled.status, TransferStatus::Cancelled);
    assert!(cancelled.local_path.ends_with("sample-40mb (1).mkv"), "{}", cancelled.local_path);
    let part = PathBuf::from(format!("{}.part", cancelled.local_path));
    let part_len = std::fs::metadata(&part).expect("partial file kept").len();
    assert!(part_len > 0 && part_len < 40_000_000, "part len {part_len}");

    // Resuming continues from the partial file: prove it by poisoning the
    // prefix and checking the final file keeps our bytes.
    std::fs::write(&part, vec![0xABu8; 10_000_000]).unwrap();
    let resumed = run(&p, cancelled.clone(), no_cancel(), Default::default()).await;
    assert_eq!(resumed.status, TransferStatus::Done, "{:?}", resumed.error);
    assert!(!part.exists());
    let bytes = std::fs::read(&cancelled.local_path).unwrap();
    assert_eq!(bytes.len(), 40_000_000);
    assert!(bytes[..10_000_000].iter().all(|b| *b == 0xAB), "resume must continue after the existing prefix");
    assert_eq!(&bytes[10_000_000..], &full[10_000_000..], "tail must match the source");

    // A partial file that is too large for the source is thrown away.
    let mut fresh = download_of("Media", "Movies/sample-40mb.mkv", false, &dest);
    fresh.local_path = dest.join("oversized.mkv").to_string_lossy().into_owned();
    std::fs::write(dest.join("oversized.mkv.part"), vec![0u8; 40_000_001]).unwrap();
    let done = run(&p, fresh, no_cancel(), Default::default()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert_eq!(std::fs::read(dest.join("oversized.mkv")).unwrap(), full);

    let _ = std::fs::remove_dir_all(&dest);
}

#[tokio::test]
async fn uploads_with_resume_and_thumbnails() {
    let Some(p) = params() else {
        eprintln!("skipped: NEATNAS_TEST_SMB_* not set");
        return;
    };
    let work = temp_dir("ul");
    let no_cancel = || Arc::new(AtomicBool::new(false));
    let mut session = Session::new(smb::connect(&p).await.expect("connect"));
    let remote_root = format!("neatnas-test-{}", uuid::Uuid::new_v4());
    {
        let mut t = session.tree("Public").await.unwrap();
        session.client.create_directory(&mut t, &remote_root).await.unwrap();
    }

    // Single file upload, then read it back.
    let payload = pseudo_random(3_500_000, 7);
    std::fs::write(work.join("photo-data.bin"), &payload).unwrap();
    let done = run(&p, upload_of("Public", &work.join("photo-data.bin"), &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert_eq!(done.kind, TransferKind::Upload);
    assert_eq!(done.remote_path, format!("{remote_root}/photo-data.bin"));
    assert_eq!(done.bytes_done, payload.len() as u64);
    let listing = session.list_dir("Public", &remote_root).await.unwrap();
    let uploaded = listing.iter().find(|e| e.name == "photo-data.bin").expect("uploaded file listed");
    assert_eq!(uploaded.size, payload.len() as u64);
    assert!(!listing.iter().any(|e| e.name.ends_with(".part")), "remote .part must be renamed away");
    let reader = session.open_reader("Public", &done.remote_path).await.unwrap();
    assert_eq!(reader.read_at(0, payload.len() as u64).await.unwrap(), payload);
    reader.close().await.unwrap();

    // Uploading the same name again does not clobber.
    let again = run(&p, upload_of("Public", &work.join("photo-data.bin"), &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(again.status, TransferStatus::Done, "{:?}", again.error);
    assert_eq!(again.remote_path, format!("{remote_root}/photo-data (1).bin"));

    // Directory upload with nested folders and non-ASCII names.
    let tree_dir = work.join("相册");
    std::fs::create_dir_all(tree_dir.join("2025/东京")).unwrap();
    std::fs::write(tree_dir.join("说明.txt"), "hello").unwrap();
    std::fs::write(tree_dir.join("2025/东京/img.bin"), pseudo_random(1_000_000, 3)).unwrap();
    std::fs::write(tree_dir.join(".DS_Store"), "junk").unwrap();
    let done = run(&p, upload_of("Public", &tree_dir, &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    assert_eq!(done.files_total, 2, ".DS_Store must be skipped");
    let nested = session.list_dir("Public", &format!("{remote_root}/相册/2025/东京")).await.unwrap();
    assert!(nested.iter().any(|e| e.name == "img.bin" && e.size == 1_000_000));

    // Upload resume: a pre-existing remote `.part` is continued, not restarted.
    let big = pseudo_random(20_000_000, 11);
    std::fs::write(work.join("resume-test.bin"), &big).unwrap();
    std::fs::write(work.join("resume-test.bin.part"), vec![0xCDu8; 5_000_000]).unwrap();
    let seeded = run(&p, upload_of("Public", &work.join("resume-test.bin.part"), &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(seeded.status, TransferStatus::Done, "{:?}", seeded.error);
    assert_eq!(seeded.remote_path, format!("{remote_root}/resume-test.bin.part"));
    let resumed = run(&p, upload_of("Public", &work.join("resume-test.bin"), &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(resumed.status, TransferStatus::Done, "{:?}", resumed.error);
    assert_eq!(resumed.remote_path, format!("{remote_root}/resume-test.bin"));
    let reader = session.open_reader("Public", &resumed.remote_path).await.unwrap();
    assert_eq!(reader.size(), 20_000_000);
    let head = reader.read_at(0, 5_000_000).await.unwrap();
    assert!(head.iter().all(|b| *b == 0xCD), "existing remote prefix must be kept");
    let tail = reader.read_at(5_000_000, 15_000_000).await.unwrap();
    assert_eq!(tail, &big[5_000_000..]);
    reader.close().await.unwrap();

    // Cancelling an upload keeps the remote `.part`, and resuming finishes it.
    let cancel = Arc::new(AtomicBool::new(false));
    let c2 = cancel.clone();
    // Loopback is fast, so the file must be big enough to see a progress
    // callback before it completes.
    const BIG: usize = 240_000_000;
    std::fs::write(work.join("cancel-me.bin"), pseudo_random(BIG, 5)).unwrap();
    let cancelled = run_transfer(
        p.clone(),
        upload_of("Public", &work.join("cancel-me.bin"), &remote_root),
        cancel,
        move |x| {
            if x.status == TransferStatus::Running && x.bytes_done > 20_000_000 {
                c2.store(true, Ordering::Relaxed);
            }
        },
    )
    .await;
    assert_eq!(cancelled.status, TransferStatus::Cancelled);
    let listing = session.list_dir("Public", &remote_root).await.unwrap();
    let part = listing.iter().find(|e| e.name == "cancel-me.bin.part").expect("remote .part kept on cancel");
    assert!(part.size > 0 && part.size < BIG as u64);
    let finished = run(&p, cancelled.clone(), no_cancel(), Default::default()).await;
    assert_eq!(finished.status, TransferStatus::Done, "{:?}", finished.error);
    let listing = session.list_dir("Public", &remote_root).await.unwrap();
    assert!(listing.iter().any(|e| e.name == "cancel-me.bin" && e.size == BIG as u64));
    assert!(!listing.iter().any(|e| e.name == "cancel-me.bin.part"));

    // Thumbnail from a real image uploaded to the share.
    let mut img = image::RgbImage::new(1600, 1200);
    for (x, y, px) in img.enumerate_pixels_mut() {
        *px = image::Rgb([(x / 7) as u8, (y / 5) as u8, ((x + y) / 11) as u8]);
    }
    img.save(work.join("gradient.png")).unwrap();
    let done = run(&p, upload_of("Public", &work.join("gradient.png"), &remote_root), no_cancel(), Default::default()).await;
    assert_eq!(done.status, TransferStatus::Done, "{:?}", done.error);
    let reader = session.open_reader("Public", &done.remote_path).await.unwrap();
    let jpeg = thumbs::build(&reader, "gradient.png").await.expect("thumbnail");
    reader.close().await.unwrap();
    let thumb = image::load_from_memory(&jpeg).unwrap();
    assert!(thumb.width() <= 320 && thumb.height() <= 320, "{}x{}", thumb.width(), thumb.height());
    assert_eq!(thumb.width(), 320, "landscape image should fill the width");
    assert!(thumbs::is_thumbnailable("IMG_0001.HEIC"));
    assert!(!thumbs::is_thumbnailable("movie.mkv"));
    let err = thumbs::build(&session.open_reader("Public", "readme.txt").await.unwrap(), "readme.txt").await.unwrap_err();
    assert_eq!(err.code, "no_thumbnail");

    remove_remote(&mut session, "Public", &remote_root).await;
    assert_eq!(session.list_dir("Public", &remote_root).await.unwrap_err().code, "not_found");
    let _ = std::fs::remove_dir_all(&work);
}
