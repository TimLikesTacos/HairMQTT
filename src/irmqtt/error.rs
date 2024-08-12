use std::fmt;

pub(crate) enum MqttError {
    MissingCredendials,
    MissingBrokerHost,
    InvalidPort,
}

impl fmt::Display for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MqttError::MissingCredendials => write!(f, "Missing MQTT credentials"),
            MqttError::MissingBrokerHost => write!(f, "Missing MQTT broker host"),
            MqttError::InvalidPort => write!(f, "Invalid MQTT port"),
        }
    }
}

impl fmt::Debug for MqttError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            MqttError::MissingCredendials => write!(f, "Missing MQTT credentials"),
            MqttError::MissingBrokerHost => write!(f, "Missing MQTT broker host"),
            MqttError::InvalidPort => write!(f, "Invalid MQTT port"),
        }
    }
}

impl std::error::Error for MqttError {}
