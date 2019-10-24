/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use std::cell::Cell;
use crate::record::{ClientTabsRecord, TabsRecordTab};
use std::result;
use sync15::{
    extract_v1_state, telemetry, CollSyncIds, CollectionRequest, IncomingChangeset,
    OutgoingChangeset, Payload, ServerTimestamp, Store, StoreSyncAssociation,
};
use sync_guid::Guid;

pub struct RemoteTab {
    title: String,
    url_history: Vec<String>,
    icon: Option<String>,
    last_used: u64, // In ms.
}

impl RemoteTab {
    fn from_record_tab(tab: &TabsRecordTab) -> Self {
        Self {
            title: tab.title.clone(),
            url_history: tab.url_history.clone(),
            icon: tab.icon.clone(),
            last_used: tab.last_used.checked_mul(1000).unwrap_or_default(),
        }
    }
    fn to_record_tab(&self) -> TabsRecordTab {
        TabsRecordTab {
            title: self.title.clone(),
            url_history: self.url_history.clone(),
            icon: self.icon.clone(),
            last_used: self.last_used.checked_div(1000).unwrap_or_default(),
        }
    }
}

pub struct ClientRemoteTabs {
    client_id: String, // Corresponds to the FxA device id of the client.
    remote_tabs: Vec<RemoteTab>,
}

impl ClientRemoteTabs {
    fn from_record(client_id: String, record: ClientTabsRecord) -> Self {
        Self {
            client_id,
            remote_tabs: record.tabs.iter().map(RemoteTab::from_record_tab).collect(),
        }
    }
    fn to_record(&self) -> ClientTabsRecord {
        ClientTabsRecord {
            id: self.client_id,
            tabs: self.remote_tabs.iter().map(RemoteTab::to_record_tab).collect(),
        }
    }
}

pub struct TabsStore {
    local_id: String, // todo Redundant with ClientRemoteTabs.client_id!
    local_tabs: Option<ClientRemoteTabs>,
    remote_tabs: Vec<ClientRemoteTabs>,
    last_sync: Cell<Option<ServerTimestamp>>, // We use a cell because `sync_finished` doesn't take a mutable reference to &self.
}

impl TabsStore {
    pub fn new(local_id: &str) -> Self {
        Self {
            local_id: local_id.to_owned(),
            remote_tabs: vec![],
            local_tabs: None,
            last_sync: Cell::new(None),
        }
    }
}

impl Store for TabsStore {
    fn collection_name(&self) -> &'static str {
        "tabs"
    }

    fn apply_incoming(
        &self,
        inbound: IncomingChangeset,
        telem: &mut telemetry::Engine,
    ) -> result::Result<OutgoingChangeset, failure::Error> {
        let mut incoming_telemetry = telemetry::EngineIncoming::new();

        self.remote_tabs.clear();
        self.remote_tabs.reserve_exact(inbound.changes.len() - 1); // -1 because one of the records is ours.

        for incoming in inbound.changes {
            if incoming.0.id() == self.local_id {
                // That's our own record, ignore it.
                continue;
            }
            let record = match ClientTabsRecord::from_payload(incoming.0) {
                Ok(record) => record,
                Err(e) => {
                    log::warn!("Error deserializing incoming record: {}", e);
                    incoming_telemetry.failed(1);
                    continue;
                }
            };
            // TODO: this is totaly wrong, we need to get fxa_client_id from the clients collection instead.
            let id = incoming.0.id().to_owned();
            self.remote_tabs.push(ClientRemoteTabs::from_record(id, record));
        }
        let mut outgoing = OutgoingChangeset::new("tabs".into(), inbound.timestamp);
        if let Some(local_tabs) = self.local_tabs {
            let payload = Payload::from_record(local_tabs.to_record())?;
            log::trace!("outgoing {:?}", payload);
            outgoing.changes.push(payload);
        }
        telem.incoming(incoming_telemetry);
        Ok(outgoing)
    }

    fn sync_finished(
        &self,
        new_timestamp: ServerTimestamp,
        records_synced: Vec<Guid>,
    ) -> result::Result<(), failure::Error> {
        log::info!(
            "sync completed after uploading {} records",
            records_synced.len()
        );
        self.last_sync.set(Some(new_timestamp));
        Ok(())
    }

    fn get_collection_request(&self) -> result::Result<CollectionRequest, failure::Error> {
        let since = self.last_sync.get().unwrap_or_default();
        Ok(CollectionRequest::new("tabs").full().newer_than(since))
    }

    fn get_sync_assoc(&self) -> result::Result<StoreSyncAssociation, failure::Error> {
        // let global = self.db.get_meta(schema::GLOBAL_SYNCID_META_KEY)?;
        // let coll = self.db.get_meta(schema::COLLECTION_SYNCID_META_KEY)?;
        // Ok(if let (Some(global), Some(coll)) = (global, coll) {
        //     StoreSyncAssociation::Connected(CollSyncIds { global, coll })
        // } else {
        //     StoreSyncAssociation::Disconnected
        // })
        unimplemented!("TODO!");
    }

    fn reset(&self, assoc: &StoreSyncAssociation) -> result::Result<(), failure::Error> {
        // self.db.reset(assoc)?;
        // Ok(())
        unimplemented!("TODO!");
    }

    fn wipe(&self) -> result::Result<(), failure::Error> {
        // self.db.wipe(&self.scope)?;
        // Ok(())
        unimplemented!("TODO!");
    }
}
