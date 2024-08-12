use rumqttc::{Client, Connection, MqttOptions, QoS, Transport};
use serde::Serialize;

use super::error::MqttError;

const APPNAME: &str = "IrMqtt";

pub(crate) type DiscoveryPrepPacket = (String, Result<Vec<u8>, Box<dyn std::error::Error>>);
pub(crate) struct MqttClient(Client);

pub(crate) struct MqttConnection {}

impl MqttConnection {
    pub fn connect() -> Result<(MqttClient, Connection), MqttError> {
        let creds = MqttCredentials::new();
        let broker = MqttBroker::new()?;

        // Setting up rumqttc in websockets is a little hokey. https://github.com/bytebeamio/rumqtt/issues/808
        let connection_string = format!("ws://{}:{}", broker.host, broker.port);
        let mut mqttoptions = MqttOptions::new(APPNAME, connection_string, broker.port);
        mqttoptions.set_transport(Transport::Ws);

        // Since we can send the entire data update, lets bump up the max packet size significantly
        mqttoptions.set_max_packet_size(10240, 10240 * 4);

        match (creds.username(), creds.password()) {
            (Some(username), Some(password)) => {
                mqttoptions.set_credentials(username, password);
            }
            (None, None) => (), // No credentials needed
            _ => return Err(MqttError::MissingCredendials),
        }

        let (client, connection) = Client::new(mqttoptions, 10);
        Ok((MqttClient(client), connection))
    }
}

impl MqttClient {
    pub fn publish_value(&mut self, topic: &str, payload: &impl Serialize) {
        if let Ok(payload) = serde_json::to_vec(payload) {
            if let Err(e) = self.0.publish(topic, QoS::AtMostOnce, false, payload) {
                log::error!("Failed to publish message for {}: {:?}", topic, e);
            }
        } else {
            log::error!("Failed to serialize payload for {}", topic);
        }
    }

    pub fn direct_publish(&mut self, topic: &str, payload: &[u8]) {
        if let Err(e) = self.0.publish(topic, QoS::AtMostOnce, false, payload) {
            log::error!("Failed to publish message for {}: {:?}", topic, e);
        }
    }

    #[allow(dead_code)]
    pub fn publish_values(&mut self, values: &[(&str, &impl Serialize)]) {
        for (topic, payload) in values {
            self.publish_value(topic, payload);
        }
    }

    pub fn publish_discovery(&mut self, item: DiscoveryPrepPacket) {
        //Todo revisit?  Sending these messages will happen each session.  I think for now it is okay to not retain them, especially with different cars having different vars.
        // Keeps from having orhpaned entities in HA
        let retain = false;
        let (topic, ser_result) = item;

        match ser_result {
            Ok(payload) => {
                if let Err(e) = self.0.publish(&topic, QoS::AtLeastOnce, retain, payload) {
                    log::error!("Failed to publish discovery message for {}: {:?}", topic, e);
                }
            }
            Err(_) => {
                log::error!("Failed to serialize payload for {}", topic);
            }
        }
    }
}

struct MqttBroker {
    host: String,
    port: u16,
}

impl MqttBroker {
    fn new() -> Result<Self, MqttError> {
        let host = std::env::var("MQTT_HOST").map_err(|_| MqttError::MissingBrokerHost)?;
        let port = std::env::var("MQTT_PORT")
            .unwrap_or("1884".to_string())
            .parse::<u16>()
            .map_err(|_| MqttError::InvalidPort)?;
        Ok(Self { host, port })
    }
}
struct MqttCredentials {
    username: Option<String>,
    password: Option<String>,
}

impl MqttCredentials {
    fn new() -> Self {
        let username = std::env::var("MQTT_USERNAME").ok();
        let password = std::env::var("MQTT_PASSWORD").ok();
        Self { username, password }
    }

    fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }
}
