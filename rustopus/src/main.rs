use chrono::prelude;
use partner_xml::products::Error;
use partner_xml::products::GetProductsAuthResult;
use partner_xml::products::Size;
use reqwest::blocking::Client;
use reqwest::header::CONTENT_TYPE;
use chrono::Utc;
use dotenv::dotenv;
use std::env;

use std::fs;
use std::io;

mod service;
use crate::service::soap;

mod o8_xml;
use crate::o8_xml::products;
use crate::o8_xml::stock;


use quick_xml::de::from_str;
use quick_xml::se::to_string;

mod partner_xml;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    dotenv().ok();
    let secret_key = env::var("AUTH").unwrap_or_else(|_| String::from("default_secret"));
    let authcode = secret_key;
    let url: &str = "https://orink.hu/services/vision.asmx";
    let xmlns: &str = "https://orink.hu/services/";
    let products_response: String = soap::get_products(url, xmlns, &authcode, &soap::get_first_date());

    let products_envelope: products::Envelope = from_str(&products_response)?;

    //println!("{:#?}", products_envelope);
    let mut eng_products: Vec<partner_xml::products::Product> = Vec::new();

    let mut i = 0;  
    for c in products_envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.cikk {
        let c_meret = c.meret.unwrap();
        let eng_product = partner_xml::products::Product {
            id: c.cikkid,
            no: c.cikkszam,
            name: c.cikknev,
            unit: c.me,
            base_unit: c.alapme,
            base_unit_qty: c.alapmenny,
            brand: c.gyarto,
            category_code: c.cikkcsoportkod,
            category_name: c.cikkcsoportnev,
            description: c.leiras,
            weight: c.tomeg,
            size: Size {
                x: c_meret.xmeret,
                y: c_meret.ymeret,
                z: c_meret.zmeret
            },
            main_category_code: c.focsoportkod,
            main_category_name: c.focsoportnev,
            sell_unit: c.ertmenny,
            origin_country: c.szarmorszag
        };
        eng_products.push(eng_product);

        i += 1;

        if i == 10 {
            break;
        } 
    }

    let verzio = products_envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.verzio;

    let hiba = products_envelope.Body.GetCikkekAuthResponse.GetCikkekAuthResult.valasz.hiba;

    let eng_answer = partner_xml::products::Answer {
        version: verzio,
        products: eng_products,
        error: hiba.map(|h| h.into())
    };

    let eng_getproductsauthresult = partner_xml::products::GetProductsAuthResult {
        answer: eng_answer
    };

    let eng_getproductsauthresponse = partner_xml::products::GetProductsAuthResponse {
        result: eng_getproductsauthresult
    };

    let eng_body = partner_xml::products::Body {
        response: eng_getproductsauthresponse
    };

    let eng_envelope = partner_xml::products::Envelope {
        body: eng_body
    };

    let eng_xml = to_string(&eng_envelope).unwrap();

    let _ = save_to_file("products.xml", &eng_xml);

    Ok(())
}


fn save_to_file(filename: &str, contents: &str) -> io::Result<()> {
    fs::write(filename, contents)?;
    Ok(())
}