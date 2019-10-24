/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::error::*;
use std::cell::RefCell;

#[derive(Clone, Debug)]
pub struct RemoteTab {
    pub title: String,
    pub url_history: Vec<String>,
    pub icon: Option<String>,
    pub last_used: u64, // In ms.
}

#[derive(Clone, Debug)]
pub struct ClientRemoteTabs {
    pub client_id: String, // Corresponds to the FxA device id of the client.
    pub remote_tabs: Vec<RemoteTab>,
}

pub struct TabsStorage {
    local_tabs: Option<ClientRemoteTabs>,
    remote_tabs: RefCell<Option<Vec<ClientRemoteTabs>>>,
}

impl TabsStorage {
    pub fn new() -> Self {
        Self {
            local_tabs: None,
            remote_tabs: RefCell::default(),
        }
    }

    pub fn update_local_state(&mut self, local_state: ClientRemoteTabs) {
        self.local_tabs.replace(local_state);
    }

    pub fn get_local_tabs(&self) -> Option<&ClientRemoteTabs> {
        self.local_tabs.as_ref()
    }

    pub fn get_remote_tabs(&self) -> Option<Vec<ClientRemoteTabs>> {
        self.remote_tabs.borrow().clone()
    }

    pub fn replace_remote_tabs(&self, new_remote_tabs: Vec<ClientRemoteTabs>) {
        let mut remote_tabs = self.remote_tabs.borrow_mut();
        remote_tabs.replace(new_remote_tabs);
    }

    pub fn wipe(&self) {
        unimplemented!("Implement me");
    }
}
