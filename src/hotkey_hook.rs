use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
#[cfg(windows)]
use std::sync::atomic::AtomicUsize;

#[cfg(windows)]
use windows_sys::Win32::{
    Devices::HumanInterfaceDevice::{HID_USAGE_GENERIC_KEYBOARD, HID_USAGE_PAGE_GENERIC},
    Foundation::{CloseHandle, HANDLE, LPARAM, LRESULT, WAIT_OBJECT_0, WPARAM},
    System::{
        LibraryLoader::GetModuleHandleW,
        Memory::{VirtualLock, VirtualUnlock},
        Threading::{
            CreateEventW, GetCurrentThread, GetCurrentThreadId, INFINITE, SetEvent,
            SetThreadPriority, THREAD_PRIORITY_HIGHEST, WaitForMultipleObjects,
        },
    },
    UI::{
        Input::{
            RAWINPUTDEVICE, RIDEV_REMOVE, RegisterRawInputDevices,
            KeyboardAndMouse::{
                GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
                SendInput, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU,
                VK_RSHIFT, VK_RWIN,
            },
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG,
            PM_NOREMOVE, PeekMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
            UnhookWindowsHookEx, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN,
            WM_SYSKEYUP,
        },
    },
};

/// Real-time left/right hardware state for all four modifier pairs (`Win`, `Ctrl`, `Alt`, `Shift`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ModifierSideState {
    pub lwin: bool,
    pub rwin: bool,
    pub lctrl: bool,
    pub rctrl: bool,
    pub lalt: bool,
    pub ralt: bool,
    pub lshift: bool,
    pub rshift: bool,
}

impl ModifierSideState {
    pub fn any_win(self) -> bool {
        self.lwin || self.rwin
    }

    pub fn any_ctrl(self) -> bool {
        self.lctrl || self.rctrl
    }

    pub fn any_alt(self) -> bool {
        self.lalt || self.ralt
    }

    pub fn any_shift(self) -> bool {
        self.lshift || self.rshift
    }
}

/// Reads the current left/right modifier state from Windows (`GetAsyncKeyState`).
pub fn read_modifier_side_state() -> ModifierSideState {
    #[cfg(windows)]
    unsafe {
        let down = |vk: u16| GetAsyncKeyState(vk as i32) < 0;
        ModifierSideState {
            lwin: down(VK_LWIN),
            rwin: down(VK_RWIN),
            lctrl: down(VK_LCONTROL),
            rctrl: down(VK_RCONTROL),
            lalt: down(VK_LMENU),
            ralt: down(VK_RMENU),
            lshift: down(VK_LSHIFT),
            rshift: down(VK_RSHIFT),
        }
    }
    #[cfg(not(windows))]
    ModifierSideState::default()
}

/// Requirement for a modifier key pair (either side-agnostic `Any` or side-specific `Left`/`Right`/`Both`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[repr(u8)]
pub enum ModifierRequirement {
    #[default]
    None = 0,
    Any = 1,
    Left = 2,
    Right = 3,
    Both = 4,
}

impl ModifierRequirement {
    pub fn add_left(&mut self) {
        *self = match *self {
            Self::None | Self::Left => Self::Left,
            Self::Right | Self::Both => Self::Both,
            Self::Any => Self::Any,
        };
    }

    pub fn add_right(&mut self) {
        *self = match *self {
            Self::None | Self::Right => Self::Right,
            Self::Left | Self::Both => Self::Both,
            Self::Any => Self::Any,
        };
    }

    pub fn add_any(&mut self) {
        *self = Self::Any;
    }

    pub fn is_side_specific(self) -> bool {
        matches!(self, Self::Left | Self::Right | Self::Both)
    }

    pub fn to_any_bool(self) -> bool {
        !matches!(self, Self::None)
    }

    pub fn matches(self, left: bool, right: bool) -> bool {
        match self {
            Self::None => !left && !right,
            Self::Any => left || right,
            Self::Left => left && !right,
            Self::Right => !left && right,
            Self::Both => left && right,
        }
    }

