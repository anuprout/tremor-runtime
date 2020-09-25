// Copyright 2018-2020, Wayfair GmbH
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

//! # UDP Offramp
//!
//! Sends each message as a udp datagram
//!
//! ## Configuration
//!
//! See [Config](struct.Config.html) for details.

use crate::sink::prelude::*;
use async_std::net::UdpSocket;

/// An offramp that write a given file
pub struct Udp {
    socket: Option<UdpSocket>,
    config: Config,
    postprocessors: Postprocessors,
}

#[derive(Deserialize, Debug)]
pub struct Config {
    /// Host to use as source
    pub host: String,
    pub port: u16,
    pub dst_host: String,
    pub dst_port: u16,
}
impl ConfigImpl for Config {}

impl offramp::Impl for Udp {
    fn from_config(config: &Option<OpConfig>) -> Result<Box<dyn Offramp>> {
        if let Some(config) = config {
            let config: Config = Config::new(config)?;
            Ok(SinkManager::new_box(Self {
                socket: None,
                config,
                postprocessors: vec![],
            }))
        } else {
            Err("Blackhole offramp requires a config".into())
        }
    }
}

#[async_trait::async_trait]
impl Sink for Udp {
    // TODO
    #[allow(clippy::used_underscore_binding)]
    async fn on_event(&mut self, _input: &str, codec: &dyn Codec, mut event: Event) -> ResultVec {
        let mut success = true;
        if let Some(socket) = &mut self.socket {
            for value in event.value_iter() {
                let raw = codec.encode(value)?;
                //TODO: Error handling
                socket.send(&raw).await?;
            }
        } else {
            success = false
        };
        if success {
            Ok(Some(vec![SinkReply::Insight(event.insight_ack())]))
        } else {
            Ok(event
                .insight_trigger()
                .and_then(|e1| event.insight_fail().map(|e2| (e1, e2)))
                .map(|(e1, e2)| vec![SinkReply::Insight(e1), SinkReply::Insight(e2)]))
        }
    }
    fn default_codec(&self) -> &str {
        "json"
    }
    async fn init(
        &mut self,
        postprocessors: &[String],
        _reply_channel: Sender<SinkReply>,
    ) -> Result<()> {
        self.postprocessors = make_postprocessors(postprocessors)?;
        let socket = UdpSocket::bind((self.config.host.as_str(), self.config.port)).await?;
        socket
            .connect((self.config.dst_host.as_str(), self.config.dst_port))
            .await?;
        self.socket = Some(socket);
        Ok(())
    }
    #[allow(clippy::used_underscore_binding)]
    async fn on_signal(&mut self, signal: Event) -> ResultVec {
        if self.socket.is_none() {
            let socket = UdpSocket::bind((self.config.host.as_str(), self.config.port)).await?;
            socket
                .connect((self.config.dst_host.as_str(), self.config.dst_port))
                .await?;
            self.socket = Some(socket);
            Ok(Some(vec![SinkReply::Insight(Event::cb_restore(
                signal.ingest_ns,
            ))]))
        } else {
            Ok(None)
        }
    }
    fn is_active(&self) -> bool {
        self.socket.is_some()
    }
    fn auto_ack(&self) -> bool {
        false
    }
}
