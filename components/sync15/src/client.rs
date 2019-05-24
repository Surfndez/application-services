/* This Source Code Form is subject to the terms of the Mozilla Public
 * License, v. 2.0. If a copy of the MPL was not distributed with this
 * file, You can obtain one at http://mozilla.org/MPL/2.0/. */

use crate::bso_record::{BsoRecord, EncryptedBso};
use crate::error::{self, ErrorKind, ErrorResponse};
use crate::record_types::MetaGlobalRecord;
use crate::request::{
    BatchPoster, CollectionRequest, InfoCollections, InfoConfiguration, PostQueue, PostResponse,
    PostResponseHandler,
};
use crate::token;
use crate::util::ServerTimestamp;
use serde_json::Value;
use std::str::FromStr;
use url::Url;
use viaduct::{
    header_names::{self, AUTHORIZATION},
    Method, Request, Response,
};

/// A response from a GET request on a Sync15StorageClient, encapsulating all
/// the variants users of this client needs to care about.
#[derive(Debug, Clone)]
pub enum Sync15ClientResponse<T> {
    Success {
        status: u16,
        record: T,
        last_modified: ServerTimestamp,
        route: String,
    },
    Error(ErrorResponse),
}

impl<T> Sync15ClientResponse<T> {
    pub fn from_response(resp: Response) -> error::Result<Self>
    where
        for<'a> T: serde::de::Deserialize<'a>,
    {
        let route: String = resp.url.path().into();
        Ok(if resp.is_success() {
            let record: T = resp.json()?;
            let last_modified = resp
                .headers
                .get(header_names::X_LAST_MODIFIED)
                .and_then(|s| ServerTimestamp::from_str(s).ok())
                .ok_or_else(|| ErrorKind::MissingServerTimestamp)?;
            Sync15ClientResponse::Success {
                status: resp.status,
                record,
                last_modified,
                route,
            }
        } else {
            let status = resp.status;
            match status {
                404 => Sync15ClientResponse::Error(ErrorResponse::NotFound { route }),
                401 => Sync15ClientResponse::Error(ErrorResponse::Unauthorized { route }),
                412 => Sync15ClientResponse::Error(ErrorResponse::PreconditionFailed { route }),
                // / TODO: 5XX errors should parse backoff etc.
                500..=600 => {
                    Sync15ClientResponse::Error(ErrorResponse::ServerError { route, status })
                }
                _ => Sync15ClientResponse::Error(ErrorResponse::RequestFailed { route, status }),
            }
        })
    }

