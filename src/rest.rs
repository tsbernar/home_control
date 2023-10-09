use crate::error::HomeError;
use crate::Config;
use crate::TempSensorData;
use log::{error, info};
use reqwest::blocking::Client;
use serde::{de, Deserialize, Deserializer, Serialize};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt::Debug;

const STATE_ENDPOINT: &str = "states";
const SET_TEMP_ENDPOINT: &str = "states";

pub(crate) fn setup_poller(config: &Config) -> Result<Poller, HomeError> {
    info!("Starting rest polling");

    let mut temp_sensors = HashMap::new();
    temp_sensors.insert("Bedroom", "sensor.bedroom_temperature");
    temp_sensors.insert("Florida Room", "sensor.florida_room_tvoc_temperature_sensor");
    temp_sensors.insert("Living Room", "sensor.indoor_temperature");
    temp_sensors.insert("Outside", "sensor.outdoor_temperature");

    let client = Client::new();

    let mut poller = Poller {
        temp_sensors,
        temps: HashMap::new(),
        humidity: HashMap::new(),
        client: client,
        base_url: config.ha_rest_url.to_owned(),
        ha_api_token: config.ha_api_token.as_ref().unwrap().to_owned(),
    };

    poller.poll();

    Ok(poller)
}

pub(crate) struct Setter {
    client: Client,
    base_url: String,
    thermostat_ha_entity_id: String,
    ha_api_token: String,
}

#[derive(Serialize, Debug)]
struct SetTempData<T> {
    state: T,
}

impl Setter {
    pub fn new(config: &Config) -> Setter {
        Setter {
            client: Client::new(),
            base_url: config.ha_rest_url.to_owned(),
            thermostat_ha_entity_id: config.thermostat_ha_entity_id.to_owned(),
            ha_api_token: config.ha_api_token.as_ref().unwrap().to_owned(),
        }
    }

    pub fn set<T: Serialize + Debug + Clone>(&self, value: T) -> Result<u32, HomeError> {
        let url = format!(
            "{base_url}/{SET_TEMP_ENDPOINT}/{entity_id}",
            base_url = self.base_url,
            entity_id = self.thermostat_ha_entity_id
        );

        info!("{:?}", url);
        info!("{:?}", SetTempData { state: value.clone() });

        let response = self
            .client
            .post(url)
            .header("Authorization", format!("Bearer {}", self.ha_api_token))
            .json(&SetTempData { state: value })
            .send()
            .unwrap(); // TODO

        info!("Response: {:?}", response);

        if response.status() != reqwest::StatusCode::OK {
            error!("Unexpected status code when setting temp state {:?}", response);
            Err(HomeError::ApiError(format!(
                "Unexpected status code when setting temp state {:?}",
                response
            )))?;
        }

        match response.json::<EntityStateResponse>() {
            Ok(entity_state_response) => {
                info!("Set sensor state: {entity_state_response:#?}");
                Ok(entity_state_response.state.round() as u32)
            }
            Err(e) => Err(HomeError::ApiError(format!(
                "Couldn't deserialize body as json {:?}",
                e
            ))),
        }
    }
}

#[allow(dead_code)] // Want to capture the actual response, this is fine
#[derive(Deserialize, Debug)]
struct EntityStateResponse {
    entity_id: String,
    #[serde(deserialize_with = "to_float")]
    state: f32,
    attributes: HashMap<String, String>,
    last_changed: String, // TODO replace with datetime
    last_updated: String,
    context: HashMap<String, Option<String>>,
}

fn to_float<'de, D: Deserializer<'de>>(deserializer: D) -> Result<f32, D::Error> {
    Ok(match Value::deserialize(deserializer)? {
        Value::String(s) => s.parse().map_err(de::Error::custom)?,
        Value::Number(num) => num.as_f64().ok_or(de::Error::custom("Invalid number"))? as f32,
        _ => return Err(de::Error::custom("wrong type")),
    })
}

pub(crate) struct Poller {
    temp_sensors: HashMap<&'static str, &'static str>,
    temps: HashMap<String, f32>,
    #[allow(dead_code)] // TODO
    humidity: HashMap<String, f32>,
    client: Client,
    base_url: String,
    ha_api_token: String,
}

impl Poller {
    pub fn get_temps(&self) -> Vec<TempSensorData> {
        self.temps
            .iter()
            .map(|(k, v)| TempSensorData {
                sensor_name: k.into(),
                sensor_value: v.round() as i32,
            })
            .collect()
    }

    pub fn poll(&mut self) {
        info!("Starting poller");
        for (&sensor_name, &sensor_id) in self.temp_sensors.iter() {
            let url = format!(
                "{base_url}/{STATE_ENDPOINT}/{entity_id}",
                base_url = self.base_url,
                entity_id = sensor_id
            );

            info!("Polling for sensor {sensor_name}");

            let res = self
                .client
                .get(url)
                .header("Authorization", format!("Bearer {}", self.ha_api_token))
                .send();

            if let Err(e) = res {
                error!("Error polling sensor {}: {}", sensor_id, e);
                continue;
            }
            let response = res.unwrap();

            if response.status() != reqwest::StatusCode::OK {
                error!(
                    "Unexpected status code when polling sensor {}: {:?}",
                    sensor_id, response
                );
                continue;
            }

            match response.json::<EntityStateResponse>() {
                Ok(entity_state_response) => {
                    info!("Got sensor state: {entity_state_response:#?}");
                    info!("Updating temp to {} for {}", entity_state_response.state, sensor_name);
                    self.temps.insert(sensor_name.to_owned(), entity_state_response.state);
                }
                Err(e) => {
                    error!("Couldn't deserialize body as json {} {:?}", sensor_id, e);
                }
            }
        }
    }
}
