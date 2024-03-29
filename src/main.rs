#[macro_use]
extern crate serde_derive;

#[macro_use]
extern crate log;

use influent::client::{Client, Credentials};
use influent::measurement::{Measurement, Value};

mod config {
    use std::env;

    pub struct Icecast {
        pub user: String,
        pub password: String,
        pub host: String,
        pub port: i16,
    }

    pub struct Influx {
        pub user: String,
        pub password: String,
        pub host: String,
        pub database: String,
    }

    pub struct Config {
        pub icecast: Icecast,
        pub influxdb: Influx,
    }

    impl Config {
        pub fn new() -> Config {
            let icecast_config = Icecast {
                user: env::var("ICECAST_USER").expect("Missing environment variable ICECAST_USER"),
                password: env::var("ICECAST_PASSWORD")
                    .expect("Missing environment variable ICECAST_PORT"),
                host: env::var("ICECAST_HOST").expect("Missing environment variable ICECAST_HOST"),
                port: env::var("ICECAST_PORT")
                    .expect("Missing environment variable ICECAST_PORT")
                    .parse::<i16>()
                    .expect("ICECAST_PORT must be between 1:65535"),
            };
            let influx_config = Influx {
                user: env::var("INFLUX_USER").expect("Missing environment variable INFLUX_USER"),
                password: env::var("INFLUX_PASSWORD")
                    .expect("Missing environment variable INFLUX_PASSWORD"),
                host: format!(
                    "http://{0}:{1}",
                    env::var("INFLUX_HOST").expect("Missing environment variable INFLUX_HOST"),
                    env::var("INFLUX_PORT")
                        .expect("Missing environment variable INFLUX_PORT")
                        .parse::<i16>()
                        .expect("INFLUX_PORT must be between 1:65535")
                ),
                database: env::var("INFLUX_DATABASE")
                    .expect("Missing environment variable INFLUX_DATABASE"),
            };
            Config {
                icecast: icecast_config,
                influxdb: influx_config,
            }
        }
    }
}

mod list_mounts {
    //! Structs for representing data from the /admin/listmounts endpoint

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Source {
        pub mount: Option<String>,
        pub fallback: Option<String>,
        pub listeners: i64,
        #[serde(rename = "content-type")]
        pub content_type: String,
    }

    #[derive(Serialize, Deserialize, Debug, Clone)]
    pub struct Icestats {
        #[serde(rename = "source")]
        pub sources: Vec<Source>,
    }
}

// Name your user agent after your app?
static APP_USER_AGENT: &str = concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION"),);

/// The main program entrypoint.
#[tokio::main(flavor = "current_thread")]
async fn main() {
    println!("Icecast InfluxDB Stats Importer");
    init_logger();

    info!("Creating config");
    let config = config::Config::new();
    info!("Creating InfluxDB Client");
    let influx_client = create_influx_client(&config.influxdb);
    info!("Creating HTTP Client");
    let client = create_icecast_client(&config.icecast);

    loop {
        let listmounts_xml = read_icecast_xml(&config.icecast, &client, "admin/listmounts").await;

        let mounts: list_mounts::Icestats =
            serde_xml_rs::deserialize(listmounts_xml.as_bytes()).unwrap();
        debug!("{:?}", mounts);

        let measurements =
            icecast_stats_to_measurements(&mounts, &config.icecast.host, &time::now());

        info!("Writing measurements to InfluxDB");
        influx_client
            .write_many(&measurements, None)
            .expect("Data not written");
        info!("Done!");
        std::thread::sleep(std::time::Duration::new(30, 0));
    }
}

fn init_logger() {
    let logger_config = fern::DispatchConfig {
        format: Box::new(
            |msg: &str, level: &log::LogLevel, _location: &log::LogLocation| {
                // This is a fairly simple format, though it's possible to do more complicated ones.
                // This closure can contain any code, as long as it produces a String message.
                format!(
                    "[{}][{}] {}",
                    time::now().strftime("%Y-%m-%d][%H:%M:%S").unwrap(),
                    level,
                    msg
                )
            },
        ),
        output: vec![fern::OutputConfig::stdout()],
        level: log::LogLevelFilter::Trace,
    };
    fern::init_global_logger(logger_config, log::LogLevelFilter::Info)
        .expect("Failed to initialise logger");
    debug!("Logger initialised");
}

async fn read_icecast_xml(
    icecast_config: &config::Icecast,
    client: &reqwest::Client,
    endpoint: &str,
) -> String {
    let uri_string = format!(
        "http://{0}:{1}/{2}",
        icecast_config.host, icecast_config.port, endpoint
    );
    info!("Reading XML from Icecast ({})", &uri_string);

    let res = client
        .get(uri_string)
        .basic_auth(&icecast_config.user, Some(&icecast_config.password))
        .send()
        .await
        .expect("XML request failed");
    assert_eq!(res.status(), reqwest::StatusCode::OK);
    info!("XML received OK");

    let contents = res.text().await.expect("XML read failed");
    info!("XML read OK");

    contents
}

fn create_influx_client(influx_config: &config::Influx) -> influent::client::http::HttpClient {
    let credentials: Credentials = Credentials {
        username: &influx_config.user,
        password: &influx_config.password,
        database: &influx_config.database,
    };

    let hosts: Vec<&str> = vec![&influx_config.host];
    influent::create_client(credentials, hosts)
}

fn create_icecast_client(_icecast_config: &config::Icecast) -> reqwest::Client {
    reqwest::ClientBuilder::new()
        .user_agent(APP_USER_AGENT)
        .build()
        .expect("Couldn't build client!")
}

fn icecast_stats_to_measurements<'a>(
    stats: &'a list_mounts::Icestats,
    host: &'a str,
    timestamp: &time::Tm,
) -> Vec<Measurement<'a>> {
    let timestamp: i64 =
        timestamp.to_timespec().sec * 1_000_000_000 + i64::from(timestamp.to_timespec().nsec);
    info!("Creating InfluxDB Measurements for timestamp {}", timestamp);

    let (mut measurements, total_listeners) = stats
        .sources
        .iter()
        .map(|source| {
            let mut measurement = Measurement::new("listeners");
            measurement.add_tag("host", host);
            measurement.add_tag("mount", source.mount.as_ref().unwrap());
            measurement.add_field("value", Value::Integer(source.listeners));
            measurement.set_timestamp(timestamp);

            (measurement, source.listeners)
        })
        .fold(
            (Vec::new(), 0),
            |(mut measurements, counts), (measurement, count)| {
                measurements.push(measurement);
                (measurements, counts + count)
            },
        );

    let mut total_measurement = Measurement::new("listenerstotal");
    total_measurement.add_tag("host", host);
    total_measurement.add_field("value", Value::Integer(total_listeners));
    total_measurement.set_timestamp(timestamp);

    measurements.push(total_measurement);

    measurements
}