    pub fn from_sides(left: bool, right: bool) -> Self {
        match (left, right) {
            (true, true) => Self::Both,
            (true, false) => Self::Left,
            (false, true) => Self::Right,
            (false, false) => Self::None,
        }
    }

    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Any,
            2 => Self::Left,
            3 => Self::Right,
            4 => Self::Both,
            _ => Self::None,
        }
    }
}

/// Specification for a global hotkey that can distinguish Left/Right modifier keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SideAwareHotkeySpec {
    pub vk_code: u32,
    pub win: ModifierRequirement,
    pub ctrl: ModifierRequirement,
    pub alt: ModifierRequirement,
    pub shift: ModifierRequirement,
}

impl SideAwareHotkeySpec {
    pub fn is_side_specific(self) -> bool {
        self.win.is_side_specific()
            || self.ctrl.is_side_specific()
            || self.alt.is_side_specific()
            || self.shift.is_side_specific()
    }

    pub fn matches_modifiers(self, state: ModifierSideState) -> bool {
        self.win.matches(state.lwin, state.rwin)
            && self.ctrl.matches(state.lctrl, state.rctrl)
            && self.alt.matches(state.lalt, state.ralt)
            && self.shift.matches(state.lshift, state.rshift)
    }

    pub fn pack_modifiers(self) -> u32 {
        (self.win as u32)
            | ((self.ctrl as u32) << 4)
            | ((self.alt as u32) << 8)
            | ((self.shift as u32) << 12)
    }

    pub fn unpack(vk_code: u32, packed: u32) -> Self {
        Self {
            vk_code,
            win: ModifierRequirement::from_u8((packed & 0xF) as u8),
            ctrl: ModifierRequirement::from_u8(((packed >> 4) & 0xF) as u8),
            alt: ModifierRequirement::from_u8(((packed >> 8) & 0xF) as u8),
            shift: ModifierRequirement::from_u8(((packed >> 12) & 0xF) as u8),
        }
    }
}

fn pack_full_spec(spec: SideAwareHotkeySpec) -> u32 {
    ((spec.vk_code & 0xFFFF) << 16) | (spec.pack_modifiers() & 0xFFFF)
}

fn unpack_full_spec(packed: u32) -> SideAwareHotkeySpec {
    SideAwareHotkeySpec::unpack(packed >> 16, packed & 0xFFFF)
}

#[repr(C, align(64))]
struct HookHotState {
    enabled: AtomicBool,
    paused: AtomicBool,
    held: AtomicBool,
    #[cfg(windows)]
    altgr_synth_lctrl: AtomicBool,
    packed_spec: AtomicU32,
    #[cfg(windows)]
    last_held_down_ms: AtomicU32,
    #[cfg(windows)]
    trigger_flags: AtomicU32,
    #[cfg(windows)]
    trigger_event: AtomicUsize,
}

static HOOK_HOT: HookHotState = HookHotState {
    enabled: AtomicBool::new(false),
    paused: AtomicBool::new(false),
    held: AtomicBool::new(false),
    #[cfg(windows)]
    altgr_synth_lctrl: AtomicBool::new(false),
    packed_spec: AtomicU32::new(0),
    #[cfg(windows)]
    last_held_down_ms: AtomicU32::new(0),
    #[cfg(windows)]
    trigger_flags: AtomicU32::new(0),
    #[cfg(windows)]
    trigger_event: AtomicUsize::new(0),
};

#[cfg(windows)]
const TRIGGER_PENDING_BIT: u32 = 1 << 0;
#[cfg(windows)]
const TRIGGER_MASK_MENU_BIT: u32 = 1 << 1;
const HELD_STALE_TIMEOUT_MS: u32 = 1250;

fn should_fire_hotkey_trigger(was_held: bool, prev_down_ms: u32, now_ms: u32) -> bool {
    !was_held || now_ms.wrapping_sub(prev_down_ms) > HELD_STALE_TIMEOUT_MS
}

