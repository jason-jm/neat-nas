//! macOS only: drag files from the app into Finder (or any drop target)
//! using file promises. Nothing is downloaded until the drop lands; Finder
//! then asks for each file and we stream it from the NAS straight into the
//! destination it chose.

#![cfg(target_os = "macos")]

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;

use block2::DynBlock;
use objc2::rc::Retained;
use objc2::runtime::{NSObject, NSObjectProtocol, ProtocolObject};
use objc2::{define_class, msg_send, AnyThread, DefinedClass, MainThreadMarker, MainThreadOnly};
use objc2_app_kit::{
    NSApp, NSDragOperation, NSDraggingContext, NSDraggingItem, NSDraggingSession, NSDraggingSource, NSEvent,
    NSEventModifierFlags, NSEventType, NSFilePromiseProvider, NSFilePromiseProviderDelegate, NSView, NSWorkspace,
};
use objc2_foundation::{NSError, NSMutableArray, NSOperationQueue, NSPoint, NSRect, NSString, NSURL};
use tauri::{AppHandle, Emitter, Manager, Window};

use crate::error::AppError;
use crate::smb::ConnectionParams;
use crate::transfer::{self, DownloadItem, TransferProgress, PROGRESS_EVENT};
use crate::AppState;

// ── File promise delegate ───────────────────────────────────────────────

pub struct PromiseIvars {
    app: AppHandle,
    params: ConnectionParams,
    server_id: String,
    share: String,
    item: DownloadItem,
    queue: Retained<NSOperationQueue>,
    done: AtomicBool,
}

define_class!(
    #[unsafe(super(NSObject))]
    #[name = "NeatNasFilePromiseDelegate"]
    #[ivars = PromiseIvars]
    struct PromiseDelegate;

    unsafe impl NSObjectProtocol for PromiseDelegate {}

    unsafe impl NSFilePromiseProviderDelegate for PromiseDelegate {
        #[unsafe(method_id(filePromiseProvider:fileNameForType:))]
        fn file_name_for_type(&self, _provider: &NSFilePromiseProvider, _file_type: &NSString) -> Retained<NSString> {
            NSString::from_str(&self.ivars().item.name)
        }

        #[unsafe(method_id(operationQueueForFilePromiseProvider:))]
        fn operation_queue(&self, _provider: &NSFilePromiseProvider) -> Retained<NSOperationQueue> {
            self.ivars().queue.clone()
        }

        /// Runs on the operation queue above (not the main thread), so it can
        /// block while the file streams from the NAS.
        #[unsafe(method(filePromiseProvider:writePromiseToURL:completionHandler:))]
        fn write_promise(
            &self,
            _provider: &NSFilePromiseProvider,
            url: &NSURL,
            completion: &DynBlock<dyn Fn(*mut NSError)>,
        ) {
            let dest = url.path().map(|p| p.to_string()).unwrap_or_default();
            let result = if dest.is_empty() {
                Err(AppError::new("bad_request", "drop destination has no path"))
            } else {
                tauri::async_runtime::block_on(fulfil(self.ivars(), PathBuf::from(dest)))
            };
            self.ivars().done.store(true, Ordering::Relaxed);
            match result {
                Ok(()) => completion.call((std::ptr::null_mut(),)),
                Err(e) => {
                    log::warn!("drag-out promise failed: {e}");
                    let err = unsafe {
                        NSError::errorWithDomain_code_userInfo(&NSString::from_str("com.neatnas.app"), 1, None)
                    };
                    completion.call((Retained::as_ptr(&err) as *mut NSError,));
                }
            }
        }
    }
);

impl PromiseDelegate {
    fn new(ivars: PromiseIvars) -> Retained<Self> {
        let this = Self::alloc().set_ivars(ivars);
        unsafe { msg_send![super(this), init] }
    }
}

