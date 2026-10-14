//! Precision touchpad gestures on Windows.
//!
//! Windows reports a touchpad's two-finger scroll as the same `WM_MOUSEWHEEL`
//! a wheel sends, so winit cannot tell the two apart, and a pinch arrives as
//! Ctrl+wheel. DirectManipulation - what Chromium, Firefox and Blender use -
//! hands the gesture over whole instead: the touchpad's contacts drive a
//! viewport whose content transform pans and scales, and a wheel never reaches
//! it. This module owns that viewport and turns its transform into the pan and
//! pinch deltas the camera takes from a trackpad on macOS and Wayland.
//!
//! The viewport is driven by hand (`MANUALUPDATE`): while a gesture is live the
//! app calls [`Trackpad::poll`] every loop iteration, which advances it and
//! collects what moved. A touchpad that is not a precision one never sends the
//! hit test that starts this, and keeps scrolling as a wheel.

use std::{
    cell::Cell,
    rc::Rc,
    time::{Duration, Instant},
};

use windows::{
    Win32::{
        Foundation::{HWND, LPARAM, LRESULT, RECT, WPARAM},
        Graphics::DirectManipulation::{
            DIRECTMANIPULATION_CONFIGURATION_INTERACTION, DIRECTMANIPULATION_CONFIGURATION_SCALING, DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X,
            DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y, DIRECTMANIPULATION_ENABLED, DIRECTMANIPULATION_INERTIA, DIRECTMANIPULATION_READY, DIRECTMANIPULATION_RUNNING,
            DIRECTMANIPULATION_STATUS, DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE, DirectManipulationManager, IDirectManipulationContent, IDirectManipulationManager,
            IDirectManipulationUpdateManager, IDirectManipulationViewport, IDirectManipulationViewportEventHandler, IDirectManipulationViewportEventHandler_Impl,
        },
        System::Com::{CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx},
        UI::{
            Input::Pointer::GetPointerType,
            Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass},
            WindowsAndMessaging::{DM_POINTERHITTEST, POINTER_INPUT_TYPE, PT_TOUCHPAD},
        },
    },
    core::{Ref, Result, implement},
};
use winit::{
    raw_window_handle::{HasWindowHandle, RawWindowHandle},
    window::Window,
};

/// The viewport's extent, in its own units. Large, so a gesture never runs
/// into its edge; the transform is reset to it whenever a gesture ends.
const VIEWPORT_EXTENT: f32 = 10_000.0;
/// Below this a transform component is taken as unchanged.
const EPSILON: f32 = 3.0e-5;
/// How long after a hit test the viewport is advanced even though it has not
/// reported a running gesture yet: it only does so from inside an update.
const START_WINDOW: Duration = Duration::from_millis(500);
const SUBCLASS_ID: usize = 0x494E_434C; // "INCL"

/// What a gesture moved since the last poll.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct TrackpadMotion {
    /// Drag in physical pixels, signed so the scene follows the fingers.
    pub(crate) pan: (f64, f64),
    /// Relative magnification: `0.1` is a tenth larger, negative shrinks.
    pub(crate) magnification: f64,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Gesture {
    None,
    Pan,
    Pinch,
}

/// Shared between the event handler, which DirectManipulation calls from
/// inside `Update`, and the app, which drains it after. Plain cells, as the
/// handler can be re-entered by the reset it makes.
struct GestureState {
    status: Cell<DIRECTMANIPULATION_STATUS>,
    gesture: Cell<Gesture>,
    last_x: Cell<f32>,
    last_y: Cell<f32>,
    last_scale: Cell<f32>,
    pan_x: Cell<f64>,
    pan_y: Cell<f64>,
    /// Product of the scale steps since the last poll.
    scale: Cell<f64>,
}

impl GestureState {
    fn new() -> Self {
        Self {
            status: Cell::new(DIRECTMANIPULATION_ENABLED),
            gesture: Cell::new(Gesture::None),
            last_x: Cell::new(0.0),
            last_y: Cell::new(0.0),
            last_scale: Cell::new(1.0),
            pan_x: Cell::new(0.0),
            pan_y: Cell::new(0.0),
            scale: Cell::new(1.0),
        }
    }

    fn running(&self) -> bool {
        matches!(self.status.get(), DIRECTMANIPULATION_RUNNING | DIRECTMANIPULATION_INERTIA)
    }
}

#[implement(IDirectManipulationViewportEventHandler)]
struct ViewportHandler {
    state: Rc<GestureState>,
}

