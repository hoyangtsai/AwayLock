//! 鎖定螢幕。

use awaylock_core::ports::ScreenLocker;
use std::io;
use std::process::Command;

pub struct SystemLocker;

impl ScreenLocker for SystemLocker {
    #[cfg(target_os = "macos")]
    fn lock(&self) -> io::Result<()> {
        // 讓顯示器休眠；搭配「需要密碼：立即」即等同鎖定
        Command::new("/usr/bin/pmset").arg("displaysleepnow").status().map(|_| ())
    }

    #[cfg(target_os = "linux")]
    fn lock(&self) -> io::Result<()> {
        Command::new("loginctl").arg("lock-session").status().map(|_| ())
    }

    #[cfg(not(any(target_os = "macos", target_os = "linux")))]
    fn lock(&self) -> io::Result<()> {
        let _ = Command::new("true");
        Err(io::Error::new(io::ErrorKind::Unsupported, "此平台尚未支援"))
    }
}
