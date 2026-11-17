//! 鎖定判定狀態機（純邏輯，可單元測試）。
//!
//! 規則：
//! 1. 電腦每次解鎖後開始一段新的「解鎖期間」。
//! 2. 期間內，允許列表中的裝置只要被掃描到、且在設定距離內，就「列入監控」。
//! 3. 列入監控的裝置中，只要有任何一個連續 `required_misses` 次檢查都超過距離
//!    或掃描不到，就鎖定。從未被掃描到的裝置不會觸發鎖定。
//! 4. 鎖定後清空監控名單，等電腦真的鎖定、再解鎖後才重新開始。

use crate::allowlist::AllowedDevice;
use crate::device::NearbyDevice;
use std::collections::{HashMap, HashSet};
use std::time::{Duration, Instant};

#[derive(Debug, Clone)]
pub struct MonitorConfig {
    /// 超過這段時間沒收到廣播，視為掃描不到。
    pub stale_after: Duration,
    /// 連續幾次檢查都離開才鎖定（避免訊號瞬間抖動誤鎖）。
    pub required_misses: u32,
    /// 發出鎖定指令後，等多久電腦還沒真的鎖定就放棄等待。
    pub await_lock_timeout: Duration,
}

impl Default for MonitorConfig {
    fn default() -> Self {
        Self {
            stale_after: Duration::from_secs(15),
            required_misses: 3,
            await_lock_timeout: Duration::from_secs(20),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Phase {
    /// 電腦已解鎖，監控中。
    Unlocked,
    /// 已發出鎖定指令，等電腦真的進入鎖定。
    AwaitingLock { since: Instant },
    /// 電腦已鎖定，等待解鎖。
    Locked,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Presence {
    pub present: bool,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    None,
    /// 請鎖定螢幕；`device_name` 是觸發鎖定的裝置。
    Lock { device_name: String },
}

/// 每次檢查時的輸入。
pub struct Snapshot<'a> {
    pub now: Instant,
    pub screen_locked: bool,
    pub bluetooth_ready: bool,
    pub allowed: &'a [AllowedDevice],
    pub devices: &'a HashMap<String, NearbyDevice>,
    /// 離開門檻（dBm）。
    pub threshold: i32,
}

pub struct Monitor {
    cfg: MonitorConfig,
    phase: Phase,
    armed: HashSet<String>,
    misses: HashMap<String, u32>,
    decision: String,
}

impl Monitor {
    pub fn new(cfg: MonitorConfig) -> Self {
        Self {
            cfg,
            phase: Phase::Unlocked,
            armed: HashSet::new(),
            misses: HashMap::new(),
            decision: "尚未檢查".into(),
        }
    }

    pub fn config(&self) -> &MonitorConfig {
        &self.cfg
    }
    pub fn phase(&self) -> Phase {
        self.phase
    }
    /// 本次解鎖期間見過、正在監控的裝置 id。
    pub fn armed(&self) -> &HashSet<String> {
        &self.armed
    }
    pub fn is_armed(&self, id: &str) -> bool {
        self.armed.contains(id)
    }
    /// 監控中裝置的最大連續離開次數。
    pub fn max_misses(&self) -> u32 {
        self.armed.iter().map(|id| self.misses.get(id).copied().unwrap_or(0)).max().unwrap_or(0)
    }
    pub fn decision(&self) -> &str {
        &self.decision
    }

    /// 判斷單一允許裝置目前是否在範圍內，並說明原因（供 Debug 顯示）。
    pub fn evaluate(
        &self,
        a: &AllowedDevice,
        devices: &HashMap<String, NearbyDevice>,
        threshold: i32,
        now: Instant,
    ) -> Presence {
        let Some(d) = devices.get(&a.id) else {
            return Presence { present: false, reason: "未收到廣播".into() };
        };
        let age = now.saturating_duration_since(d.last_seen);
        if age >= self.cfg.stale_after {
            return Presence { present: false, reason: format!("已 {} 秒沒收到廣播", age.as_secs()) };
        }
        // 門檻依裝置的發射功率調整（iPhone 較強，門檻較高）；使用者校正過就用校正值
        let threshold = d.threshold_calibrated(threshold, a.rssi_at_1m);
        if d.rssi < threshold {
            return Presence { present: false, reason: format!("訊號 {} < 門檻 {}", d.rssi, threshold) };
        }
        Presence { present: true, reason: format!("訊號 {} ≥ 門檻 {}", d.rssi, threshold) }
    }

    /// 執行一次檢查，回傳要採取的動作。
    pub fn tick(&mut self, s: &Snapshot) -> Action {
        // 先處理電腦鎖定／解鎖的狀態切換
        match self.phase {
            Phase::Locked => {
                if s.screen_locked {
                    self.decision = "電腦已鎖定，等待解鎖".into();
                } else {
                    self.begin_session("電腦已解鎖，開始新的監控期間");
                }
                return Action::None;
            }
            Phase::AwaitingLock { since } => {
                if s.screen_locked {
                    self.enter_locked();
                } else if s.now.saturating_duration_since(since) >= self.cfg.await_lock_timeout {
                    self.begin_session("鎖定指令未生效，重新開始監控");
                }
                return Action::None;
            }
            Phase::Unlocked => {
                if s.screen_locked {
                    self.enter_locked();
                    return Action::None;
                }
            }
        }

        if !s.bluetooth_ready {
            self.decision = "藍牙不可用，不動作".into();
            return Action::None;
        }
        if s.allowed.is_empty() {
            self.armed.clear();
            self.misses.clear();
            self.decision = "允許列表是空的，不動作".into();
            return Action::None;
        }

        // 移除已從允許列表刪除的裝置
        let allowed_ids: HashSet<&str> = s.allowed.iter().map(|a| a.id.as_str()).collect();
        self.armed.retain(|id| allowed_ids.contains(id.as_str()));
        self.misses.retain(|id, _| allowed_ids.contains(id.as_str()));

        for a in s.allowed {
            if self.evaluate(a, s.devices, s.threshold, s.now).present {
                self.armed.insert(a.id.clone()); // 見過了：列入監控
                self.misses.insert(a.id.clone(), 0);
            } else if self.armed.contains(&a.id) {
                *self.misses.entry(a.id.clone()).or_insert(0) += 1; // 監控中的裝置離開了
            }
        }

        // 任何一個監控中的裝置連續離開，就鎖定
        let required = self.cfg.required_misses;
        let lost = s.allowed.iter().find(|a| {
            self.armed.contains(&a.id) && self.misses.get(&a.id).copied().unwrap_or(0) >= required
        });
        if let Some(lost) = lost {
            let name = lost.name.clone();
            self.phase = Phase::AwaitingLock { since: s.now };
            self.armed.clear();
            self.misses.clear();
            self.decision = format!("「{name}」離開，已鎖定螢幕");
            return Action::Lock { device_name: name };
        }

        let max = self.max_misses();
        self.decision = if self.armed.is_empty() {
            "本次解鎖期間尚未掃描到允許的裝置，不動作".into()
        } else if max > 0 {
            format!("有監控中的裝置離開（{}/{}）", max, required)
        } else {
            format!("監控中（已見過 {} 個允許裝置）", self.armed.len())
        };
        Action::None
    }

    fn enter_locked(&mut self) {
        self.phase = Phase::Locked;
        self.armed.clear();
        self.misses.clear();
        self.decision = "電腦已鎖定，等待解鎖".into();
    }

    fn begin_session(&mut self, why: &str) {
        self.phase = Phase::Unlocked;
        self.armed.clear();
        self.misses.clear();
        self.decision = why.into();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TH: i32 = -76;

    struct Fixture {
        t0: Instant,
        step: u32,
        monitor: Monitor,
        allowed: Vec<AllowedDevice>,
        devices: HashMap<String, NearbyDevice>,
    }

    impl Fixture {
        fn new(ids: &[&str]) -> Self {
            Self {
                t0: Instant::now(),
                step: 0,
                monitor: Monitor::new(MonitorConfig::default()),
                allowed: ids
                    .iter()
                    .map(|i| AllowedDevice { id: (*i).into(), name: format!("dev-{i}"), rssi_at_1m: None })
                    .collect(),
                devices: HashMap::new(),
            }
        }

        fn now(&self) -> Instant {
            self.t0 + Duration::from_secs(u64::from(self.step) * 5)
        }

        /// 讓某裝置此刻剛收到廣播。
        fn see(&mut self, id: &str, rssi: i32) {
            let d = NearbyDevice::new(id.into(), None, rssi, false, self.now());
            self.devices.insert(id.into(), d);
        }

        fn forget(&mut self, id: &str) {
            self.devices.remove(id);
        }

        fn tick_with(&mut self, screen_locked: bool, ready: bool) -> Action {
            let snap = Snapshot {
                now: self.now(),
                screen_locked,
                bluetooth_ready: ready,
                allowed: &self.allowed,
                devices: &self.devices,
                threshold: TH,
            };
            let a = self.monitor.tick(&snap);
            self.step += 1;
            a
        }

        fn tick(&mut self) -> Action {
            self.tick_with(false, true)
        }
    }

    fn is_lock(a: &Action) -> bool {
        matches!(a, Action::Lock { .. })
    }

    #[test]
    fn never_seen_device_never_locks() {
        let mut f = Fixture::new(&["a"]);
        for _ in 0..10 {
            assert_eq!(f.tick(), Action::None);
        }
        assert!(f.monitor.armed().is_empty());
    }

    #[test]
    fn locks_after_required_misses_once_armed() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        assert_eq!(f.tick(), Action::None);
        assert!(f.monitor.is_armed("a"));

        f.forget("a");
        assert_eq!(f.tick(), Action::None); // 1/3
        assert_eq!(f.tick(), Action::None); // 2/3
        let a = f.tick(); // 3/3
        assert_eq!(a, Action::Lock { device_name: "dev-a".into() });
    }

    #[test]
    fn single_miss_recovers() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        f.forget("a");
        f.tick();
        f.tick();
        f.see("a", -60);
        assert_eq!(f.tick(), Action::None);
        assert_eq!(f.monitor.max_misses(), 0);
    }

    #[test]
    fn any_one_of_two_triggers() {
        let mut f = Fixture::new(&["a", "b"]);
        f.see("a", -60);
        f.see("b", -60);
        f.tick();
        let mut locked = None;
        for _ in 0..3 {
            f.see("a", -60); // a 一直在
            f.forget("b"); // b 消失
            let act = f.tick();
            if is_lock(&act) {
                locked = Some(act);
            }
        }
        assert_eq!(locked, Some(Action::Lock { device_name: "dev-b".into() }));
    }

    #[test]
    fn iphone_threshold_is_adjusted_for_its_stronger_signal() {
        let f = Fixture::new(&["a"]);
        let mut devices = HashMap::new();
        // -70 dBm：一般裝置在 5 公尺門檻（-76）內，但 iPhone 訊號較強，-70 已約 6 公尺
        let phone = NearbyDevice::new("a".into(), Some("My iPhone".into()), -70, true, f.now());
        devices.insert("a".to_string(), phone);
        let p = f.monitor.evaluate(&f.allowed[0], &devices, TH, f.now());
        assert!(!p.present, "{}", p.reason);
        assert!(p.reason.contains("-67"), "{}", p.reason);

        let other = NearbyDevice::new("a".into(), None, -70, false, f.now());
        devices.insert("a".to_string(), other);
        assert!(f.monitor.evaluate(&f.allowed[0], &devices, TH, f.now()).present);
    }

    #[test]
    fn calibrated_reference_overrides_default_threshold() {
        let mut f = Fixture::new(&["a"]);
        // 使用者在 1 公尺處量到 -45：門檻 = -76 + (-45 - -59) = -62
        f.allowed[0].rssi_at_1m = Some(-45.0);
        let mut devices = HashMap::new();
        devices.insert("a".to_string(), NearbyDevice::new("a".into(), Some("My iPhone".into()), -64, true, f.now()));
        let p = f.monitor.evaluate(&f.allowed[0], &devices, TH, f.now());
        assert!(!p.present, "{}", p.reason);
        assert!(p.reason.contains("-62"), "{}", p.reason);
    }

    #[test]
    fn too_far_counts_as_away() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        let mut got = false;
        for _ in 0..3 {
            f.see("a", -90); // 訊號低於門檻
            got |= is_lock(&f.tick());
        }
        assert!(got);
    }

    #[test]
    fn stale_advertisement_counts_as_away() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        // 之後不再更新 last_seen，過 15 秒以上就算掃描不到
        let mut got = false;
        for _ in 0..8 {
            got |= is_lock(&f.tick());
        }
        assert!(got);
    }