impl IDirectManipulationViewportEventHandler_Impl for ViewportHandler_Impl {
    fn OnViewportStatusChanged(&self, viewport: Ref<IDirectManipulationViewport>, current: DIRECTMANIPULATION_STATUS, previous: DIRECTMANIPULATION_STATUS) -> Result<()> {
        if current == previous {
            return Ok(());
        }
        self.state.status.set(current);
        // A finished gesture puts the content back where it started, so the
        // next one measures from identity. The bookkeeping is reset first:
        // the zoom below reports the identity transform straight back.
        if current == DIRECTMANIPULATION_READY || previous == DIRECTMANIPULATION_ENABLED {
            let moved = self.state.gesture.replace(Gesture::None) != Gesture::None;
            self.state.last_x.set(0.0);
            self.state.last_y.set(0.0);
            self.state.last_scale.set(1.0);
            if moved && let Some(viewport) = viewport.as_ref() {
                unsafe { viewport.ZoomToRect(0.0, 0.0, VIEWPORT_EXTENT, VIEWPORT_EXTENT, false)? };
            }
        }
        Ok(())
    }

    fn OnViewportUpdated(&self, _viewport: Ref<IDirectManipulationViewport>) -> Result<()> {
        Ok(())
    }

    fn OnContentUpdated(&self, _viewport: Ref<IDirectManipulationViewport>, content: Ref<IDirectManipulationContent>) -> Result<()> {
        let Some(content) = content.as_ref() else {
            return Ok(());
        };
        let mut transform = [0.0_f32; 6];
        unsafe { content.GetContentTransform(&mut transform)? };
        let (scale, x, y) = (transform[0], transform[4], transform[5]);
        let state = &self.state;
        if scale <= 0.0 || ((scale - state.last_scale.get()).abs() <= EPSILON && (x - state.last_x.get()).abs() <= EPSILON && (y - state.last_y.get()).abs() <= EPSILON) {
            return Ok(());
        }
        // Every gesture starts as a pan, and becomes a pinch once the scale
        // moves - DirectManipulation can take a few updates to see one. The
        // two are never mixed: a pinch's translation only recentres the
        // scale, and the camera zooms towards the cursor itself.
        if state.gesture.get() == Gesture::None {
            state.gesture.set(Gesture::Pan);
        }
        if state.gesture.get() == Gesture::Pan && (scale - 1.0).abs() > EPSILON {
            state.gesture.set(Gesture::Pinch);
        }
        match state.gesture.get() {
            Gesture::Pinch => state.scale.set(state.scale.get() * f64::from(scale / state.last_scale.get())),
            Gesture::Pan => {
                state.pan_x.set(state.pan_x.get() + f64::from(x - state.last_x.get()));
                state.pan_y.set(state.pan_y.get() + f64::from(y - state.last_y.get()));
            }
            Gesture::None => {}
        }
        state.last_x.set(x);
        state.last_y.set(y);
        state.last_scale.set(scale);
        Ok(())
    }
}

/// DirectManipulation on the app window, for as long as the window lives.
pub(crate) struct Trackpad {
    hwnd: HWND,
    manager: IDirectManipulationManager,
    update_manager: IDirectManipulationUpdateManager,
    viewport: IDirectManipulationViewport,
    handler_cookie: u32,
    state: Rc<GestureState>,
    hit_test_at: Rc<Cell<Option<Instant>>>,
    /// The subclass procedure's reference data, freed once it is removed.
    subclass_data: *mut SubclassData,
}

struct SubclassData {
    viewport: IDirectManipulationViewport,
    hit_test_at: Rc<Cell<Option<Instant>>>,
}

