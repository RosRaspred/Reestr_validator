// Copyright 2017 The Exonum Team
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//   http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

// Import crates with necessary types into a new project.

extern crate serde;
extern crate serde_json;
#[macro_use]
extern crate serde_derive;
#[macro_use]
extern crate exonum;
extern crate router;
extern crate bodyparser;
extern crate iron;

// Import necessary types from crates.

use exonum::blockchain::{Blockchain, Service, GenesisConfig, ValidatorKeys, Transaction,
                         ApiContext};
use exonum::node::{Node, NodeConfig, NodeApiConfig, TransactionSend, ApiSender};
use exonum::messages::{RawTransaction, FromRaw, Message};
use exonum::storage::{Fork, MemoryDB, MapIndex};
use exonum::crypto::{PublicKey, Hash, HexValue};
use exonum::encoding;
use exonum::api::{Api, ApiError};
use iron::prelude::*;
use iron::Handler;
use router::Router;


const SERVICE_ID: u16 = 1;
const TX_CREATE_PROPERTY_ID: u16 = 1;
const TX_TRANSFER_ID: u16 = 2;

use property::Property;
pub mod property;


pub struct PropertySchema<'a> {
    view: &'a mut Fork,
}

impl<'a> PropertySchema<'a> {
    pub fn properties(&mut self) -> MapIndex<&mut Fork, PublicKey, Property> {
        MapIndex::new("propertychain.property", self.view)
    }
    pub fn property(&mut self, pub_key: &PublicKey) -> Option<Property> {
        self.properties().get(pub_key)
    }
}

message! {
    struct TxCreateProperty {
        const TYPE = SERVICE_ID;
        const ID = TX_CREATE_PROPERTY_ID;
        const SIZE = 88;

        field property_id:            &PublicKey  [00 => 32]
        field registrator_id:         &PublicKey  [32 => 64]
        field object_value:           u64         [64 => 72]
        field owner_name:             &str         [72 => 80]
        field status:                u64         [80 => 88]
    }
}

message! {
    struct TxChangeStatus {
        const TYPE = SERVICE_ID;
        const ID = TX_TRANSFER_ID;
        const SIZE = 88;

        field property_id:            &PublicKey  [00 => 32]
        field registrator_id:         &PublicKey  [32 => 64]
        field object_value:           u64         [64 => 72]
        field owner_name:             &str         [72 => 80]
        field status:                u64         [80 => 88]
    }
}

impl Transaction for TxCreateProperty {
    fn verify(&self) -> bool {
        self.verify_signature(self.property_id())
    }
    fn execute(&self, view: &mut Fork) {
        let mut schema = PropertySchema { view };
        if schema.property(self.property_id()).is_none() {
            let prop = Property::new(self.property_id(), self.registrator_id(), self.object_value(), self.owner_name(), 1);
            schema.properties().put(self.property_id(), prop)
        }
    }

    fn info(&self) -> serde_json::Value {
        serde_json::to_value(&self).expect("Cannot serialize transaction to JSON")
    }
}

impl Transaction for TxChangeStatus {
    fn verify(&self) -> bool {
        true
    }

    fn execute(&self, view: &mut Fork) {
        let mut schema = PropertySchema { view };

        let prop = Property::new(self.property_id(), self.registrator_id(), self.object_value(), self.owner_name(), 2);
            schema.properties().put(self.property_id(),  prop)
    }

    fn info(&self) -> serde_json::Value {
        serde_json::to_value(&self).expect("Cannot serialize transaction to JSON")
    }
}

#[derive(Clone)]
struct PropertyApi {
    channel: ApiSender,
    blockchain: Blockchain,
}

impl PropertyApi {
    fn get_property(&self, pub_key: &PublicKey) -> Option<Property> {
        let mut view = self.blockchain.fork();
        let mut schema = PropertySchema { view: &mut view };
        schema.property(pub_key)
    }

    fn get_properties(&self) -> Option<Vec<Property>> {
        let mut view = self.blockchain.fork();
        let mut schema = PropertySchema { view: &mut view };
        let idx = schema.properties();
        let properties: Vec<Property> = idx.values().collect();
        if properties.is_empty() {
            None
        } else {
            Some(properties)
        }
    }
}

#[serde(untagged)]
#[derive(Clone, Serialize, Deserialize)]
enum TransactionRequest {
    CreateProperty(TxCreateProperty),
    Transfer(TxChangeStatus),
}

