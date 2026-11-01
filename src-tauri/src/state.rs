use ibs_store::Store;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

pub struct AppState {
    /// App-data dir (DB, trial markers).
    pub data_dir: PathBuf,
    pub store: Store,
    /// Cancel token of the running batch, if any. One batch runs at a time.
    pub running_job: Mutex<Option<Arc<AtomicBool>>>,
    /// Output folder of the latest batch (validated by the engine).
    pub last_output_dir: Mutex<Option<PathBuf>>,
    /// Inputs and written outputs of the latest batch: the only files
    /// `load_image` will read.
    pub viewable: Mutex<HashSet<PathBuf>>,
}