impl Trackpad {
    pub(crate) fn new(window: &Window) -> Result<Self> {
        let Ok(RawWindowHandle::Win32(handle)) = window.window_handle().map(|handle| handle.as_raw()) else {
            return Err(windows::core::Error::from_hresult(windows::Win32::Foundation::E_INVALIDARG));
        };
        let hwnd = HWND(handle.hwnd.get() as *mut _);
        unsafe {
            // winit has already initialised COM on this thread for drag and
            // drop; this only covers a build without it, and its "already
            // initialised" answers are not failures.
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            let manager: IDirectManipulationManager = CoCreateInstance(&DirectManipulationManager, None, CLSCTX_INPROC_SERVER)?;
            let update_manager: IDirectManipulationUpdateManager = manager.GetUpdateManager()?;
            let viewport: IDirectManipulationViewport = manager.CreateViewport(None, hwnd)?;
            viewport.ActivateConfiguration(
                DIRECTMANIPULATION_CONFIGURATION_INTERACTION
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_X
                    | DIRECTMANIPULATION_CONFIGURATION_TRANSLATION_Y
                    | DIRECTMANIPULATION_CONFIGURATION_SCALING,
            )?;
            viewport.SetViewportOptions(DIRECTMANIPULATION_VIEWPORT_OPTIONS_MANUALUPDATE)?;
            let state = Rc::new(GestureState::new());
            let handler: IDirectManipulationViewportEventHandler = ViewportHandler { state: state.clone() }.into();
            let handler_cookie = viewport.AddEventHandler(Some(hwnd), &handler)?;
            let extent = VIEWPORT_EXTENT as i32;
            viewport.SetViewportRect(&RECT {
                left: 0,
                top: 0,
                right: extent,
                bottom: extent,
            })?;
            manager.Activate(hwnd)?;
            viewport.Enable()?;

            let hit_test_at = Rc::new(Cell::new(None));
            let subclass_data = Box::into_raw(Box::new(SubclassData {
                viewport: viewport.clone(),
                hit_test_at: hit_test_at.clone(),
            }));
            if !SetWindowSubclass(hwnd, Some(subclass_proc), SUBCLASS_ID, subclass_data as usize).as_bool() {
                drop(Box::from_raw(subclass_data));
                let _ = viewport.RemoveEventHandler(handler_cookie);
                let _ = manager.Deactivate(hwnd);
                return Err(windows::core::Error::from_thread());
            }
            Ok(Self {
                hwnd,
                manager,
                update_manager,
                viewport,
                handler_cookie,
                state,
                hit_test_at,
                subclass_data,
            })
        }
    }

    /// Whether a gesture is live, so the loop must keep polling rather than
    /// wait for an event: the touchpad's movement reaches DirectManipulation,
    /// not the window.
    pub(crate) fn active(&self) -> bool {
        self.state.running() || self.hit_test_at.get().is_some_and(|at| at.elapsed() < START_WINDOW)
    }

    /// Advance a live gesture and take what it moved since the last poll.
    pub(crate) fn poll(&mut self) -> Option<TrackpadMotion> {
        if !self.active() {
            return None;
        }
        if self.state.running() {
            self.hit_test_at.set(None);
        }
        if let Err(error) = unsafe { self.update_manager.Update(None) } {
            log::warn!("DirectManipulation update failed: {error}");
        }
        let state = &self.state;
        let motion = TrackpadMotion {
            pan: (state.pan_x.replace(0.0), state.pan_y.replace(0.0)),
            magnification: state.scale.replace(1.0) - 1.0,
        };
        (motion.pan != (0.0, 0.0) || motion.magnification != 0.0).then_some(motion)
    }
}

impl Drop for Trackpad {
    fn drop(&mut self) {
        unsafe {
            let _ = RemoveWindowSubclass(self.hwnd, Some(subclass_proc), SUBCLASS_ID);
            drop(Box::from_raw(self.subclass_data));
            let _ = self.viewport.RemoveEventHandler(self.handler_cookie);
            let _ = self.viewport.Abandon();
            let _ = self.manager.Deactivate(self.hwnd);
        }
    }
}

/// Sits in front of winit's window procedure for the one message it does not
/// handle: Windows asks the window, as a touchpad gesture begins, whether
/// DirectManipulation should take it. Everything else passes straight on.
unsafe extern "system" fn subclass_proc(hwnd: HWND, message: u32, wparam: WPARAM, lparam: LPARAM, _id: usize, data: usize) -> LRESULT {
    if message == DM_POINTERHITTEST {
        let data = unsafe { &*(data as *const SubclassData) };
        let pointer_id = (wparam.0 & 0xFFFF) as u32;
        let mut pointer_type = POINTER_INPUT_TYPE::default();
        if unsafe { GetPointerType(pointer_id, &mut pointer_type) }.is_ok() && pointer_type == PT_TOUCHPAD {
            if let Err(error) = unsafe { data.viewport.SetContact(pointer_id) } {
                log::warn!("DirectManipulation could not take the touchpad contact: {error}");
            }
            data.hit_test_at.set(Some(Instant::now()));
        }
    }
    unsafe { DefSubclassProc(hwnd, message, wparam, lparam) }
}