/// Stream one promised item into `dest`, reporting it in the transfer list
/// like any other download.
async fn fulfil(ivars: &PromiseIvars, dest: PathBuf) -> Result<(), AppError> {
    let app = ivars.app.clone();
    let task_id = uuid::Uuid::new_v4().to_string();
    let progress = TransferProgress::queued_exact(task_id.clone(), ivars.server_id.clone(), ivars.share.clone(), &ivars.item, &dest);
    let state = app.state::<AppState>();
    let cancel = state.transfers.register(&task_id);
    state.store.upsert(&progress);
    let _ = app.emit(PROGRESS_EVENT, progress.clone());

    let emitter = app.clone();
    let done = transfer::run_transfer(ivars.params.clone(), progress, cancel, move |p| {
        let _ = emitter.emit(PROGRESS_EVENT, p.clone());
        if let Some(s) = emitter.try_state::<AppState>() {
            s.store.upsert(p);
        }
    })
    .await;
    state.transfers.finish(&task_id);
    state.store.upsert(&done);
    match done.status {
        transfer::TransferStatus::Done => Ok(()),
        _ => Err(AppError::new(
            done.error_code.unwrap_or_else(|| "transfer_failed".into()),
            done.error.unwrap_or_else(|| "transfer did not complete".into()),
        )),
    }
}

// ── Dragging source ─────────────────────────────────────────────────────

define_class!(
    #[unsafe(super(NSObject))]
    #[thread_kind = MainThreadOnly]
    #[name = "NeatNasDragSource"]
    struct DragSource;

    unsafe impl NSObjectProtocol for DragSource {}

    unsafe impl NSDraggingSource for DragSource {
        #[unsafe(method(draggingSession:sourceOperationMaskForDraggingContext:))]
        fn source_operation_mask(&self, _session: &NSDraggingSession, _context: NSDraggingContext) -> NSDragOperation {
            NSDragOperation::Copy
        }
    }
);

impl DragSource {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(());
        unsafe { msg_send![super(this), init] }
    }
}

// The provider's delegate property is weak, so delegates must be kept alive
// until their promise has been written. Objective-C objects are safe to
// hand between threads; the wrapper only exists to say so to Rust.
struct Keep(Retained<PromiseDelegate>);
unsafe impl Send for Keep {}
static LIVE_DELEGATES: Mutex<Vec<Keep>> = Mutex::new(Vec::new());

fn uti_for(item: &DownloadItem) -> &'static str {
    if item.is_dir {
        return "public.folder";
    }
    match item.name.rsplit('.').next().unwrap_or("").to_ascii_lowercase().as_str() {
        "jpg" | "jpeg" => "public.jpeg",
        "png" => "public.png",
        "gif" => "com.compuserve.gif",
        "heic" | "heif" => "public.heic",
        "tif" | "tiff" => "public.tiff",
        "pdf" => "com.adobe.pdf",
        "txt" | "md" | "log" => "public.plain-text",
        "mp4" | "m4v" => "public.mpeg-4",
        "mov" => "com.apple.quicktime-movie",
        "mkv" => "org.matroska.mkv",
        "mp3" => "public.mp3",
        "m4a" => "public.mpeg-4-audio",
        "flac" => "org.xiph.flac",
        "wav" => "com.microsoft.waveform-audio",
        "zip" => "public.zip-archive",
        "dmg" => "com.apple.disk-image-udif",
        "iso" => "public.iso-image",
        "doc" | "docx" => "org.openxmlformats.wordprocessingml.document",
        "xls" | "xlsx" => "org.openxmlformats.spreadsheetml.sheet",
        "ppt" | "pptx" => "org.openxmlformats.presentationml.presentation",
        _ => "public.data",
    }
}

/// Begin a native drag session for `items`. Must be called while the mouse
/// button is down (the webview reports the gesture; we attach to it).
pub fn start(window: Window, app: AppHandle, params: ConnectionParams, server_id: String, share: String, items: Vec<DownloadItem>) -> Result<(), AppError> {
    if items.is_empty() {
        return Ok(());
    }
    let win = window.clone();
    window
        .run_on_main_thread(move || {
            if let Err(e) = begin_on_main(&win, app, params, server_id, share, items) {
                log::warn!("drag-out could not start: {e}");
            }
        })
        .map_err(|e| AppError::new("drag_error", e.to_string()))
}

