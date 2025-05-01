use std::{collections::HashMap, env, fs, process::Command};
use mysql::*;
use mysql::prelude::*;
use chrono::Local;

fn main() -> Result <(), Box<dyn std::error::Error>> {

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
    
    for (key, value) in upsc_hashmap {
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
        return Err("Missing required config values");
    }

    Ok(format!("mysql://{}:{}@{}/{}", user, pass, host, db))
}

fn log(msg: &str) -> String{
    let now = Local::now();
    format!("[{}] {}", now.format("%d.%m.%Y %H:%M"), msg)
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