/// Unregisters keyboard Raw Input (`HID_USAGE_GENERIC_KEYBOARD`) for this process so
/// Windows 11 does not bypass `WH_KEYBOARD_LL` hooks (such as PowerToys Keyboard Manager)
/// when the app's window first gains focus, while keeping `WM_KEY*`/`WM_CHAR` and mouse
/// Raw Input intact.
pub fn unregister_raw_keyboard_input() -> bool {
    #[cfg(windows)]
    unsafe {
        let dev = RAWINPUTDEVICE {
            usUsagePage: HID_USAGE_PAGE_GENERIC,
            usUsage: HID_USAGE_GENERIC_KEYBOARD,
            dwFlags: RIDEV_REMOVE,
            hwndTarget: std::ptr::null_mut(),
        };
        RegisterRawInputDevices(&dev, 1, std::mem::size_of::<RAWINPUTDEVICE>() as u32) != 0
    }
    #[cfg(not(windows))]
    {
        true
    }
}

/// Returns `true` when a `SideAwareHotkeyHook` is currently active.
pub fn is_side_aware_hook_active() -> bool {
    HOOK_HOT.enabled.load(Ordering::Relaxed)
}

/// Low-level Windows keyboard hook (`WH_KEYBOARD_LL`) that intercepts side-specific global hotkeys
/// while letting all other combinations (including the opposite modifier side) pass through untouched.
pub struct SideAwareHotkeyHook {
    #[cfg(windows)]
    hook_thread_id: u32,
    #[cfg(windows)]
    hook_join_handle: Option<std::thread::JoinHandle<()>>,
    #[cfg(windows)]
    worker_join_handle: Option<std::thread::JoinHandle<()>>,
    #[cfg(windows)]
    trigger_event: usize,
    #[cfg(windows)]
    stop_event: usize,
}

