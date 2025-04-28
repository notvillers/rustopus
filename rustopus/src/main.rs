use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use chrono::Utc;
use dotenv::dotenv;
use std::env;

use std::fs;
use std::io;

mod service;
use crate::service::soap::get_products;

mod o8_xml;
use crate::o8_xml::products;
use quick_xml::de::from_str;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let secret_key = env::var("AUTH").unwrap_or_else(|_| String::from("default_secret"));

    let web_update = Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string();
    let authcode = secret_key;
    let soap_request = format!(
        r#"<?xml version="1.0" encoding="utf-8"?>
            <soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
            <soap:Body>
                <GetCikkekAuth xmlns="https://orink.hu/services/">
                <web_update>{}</web_update>
                <authcode>{}</authcode>
                </GetCikkekAuth>
            </soap:Body>
            </soap:Envelope>
        "#,
        web_update,
        authcode
    );

    let client = Client::new();
    let response = client
        .post("https://orink.hu/services/vision.asmx")
        .header(CONTENT_TYPE, "text/xml; charset=utf-8")
        .body(soap_request)
        .send()?;

    let response_text = response.text()?;
    
    let envelope: products::Envelope = from_str(&response_text)?;
    //println!("{:#?}", envelope);

    for cikk in envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.cikk {
        println!("{}, {}: {}", cikk.cikknev, cikk.cikkszam, cikk.cikknev)
    }

    Ok(())
}

fn save_to_file(filename: &str, contents: &str) -> io::Result<()> {
    fs::write(filename, contents)?;
    Ok(())
}