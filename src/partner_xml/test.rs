// Test struct
use serde::Serialize;
use quick_xml;
use chrono::{DateTime, Local};

use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<(Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)> for Envelope {
    fn from((verion, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)) -> Self {
        Envelope {
            body: (verion, ip, uuid, error).into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}

impl From<(Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)> for Body {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)) -> Self {
        Body {
            response: (version, ip, uuid, error).into()
        }
    }
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}

impl From<(Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)> for Response {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)) -> Self {
        Response {
            result: (version, ip, uuid, error).into()
        }
    }
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}

impl From<(Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)> for Result {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)) -> Self {
        Result {
            answer: (version, ip, uuid, error).into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub data: Option<Data>,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<(Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)> for Answer {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<partner_xml::defaults::Error>)) -> Self {
        Answer {
            version: version.unwrap_or("1.0".to_string()),
            data: if ip.is_some() || uuid.is_some() { Some((ip, uuid).into()) } else { None },
            error: error
        }
    }
}


#[derive(Serialize)]
pub struct Data {
    pub ip: Option<String>,
    pub uuid: Option<String>,
    pub time: Option<DateTime<Local>>
}

impl From<(Option<String>, Option<String>)> for Data {
    fn from((ip, uuid): (Option<String>, Option<String>)) -> Self {
        Data {
            ip: ip,
            uuid: uuid,
            time: Some(Local::now())
        }
    }
}


pub fn create_xml(envelope: Envelope) -> String {
    quick_xml::se::to_string(&envelope).unwrap_or("<Envelope></Envelope>".to_string())
}
