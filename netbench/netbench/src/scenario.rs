// Copyright Amazon.com, Inc. or its affiliates. All Rights Reserved.
// SPDX-License-Identifier: Apache-2.0

use crate::{
    docker::{Compose, Service},
    operation as op, Result,
};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};

pub mod builder;
mod id;

pub use builder::Builder;
pub use id::Id;

#[derive(Clone, Debug, Default, Deserialize, Serialize, Hash)]
pub struct Scenario {
    pub id: Id,
    pub clients: Vec<Client>,
    pub servers: Vec<Server>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub routers: Vec<Router>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub traces: Vec<String>,
}

impl Scenario {
    pub fn build<F: FnOnce(&mut builder::Builder)>(f: F) -> Self {
        let mut builder = builder::Builder::new();
        f(&mut builder);
        builder.finish()
    }

    pub fn open(path: &Path) -> Result<Self> {
        let file = std::fs::File::open(path)?;
        let mut file = std::io::BufReader::new(file);
        let scenario = serde_json::from_reader(&mut file)?;
        Ok(scenario)
    }

    pub fn write<W: std::io::Write>(&self, out: &mut W) -> std::io::Result<()> {
        serde_json::to_writer_pretty(out, self)?;
        Ok(())
    }

    pub fn compose(&self) -> Compose {
        let mut compose = Compose::default();
        compose
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Hash)]
pub struct Client {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub name: String,
    pub scenario: Vec<op::Client>,
    pub connections: Vec<Connection>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub configuration: BTreeMap<String, String>,
}

impl Client {
    pub fn compose(&self, id: &Id, idx: usize) -> Service {
        Service {
            hostname: Some(if self.name.is_empty() {
                format!("{}.client.{}", idx, id)
            } else {
                format!("{}.client.{}", idx, self.name)
            }),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, Hash)]
pub struct Server {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub name: String,
    pub connections: Vec<Connection>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub configuration: BTreeMap<String, String>,
}

impl Server {
    pub fn compose(&self, id: &Id, idx: usize) -> Service {
        Service {
            hostname: Some(if self.name.is_empty() {
                format!("{}.server.{}", idx, id)
            } else {
                format!("{}.server.{}", idx, self.name)
            }),
            ..Default::default()
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash)]
pub struct Connection {
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub ops: Vec<op::Connection>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub peer_streams: Vec<Vec<op::Connection>>,
}

#[derive(Clone, Debug, Deserialize, Serialize, Hash)]
pub struct Router {
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub name: String,
    pub scenario: Vec<op::Router>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty", default)]
    pub configuration: BTreeMap<String, String>,
}

impl Router {
    pub fn compose(&self, id: &Id, idx: usize) -> Service {
        Service {
            hostname: Some(if self.name.is_empty() {
                format!("{}.router.{}", idx, id)
            } else {
                format!("{}.router.{}", idx, self.name)
            }),
            ..Default::default()
        }
    }
}
