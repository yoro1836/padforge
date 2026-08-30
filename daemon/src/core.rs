// FFI types, ioctl constants, and low-level device handling.
use serde::{Deserialize, Serialize};
use std::ffi::{CString, c_char};
use std::fs;
use std::io::{self, ErrorKind};
use std::mem;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::path::{Path, PathBuf};

// ---------------------------------------------------------------------------
// Raw FFI
// ---------------------------------------------------------------------------
unsafe extern "C" {
    pub fn open(path: *const c_char, flags: i32, mode: u32) -> i32;
    pub fn close(fd: i32) -> i32;
    pub fn read(fd: i32, buf: *mut u8, count: usize) -> isize;
    pub fn write(fd: i32, buf: *const u8, count: usize) -> isize;
    pub fn ioctl(fd: i32, request: u32, ...) -> i32;
    pub fn epoll_create1(flags: i32) -> i32;
    pub fn epoll_ctl(epfd: i32, op: i32, fd: i32, event: *mut EpollEvent) -> i32;
    pub fn epoll_wait(epfd: i32, events: *mut EpollEvent, maxevents: i32, timeout: i32) -> i32;
    pub fn inotify_init1(flags: i32) -> i32;
    pub fn inotify_add_watch(fd: i32, pathname: *const c_char, mask: u32) -> i32;
}

// ---------------------------------------------------------------------------
// ioctl helpers
// ---------------------------------------------------------------------------
const fn ioc(dir: u32, ty: u8, nr: u8, size: u8) -> u32 {
    (dir << 30) | ((ty as u32) << 8) | (nr as u32) | ((size as u32) << 16)
}
const fn io_u(ty: u8, nr: u8) -> u32 {
    ioc(0, ty, nr, 0)
}
const fn ior(ty: u8, nr: u8, size: u8) -> u32 {
    ioc(2, ty, nr, size)
}
const fn iow(ty: u8, nr: u8, size: u8) -> u32 {
    ioc(1, ty, nr, size)
}