    pub fn create_storage_error(self) -> ErrorKind {
        let inner = match self {
            Sync15ClientResponse::Success { status, route, .. } => {
                // This should never happen as callers are expected to have
                // already special-cased this response, so warn if it does.
                // (or maybe we could panic?)
                log::warn!("Converting success response into an error");
                ErrorResponse::RequestFailed { status, route }
            }
            Sync15ClientResponse::Error(e) => e,
        };
        ErrorKind::StorageHttpError(inner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Sync15StorageClientInit {
    pub key_id: String,
    pub access_token: String,
    pub tokenserver_url: Url,
}

/// A trait containing the methods required to run through the setup state
/// machine. This is factored out into a separate trait to make mocking
/// easier.
pub trait SetupStorageClient {
    fn fetch_info_configuration(&self) -> error::Result<Sync15ClientResponse<InfoConfiguration>>;
    fn fetch_info_collections(&self) -> error::Result<Sync15ClientResponse<InfoCollections>>;
    fn fetch_meta_global(&self) -> error::Result<Sync15ClientResponse<MetaGlobalRecord>>;
    fn fetch_crypto_keys(&self) -> error::Result<Sync15ClientResponse<EncryptedBso>>;

    fn put_meta_global(
        &self,
        xius: ServerTimestamp,
        global: &MetaGlobalRecord,
    ) -> error::Result<()>;
    fn put_crypto_keys(&self, xius: ServerTimestamp, keys: &EncryptedBso) -> error::Result<()>;
    fn wipe_all_remote(&self) -> error::Result<()>;
}

#[derive(Debug)]
pub struct Sync15StorageClient {
    tsc: token::TokenProvider,
}

impl SetupStorageClient for Sync15StorageClient {
    fn fetch_info_configuration(&self) -> error::Result<Sync15ClientResponse<InfoConfiguration>> {
        self.relative_storage_request(Method::Get, "info/configuration")
    }

    fn fetch_info_collections(&self) -> error::Result<Sync15ClientResponse<InfoCollections>> {
        self.relative_storage_request(Method::Get, "info/collections")
    }

    fn fetch_meta_global(&self) -> error::Result<Sync15ClientResponse<MetaGlobalRecord>> {
        // meta/global is a Bso, so there's an extra dance to do.
        let got: Sync15ClientResponse<BsoRecord<MetaGlobalRecord>> =
            self.relative_storage_request(Method::Get, "storage/meta/global")?;
        Ok(match got {
            Sync15ClientResponse::Success {
                record,
                last_modified,
                route,
                status,
            } => Sync15ClientResponse::Success {
                record: record.payload,
                last_modified,
                route,
                status,
            },
            Sync15ClientResponse::Error(e) => Sync15ClientResponse::Error(e),
        })
    }

    fn fetch_crypto_keys(&self) -> error::Result<Sync15ClientResponse<EncryptedBso>> {
        self.relative_storage_request(Method::Get, "storage/crypto/keys")
    }

    fn put_meta_global(
        &self,
        xius: ServerTimestamp,
        global: &MetaGlobalRecord,
    ) -> error::Result<()> {
        let bso = BsoRecord::new_record("global".into(), "meta".into(), global);
        self.put("storage/meta/global", xius, &bso)
    }

    fn put_crypto_keys(&self, xius: ServerTimestamp, keys: &EncryptedBso) -> error::Result<()> {
        self.put("storage/crypto/keys", xius, keys)
    }

    fn wipe_all_remote(&self) -> error::Result<()> {
        let s = self.tsc.api_endpoint()?;
        let url = Url::parse(&s)?;

        let req = self.build_request(Method::Delete, url)?;
        match self.exec_request::<Value>(req, false) {
            Ok(Sync15ClientResponse::Error(ErrorResponse::NotFound { .. }))
            | Ok(Sync15ClientResponse::Success { .. }) => Ok(()),
            Ok(resp) => Err(resp.create_storage_error().into()),
            Err(e) => Err(e),
        }
    }
}

impl Sync15StorageClient {
    pub fn new(init_params: Sync15StorageClientInit) -> error::Result<Sync15StorageClient> {
        let tsc = token::TokenProvider::new(
            init_params.tokenserver_url,
            init_params.access_token,
            init_params.key_id,
        )?;
        Ok(Sync15StorageClient { tsc })
    }

    pub fn get_encrypted_records(
        &self,
        collection_request: &CollectionRequest,
    ) -> error::Result<Sync15ClientResponse<Vec<EncryptedBso>>> {
        self.collection_request(Method::Get, collection_request)
    }

    #[inline]
    fn authorized(&self, req: Request) -> error::Result<Request> {
        let hawk_header_value = self.tsc.authorization(&req)?;
        Ok(req.header(AUTHORIZATION, hawk_header_value)?)
    }

    // TODO: probably want a builder-like API to do collection requests (e.g. something
    // that occupies roughly the same conceptual role as the Collection class in desktop)
    fn build_request(&self, method: Method, url: Url) -> error::Result<Request> {
        self.authorized(Request::new(method, url).header(header_names::ACCEPT, "application/json")?)
    }

    fn relative_storage_request<P, T>(
        &self,
        method: Method,
        relative_path: P,
    ) -> error::Result<Sync15ClientResponse<T>>
    where
        P: AsRef<str>,
        for<'a> T: serde::de::Deserialize<'a>,
    {
        let s = self.tsc.api_endpoint()? + "/";
        let url = Url::parse(&s)?.join(relative_path.as_ref())?;
        self.exec_request(self.build_request(method, url)?, false)
    }

    fn exec_request<T>(
        &self,
        req: Request,
        require_success: bool,
    ) -> error::Result<Sync15ClientResponse<T>>
    where
        for<'a> T: serde::de::Deserialize<'a>,
    {
        log::trace!("request: {} {}", req.method, req.url.path());
        let resp = req.send()?;
        log::trace!("response: {}", resp.status);

        let result = Sync15ClientResponse::from_response(resp)?;
        match result {
            Sync15ClientResponse::Success { .. } => Ok(result),
            _ => {
                if require_success {
                    Err(result.create_storage_error().into())
                } else {
                    Ok(result)
                }
            }
        }
    }

    fn collection_request<T>(
        &self,
        method: Method,
        r: &CollectionRequest,
    ) -> error::Result<Sync15ClientResponse<T>>
    where
        for<'a> T: serde::de::Deserialize<'a>,
    {
        let url = r.build_url(Url::parse(&self.tsc.api_endpoint()?)?)?;
        self.exec_request(self.build_request(method, url)?, false)
    }

    pub fn new_post_queue<'a, F: PostResponseHandler>(
        &'a self,
        coll: &str,
        config: &InfoConfiguration,
        ts: ServerTimestamp,
        on_response: F,
    ) -> error::Result<PostQueue<PostWrapper<'a>, F>> {
        let pw = PostWrapper {
            client: self,
            coll: coll.into(),
        };
        Ok(PostQueue::new(config, ts, pw, on_response))
    }

    fn put<P, B>(&self, relative_path: P, xius: ServerTimestamp, body: &B) -> error::Result<()>
    where
        P: AsRef<str>,
        B: serde::ser::Serialize,
    {
        let s = self.tsc.api_endpoint()? + "/";
        let url = Url::parse(&s)?.join(relative_path.as_ref())?;

        let req = self
            .build_request(Method::Put, url)?
            .json(body)
            .header(header_names::X_IF_UNMODIFIED_SINCE, format!("{}", xius))?;

        let _ = self.exec_request::<Value>(req, true)?;

        Ok(())
    }

    pub fn hashed_uid(&self) -> error::Result<String> {
        self.tsc.hashed_uid()
    }
}

pub struct PostWrapper<'a> {
    client: &'a Sync15StorageClient,
    coll: String,
}

impl<'a> BatchPoster for PostWrapper<'a> {
    fn post<T, O>(
        &self,
        bytes: Vec<u8>,
        xius: ServerTimestamp,
        batch: Option<String>,
        commit: bool,
        _: &PostQueue<T, O>,
    ) -> error::Result<PostResponse> {
        let url = CollectionRequest::new(self.coll.clone())
            .batch(batch)
            .commit(commit)
            .build_url(Url::parse(&self.client.tsc.api_endpoint()?)?)?;

        let req = self
            .client
            .build_request(Method::Post, url)?
            .header(header_names::CONTENT_TYPE, "application/json")?
            .header(header_names::X_IF_UNMODIFIED_SINCE, format!("{}", xius))?
            .body(bytes);
        self.client.exec_request(req, false)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    #[test]
    fn test_send() {
        fn ensure_send<T: Send>() {}
        // Compile will fail if not send.
        ensure_send::<Sync15StorageClient>();
    }
}