fn begin_on_main(window: &Window, app: AppHandle, params: ConnectionParams, server_id: String, share: String, items: Vec<DownloadItem>) -> Result<(), AppError> {
    // We are inside `run_on_main_thread`.
    let mtm = unsafe { MainThreadMarker::new_unchecked() };
    let view_ptr = window.ns_view().map_err(|e| AppError::new("drag_error", e.to_string()))?;
    let ns_view: &NSView = unsafe { &*(view_ptr as *const NSView) };
    let ns_window = ns_view.window().ok_or_else(|| AppError::new("drag_error", "view has no window"))?;
    let content_view = ns_window.contentView().ok_or_else(|| AppError::new("drag_error", "window has no content view"))?;
    let position: NSPoint = ns_window.mouseLocationOutsideOfEventStream();
    let workspace = NSWorkspace::sharedWorkspace();

    // Prune delegates whose promises have already been written.
    {
        let mut live = LIVE_DELEGATES.lock().unwrap_or_else(|p| p.into_inner());
        live.retain(|k| !k.0.ivars().done.load(Ordering::Relaxed));
    }

    let dragging_items = NSMutableArray::<NSDraggingItem>::new();
    let count = items.len();
    for (i, item) in items.into_iter().enumerate() {
        let uti = uti_for(&item);
        let delegate = PromiseDelegate::new(PromiseIvars {
            app: app.clone(),
            params: params.clone(),
            server_id: server_id.clone(),
            share: share.clone(),
            item,
            queue: NSOperationQueue::new(),
            done: AtomicBool::new(false),
        });
        let provider = NSFilePromiseProvider::initWithFileType_delegate(
            NSFilePromiseProvider::alloc(),
            &NSString::from_str(uti),
            ProtocolObject::from_ref(&*delegate),
        );
        LIVE_DELEGATES.lock().unwrap_or_else(|p| p.into_inner()).push(Keep(delegate));

        #[allow(deprecated)] // iconForContentType needs UniformTypeIdentifiers, which objc2-app-kit does not bind
        let icon = workspace.iconForFileType(&NSString::from_str(uti));
        let size = icon.size();
        let scale = 64.0 / size.height.max(1.0);
        let w = size.width * scale;
        let h = size.height * scale;
        // Stack the icons slightly so a multi-item drag reads as a bundle.
        let offset = (i.min(3) as f64) * 6.0;
        let frame = NSRect::new(
            NSPoint::new(position.x - w / 2.0 + offset, position.y - h / 2.0 - offset),
            objc2_foundation::NSSize::new(w, h),
        );
        let drag_item = NSDraggingItem::initWithPasteboardWriter(NSDraggingItem::alloc(), ProtocolObject::from_ref(&*provider));
        unsafe { drag_item.setDraggingFrame_contents(frame, Some(&*icon)) };
        dragging_items.addObject(&drag_item);
    }
    log::info!("drag-out: starting session with {count} promised item(s)");

    let timestamp = NSApp(mtm).currentEvent().map(|e| e.timestamp()).unwrap_or(0.0);
    let event = NSEvent::mouseEventWithType_location_modifierFlags_timestamp_windowNumber_context_eventNumber_clickCount_pressure(
        NSEventType::LeftMouseDragged,
        position,
        NSEventModifierFlags::empty(),
        timestamp,
        ns_window.windowNumber(),
        None,
        0,
        1,
        1.0,
    )
    .ok_or_else(|| AppError::new("drag_error", "could not synthesise drag event"))?;

    let source = DragSource::new(mtm);
    let _session = content_view.beginDraggingSessionWithItems_event_source(
        &dragging_items,
        &event,
        ProtocolObject::from_ref(&*source),
    );
    // The session retains the source; nothing else to keep.
    Ok(())
}
