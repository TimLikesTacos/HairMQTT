use dotenvy::dotenv;
use entity_builders::BinarySensorBuilder;
use entity_builders::SensorBuilder;
use ha_mqtt::components::binary_sensor::BinarySensor;
use ha_mqtt::components::sensor::SensorClass;
use ha_mqtt::device::Device;
use ha_mqtt::discoverable::Discoverable;
use ir_telemetry::client::UpdatePacket;
use ir_telemetry::mapped_file::var_header::VarHeader;
use ir_telemetry::Client as IracingClient;
use ir_telemetry::IrData;
use ir_telemetry::Session;
use irmqtt::client::DiscoveryPrepPacket;
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::HashMap;
use std::time::Duration;

const VERSION: Option<&str> = option_env!("CARGO_PKG_VERSION");
const TELEMETRY_STATE: &str = "hairmqtt/telemetry";
const SESSION_STATE: &str = "hairmqtt/session";

pub(crate) mod irmqtt {
    pub(crate) mod client;
    pub(crate) mod error;
}
pub(crate) mod entity_builders;

fn main() {
    pretty_env_logger::init_timed();
    if let Err(_) = dotenv() {
        log::error!("Did not find .env file");
    }

    // Update 2 times a second.  This is reasonable for this application
    let telemetry = IracingClient::connect(2.);

    let (mut client, mut connection) = irmqtt::client::MqttConnection::connect().unwrap();

    std::thread::spawn(move || {
        // Common device.  This groups everything in HA under one device.
        let device = Device::new()
            .with_name("Iracing Telemetry")
            .with_manufacturer("Tim Reed")
            .with_sw_version(VERSION.unwrap_or("unavailable"))
            .with_identifiers(vec!["hairmqtt".to_string()]);

        let mut var_headers: HashMap<String, VarHeader> = HashMap::new();

        // Session discovery packet is only sent once per session.
        let mut session_discory_sent: bool = false;

        for packet in telemetry {
            match packet {
                UpdatePacket::Data(data) => {
                    client.direct_publish("hairmqtt/connected", "connected".as_bytes());
                    let payload = handle_data(&data, &var_headers);
                    client.publish_value(TELEMETRY_STATE, &payload);
                }

                UpdatePacket::SessionInfo(session) => {
                    let session: Session = serde_yaml::from_str(&session).unwrap();
                    if !session_discory_sent {
                        let entities = session_discovery_packet(&session, &device);
                        for entity in entities.into_iter() {
                            client.publish_discovery(entity);
                        }
                        session_discory_sent = true;
                        log::trace!("Session Discovery sent");
                    }

                    let payload = handle_session(&session);
                    client.publish_value(SESSION_STATE, &payload);

                    log::trace!("Session Info updated");
                }

                // Clears session specific data
                UpdatePacket::NotConnected => {
                    var_headers.clear();
                    session_discory_sent = false;

                    client.direct_publish("hairmqtt/connected", "disconnected".as_bytes());
                    log::trace!("Ir-telemetry is not connected");
                }

                // This update packet should only be recieved when the race session loads.
                UpdatePacket::VariableHeaders(var_header) => {
                    var_headers = var_header;

                    let entities = discovery_packet(&var_headers, &device);
                    for entity in entities.into_iter() {
                        client.publish_discovery(entity);
                    }
                    log::trace!("Updated Variable Headers");
                }
                _ => {
                    // UpdatePacket is a non-exhaustive enum.  This is a catch all for any new packet types.
                    log::info!("Ir_telemetry has been updated to send a new packet type and this type has not been processed");
                }
            }
        }
    });

    // Need to loop over connection to move the event loop along
    for msg in connection.iter() {
        if let Err(error) = msg {
            log::error!("Error: {:?}", error);
            std::thread::sleep(Duration::from_secs(10));
        }
    }
}

