// Test struct
use chrono::{DateTime, Local};
use quick_xml;

use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::out::xml::defaults as p_defaults
};

OutModelDeriveSerializeOnly! {
    pub struct Envelope {
        pub body: Body
    }

    pub struct Body {
        pub response: Response
    }

    pub struct Response {
        pub result: Result
    }

    pub struct Result {
        pub answer: Answer
    }

    pub struct Answer {
        pub version: String,
        pub data: Option<Data>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }

    pub struct Data {
        pub ip: Option<String>,
        pub uuid: Option<String>,
        pub time: DateTime<Local>
    }
}

impl From<(Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)> for Envelope {
    fn from((verion, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)) -> Self {
        Self {
            body: (verion, ip, uuid, error).into()
        }
    }
}


impl From<(Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)> for Body {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)) -> Self {
        Self {
            response: (version, ip, uuid, error).into()
        }
    }
}


impl From<(Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)> for Response {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)) -> Self {
        Self {
            result: (version, ip, uuid, error).into()
        }
    }
}


impl From<(Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)> for Result {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)) -> Self {
        Self {
            answer: (version, ip, uuid, error).into()
        }
    }
}


impl From<(Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)> for Answer {
    fn from((version, ip, uuid, error): (Option<String>, Option<String>, Option<String>, Option<p_defaults::Error>)) -> Self {
        Self {
            version: version.unwrap_or("1.0".into()),
            data: if ip.is_some() || uuid.is_some() { Some((ip, uuid).into()) } else { None },
            error: error
        }
    }
}


impl From<(Option<String>, Option<String>)> for Data {
    fn from((ip, uuid): (Option<String>, Option<String>)) -> Self {
        Self {
            ip: ip,
            uuid: uuid,
            time: Local::now()
        }
    }
}


pub fn create_xml(envelope: Envelope) -> String {
    quick_xml::se::to_string(&envelope).unwrap_or("<Envelope></Envelope>".into())
}
