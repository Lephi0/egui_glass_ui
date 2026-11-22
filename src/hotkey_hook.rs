use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Mutex, OnceLock};

#[cfg(windows)]
use windows_sys::Win32::{
    Foundation::{LPARAM, LRESULT, WPARAM},
    System::Threading::GetCurrentThreadId,
    UI::{
        Input::KeyboardAndMouse::{
            GetAsyncKeyState, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
            SendInput, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_LWIN, VK_RCONTROL, VK_RMENU,
            VK_RSHIFT, VK_RWIN,
        },
        WindowsAndMessaging::{
            CallNextHookEx, DispatchMessageW, GetMessageW, HHOOK, KBDLLHOOKSTRUCT, MSG,
            PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
            WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_QUIT, WM_SYSKEYDOWN, WM_SYSKEYUP,
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

static HOOK_ENABLED: AtomicBool = AtomicBool::new(false);
static HOOK_PAUSED: AtomicBool = AtomicBool::new(false);
static HOOK_TARGET_VK: AtomicU32 = AtomicU32::new(0);
static HOOK_MOD_SPEC: AtomicU32 = AtomicU32::new(0);
static HOOK_HELD: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
static HOOK_THREAD_ID: AtomicU32 = AtomicU32::new(0);
#[cfg(windows)]
static HOOK_ALTGR_SYNTH_LCTRL: AtomicBool = AtomicBool::new(false);
#[cfg(windows)]
const WM_HOOK_TRIGGERED: u32 = 0x8001; // WM_APP + 1

type TriggerCallback = Box<dyn Fn() + Send + Sync + 'static>;
static HOOK_CALLBACK: OnceLock<Mutex<Option<TriggerCallback>>> = OnceLock::new();

/// Low-level Windows keyboard hook (`WH_KEYBOARD_LL`) that intercepts side-specific global hotkeys
/// while letting all other combinations (including the opposite modifier side) pass through untouched.
pub struct SideAwareHotkeyHook {
    #[cfg(windows)]
    thread_id: u32,
    #[cfg(windows)]
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl SideAwareHotkeyHook {
    pub fn new(
        spec: SideAwareHotkeySpec,
        on_trigger: impl Fn() + Send + Sync + 'static,
    ) -> Result<Self, String> {
        let cb_slot = HOOK_CALLBACK.get_or_init(|| Mutex::new(None));
        if let Ok(mut guard) = cb_slot.lock() {
            *guard = Some(Box::new(on_trigger));
        }

        HOOK_TARGET_VK.store(spec.vk_code, Ordering::SeqCst);
        HOOK_MOD_SPEC.store(spec.pack_modifiers(), Ordering::SeqCst);
        HOOK_HELD.store(false, Ordering::SeqCst);
        HOOK_PAUSED.store(false, Ordering::SeqCst);
        HOOK_ENABLED.store(true, Ordering::SeqCst);

        #[cfg(windows)]
        {
            HOOK_ALTGR_SYNTH_LCTRL.store(false, Ordering::SeqCst);
            let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<u32, String>>();
            let join_handle = std::thread::Builder::new()
                .name("rapid_side_hotkey_hook".to_string())
                .spawn(move || unsafe {
                    let tid = GetCurrentThreadId();
                    HOOK_THREAD_ID.store(tid, Ordering::SeqCst);
                    let hhook: HHOOK =
                        SetWindowsHookExW(WH_KEYBOARD_LL, Some(ll_keyboard_proc), std::ptr::null_mut(), 0);
                    if hhook.is_null() {
                        HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                        let _ = ready_tx.send(Err(format!(
                            "Failed to install keyboard hook: {}",
                            std::io::Error::last_os_error()
                        )));
                        return;
                    }
                    let _ = ready_tx.send(Ok(tid));

                    let mut msg: MSG = std::mem::zeroed();
                    while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                        if msg.message == WM_HOOK_TRIGGERED {
                            if msg.wParam != 0 {
                                suppress_menu_on_modifier_release();
                            }
                            if let Some(lock) = HOOK_CALLBACK.get() {
                                if let Ok(guard) = lock.lock() {
                                    if let Some(ref cb) = *guard {
                                        cb();
                                    }
                                }
                            }
                            continue;
                        }
                        let _ = TranslateMessage(&msg);
                        let _ = DispatchMessageW(&msg);
                    }
                    let _ = UnhookWindowsHookEx(hhook);
                    HOOK_THREAD_ID.store(0, Ordering::SeqCst);
                })
                .map_err(|e| format!("Failed to spawn hotkey hook thread: {e}"))?;

            match ready_rx.recv() {
                Ok(Ok(thread_id)) => Ok(Self {
                    thread_id,
                    join_handle: Some(join_handle),
                }),
                Ok(Err(err)) => {
                    HOOK_ENABLED.store(false, Ordering::SeqCst);
                    let _ = join_handle.join();
                    Err(err)
                }
                Err(_) => {
                    HOOK_ENABLED.store(false, Ordering::SeqCst);
                    let _ = join_handle.join();
                    Err("Keyboard hook thread exited unexpectedly".to_string())
                }
            }
        }

        #[cfg(not(windows))]
        {
            Ok(Self {})
        }
    }

    pub fn update_spec(&self, spec: SideAwareHotkeySpec) {
        HOOK_TARGET_VK.store(spec.vk_code, Ordering::SeqCst);
        HOOK_MOD_SPEC.store(spec.pack_modifiers(), Ordering::SeqCst);
        HOOK_HELD.store(false, Ordering::SeqCst);
    }

    pub fn set_paused(&self, paused: bool) {
        HOOK_PAUSED.store(paused, Ordering::SeqCst);
        HOOK_HELD.store(false, Ordering::SeqCst);
    }
}

impl Drop for SideAwareHotkeyHook {
    fn drop(&mut self) {
        HOOK_ENABLED.store(false, Ordering::SeqCst);
        HOOK_HELD.store(false, Ordering::SeqCst);
        if let Some(lock) = HOOK_CALLBACK.get() {
            if let Ok(mut guard) = lock.lock() {
                *guard = None;
            }
        }
        #[cfg(windows)]
        {
            unsafe {
                let _ = PostThreadMessageW(self.thread_id, WM_QUIT, 0, 0);
            }
            if let Some(handle) = self.join_handle.take() {
                let _ = handle.join();
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
        if ncode < 0 || !HOOK_ENABLED.load(Ordering::Relaxed) {
            return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
        }

        let kb = &*(lparam as *const KBDLLHOOKSTRUCT);
        const LLKHF_INJECTED: u32 = 0x10;
        if (kb.flags & LLKHF_INJECTED) != 0 {
            return CallNextHookEx(std::ptr::null_mut(), ncode, wparam, lparam);
        }

        let msg = wparam as u32;
        let is_down = msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN;
        let is_up = msg == WM_KEYUP || msg == WM_SYSKEYUP;

        // Track Windows' synthetic LControl generated by AltGr (RAlt on international layouts)
        if kb.vkCode == VK_LCONTROL as u32 {
            if is_down {
                HOOK_ALTGR_SYNTH_LCTRL.store(kb.scanCode == 0x21D, Ordering::Relaxed);
            } else if is_up {
                HOOK_ALTGR_SYNTH_LCTRL.store(false, Ordering::Relaxed);
            }
        }

        let target_vk = HOOK_TARGET_VK.load(Ordering::Relaxed);
        if kb.vkCode == target_vk {
            if is_up {
                HOOK_HELD.store(false, Ordering::Relaxed);
            } else if is_down && !HOOK_PAUSED.load(Ordering::Relaxed) {
                let spec = SideAwareHotkeySpec::unpack(
                    target_vk,
                    HOOK_MOD_SPEC.load(Ordering::Relaxed),
                );
                let mut state = read_modifier_side_state();
                if state.ralt && state.lctrl && HOOK_ALTGR_SYNTH_LCTRL.load(Ordering::Relaxed) {
                    state.lctrl = false;
                }

                if spec.matches_modifiers(state) {
                    if !HOOK_HELD.swap(true, Ordering::SeqCst) {
                        let mask_menu = spec.win != ModifierRequirement::None
                            || spec.alt != ModifierRequirement::None;
                        let tid = HOOK_THREAD_ID.load(Ordering::Relaxed);
                        if tid != 0 {
                            let _ = PostThreadMessageW(
                                tid,
                                WM_HOOK_TRIGGERED,
                                usize::from(mask_menu),
                                0,
                            );
                        }
                    }
                    return 1;
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
}
