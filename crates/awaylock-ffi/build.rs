// macOS：讓動態函式庫的安裝路徑為 @rpath，之後可放進 .app/Contents/Frameworks。
// 在連結時設定（不用 install_name_tool 事後修改，避免破壞已 strip 的檔案）。
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("macos") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,-install_name,@rpath/libawaylock_ffi.dylib");
    }
}
