mod influxdb_config;

use std::{collections::HashMap, env, fs, process::Command};
use mysql::*;
use mysql::prelude::*;
use chrono::{Local};
use crate::influxdb_config::{InfluxDbConfig, InfluxDbFieldSet, InfluxDbLp, InfluxDbProtocol, InfluxDbTagSet};
use reqwest;
use tokio;

#[tokio::main]
async fn main() -> Result <(), Box<dyn std::error::Error>> {

    const DB_CONFIG: &str = "/etc/usv-to-db-tool/database.conf" ;

    // Get CLI argument
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage {} <upsc-parameter>", args[0]);
        std::process::exit(1);
    }
    
    let upsc_parameter = &args[1];
    let upsc_hashmap = get_upsc_output(&upsc_parameter);

    // Parse the DB config file
    let config_content = fs::read_to_string(DB_CONFIG)?;
    let db_url = parse_db_config(&config_content)?;

    // Create the options variable
    let opts = Opts::from_url(&db_url)?;

    // Create the DB pool and open connection
    let pool = Pool::new(opts)?;
    let mut conn = pool.get_conn().expect(format!("{}",log("failed to get database connection")).as_str());


    conn.query_drop(
        r"CREATE TABLE IF NOT EXISTS status (
                `Key` varchar(255) NOT NULL,
                Value varchar(255) NOT NULL,
                PRIMARY KEY (`Key`)
              )
              ENGINE = INNODB,
              CHARACTER SET latin1,
              COLLATE latin1_swedish_ci;"
    )?;

    // We're using transaction to optimize the performance and reduce the data transfer to a single
    // commit.
    let mut sql_transaction = conn.start_transaction(TxOpts::default())?;
    
    for (key, value) in &upsc_hashmap {
        sql_transaction.exec_drop(
            r"INSERT INTO status (`Key`, `Value`) VALUES (:key, :value) ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)",
            params! { "key" => key, "value" => value, },
        )?;
    }
        
    // Add an extra DB row for the update timestamp
    let timestamp_now = Local::now();
    
    
    sql_transaction.exec_drop(
        r"INSERT INTO status (`Key`, `Value`) VALUES (:key, :value) ON DUPLICATE KEY UPDATE `value` = VALUES(`value`)",
        params! {"key" => "stats.updated", "value" => timestamp_now.format("%d.%m.%Y %H:%M:%s").to_string()},
    )?;
    
    
    // Send the transaction
    sql_transaction.commit()?;

    println!("{}", log("Database successfully updated."));

    
    
    // ============================================================
    // MARIADB DATABASE UPDATE DONE. CONTINUING NOW WITH INFLUXDB 3.
    // ============================================================
    let influxdb_config = parse_influx_config(&config_content)?;

    let influxdb_lp = InfluxDbLp {
        table: String::from("measurement__power"),
        tag_set: vec![
            InfluxDbTagSet { key: "device_serial".to_string(), value: upsc_hashmap.get("device.serial").unwrap().to_string() },
            InfluxDbTagSet { key: "device_model".to_string(), value: upsc_hashmap.get("device.model").unwrap().to_string() }
        ],
        field_set: vec![
                        InfluxDbFieldSet { key: "ups_realpower".to_string(), value: upsc_hashmap.get("ups.realpower").unwrap().to_string()},
                        InfluxDbFieldSet { key: "ups_power".to_string(), value: upsc_hashmap.get("ups.power").unwrap().to_string() },
                        InfluxDbFieldSet { key: "battery_charge".to_string(), value: upsc_hashmap.get("battery.charge").unwrap().to_string() }
        ],
        timestamp: timestamp_now.timestamp_millis()
    };

    send_to_influxdb(&influxdb_config, &influxdb_lp).await?;

    Ok(())
}


fn parse_db_config(config: &str) -> Result<String, &'static str> {
    let mut host = "localhost";
    let mut user = "root";
    let mut pass = "";
    let mut db = "ups";

    for line in config.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "host" => host = v.trim(),
                "user" => user = v.trim(),
                "password" => pass = v.trim(),
                "database" => db = v.trim(),
                _ => {}
            }
        }
    }

    if user.is_empty() || db.is_empty() {
        return Err("Missing required config values for MariaDB");
    }

    Ok(format!("mysql://{}:{}@{}/{}", user, pass, host, db))
}

fn parse_influx_config(config: &str) -> Result<InfluxDbConfig, &'static str> {
    let mut host = "";
    let mut token = "";
    let mut database = "";
    let mut port: u16 = 8181;
    let mut scheme: InfluxDbProtocol = InfluxDbProtocol::Http;

    for line in config.lines() {
        if let Some((k, v)) = line.split_once('=') {
            match k.trim() {
                "influx_host" => host = v.trim(),
                "influx_token" => token = v.trim(),
                "influx_database" => database = v.trim(),
                "influx_port" => port = v.trim().parse::<u16>().expect("Invalid port number."),
                "influx_scheme" => match v.trim() {
                    "https" => scheme = InfluxDbProtocol::Https,
                    _ => scheme = InfluxDbProtocol::Http,
                },
                _ => {}
            }
        }
    }

    if token.is_empty() || host.is_empty() || database.is_empty() {
        return Err("Missing required config values for InfluxDB");
    }

    Ok(InfluxDbConfig {
        hostname: host.to_string(),
        token: token.to_string(),
        database: database.to_string(),
        port: port,
        protocol: scheme
    })
}

fn log(msg: &str) -> String{
    let now = Local::now();
    format!("[{}] {}", now.format("%d.%m.%Y %H:%M"), msg)
}

async fn send_to_influxdb(config: &InfluxDbConfig, influx_db_lp: &InfluxDbLp) -> Result<(), Box<dyn std::error::Error>>{
    let client = reqwest::Client::new();

    let response = client
        .post(config.build_url())
        .header("Authorization", format!("Bearer {}", config.token))
        .header("Content-Type", "text/plain; charset=utf-8")
        .body(influx_db_lp.to_string())
        .send()
        .await?;

    println!("Status: {}", response.status());
    Ok(())
}

fn get_upsc_output(upsc_parameter: &String) -> HashMap<String,String> {
    // Execute upsc and get the output
    let output = Command::new("upsc")
        .arg(upsc_parameter)
        .output()
        .expect("failed to execute process");

    if !output.status.success() {
        eprintln!("upsc command failed: {:?}", output);
        std::process::exit(1);
    }
    
    let output = String::from_utf8_lossy(&output.stdout);

    // Parse the output ink key-value map
    let mut data = HashMap::new();
    for line in output.lines() {
        if let Some((k,v)) = line.split_once(":") {
            data.insert(k.trim().to_string(), v.trim().to_string());
        }
    }
    
    data
}