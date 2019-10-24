/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::error::*;
use crate::storage::{ClientRemoteTabs, TabsStorage};
use crate::sync::store::TabsStore;
use interrupt::NeverInterrupts;
use std::cell::Cell;
use sync15::{sync_multiple, telemetry, KeyBundle, MemoryCachedState, Sync15StorageClientInit};

pub struct TabsEngine {
    storage: TabsStorage,
    mem_cached_state: Cell<MemoryCachedState>,
}

impl TabsEngine {
    pub fn new() -> Self {
        Self {
            storage: TabsStorage::new(),
            mem_cached_state: Cell::default(),
        }
    }

    pub fn remote_tabs(&self) -> Option<Vec<ClientRemoteTabs>> {
        self.storage.get_remote_tabs()
    }

    /// A convenience wrapper around sync_multiple.
    pub fn sync(
        &self,
        local_id: &str,
        storage_init: &Sync15StorageClientInit,
        root_sync_key: &KeyBundle,
    ) -> Result<telemetry::SyncTelemetryPing> {
        let mut mem_cached_state = self.mem_cached_state.take();
        let store = TabsStore::new(local_id, &self.storage);

        let mut result = sync_multiple(
            &[&store],
            &mut None,
            &mut mem_cached_state,
            storage_init,
            root_sync_key,
            &NeverInterrupts,
            None,
        );

        // for b/w compat reasons, we do some dances with the result.
        // XXX - note that this means telemetry isn't going to be reported back
        // to the app - we need to check with lockwise about whether they really
        // need these failures to be reported or whether we can loosen this.
        if let Err(e) = result.result {
            return Err(e.into());
        }
        match result.engine_results.remove("tabs") {
            None | Some(Ok(())) => Ok(result.telemetry),
            Some(Err(e)) => Err(e.into()),
        }
    }
}