    #[test]
    fn after_lock_waits_for_real_lock_then_unlock() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        f.forget("a");
        f.tick();
        f.tick();
        assert!(is_lock(&f.tick()));
        assert!(matches!(f.monitor.phase(), Phase::AwaitingLock { .. }));

        // 電腦還沒鎖定時，不會重複觸發
        f.see("a", -60);
        assert_eq!(f.tick(), Action::None);
        assert!(f.monitor.armed().is_empty());

        // 電腦真的鎖定
        f.tick_with(true, true);
        assert_eq!(f.monitor.phase(), Phase::Locked);

        // 解鎖：開始新的期間，名單是空的
        f.tick_with(false, true);
        assert_eq!(f.monitor.phase(), Phase::Unlocked);
        assert!(f.monitor.armed().is_empty());

        // 裝置再次出現才重新列入監控
        f.see("a", -60);
        f.tick();
        assert!(f.monitor.is_armed("a"));
    }

    #[test]
    fn lock_that_never_takes_effect_times_out() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        f.forget("a");
        f.tick();
        f.tick();
        assert!(is_lock(&f.tick()));
        // 一直沒有進入鎖定狀態；20 秒後放棄等待
        for _ in 0..5 {
            f.tick();
        }
        assert_eq!(f.monitor.phase(), Phase::Unlocked);
    }

    #[test]
    fn removed_from_allowlist_is_dropped() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        assert!(f.monitor.is_armed("a"));
        f.allowed.clear();
        f.tick();
        assert!(f.monitor.armed().is_empty());
    }

    #[test]
    fn bluetooth_unavailable_does_nothing() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        f.forget("a");
        for _ in 0..6 {
            assert_eq!(f.tick_with(false, false), Action::None);
        }
    }

    #[test]
    fn manual_lock_resets_session() {
        let mut f = Fixture::new(&["a"]);
        f.see("a", -60);
        f.tick();
        assert!(f.monitor.is_armed("a"));
        f.tick_with(true, true);
        assert_eq!(f.monitor.phase(), Phase::Locked);
        assert!(f.monitor.armed().is_empty());
    }
}
