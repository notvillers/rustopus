// Test CSV
use chrono::{DateTime, Local};
use macro_rules_attribute::apply;
use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::out::xml::test as p_test
};

#[apply(OutModelDeriveSerializeOnly)]
pub struct Data {
    pub ip: Option<String>,
    pub uuid: Option<String>,
    pub time: Option<DateTime<Local>>
}

impl From<p_test::Envelope> for Data {
    fn from(e: p_test::Envelope) -> Self {
        match e.body.response.result.answer.data {
            Some(d) => Self {
                ip: d.ip,
                uuid: d.uuid,
                time: Some(d.time)
            },
            None => Self {
                ip: None,
                uuid: None,
                time: None
            }
        }
    }
}
