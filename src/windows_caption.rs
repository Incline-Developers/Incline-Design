//! Snap Layouts for the maximise button Incline draws on Windows.
//!
//! Windows 11 offers its snap layouts when the pointer rests on a window's
//! maximise button, and it knows the button only by the window answering
//! `HTMAXBUTTON` when asked what is under the pointer. The title bar is drawn
//! in egui (`ui/elements/title_bar.rs`), so this sits in front of winit's
//! window procedure and gives that answer over the rect the bar reports.
//!
//! The rect is then the system's, not the window's: the pointer over it
//! arrives as non-client messages that egui never sees. So the press and the
//! click are taken here, and the hover and press are kept for the bar to
//! light the button by.

use std::{cell::Cell, rc::Rc};

use windows::Win32::{
    Foundation::{HWND, LPARAM, LRESULT, POINT, RECT, WPARAM},
    Graphics::Gdi::{InvalidateRect, ScreenToClient},
    UI::{
        Input::KeyboardAndMouse::{TME_LEAVE, TME_NONCLIENT, TRACKMOUSEEVENT, TrackMouseEvent},
        Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
        WindowsAndMessaging::{
            HTMAXBUTTON, IsZoomed, SW_MAXIMIZE, SW_RESTORE, ShowWindow, WM_NCHITTEST, WM_NCLBUTTONDBLCLK, WM_NCLBUTTONDOWN, WM_NCLBUTTONUP, WM_NCMOUSELEAVE, WM_NCMOUSEMOVE,
        },
    },
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

const SUBCLASS_ID: usize = 0x494E_4D58; // "INMX"

/// Shared between the window procedure and the app, on the one UI thread.
#[derive(Default)]
struct ButtonState {
    /// The button in client-area physical pixels, as the bar last drew it.
    rect: Cell<Option<RECT>>,
    hovered: Cell<bool>,
    pressed: Cell<bool>,
}

/// The hook on the main window, for as long as it lives.
pub(crate) struct MaximizeButton {
    hwnd: HWND,
    state: Rc<ButtonState>,
    subclass_data: *const ButtonState,
}

impl MaximizeButton {
    pub(crate) fn new(window: &Window) -> windows::core::Result<Self> {
        let Ok(RawWindowHandle::Win32(handle)) = window.window_handle().map(|handle| handle.as_raw()) else {
            return Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG));
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        let state = Rc::new(ButtonState::default());
        let subclass_data = Rc::into_raw(state.clone());
        if !unsafe { SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, subclass_data as usize) }.as_bool() {
            drop(unsafe { Rc::from_raw(subclass_data) });
            return Err(windows::core::Error::from_thread());
        }
        Ok(Self { hwnd, state, subclass_data })
    }

    /// Where the bar drew the button, in client-area physical pixels, or
    /// `None` while it is not drawn.
    pub(crate) fn set_rect(&self, rect: Option<[i32; 4]>) {
        let rect = rect.map(|[left, top, right, bottom]| RECT { left, top, right, bottom });
        self.state.rect.set(rect);
        if rect.is_none() {
            self.state.hovered.set(false);
            self.state.pressed.set(false);
        }
    }

    pub(crate) fn hovered(&self) -> bool {
        self.state.hovered.get()
    }

    pub(crate) fn pressed(&self) -> bool {
        self.state.pressed.get()
    }
}

impl Drop for MaximizeButton {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(self.hwnd, Some(subclass_proc), SUBCLASS_ID);
            drop(Rc::from_raw(self.subclass_data));
        }
    }
}

/// Answer `HTMAXBUTTON` over the button, and act on the messages that answer
/// brings; everything else passes on to winit.
unsafe extern "system" fn subclass_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, data: usize) -> LRESULT {
    let state = unsafe { &*(data as *const ButtonState) };
    let on_button = wparam.0 == HTMAXBUTTON as usize;
    match message {
        WM_NCHITTEST => {
            if let Some(rect) = state.rect.get() {
                // Screen coordinates, as signed words.
                let mut point = POINT {
                    x: i32::from((lparam.0 & 0xFFFF) as u16 as i16),
                    y: i32::from(((lparam.0 >> 16) & 0xFFFF) as u16 as i16),
                };
                if unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() && point.x >= rect.left && point.x < rect.right && point.y >= rect.top && point.y < rect.bottom {
                    return LRESULT(HTMAXBUTTON as isize);
                }
            }
        }
        WM_NCMOUSEMOVE => {
            if on_button != state.hovered.get() {
                state.hovered.set(on_button);
                if on_button {
                    // Ask for the leave, which non-client tracking only sends when asked.
                    let mut track = TRACKMOUSEEVENT {
                        cbSize: size_of::<TRACKMOUSEEVENT>() as u32,
                        dwFlags: TME_LEAVE | TME_NONCLIENT,
                        hwndTrack: hwnd,
                        dwHoverTime: 0,
                    };
                    let _ = unsafe { TrackMouseEvent(&mut track) };
                } else {
                    state.pressed.set(false);
                }
                repaint(hwnd);
            }
            // The system would draw a classic button of its own here.
            if on_button {
                return LRESULT(0);
            }
        }
        WM_NCMOUSELEAVE => {
            if state.hovered.replace(false) | state.pressed.replace(false) {
                repaint(hwnd);
            }
        }
        WM_NCLBUTTONDOWN | WM_NCLBUTTONDBLCLK if on_button => {
            state.pressed.set(true);
            repaint(hwnd);
            return LRESULT(0);
        }
        WM_NCLBUTTONUP if on_button => {
            if state.pressed.replace(false) {
                let command = if unsafe { IsZoomed(hwnd) }.as_bool() { SW_RESTORE } else { SW_MAXIMIZE };
                let _ = unsafe { ShowWindow(hwnd, command) };
            }
            repaint(hwnd);
            return LRESULT(0);
        }
        _ => {}
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}

/// Have winit draw a frame, so the bar shows the button's new state.
fn repaint(hwnd: HWND) {
    let _ = unsafe { InvalidateRect(Some(hwnd), None, false) };
}
