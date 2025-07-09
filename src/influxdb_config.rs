use std::fmt;

pub enum InfluxDbProtocol {
    Http,
    Https
}

pub struct InfluxDbConfig {
    pub token: String,
    pub database: String,
    pub hostname: String,
    pub protocol: InfluxDbProtocol,
    pub port: u16,
}

impl InfluxDbConfig {
    pub fn build_url(&self) -> String {
        let proto = match self.protocol {
            InfluxDbProtocol::Http => "http",
            InfluxDbProtocol::Https => "https"
        };
        format!("{}://{}:{}/api/v3/write_lp?db={}&precision=millisecond",proto, self.hostname, self.port,self.database)
    }
}

#[derive(Debug)]
pub struct InfluxDbLp {
    pub table: String,
    pub tag_set: Vec<InfluxDbTagSet>,
    pub field_set: Vec<InfluxDbFieldSet>,
    pub timestamp: i64,
}

impl InfluxDbLp {
    pub fn to_string(&self) -> String {
        let tag_set = self.tag_set.iter()
            .map(|tag| tag.to_string())
            .collect::<Vec<_>>()
            .join(",");

        let field_set = self.field_set.iter()
            .map(|tag| tag.to_string())
            .collect::<Vec<_>>()
            .join(",");
        format!("{},{} {} {}", self.table, tag_set, field_set, self.timestamp)
    }
}


#[derive(Debug)]
pub struct InfluxDbTagSet {
    pub key: String,
    pub value: String
}
impl fmt::Display for InfluxDbTagSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}={}", self.key, self.value.replace(" ", "\\ "))
    }
}

#[derive(Debug)]
pub struct InfluxDbFieldSet {
    pub key: String,
    pub value: String
}

impl fmt::Display for InfluxDbFieldSet {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let trimmed = self.value.trim();

        // Check if it's a valid integer (handles negative numbers too)
        if trimmed.parse::<i64>().is_ok() {
            write!(f, "{}={}i", self.key, trimmed)
        } else {
            write!(f, "{}=\"{}\"", self.key, self.value)
        }
    }
}