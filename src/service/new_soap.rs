use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, TimeZone, Utc};
use quick_xml;
use lazy_static::lazy_static;
use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use crate::global::errors;
use crate::service::log::logger;

lazy_static! {
    static ref FIRST_DATE: DateTime<Utc> = get_first_date();
}

pub fn get_first_date() -> DateTime<Utc> {
    get_date_from_parts(None, None, None, None, None, None)
}


pub fn get_date_from_parts(year: Option<i32>, month: Option<u32>, day: Option<u32>, hour: Option<u32>, min: Option<u32>, sec: Option<u32>) -> DateTime<Utc> {
    Utc.from_utc_datetime(
        &NaiveDateTime::new(
            chrono::NaiveDate::from_ymd_opt(year.unwrap_or(1900), month.unwrap_or(1), day.unwrap_or(1)).unwrap_or(NaiveDate::MIN),
            chrono::NaiveTime::from_hms_opt(hour.unwrap_or(0), min.unwrap_or(0), sec.unwrap_or(1)).unwrap_or(NaiveTime::MIN)
        )
    )
}


pub enum RequestGet {
    Products(o8_xml::defaults::CallData),
    Stocks(o8_xml::defaults::CallData),
    Prices(o8_xml::defaults::CallData),
    Images(o8_xml::defaults::CallData)
}


#[derive(serde::Serialize)]
#[serde(untagged)]
pub enum ResponseGet {
    Products(partner_xml::products::Envelope),
    Stocks(partner_xml::stocks::Envelope),
    Prices(partner_xml::prices::Envelope),
    Images(partner_xml::images::Envelope)
}


impl RequestGet {
    pub async fn to_envelope(self) -> ResponseGet {
        match self {
            RequestGet::Products(call_data) => ResponseGet::Products(get_products(call_data).await),
            RequestGet::Stocks(call_data) => ResponseGet::Stocks(get_stocks(call_data).await),
            RequestGet::Prices(call_data) => ResponseGet::Prices(get_prices(call_data).await),
            RequestGet::Images(call_data) => ResponseGet::Images(get_images(call_data).await)
        }
    }
    pub async fn to_xml(self) -> String {
        to_xml_string(&self.to_envelope().await)
    }
}


fn to_xml_string<T: serde::Serialize>(val: &T) -> String {
    quick_xml::se::to_string(val).unwrap_or("<Envelope></Envelope>".to_string())
}


async fn get_products(call_data: o8_xml::defaults::CallData) -> partner_xml::products::Envelope {
    let request = o8_xml::products::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::products::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            logger(format!("{}: {} ({})", error.code, error.description, de_error));
            return partner_xml::products::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_stocks(call_data: o8_xml::defaults::CallData) -> partner_xml::stocks::Envelope {
    let request = o8_xml::stocks::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::stocks::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            logger(format!("{}: {} ({})", error.code, error.description, de_error));
            return partner_xml::stocks::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}


async fn get_prices(call_data: o8_xml::defaults::CallData) -> partner_xml::prices::Envelope {
    match call_data.pid {
        Some(pid) => {
            let request = o8_xml::prices::get_request_string(&call_data.xmlns, &call_data.authcode, &pid);
            let response = soap::get_response(&call_data.url, request).await;
            let hu_envelope: o8_xml::prices::Envelope = match quick_xml::de::from_str(&response) {
                Ok(envelope) => envelope,
                Err(de_error) => {
                    let error = errors::GLOBAL_GET_DATA_ERROR;
                    logger(format!("{}: {} ({})", error.code, error.description, de_error));
                    return partner_xml::prices::error_struct(error.code, error.description)
                }
            };
            hu_envelope.to_en()
        }
        _ => {
            let error = errors::GLOBAL_PID_ERROR;
            return partner_xml::prices::error_struct(error.code, error.description)
        }
    }
}


async fn get_images(call_data: o8_xml::defaults::CallData) -> partner_xml::images::Envelope {
    let request = o8_xml::images::get_request_string(&call_data.xmlns, &FIRST_DATE, &call_data.authcode);
    let response = soap::get_response(&call_data.url, request).await;
    let hu_envelope: o8_xml::images::Envelope = match quick_xml::de::from_str(&response) {
        Ok(envelope) => envelope,
        Err(de_error) => {
            let error = errors::GLOBAL_GET_DATA_ERROR;
            logger(format!("{}: {} ({})", error.code, error.description, de_error));
            return partner_xml::images::error_struct(error.code, error.description)
        }
    };
    hu_envelope.to_en()
}