impl Into<Box<Transaction>> for TransactionRequest {
    fn into(self) -> Box<Transaction> {
        match self {
            TransactionRequest::CreateProperty(trans) => Box::new(trans),
            TransactionRequest::Transfer(trans) => Box::new(trans),
        }
    }
}

#[derive(Serialize, Deserialize)]
struct TransactionResponse {
    tx_hash: Hash,
}

impl Api for PropertyApi {
    fn wire(&self, router: &mut Router) {
        let self_ = self.clone();
        let transaction = move |req: &mut Request| -> IronResult<Response> {
            match req.get::<bodyparser::Struct<TransactionRequest>>() {
                Ok(Some(transaction)) => {
                    let transaction: Box<Transaction> = transaction.into();
                    let tx_hash = transaction.hash();
                    self_.channel.send(transaction).map_err(ApiError::from)?;
                    let json = TransactionResponse { tx_hash };
                    self_.ok_response(&serde_json::to_value(&json).unwrap())
                }
                Ok(None) => Err(ApiError::IncorrectRequest("Empty request body".into()))?,
                Err(e) => Err(ApiError::IncorrectRequest(Box::new(e)))?,
            }
        };

        let self_ = self.clone();
        let properties_info = move |_: &mut Request| -> IronResult<Response> {
            if let Some(properties) = self_.get_properties() {
                self_.ok_response(&serde_json::to_value(properties).unwrap())
            } else {
                self_.not_found_response(
                    &serde_json::to_value("")
                        .unwrap(),
                )
            }
        };

        let self_ = self.clone();
        let property_info = move |req: &mut Request| -> IronResult<Response> {
            let path = req.url.path();
            let property_key = path.last().unwrap();
            let public_key = PublicKey::from_hex(property_key).map_err(ApiError::FromHex)?;
            if let Some(property) = self_.get_property(&public_key) {
                self_.ok_response(&serde_json::to_value(property).unwrap())
            } else {
                self_.not_found_response(&serde_json::to_value("").unwrap())
            }
        };

        router.post("/v1/properties/transaction", transaction, "transaction");
        router.get("/v1/properties", properties_info, "properties_info");
        router.get("/v1/property/:pub_key", property_info, "property_info");
    }
}

struct PropertyService;

impl Service for PropertyService {
    fn service_name(&self) -> &'static str {
        "property"
    }

    fn service_id(&self) -> u16 {
        SERVICE_ID
    }

    fn tx_from_raw(&self, raw: RawTransaction) -> Result<Box<Transaction>, encoding::Error> {
        let trans: Box<Transaction> = match raw.message_type() {
            TX_TRANSFER_ID => Box::new(TxChangeStatus::from_raw(raw)?),
            TX_CREATE_PROPERTY_ID => Box::new(TxCreateProperty::from_raw(raw)?),
            _ => {
                return Err(encoding::Error::IncorrectMessageType {
                    message_type: raw.message_type(),
                });
            }
        };
        Ok(trans)
    }

    fn public_api_handler(&self, ctx: &ApiContext) -> Option<Box<Handler>> {
        let mut router = Router::new();
        let api = PropertyApi {
            channel: ctx.node_channel().clone(),
            blockchain: ctx.blockchain().clone(),
        };
        api.wire(&mut router);
        Some(Box::new(router))
    }
}

fn main() {
    exonum::helpers::init_logger().unwrap();

    let db = MemoryDB::new();
    let services: Vec<Box<Service>> = vec![Box::new(PropertyService)];
    let blockchain = Blockchain::new(Box::new(db), services);

    let (consensus_public_key, consensus_secret_key) = exonum::crypto::gen_keypair();
    let (service_public_key, service_secret_key) = exonum::crypto::gen_keypair();

    let validator_keys = ValidatorKeys {
        consensus_key: consensus_public_key,
        service_key: service_public_key,
    };
    let genesis = GenesisConfig::new(vec![validator_keys].into_iter());

    let api_address = "0.0.0.0:8000".parse().unwrap();
    let api_cfg = NodeApiConfig {
        public_api_address: Some(api_address),
        ..Default::default()
    };

    let peer_address = "0.0.0.0:2000".parse().unwrap();

    let node_cfg = NodeConfig {
        listen_address: peer_address,
        peers: vec![],
        service_public_key,
        service_secret_key,
        consensus_public_key,
        consensus_secret_key,
        genesis,
        external_address: None,
        network: Default::default(),
        whitelist: Default::default(),
        api: api_cfg,
        mempool: Default::default(),
        services_configs: Default::default(),
    };

    let node = Node::new(blockchain, node_cfg);

    node.run().unwrap();
}