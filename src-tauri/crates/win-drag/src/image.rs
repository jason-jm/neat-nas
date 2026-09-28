//! The picture under the pointer while dragging: the system icons of the
//! first few items, stacked, with Explorer's "Copy to …" label allowed.

use std::mem::size_of;

use windows::core::{Interface, Result, HSTRING};
use windows::Win32::Foundation::{COLORREF, POINT, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP,
};
use windows::Win32::Storage::FileSystem::{FILE_ATTRIBUTE_DIRECTORY, FILE_ATTRIBUTE_NORMAL};
use windows::Win32::System::Com::{CoCreateInstance, IDataObject, CLSCTX_INPROC_SERVER};
use windows::Win32::UI::Controls::{IImageList, ILD_TRANSPARENT};
use windows::Win32::UI::Shell::{
    IDragSourceHelper, IDragSourceHelper2, SHGetFileInfoW, SHGetImageList, CLSID_DragDropHelper, DSH_ALLOWDROPDESCRIPTIONTEXT,
    SHDRAGIMAGE, SHFILEINFOW, SHGFI_SYSICONINDEX, SHGFI_USEFILEATTRIBUTES, SHIL_EXTRALARGE,
};
use windows::Win32::UI::WindowsAndMessaging::{DestroyIcon, DrawIconEx, DI_NORMAL};

use crate::Preview;

/// Icons drawn for a multi-item drag, and the offset between them.
const STACK: usize = 3;
const STEP: i32 = 6;

pub(crate) fn attach(data: &IDataObject, items: &[Preview]) -> Result<()> {
    if items.is_empty() {
        return Ok(());
    }
    let helper: IDragSourceHelper = unsafe { CoCreateInstance(&CLSID_DragDropHelper, None, CLSCTX_INPROC_SERVER)? };
    if let Ok(labels) = helper.cast::<IDragSourceHelper2>() {
        let _ = unsafe { labels.SetFlags(DSH_ALLOWDROPDESCRIPTIONTEXT.0 as u32) };
    }
    let (bitmap, size) = render(items)?;
    let image = SHDRAGIMAGE {
        sizeDragImage: size,
        ptOffset: POINT { x: size.cx / 2, y: size.cy / 2 },
        hbmpDragImage: bitmap,
        // Per-pixel alpha, no colour key.
        crColorKey: COLORREF(0xFFFF_FFFF),
    };
    let result = unsafe { helper.InitializeFromBitmap(&image, data) };
    if result.is_err() {
        // The helper owns the bitmap only when it succeeds.
        unsafe {
            let _ = DeleteObject(bitmap.into());
        }
    }
    result
}

/// A premultiplied 32-bit bitmap with up to three icons, back to front.
fn render(items: &[Preview]) -> Result<(HBITMAP, SIZE)> {
    let list: IImageList = unsafe { SHGetImageList(SHIL_EXTRALARGE as i32)? };
    let (mut icon_w, mut icon_h) = (48, 48);
    unsafe {
        let _ = list.GetIconSize(&mut icon_w, &mut icon_h);
    }
    let shown = items.len().min(STACK);
    let offset = STEP * (shown as i32 - 1);
    let size = SIZE { cx: icon_w + offset, cy: icon_h + offset };

    let info = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: size.cx,
            biHeight: -size.cy, // top-down
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    unsafe {
        let screen = GetDC(None);
        let dc = CreateCompatibleDC(Some(screen));
        ReleaseDC(None, screen);
        let mut bits = std::ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(e) => {
                let _ = DeleteDC(dc);
                return Err(e);
            }
        };
        let previous = SelectObject(dc, bitmap.into());
        for (i, item) in items.iter().take(shown).enumerate().rev() {
            let mut file_info = SHFILEINFOW::default();
            let attributes = if item.is_dir { FILE_ATTRIBUTE_DIRECTORY } else { FILE_ATTRIBUTE_NORMAL };
            SHGetFileInfoW(
                &HSTRING::from(item.name.as_str()),
                attributes,
                Some(&mut file_info),
                size_of::<SHFILEINFOW>() as u32,
                SHGFI_SYSICONINDEX | SHGFI_USEFILEATTRIBUTES,
            );
            if let Ok(icon) = list.GetIcon(file_info.iIcon, ILD_TRANSPARENT.0) {
                let shift = STEP * i as i32;
                let _ = DrawIconEx(dc, offset - shift, shift, icon, icon_w, icon_h, 0, None, DI_NORMAL);
                let _ = DestroyIcon(icon);
            }
        }
        SelectObject(dc, previous);
        let _ = DeleteDC(dc);
        // Icons without an alpha channel leave alpha at zero: make them opaque.
        let pixels = std::slice::from_raw_parts_mut(bits.cast::<u32>(), (size.cx * size.cy) as usize);
        for px in pixels.iter_mut() {
            if *px >> 24 == 0 && *px & 0x00FF_FFFF != 0 {
                *px |= 0xFF00_0000;
            }
        }
        Ok((bitmap, size))
    }
}