impl SideAwareHotkeyHook {
    pub fn new(
        spec: SideAwareHotkeySpec,
        on_trigger: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        #[cfg(windows)]
        {
            let trigger_event = unsafe { CreateEventW(std::ptr::null(), 0, 0, std::ptr::null()) };
            if trigger_event.is_null() {
                return Err(format!(
                    "Failed to create hotkey trigger event: {}",
                    std::io::Error::last_os_error()
                ));
            }
            let stop_event = unsafe { CreateEventW(std::ptr::null(), 1, 0, std::ptr::null()) };
            if stop_event.is_null() {
                let err = std::io::Error::last_os_error();
                unsafe {
                    let _ = CloseHandle(trigger_event);
                }
                return Err(format!("Failed to create hotkey stop event: {err}"));
            }

            let trigger_raw = trigger_event as usize;
            let stop_raw = stop_event as usize;

            HOOK_HOT
                .packed_spec
                .store(pack_full_spec(spec), Ordering::SeqCst);
            HOOK_HOT.held.store(false, Ordering::SeqCst);
            HOOK_HOT.last_held_down_ms.store(0, Ordering::SeqCst);
            HOOK_HOT.paused.store(false, Ordering::SeqCst);
            HOOK_HOT.altgr_synth_lctrl.store(false, Ordering::SeqCst);
            HOOK_HOT.trigger_flags.store(0, Ordering::SeqCst);
            HOOK_HOT
                .trigger_event
                .store(trigger_raw, Ordering::Release);
            HOOK_HOT.enabled.store(true, Ordering::SeqCst);

            let worker_res = std::thread::Builder::new()
                .name("rapid_side_hotkey_worker".to_string())
                .spawn(move || unsafe {
                    let handles = [stop_raw as HANDLE, trigger_raw as HANDLE];
                    const WAIT_TRIGGER: u32 = WAIT_OBJECT_0 + 1;
                    loop {
                        let wait = WaitForMultipleObjects(2, handles.as_ptr(), 0, INFINITE);
                        if wait == WAIT_TRIGGER {
                            let flags = HOOK_HOT.trigger_flags.swap(0, Ordering::AcqRel);
                            if (flags & TRIGGER_PENDING_BIT) != 0
                                && HOOK_HOT.enabled.load(Ordering::Acquire)
                            {
                                if (flags & TRIGGER_MASK_MENU_BIT) != 0 {
                                    suppress_menu_on_modifier_release();
                                }
                                on_trigger();
                            }
                        } else {
                            break;
                        }
                    }
                });

            let worker_join_handle = match worker_res {
                Ok(handle) => handle,
                Err(e) => {
                    HOOK_HOT.enabled.store(false, Ordering::SeqCst);
                    HOOK_HOT.trigger_event.store(0, Ordering::SeqCst);
                    unsafe {
                        let _ = CloseHandle(trigger_event);
                        let _ = CloseHandle(stop_event);
                    }
                    return Err(format!("Failed to spawn hotkey worker thread: {e}"));
                }
            };

            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
            let hook_res = std::thread::Builder::new()
                .name("rapid_side_hotkey_hook".to_string())
                .spawn(move || unsafe {
                    let _ = SetThreadPriority(GetCurrentThread(), THREAD_PRIORITY_HIGHEST);
                    let mut msg: MSG = std::mem::zeroed();
                    let _ = PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, PM_NOREMOVE);

                    // Best-effort pin hot static state, hook callback, and active stack page so
                    // system working-set pressure cannot trigger hard page faults inside WH_KEYBOARD_LL.
                    let mut locked_regions: [(*const core::ffi::c_void, usize); 3] =
                        [(std::ptr::null(), 0); 3];
                    let mut locked_count = 0usize;
                    let mut try_lock = |addr: *const core::ffi::c_void, len: usize| -> bool {
                        if !addr.is_null() && len > 0 && VirtualLock(addr, len) != 0 {
                            locked_regions[locked_count] = (addr, len);
                            locked_count += 1;
                            true
                        } else {
                            false
                        }
                    };
                    let _ = try_lock(
                        &HOOK_HOT as *const HookHotState as *const core::ffi::c_void,
                        std::mem::size_of::<HookHotState>(),
                    );
                    let proc_addr = ll_keyboard_proc as *const () as *const core::ffi::c_void;
                    if !try_lock(proc_addr, 512) {
                        let _ = try_lock(proc_addr, 1);
                    }
                    let _ = try_lock(
                        &msg as *const MSG as *const core::ffi::c_void,
                        std::mem::size_of::<MSG>(),
                    );

                    let unlock_all = |regions: &[(*const core::ffi::c_void, usize)]| {
                        for &(addr, len) in regions {
                            if !addr.is_null() && len > 0 {
                                let _ = VirtualUnlock(addr, len);
                            }
                        }
                    };

                    let tid = GetCurrentThreadId();
                    let hmod = GetModuleHandleW(std::ptr::null());
                    let hhook: HHOOK =
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), hmod, 0);
                    if hhook.is_null() {
                        let err = std::io::Error::last_os_error();
                        unlock_all(&locked_regions[..locked_count]);
                        let _ = ready_tx.send(Err(format!(
                            "Failed to install keyboard hook: {err}"
                        )));
                        return;
                    }
                    let _ = ready_tx.send(Ok(tid));

                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        let _ = TranslateMessage(&msg);
                        let _ = DispatchMessageW(&msg);
                    }
                    let _ = UnhookWindowsHookEx(hhook);
                    unlock_all(&locked_regions[..locked_count]);
                });

            let hook_join_handle = match hook_res {
                Ok(handle) => handle,
                Err(e) => {
                    HOOK_HOT.enabled.store(false, Ordering::SeqCst);
                    HOOK_HOT.trigger_event.store(0, Ordering::SeqCst);
                    unsafe {
                        let _ = SetEvent(stop_event);
                    }
                    let _ = worker_join_handle.join();
                    unsafe {
                        let _ = CloseHandle(trigger_event);
                        let _ = CloseHandle(stop_event);
                    }
                    return Err(format!("Failed to spawn hotkey hook thread: {e}"));
                }
            };

            match ready_rx.recv() {
                Ok(Ok(hook_thread_id)) => Ok(Self {
                    hook_thread_id,
                    hook_join_handle: Some(hook_join_handle),
                    worker_join_handle: Some(worker_join_handle),
                    trigger_event: trigger_raw,
                    stop_event: stop_raw,
                }),
                Ok(Err(err)) => {
                    HOOK_HOT.enabled.store(false, Ordering::SeqCst);
                    let _ = hook_join_handle.join();
                    HOOK_HOT.trigger_event.store(0, Ordering::SeqCst);
                    unsafe {
                        let _ = SetEvent(stop_event);
                    }
                    let _ = worker_join_handle.join();
                    unsafe {
                        let _ = CloseHandle(trigger_event);
                        let _ = CloseHandle(stop_event);
                    }
                    Err(err)
                }
                Err(_) => {
                    HOOK_HOT.enabled.store(false, Ordering::SeqCst);
                    let _ = hook_join_handle.join();
                    HOOK_HOT.trigger_event.store(0, Ordering::SeqCst);
                    unsafe {
                        let _ = SetEvent(stop_event);
                    }
                    let _ = worker_join_handle.join();
                    unsafe {
                        let _ = CloseHandle(trigger_event);
                        let _ = CloseHandle(stop_event);
                    }
                    Err("Keyboard hook thread exited unexpectedly".to_string())
                }
            }
        }

        #[cfg(not(windows))]
        {
            let _ = on_trigger;
            HOOK_HOT
                .packed_spec
                .store(pack_full_spec(spec), Ordering::SeqCst);
            HOOK_HOT.held.store(false, Ordering::SeqCst);
            HOOK_HOT.paused.store(false, Ordering::SeqCst);
            HOOK_HOT.enabled.store(true, Ordering::SeqCst);
            Ok(Self {})
        }
    }

    pub fn update_spec(&self, spec: SideAwareHotkeySpec) {
        HOOK_HOT
            .packed_spec
            .store(pack_full_spec(spec), Ordering::SeqCst);
        HOOK_HOT.held.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        HOOK_HOT.last_held_down_ms.store(0, Ordering::SeqCst);
    }

    pub fn set_paused(&self, paused: bool) {
        HOOK_HOT.paused.store(paused, Ordering::SeqCst);
        HOOK_HOT.held.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        HOOK_HOT.last_held_down_ms.store(0, Ordering::SeqCst);
    }
}

