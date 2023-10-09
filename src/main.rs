mod error;
mod rest;

slint::include_modules!();
use crate::rest::{setup_poller, Setter};
use dotenv::dotenv;
use log::{error, info};
use serde::Deserialize;
use serde_yaml;
use std::{env, future::Future};

#[derive(Deserialize, Debug)]
struct Config {
    pub ha_rest_url: String,
    pub ha_api_token: Option<String>,
    pub thermostat_ha_entity_id: String,
    pub sensor_refresh_interval_s: u64,
}

fn setup() -> Config {
    env_logger::init();
    dotenv().expect("Couldn't load .env file");
    let ha_api_token = env::var("HA_API_TOKEN").expect("HA_API_TOKEN not found");
    let mut config: Config =
        serde_yaml::from_reader(std::fs::File::open("config.yaml").expect("Couldn't read config file"))
            .expect("Couldn't read config file");
    config.ha_api_token = Some(ha_api_token);
    config
}

fn main() -> Result<(), slint::PlatformError> {
    let config = setup();
    info!("Staring with config: {config:?}");
    let ui = AppWindow::new()?;
    let handle_weak = ui.as_weak();

    let sensor_time = config.sensor_refresh_interval_s;
    let mut poller = setup_poller(&config).unwrap();

    let _ = std::thread::spawn(move || loop {
        let temps = poller.get_temps();
        info!("Updating tmeps! {:?}", temps);
        let handle_copy = handle_weak.clone();

        let result =
            slint::invoke_from_event_loop(move || handle_copy.unwrap().set_temp_sensors(temps.as_slice().into()));
        if let Err(e) = result {
            error!("Error setting temp sensors! {}", e);
        }

        std::thread::sleep(std::time::Duration::from_secs(sensor_time));
        poller.poll();
    });

    let handle_weak = ui.as_weak();
    let setter = Setter::new(&config);
    ui.on_change_temp({
        move |value| {
            info!("on temp change {}", value);
            if let Err(e) = setter.set(value) {
                error!("Error setting temp sensors! {}", e);
                return false;
            }
            // Set again to result to be safe
            handle_weak.unwrap().set_temp(value);
            true
        }
    });

    ui.run()
}