// ---------------------------------------------------------------------------
// Kernel ABI structs
// ---------------------------------------------------------------------------
#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct TimeVal {
    pub tv_sec: isize,
    pub tv_usec: isize,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct InputEvent {
    pub time: TimeVal,
    pub type_: u16,
    pub code: u16,
    pub value: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct InputId {
    pub bustype: u16,
    pub vendor: u16,
    pub product: u16,
    pub version: u16,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct InputAbsInfo {
    pub value: i32,
    pub minimum: i32,
    pub maximum: i32,
    pub fuzz: i32,
    pub flat: i32,
    pub resolution: i32,
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct UinputAbsSetup {
    pub code: u16,
    __pad: [u8; 2],
    pub absinfo: InputAbsInfo,
}

#[repr(C)]
pub struct UinputSetup {
    pub id: InputId,
    pub name: [u8; UINPUT_MAX_NAME_SIZE],
    pub ff_effects_max: u32,
}

impl Default for UinputSetup {
    fn default() -> Self {
        UinputSetup {
            id: InputId::default(),
            name: [0u8; UINPUT_MAX_NAME_SIZE],
            ff_effects_max: 0,
        }
    }
}

#[repr(C)]
#[derive(Default, Clone, Copy)]
pub struct EpollEvent {
    pub events: u32,
    pub data: u64,
}

pub const EPOLL_CTL_ADD: i32 = 1;
pub const EPOLL_CTL_DEL: i32 = 2;
pub const EPOLLIN: u32 = 0x001;
pub const EPOLLHUP: u32 = 0x010;
pub const EPOLLERR: u32 = 0x008;

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------
pub const INPUT_DIR: &str = "/dev/input";
pub const UINPUT_MAX_NAME_SIZE: usize = 80;

pub const EV_SYN: i32 = 0x00;
pub const EV_KEY: i32 = 0x01;
pub const EV_ABS: i32 = 0x03;
pub const SYN_REPORT: u32 = 0;
pub const ABS_X: u32 = 0x00;
pub const ABS_Y: u32 = 0x01;
pub const ABS_Z: u32 = 0x02;
pub const ABS_RZ: u32 = 0x05;
pub const ABS_RX: u32 = 0x03;
pub const ABS_RY: u32 = 0x04;
pub const KEY_CNT: usize = 768;
pub const ABS_CNT: usize = 0x40;
pub const BUS_USB: u16 = 0x03;

pub const EVIOCGID: u32 = ior(b'E', 0x02, mem::size_of::<InputId>() as u8);
pub const EVIOCGRAB: u32 = iow(b'E', 0x90, mem::size_of::<i32>() as u8);
#[allow(dead_code)]
pub const fn eviocgbit(ev: u8, len: u8) -> u32 {
    ioc(2, b'E', 0x20 + ev, len)
}
#[allow(dead_code)]
pub const fn eviocgabs(abs: u8) -> u32 {
    ior(b'E', 0x40 + abs, mem::size_of::<InputAbsInfo>() as u8)
}

pub const UI_DEV_CREATE: u32 = io_u(b'U', 1);
pub const UI_DEV_DESTROY: u32 = io_u(b'U', 2);
pub const UI_DEV_SETUP: u32 = iow(b'U', 3, mem::size_of::<UinputSetup>() as u8);
pub const UI_ABS_SETUP: u32 = iow(b'U', 4, mem::size_of::<UinputAbsSetup>() as u8);
pub const UI_SET_EVBIT: u32 = iow(b'U', 100, mem::size_of::<i32>() as u8);
pub const UI_SET_KEYBIT: u32 = iow(b'U', 101, mem::size_of::<i32>() as u8);
pub const UI_SET_ABSBIT: u32 = iow(b'U', 103, mem::size_of::<i32>() as u8);

pub const IN_NONBLOCK: i32 = 0o4000;
pub const IN_CREATE: u32 = 0x0000_0100;
pub const IN_DELETE: u32 = 0x0000_0200;

pub const O_RDONLY: i32 = 0o0;
pub const O_WRONLY: i32 = 0o1;
pub const O_NONBLOCK: i32 = 0o4000;
pub const EAGAIN: i32 = 11;

pub const RAW_FILE_L: &str = "/tmp/keyforge_raw_L";
pub const RAW_FILE_R: &str = "/tmp/keyforge_raw_R";
pub const PLUGIN_MANIFEST: &str = "/sdcard/.keyforge/manifest.json";

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------
pub fn get_errno() -> i32 {
    io::Error::last_os_error().raw_os_error().unwrap_or(0)
}

pub unsafe fn do_ioctl(fd: i32, req: u32, arg: usize) -> i32 {
    unsafe { ioctl(fd, req, arg) }
}

pub unsafe fn do_ioctl_ptr<T>(fd: i32, req: u32, arg: *mut T) -> i32 {
    unsafe { ioctl(fd, req, arg as usize) }
}

pub unsafe fn write_ev(ufd: i32, ev: &InputEvent) {
    unsafe {
        write(
            ufd,
            ev as *const _ as *const u8,
            mem::size_of::<InputEvent>(),
        );
    }
}

// ---------------------------------------------------------------------------
// Device
// ---------------------------------------------------------------------------
#[derive(Clone, Debug, Deserialize, Serialize)]
struct HiddenDeviceState {
    path: PathBuf,
    mode: u32,
    dev: u64,
    ino: u64,
    rdev: u64,
}

impl HiddenDeviceState {
    fn capture(path: &Path) -> io::Result<Self> {
        let metadata = fs::metadata(path)?;
        Ok(Self {
            path: path.to_path_buf(),
            mode: metadata.mode() & 0o7777,
            dev: metadata.dev(),
            ino: metadata.ino(),
            rdev: metadata.rdev(),
        })
    }

    fn matches(&self, metadata: &fs::Metadata) -> bool {
        self.dev == metadata.dev() && self.ino == metadata.ino() && self.rdev == metadata.rdev()
    }

    fn save(&self, state_path: &Path) -> io::Result<()> {
        let temporary = state_path.with_extension("tmp");
        let encoded = serde_json::to_vec(self)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))?;
        fs::write(&temporary, encoded)?;
        fs::set_permissions(&temporary, fs::Permissions::from_mode(0o600))?;
        fs::rename(temporary, state_path)
    }

    fn load(state_path: &Path) -> io::Result<Self> {
        let encoded = fs::read(state_path)?;
        serde_json::from_slice(&encoded)
            .map_err(|error| io::Error::new(ErrorKind::InvalidData, error))
    }

    fn restore(&self) -> io::Result<bool> {
        let metadata = match fs::metadata(&self.path) {
            Ok(metadata) => metadata,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(false),
            Err(error) => return Err(error),
        };
        if !self.matches(&metadata) {
            return Ok(false);
        }
        fs::set_permissions(&self.path, fs::Permissions::from_mode(self.mode))?;
        Ok(true)
    }
}

pub struct Device {
    pub fd: i32,
    pub ufd: i32,
    pub path: Option<PathBuf>,
    pub lx: i32,
    pub ly: i32,
    pub rx: i32,
    pub ry: i32,
    pub ld: bool,
    pub rd: bool,
    hide_state_path: Option<PathBuf>,
    hidden: Option<HiddenDeviceState>,
}

impl Device {
    pub fn new(hide_state_path: Option<PathBuf>) -> Self {
        Device {
            fd: -1,
            ufd: -1,
            path: None,
            lx: 0,
            ly: 0,
            rx: 0,
            ry: 0,
            ld: false,
            rd: false,
            hide_state_path,
            hidden: None,
        }
    }

    pub fn restore_hidden_state(state_path: &Path) -> io::Result<bool> {
        if !state_path.exists() {
            return Ok(false);
        }
        let state = HiddenDeviceState::load(state_path)?;
        let restored = state.restore()?;
        match fs::remove_file(state_path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => return Err(error),
        }
        Ok(restored)
    }

    pub fn set_hidden(&mut self, hide: bool) -> io::Result<()> {
        if !hide {
            let restored = if let Some(state) = self.hidden.as_ref() {
                state.restore().map(|_| ())
            } else if let Some(state_path) = self.hide_state_path.as_ref()
                && state_path.exists()
            {
                Self::restore_hidden_state(state_path).map(|_| ())
            } else {
                Ok(())
            };
            if restored.is_ok()
                && self.hidden.is_some()
                && let Some(state_path) = self.hide_state_path.as_ref()
            {
                match fs::remove_file(state_path) {
                    Ok(()) => {}
                    Err(error) if error.kind() == ErrorKind::NotFound => {}
                    Err(error) => return Err(error),
                }
            }
            self.hidden = None;
            return restored;
        }

        if self.hidden.is_some() {
            return Ok(());
        }
        let path = self.path.as_ref().ok_or_else(|| {
            io::Error::new(ErrorKind::NotFound, "no physical input device is connected")
        })?;
        let state_path = self.hide_state_path.as_ref().ok_or_else(|| {
            io::Error::new(
                ErrorKind::InvalidInput,
                "hidden-device state path was not provided",
            )
        })?;

        if state_path.exists() {
            Self::restore_hidden_state(state_path)?;
        }
        let state = HiddenDeviceState::capture(path)?;
        state.save(state_path)?;
        if let Err(error) = fs::set_permissions(path, fs::Permissions::from_mode(0o000)) {
            let _ = fs::remove_file(state_path);
            return Err(error);
        }
        self.hidden = Some(state);
        Ok(())
    }

    pub fn deinit(&mut self) {
        let _ = self.set_hidden(false);
        unsafe {
            if self.ufd >= 0 {
                do_ioctl(self.ufd, UI_DEV_DESTROY, 0);
                close(self.ufd);
                self.ufd = -1;
            }
            if self.fd >= 0 {
                do_ioctl(self.fd, EVIOCGRAB, 0);
                close(self.fd);
                self.fd = -1;
            }
        }
        self.path = None;
    }

    pub fn init_u(&mut self, fd: i32, vid: u16) -> bool {
        unsafe {
            let cpath = CString::new("/dev/uinput").unwrap();
            let u = open(cpath.as_ptr(), O_WRONLY | O_NONBLOCK, 0);
            if u < 0 {
                return false;
            }
            do_ioctl(u, UI_SET_EVBIT, EV_KEY as usize);
            do_ioctl(u, UI_SET_EVBIT, EV_ABS as usize);
            do_ioctl(u, UI_SET_EVBIT, EV_SYN as usize);

            // Copy KEY bits from physical device (same as memo.c)
            let key_bytes = KEY_CNT.div_ceil(8);
            let mut kbuf: Vec<u8> = vec![0u8; key_bytes];
            if do_ioctl_ptr(
                fd,
                eviocgbit(EV_KEY as u8, key_bytes as u8),
                kbuf.as_mut_ptr(),
            ) >= 0
            {
                for i in 0..KEY_CNT {
                    if (kbuf[i / 8] >> (i % 8)) & 1 != 0 {
                        do_ioctl(u, UI_SET_KEYBIT, i);
                    }
                }
            }

            // Copy ABS bits + absinfo from physical device (same as memo.c)
            let abs_bytes = ABS_CNT.div_ceil(8);
            let mut abuf: Vec<u8> = vec![0u8; abs_bytes];
            if do_ioctl_ptr(
                fd,
                eviocgbit(EV_ABS as u8, abs_bytes as u8),
                abuf.as_mut_ptr(),
            ) >= 0
            {
                for i in 0..ABS_CNT as u32 {
                    if (abuf[i as usize / 8] >> (i as usize % 8)) & 1 != 0 {
                        let mut info = InputAbsInfo::default();
                        if do_ioctl_ptr(fd, eviocgabs(i as u8), &mut info) >= 0 {
                            do_ioctl(u, UI_SET_ABSBIT, i as usize);
                            // Override stick axes range (same as memo.c)
                            if i == ABS_X || i == ABS_Y || i == ABS_RX || i == ABS_RY {
                                info.minimum = -32767;
                                info.maximum = 32767;
                                info.flat = 0;
                                info.fuzz = 0;
                            }
                            let mut s = UinputAbsSetup {
                                code: i as u16,
                                __pad: [0; 2],
                                absinfo: info,
                            };
                            do_ioctl_ptr(u, UI_ABS_SETUP, &mut s);
                        }
                    }
                }
            }

            // identity
            let mut us = UinputSetup::default();
            us.id.bustype = BUS_USB;
            us.id.vendor = vid;
            us.id.product = 0x02d1;
            us.id.version = 1;
            let label = b"KeyForge Virtual Controller";
            us.name[..label.len()].copy_from_slice(label);

            if do_ioctl_ptr(u, UI_DEV_SETUP, &mut us) < 0 || do_ioctl(u, UI_DEV_CREATE, 0) < 0 {
                close(u);
                return false;
            }
            self.ufd = u;
            true
        }
    }

    pub fn find_device(vid: u16, pid: u16) -> Option<(i32, PathBuf)> {
        let dir = fs::read_dir(INPUT_DIR).ok()?;
        for entry in dir.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if !name_str.starts_with("event") {
                continue;
            }
            let path = entry.path();
            let cpath = match CString::new(path.to_string_lossy().as_bytes()) {
                Ok(path) => path,
                Err(_) => continue,
            };
            unsafe {
                let fd = open(cpath.as_ptr(), O_RDONLY | O_NONBLOCK, 0);
                if fd < 0 {
                    continue;
                }
                let mut id = InputId::default();
                if do_ioctl_ptr(fd, EVIOCGID, &mut id) == 0
                    && id.vendor == vid
                    && id.product == pid
                    && do_ioctl(fd, EVIOCGRAB, 1) == 0
                {
                    return Some((fd, path));
                }
                close(fd);
            }
        }
        None
    }

    /// Check if the physical device is still connected by doing a no-op ioctl.
    /// Returns false if the fd is invalid or the device is gone.
    pub fn is_alive(fd: i32) -> bool {
        if fd < 0 {
            return false;
        }
        let mut id = InputId::default();
        unsafe { do_ioctl_ptr(fd, EVIOCGID, &mut id) == 0 }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.deinit();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!("keyforge-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        path
    }

    #[test]
    fn hidden_state_restores_original_mode() {
        let dir = test_dir("restore-mode");
        let device = dir.join("event7");
        let state_path = dir.join("hidden.json");
        fs::write(&device, b"device").unwrap();
        fs::set_permissions(&device, fs::Permissions::from_mode(0o640)).unwrap();

        let state = HiddenDeviceState::capture(&device).unwrap();
        state.save(&state_path).unwrap();
        fs::set_permissions(&device, fs::Permissions::from_mode(0o000)).unwrap();

        assert!(Device::restore_hidden_state(&state_path).unwrap());
        assert_eq!(fs::metadata(&device).unwrap().mode() & 0o7777, 0o640);
        assert!(!state_path.exists());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn hidden_state_never_changes_a_replaced_node() {
        let dir = test_dir("replaced-node");
        let device = dir.join("event7");
        let replacement = dir.join("replacement");
        let state_path = dir.join("hidden.json");
        fs::write(&device, b"original").unwrap();
        fs::set_permissions(&device, fs::Permissions::from_mode(0o640)).unwrap();
        let state = HiddenDeviceState::capture(&device).unwrap();
        state.save(&state_path).unwrap();

        fs::write(&replacement, b"replacement").unwrap();
        fs::set_permissions(&replacement, fs::Permissions::from_mode(0o600)).unwrap();
        fs::rename(&replacement, &device).unwrap();

        assert!(!Device::restore_hidden_state(&state_path).unwrap());
        assert_eq!(fs::metadata(&device).unwrap().mode() & 0o7777, 0o600);
        assert!(!state_path.exists());
        fs::remove_dir_all(dir).unwrap();
    }
}
