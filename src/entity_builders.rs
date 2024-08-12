use ha_mqtt::components::binary_sensor::BinarySensor;
use ha_mqtt::components::binary_sensor::BinarySensorClass;

use ha_mqtt::components::sensor::{Sensor, SensorClass};
use ha_mqtt::device::Device;
use ir_telemetry::Session;
use ir_telemetry::VarHeader;

//* Simplified version of the ha_mqtt ones.  I don't need all the options they have, and structures the device and value_json attrs to be specific to this project*//
pub struct BinarySensorBuilder<'a> {
    pub item: BinarySensor<'a>,
}

impl<'a> BinarySensorBuilder<'a> {
    pub fn new_var(var: &VarHeader, state_topic: impl ToString, device: &'a Device) -> Self {
        // Double escape curly braces for templating
        let template_location = format!("{{{{ value_json.{} }}}}", var.name());
        let item = BinarySensor::new(state_topic.to_string())
            .with_name(var.name())
            .with_unique_id(format!("hairmqtt-{}", var.name()))
            .with_object_id(var.name())
            .with_expire_after(10)
            .with_device(device)
            .with_value_template(template_location);

        Self { item }
    }

    #[allow(dead_code)]
    pub fn new_session(
        session: &Session,
        var_name: &str,
        state_topic: impl ToString,
        device: &'a Device,
    ) -> Self {
        let driver_idx = session.driver_info.driver_car_idx as usize;
        let mut item = BinarySensor::new(state_topic.to_string())
            .with_name(var_name.to_string())
            .with_unique_id(format!("hairmqtt-{}", var_name))
            .with_object_id(var_name.to_string())
            .with_expire_after(15)
            .with_device(device);

        if let Some(dot_path) = determine_dot_path(var_name, driver_idx) {
            item = item.with_value_template(format!("{{{{ value_json.{} }}}}", dot_path));
        }

        Self { item }
    }

    #[allow(dead_code)]
    pub fn with_device_class(mut self, device_class: BinarySensorClass) -> Self {
        self.item.device_class = Some(device_class);
        self
    }

    pub fn with_icon(mut self, icon: impl ToString) -> Self {
        self.item.icon = Some(icon.to_string());
        self
    }

    pub fn with_payload_on(mut self, payload: impl ToString) -> Self {
        self.item.payload_on = Some(payload.to_string());
        self
    }

    pub fn with_payload_off(mut self, payload: impl ToString) -> Self {
        self.item.payload_off = Some(payload.to_string());
        self
    }

    pub fn with_value_tempate(mut self, template: impl ToString) -> Self {
        self.item.value_template = Some(template.to_string());
        self
    }

    pub fn build(self) -> BinarySensor<'a> {
        self.item
    }
}

pub struct SensorBuilder<'a> {
    pub item: Sensor<'a>,
}

impl<'a> SensorBuilder<'a> {
    pub fn new_var(var: &VarHeader, state_topic: impl ToString, device: &'a Device) -> Self {
        let template_location = format!("{{{{ value_json.{} }}}}", var.name());
        let item = Sensor::new(state_topic.to_string())
            .with_unit_of_measurement(var.units().to_owned())
            .with_name(var.name())
            .with_unique_id(format!("hairmqtt-{}", var.name()))
            .with_object_id(var.name())
            .with_expire_after(15)
            .with_device(device)
            .with_value_template(template_location);

        Self { item }
    }

    pub fn new_session(
        session: &Session,
        var_name: &str,
        state_topic: impl ToString,
        device: &'a Device,
        array_idx: Option<usize>,
    ) -> Self {
        let driver_idx = session.driver_info.driver_car_idx as usize;
        let mut item = Sensor::new(state_topic.to_string())
            .with_name(var_name.to_string())
            .with_unique_id(format!("hairmqtt-{}", var_name))
            .with_object_id(var_name.to_string())
            .with_expire_after(60)
            .with_device(device);

        if let Some(dot_path) = determine_dot_path(var_name, array_idx.unwrap_or(driver_idx)) {
            item = item.with_value_template(format!("{{{{ value_json.{} }}}}", dot_path));
        }

        Self { item }
    }

    pub fn with_device_class(mut self, device_class: SensorClass) -> Self {
        self.item.device_class = Some(device_class);
        self
    }

    pub fn with_icon(mut self, icon: impl ToString) -> Self {
        self.item.icon = Some(icon.to_string());
        self
    }

    #[allow(dead_code)]
    pub fn with_template_location(mut self, template_location: impl std::fmt::Display) -> Self {
        let location = format!("{{{{ value_json.{} }}}}", template_location);
        self.item.value_template = Some(location);
        self
    }

    pub fn with_unit_of_measurement(mut self, unit: Option<impl ToString>) -> Self {
        self.item.unit_of_measurement = unit.map(|s| s.to_string());
        self
    }

    pub fn with_value_tempate(mut self, template: impl ToString) -> Self {
        self.item.value_template = Some(template.to_string());
        self
    }

    pub fn with_name(mut self, name: impl ToString) -> Self {
        self.item.name = Some(name.to_string());
        self
    }
    pub fn build(self) -> Sensor<'a> {
        self.item
    }
}

/// Determines the dot path to the variable in the session struct.  
/// Each variable is unique (based off on the current struct), so the first occurance should be the only.
/// This is a "best guess" and may not be accurate for all variables.
fn determine_dot_path(var_name: &str, car_idx: usize) -> Option<String> {
    let mut path = vec![];
    let ser = serde_json::to_value(Session::default())
        .expect("Cannot serialize default session, this would be an other library issue");
    recurse_find(&ser, var_name, &mut path, car_idx);
    if path.is_empty() {
        None
    } else {
        Some(path.join("."))
    }
}

// Recurrsively finds the key that matches the target.  Finds the first occurance
fn recurse_find(
    value: &serde_json::Value,
    target: &str,
    path: &mut Vec<String>,
    car_idx: usize,
) {
    match value {
        serde_json::Value::Object(map) => {
            for (key, value) in map {
                path.push(key.clone());
                if key == target {
                    return;
                }

                recurse_find(value, target, path, car_idx);
                if path.last().unwrap() == key {
                    path.pop();
                }
            }
        }
        serde_json::Value::Array(vec) => {
            let value = vec.get(car_idx);
            if let Some(value) = value {
                let idx = car_idx.to_string();
                path.push(idx);
                recurse_find(value, target, path, car_idx);

                if path.last().unwrap() == &value.to_string() {
                    path.pop();
                }
            }
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_determine_dot_path() {
        let path = determine_dot_path("SessionInfo", 0);
        assert_eq!(path, Some("session_info".to_string()));
    }

    #[test]
    fn test_determine_dot_path_nested() {
        let path = determine_dot_path("WeekendInfo", 0);
        assert_eq!(path, Some("weekend_info".to_string()));
    }

    #[test]
    fn test_determine_dot_path_nested_nested() {
        let path = determine_dot_path("TrackName", 0);
        assert_eq!(path, Some("weekend_info.track_name".to_string()));
    }

    #[test]
    fn should_determine_car_driver_idx() {
        let path = determine_dot_path("CarDriverIdx", 0);
        assert_eq!(path, Some("driver_info.car_driver_idx".to_string()));
    }
}
