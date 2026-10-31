use ibs_store::Store;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct AppState {
    pub store: Store,
    /// Cancel token of the running batch, if any. One batch runs at a time.
    pub running_job: Mutex<Option<Arc<AtomicBool>>>,
}
