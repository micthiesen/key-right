//! Only local evidence drives recovery. An absent AP/controller/internet is idle.
#[derive(Default)]
pub struct LocalNetHealth {
    missing_since: Option<u64>,
}
impl LocalNetHealth {
    pub fn restart_due(&mut self, now_ms: u64, associated: bool, local_ready: bool) -> bool {
        if !associated || local_ready {
            self.missing_since = None;
            return false;
        }
        let since = self.missing_since.get_or_insert(now_ms);
        now_ms.saturating_sub(*since) >= 60_000
    }
}