/// Creates a list of discoverable entities from the telemetry data.  Bit long and could be refactored.
fn discovery_packet(
    var_headers: &HashMap<String, VarHeader>,
    device: &Device,
) -> Vec<DiscoveryPrepPacket> {
    let mut discoverables: Vec<DiscoveryPrepPacket> = Vec::new();

    if let Some(var) = var_headers.get("AirTemp") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_device_class(SensorClass::Temperature)
            .with_icon("mdi:thermometer")
            .with_value_tempate("{{ value_json.AirTemp | float | round(2) }}")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("TrackTempCrew") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_device_class(SensorClass::Temperature)
            .with_icon("mdi:thermometer")
            .with_name("Track Temperature")
            .with_value_tempate("{{ value_json.TrackTempCrew | float | round(2) }}")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("WindDir") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_unit_of_measurement(Some("degrees"))
            .with_value_tempate("{{ (value_json.WindDir | float * 180 / pi) | float | round(2)}}")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("WindVel") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_unit_of_measurement(Some("km/h"))
            .with_value_tempate("{{ (value_json.WindVel | float * 3.6) | round(2)}}")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("IsOnTrack") {
        let sensor = BinarySensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:go-kart-track")
            .with_payload_on("on")
            .with_payload_off("off")
            .with_value_tempate("{{ 'on' if value_json.IsOnTrack == true else 'off' }}")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("Lap") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:counter")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("SessionState") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:state-machine")
            .with_unit_of_measurement(None::<&str>) // Data is a bitfield, TODO fix this issue with sending data.
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("PlayerCarClassPosition") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:podium")
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("TrackWetness") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:weather-rainy")
            .with_unit_of_measurement(None::<&str>) // Data is a bitfield, TODO fix this issue with sending data.
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("SolarAzimuth") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:sun-compass")
            .with_unit_of_measurement(Some("degrees"))
            .with_value_tempate(
                "{{ (value_json.SolarAzimuth | float * 180 / pi) | float | round(2)}}",
            )
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    if let Some(var) = var_headers.get("SolarAltitude") {
        let sensor = SensorBuilder::new_var(var, TELEMETRY_STATE, device)
            .with_icon("mdi:sun-angle")
            .with_unit_of_measurement(Some("degrees"))
            .with_value_tempate(
                "{{ (value_json.SolarAltitude | float * 180 / pi) | float | round(2)}}",
            )
            .build();
        discoverables.push(prepare_payload(sensor));
    }

    let yellow = BinarySensor::new(TELEMETRY_STATE)
        .with_name("Yellow Flag")
        .with_device(device)
        .with_expire_after(5)
        .with_icon("mdi:flag")
        .with_payload_on("on")
        .with_payload_off("off")
        .with_value_template("{{ 'on' if 'Yellow' in value_json.SessionFlags else 'off' }}")
        .with_unique_id("hairmqtt-yellow-flag")
        .with_object_id("yellow_flag");

    let green = BinarySensor::new(TELEMETRY_STATE)
        .with_name("Green Flag")
        .with_device(device)
        .with_icon("mdi:flag")
        .with_expire_after(5)
        .with_payload_on("on")
        .with_payload_off("off")
        .with_value_template("{{ 'on' if 'Green' in value_json.SessionFlags else 'off' }}")
        .with_unique_id("hairmqtt-green-flag")
        .with_object_id("green_flag");

    let checkered = BinarySensor::new(TELEMETRY_STATE)
        .with_name("Checkered Flag")
        .with_device(device)
        .with_expire_after(5)
        .with_icon("mdi:flag")
        .with_payload_on("on")
        .with_payload_off("off")
        .with_value_template("{{ 'on' if 'Checkered' in value_json.SessionFlags else 'off' }}")
        .with_unique_id("hairmqtt-checkered-flag")
        .with_object_id("checkered_flag");

    let white = BinarySensor::new(TELEMETRY_STATE)
        .with_name("White Flag")
        .with_device(device)
        .with_expire_after(5)
        .with_icon("mdi:flag")
        .with_payload_on("on")
        .with_payload_off("off")
        .with_value_template("{{ 'on' if 'White' in value_json.SessionFlags else 'off' }}")
        .with_unique_id("hairmqtt-white-flag")
        .with_object_id("white_flag");

    let blue = BinarySensor::new(TELEMETRY_STATE)
        .with_name("Blue Flag")
        .with_device(device)
        .with_expire_after(5)
        .with_icon("mdi:flag")
        .with_payload_on("on")
        .with_payload_off("off")
        .with_value_template("{{ 'on' if 'Blue' in value_json.SessionFlags else 'off' }}")
        .with_unique_id("hairmqtt-blue-flag")
        .with_object_id("blue_flag");

    discoverables.push(prepare_payload(yellow));

    discoverables.push(prepare_payload(white));

    discoverables.push(prepare_payload(green));

    discoverables.push(prepare_payload(blue));

    discoverables.push(prepare_payload(checkered));

    discoverables
}

/// Sends the full telemetry data to HA
fn handle_data(data: &IrData, var_headers: &HashMap<String, VarHeader>) -> Map<String, Value> {
    let mut map = Map::new();

    for (name, value) in var_headers {
        let value: Option<Value> = data.get_into(Some(value));
        if let Some(value) = value {
            if let Ok(ser) = serde_json::to_value(value) {
                map.insert(name.clone(), ser);
            } else {
                log::error!("Failed to serialize value for {}", name);
            }
        }
    }
    map
}

/// Serializes the session data.
fn handle_session(session: &Session) -> Map<String, Value> {
    match serde_json::to_value(session) {
        Ok(Value::Object(map)) => map,
        _ => {
            log::error!("Failed to serialize session");
            Map::new()
        }
    }
}

/// Creates a list of discoverable entities from the session data.
fn session_discovery_packet(session: &Session, device: &Device) -> Vec<DiscoveryPrepPacket> {
    let mut discoverables: Vec<DiscoveryPrepPacket> = Vec::new();

    discoverables.push(prepare_payload(
        SensorBuilder::new_session(session, "DriverCarIdx", SESSION_STATE, device, None)
            .with_icon("mdi:account")
            .build(),
    ));

    discoverables.push(prepare_payload(
        SensorBuilder::new_session(session, "DriverSetupName", SESSION_STATE, device, None)
            .with_icon("mdi:cog")
            .build(),
    ));

    discoverables.push(prepare_payload(
        SensorBuilder::new_session(session, "TrackName", SESSION_STATE, device, Some(3))
            .with_icon("mdi:go-kart-track")
            .build(),
    ));

    discoverables.push(prepare_payload(
        BinarySensor::new("hairmqtt/connected")
            .with_name("Connection")
            .with_device(device)
            .with_icon("mdi:connection")
            .with_payload_on("connected")
            .with_payload_off("disconnected")
            .with_unique_id("hairmqtt-connection")
            .with_object_id("connection"),
    ));

    discoverables
}

// May use this in the future. Dead code for now
#[allow(dead_code)]
fn prepare_payload_opt<T>(item: Option<T>) -> Option<DiscoveryPrepPacket>
where
    T: Discoverable + Serialize,
{
    item.map(|x| prepare_payload(x))
}

fn prepare_payload<T>(item: T) -> DiscoveryPrepPacket
where
    T: Discoverable + Serialize,
{
    (
        item.config_topic(),
        serde_json::to_vec(&item).map_err(|e| e.into()),
    )
}
