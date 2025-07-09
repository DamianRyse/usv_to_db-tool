# usv_to_db-tool
Simple tool to execute `upsc` to read data from your connected UPS (Uninterruptible Power Supply)
and store them in a MySQL/MariaDB database.

### Configuration

Create a configuration file:
**/etc/usv-to-db-tool/database.conf**
```ini
host	 = <IP or hostname of your database>
user	 = <DB username>
password = <DB password>
database = <DB name>

influx_host     = <IP or hostname of your InfluxDB 3>
influx_token    = <Token with Write access>
influx_database = <database>
influx_port     = <Port>
influx_scheme   = <http or https>
```

### Run the application
```bash
./usv_to_db-tool <same-param-as-you-use-for-upcs>
```