impl Drop for SideAwareHotkeyHook {
    fn drop(&mut self) {
        HOOK_HOT.enabled.store(false, Ordering::SeqCst);
        HOOK_HOT.held.store(false, Ordering::SeqCst);
        #[cfg(windows)]
        {
            HOOK_HOT.last_held_down_ms.store(0, Ordering::SeqCst);
            HOOK_HOT.trigger_flags.store(0, Ordering::SeqCst);
            unsafe {
                let _ = PostThreadMessageW(self.hook_thread_id, WM_QUIT, 0, 0);
            }
            if let Some(handle) = self.hook_join_handle.take() {
                let _ = handle.join();
            }
            HOOK_HOT.trigger_event.store(0, Ordering::SeqCst);
            unsafe {
                let _ = SetEvent(self.stop_event as HANDLE);
            }
            if let Some(handle) = self.worker_join_handle.take() {
                let _ = handle.join();
            }
            unsafe {
                let _ = CloseHandle(self.trigger_event as HANDLE);
                let _ = CloseHandle(self.stop_event as HANDLE);
            }
        }
    }
}

#[cfg(windows)]
unsafe fn suppress_menu_on_modifier_release() {
    // Send a paired down+up of an unassigned virtual key (0xE8) so releasing Win/Alt does not open Start or a menu bar.
    const VK_MENU_MASK: u16 = 0xE8;
    unsafe {
        let make_input = |flags: u32| INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MENU_MASK,
                    wScan: 0,
                    dwFlags: flags,
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        };
        let inputs = [make_input(0), make_input(KEYEVENTF_KEYUP)];
        let _ = SendInput(
            inputs.len() as u32,
            inputs.as_ptr(),
            std::mem::size_of::<INPUT>() as i32,
        );
    }
}

