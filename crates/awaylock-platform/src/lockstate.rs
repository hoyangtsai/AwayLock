//! 查詢電腦目前是否處於鎖定狀態（輪詢）。

use awaylock_core::ports::LockStateProbe;

pub struct SystemLockProbe;

#[cfg(target_os = "macos")]
impl LockStateProbe for SystemLockProbe {
    fn is_locked(&self) -> bool {
        use core_foundation::base::{CFType, TCFType};
        use core_foundation::boolean::CFBoolean;
        use core_foundation::dictionary::{CFDictionary, CFDictionaryRef};
        use core_foundation::string::CFString;

        #[link(name = "CoreGraphics", kind = "framework")]
        extern "C" {
            fn CGSessionCopyCurrentDictionary() -> CFDictionaryRef;
        }

        // SAFETY: 回傳的是 Create 規則的 CFDictionary（呼叫端擁有），或 NULL。
        unsafe {
            let raw = CGSessionCopyCurrentDictionary();
            if raw.is_null() {
                return false;
            }
            let dict: CFDictionary<CFString, CFType> = CFDictionary::wrap_under_create_rule(raw);
            let key = CFString::from_static_string("CGSSessionScreenIsLocked");
            dict.find(&key)
                .and_then(|v| v.downcast::<CFBoolean>())
                .map(bool::from)
                .unwrap_or(false)
        }
    }
}

#[cfg(target_os = "linux")]
impl LockStateProbe for SystemLockProbe {
    fn is_locked(&self) -> bool {
        let session = std::env::var("XDG_SESSION_ID").unwrap_or_else(|_| "auto".into());
        std::process::Command::new("loginctl")
            .args(["show-session", &session, "-p", "LockedHint", "--value"])
            .output()
            .map(|o| String::from_utf8_lossy(&o.stdout).trim() == "yes")
            .unwrap_or(false)
    }
}

#[cfg(not(any(target_os = "macos", target_os = "linux")))]
impl LockStateProbe for SystemLockProbe {
    fn is_locked(&self) -> bool {
        false
    }
}
