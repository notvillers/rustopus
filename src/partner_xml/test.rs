use serde::Serialize;
use quick_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl Envelope {
    pub fn load(version: Option<String>, ip: Option<String>, uuid: Option<String>, error: Option<partner_xml::defaults::Error>) -> Self {
        Envelope {
            body: Body::load(version, ip, uuid, error)
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: Response
}

impl Body {
    pub fn load(version: Option<String>, ip: Option<String>, uuid: Option<String>, error: Option<partner_xml::defaults::Error>) -> Self {
        Body {
            response: Response::load(version, ip, uuid, error)
        }
    }
}


#[derive(Serialize)]
pub struct Response {
    pub result: Result
}

impl Response {
    pub fn load(version: Option<String>, ip: Option<String>, uuid: Option<String>, error: Option<partner_xml::defaults::Error>) -> Self {
        Response {
            result: Result::load(version, ip, uuid, error)
        }
    }
}


#[derive(Serialize)]
pub struct Result {
    pub answer: Answer
}

impl Result {
    pub fn load(version: Option<String>, ip: Option<String>, uuid: Option<String>, error: Option<partner_xml::defaults::Error>) -> Self {
        Result {
            answer: Answer::load(version, ip, uuid, error)
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub data: Option<Data>,
    pub error: Option<partner_xml::defaults::Error>
}

impl Answer {
    pub fn load(version: Option<String>, ip: Option<String>, uuid: Option<String>, error: Option<partner_xml::defaults::Error>) -> Self {
        Answer {
            version: version.unwrap_or("1.0".to_string()),
            data: if ip.is_some() || uuid.is_some() { Some(Data::load(ip, uuid)) } else { None },
            error: error
        }
    }
}


#[derive(Serialize)]
pub struct Data {
    pub ip: Option<String>,
    pub uuid: Option<String>
}

impl Data {
    pub fn load(ip: Option<String>, uuid: Option<String>) -> Self {
        Data {
            ip: ip,
            uuid: uuid
        }
    }
}

pub fn create_xml(envelope: Envelope) -> String {
    quick_xml::se::to_string(&envelope).unwrap_or("<Envelope></Envelope>".to_string())
}