#[cfg(windows)]
unsafe extern "system" fn ll_keyboard_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe {
        if ncode < 0 || !HOOK_HOT.enabled.load(Ordering::Relaxed) {
            return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
        }

        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        const LLKHF_INJECTED: u32 = 0x10;

        if (kb.flags & LLKHF_INJECTED) == 0 {
            let msg = wparam as u32;
            let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
            let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

            // Track Windows' synthetic LControl generated by AltGr (RAlt on international layouts)
            if kb.vkCode == VK_LCONTROL as u32 {
                if is_down {
                    HOOK_HOT
                        .altgr_synth_lctrl
                        .store(kb.scanCode == 0x21D, Ordering::Relaxed);
                } else if is_up {
                    HOOK_HOT.altgr_synth_lctrl.store(false, Ordering::Relaxed);
                }
            }

            let packed_spec = HOOK_HOT.packed_spec.load(Ordering::Relaxed);
            let target_vk = packed_spec >> 16;
            if kb.vkCode == target_vk {
                if is_up {
                    HOOK_HOT.held.store(false, Ordering::Relaxed);
                } else if is_down && !HOOK_HOT.paused.load(Ordering::Relaxed) {
                    let spec = unpack_full_spec(packed_spec);
                    let mut state = read_modifier_side_state();
                    if state.ralt
                        && state.lctrl
                        && HOOK_HOT.altgr_synth_lctrl.load(Ordering::Relaxed)
                    {
                        state.lctrl = false;
                    }

                    if spec.matches_modifiers(state) {
                        let now_ms = kb.time;
                        let prev_ms = HOOK_HOT.last_held_down_ms.swap(now_ms, Ordering::Relaxed);
                        let was_held = HOOK_HOT.held.swap(true, Ordering::SeqCst);
                        if should_fire_hotkey_trigger(was_held, prev_ms, now_ms) {
                            let mask_menu = spec.win != ModifierRequirement::None
                                || spec.alt != ModifierRequirement::None;
                            let flags = TRIGGER_PENDING_BIT
                                | if mask_menu { TRIGGER_MASK_MENU_BIT } else { 0 };
                            HOOK_HOT.trigger_flags.fetch_or(flags, Ordering::Release);
                            let ev = HOOK_HOT.trigger_event.load(Ordering::Acquire);
                            if ev != 0 {
                                let _ = SetEvent(ev as HANDLE);
                            }
                        }
                        return 1;
                    }
                }
            }
        }

        CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifier_requirement_matching() {
        assert!(ModifierRequirement::None.matches(false, false));
        assert!(!ModifierRequirement::None.matches(true, false));

        assert!(ModifierRequirement::Any.matches(true, false));
        assert!(ModifierRequirement::Any.matches(false, true));
        assert!(ModifierRequirement::Any.matches(true, true));
        assert!(!ModifierRequirement::Any.matches(false, false));

        assert!(ModifierRequirement::Left.matches(true, false));
        assert!(!ModifierRequirement::Left.matches(false, true));
        assert!(!ModifierRequirement::Left.matches(true, true));

        assert!(ModifierRequirement::Right.matches(false, true));
        assert!(!ModifierRequirement::Right.matches(true, false));
        assert!(!ModifierRequirement::Right.matches(true, true));

        assert!(ModifierRequirement::Both.matches(true, true));
        assert!(!ModifierRequirement::Both.matches(true, false));
    }

    #[test]
    fn test_side_aware_spec_pack_unpack_and_matching() {
        let spec = SideAwareHotkeySpec {
            vk_code: 0x41,
            win: ModifierRequirement::Left,
            ctrl: ModifierRequirement::None,
            alt: ModifierRequirement::Any,
            shift: ModifierRequirement::Right,
        };
        assert!(spec.is_side_specific());

        let packed = spec.pack_modifiers();
        let unpacked = SideAwareHotkeySpec::unpack(0x41, packed);
        assert_eq!(unpacked, spec);

        let matching_state = ModifierSideState {
            lwin: true,
            ralt: true,
            rshift: true,
            ..Default::default()
        };
        assert!(unpacked.matches_modifiers(matching_state));

        let wrong_shift = ModifierSideState {
            lwin: true,
            ralt: true,
            lshift: true,
            ..Default::default()
        };
        assert!(!unpacked.matches_modifiers(wrong_shift));

        let wrong_win = ModifierSideState {
            rwin: true,
            ralt: true,
            rshift: true,
            ..Default::default()
        };
        assert!(!unpacked.matches_modifiers(wrong_win));
    }

    #[test]
    fn test_pack_unpack_full_spec() {
        let reqs = [
            ModifierRequirement::None,
            ModifierRequirement::Any,
            ModifierRequirement::Left,
            ModifierRequirement::Right,
            ModifierRequirement::Both,
        ];
        for (idx, &win) in reqs.iter().enumerate() {
            for &ctrl in &reqs {
                for &alt in &reqs {
                    for &shift in &reqs {
                        let spec = SideAwareHotkeySpec {
                            vk_code: 0x41 + (idx as u32),
                            win,
                            ctrl,
                            alt,
                            shift,
                        };
                        let packed = pack_full_spec(spec);
                        let unpacked = unpack_full_spec(packed);
                        assert_eq!(unpacked, spec);
                    }
                }
            }
        }
    }

    #[test]
    fn test_side_aware_hook_lifecycle_and_active_flag() {
        assert!(!is_side_aware_hook_active());
        let spec = SideAwareHotkeySpec {
            vk_code: 0x41,
            win: ModifierRequirement::None,
            ctrl: ModifierRequirement::None,
            alt: ModifierRequirement::None,
            shift: ModifierRequirement::Right,
        };
        let hook = SideAwareHotkeyHook::new(spec, || {}).expect("install side-aware hook");
        assert!(is_side_aware_hook_active());

        let updated = SideAwareHotkeySpec {
            vk_code: 0x42,
            win: ModifierRequirement::Left,
            ctrl: ModifierRequirement::None,
            alt: ModifierRequirement::None,
            shift: ModifierRequirement::None,
        };
        hook.update_spec(updated);
        assert_eq!(unpack_full_spec(HOOK_HOT.packed_spec.load(Ordering::SeqCst)), updated);

        hook.set_paused(true);
        assert!(HOOK_HOT.paused.load(Ordering::SeqCst));
        hook.set_paused(false);
        assert!(!HOOK_HOT.paused.load(Ordering::SeqCst));

        // Trimming the working set while the hook is active must remain safe (hot hook pages are VirtualLock'd)
        crate::dwm::trim_process_working_set();
        assert!(is_side_aware_hook_active());

        drop(hook);
        assert!(!is_side_aware_hook_active());
    }

    #[test]
    fn test_should_fire_hotkey_trigger_blocks_repeats_and_recovers_stale_held() {
        assert!(should_fire_hotkey_trigger(false, 5_000, 5_000));
        assert!(should_fire_hotkey_trigger(false, 5_000, 5_010));

        // Hardware auto-repeats within the stale timeout remain suppressed while held
        assert!(!should_fire_hotkey_trigger(true, 10_000, 10_030));
        assert!(!should_fire_hotkey_trigger(true, 10_000, 10_500));
        assert!(!should_fire_hotkey_trigger(true, 10_000, 10_000 + HELD_STALE_TIMEOUT_MS));

        // Missed key-up (e.g., UAC / Secure Desktop) recovers once elapsed exceeds timeout
        assert!(should_fire_hotkey_trigger(true, 10_000, 10_000 + HELD_STALE_TIMEOUT_MS + 1));

        // Works safely across u32::MAX GetTickCount wraparound
        let prev = u32::MAX - 200;
        assert!(!should_fire_hotkey_trigger(true, prev, 100));
        assert!(should_fire_hotkey_trigger(true, prev, HELD_STALE_TIMEOUT_MS - 199));
    }
}
