/// Images english struct(s) for XML(s) got from the Octopus call

use serde::Serialize;

use crate::o8_xml;
use crate::partner_xml;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_xml::images::Envelope> for Envelope {
    fn from(e: o8_xml::images::Envelope) -> Self {
        Envelope {
            body: e.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetProductImagesAuthResponse
}

impl From<o8_xml::images::Body> for Body {
    fn from(b: o8_xml::images::Body) -> Self {
        Body {
            response: b.get_cikk_kepek_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductImagesAuthResponse {
    pub result: GetProductImagesAuthResult
}

impl From<o8_xml::images::GetCikkKepekAuthResponse> for GetProductImagesAuthResponse {
    fn from(r: o8_xml::images::GetCikkKepekAuthResponse) -> Self {
        GetProductImagesAuthResponse {
            result: r.get_cikk_kepek_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductImagesAuthResult {
    pub answer: Answer
}

impl From<o8_xml::images::GetCikkKepekAuthResult> for GetProductImagesAuthResult{
    fn from(r: o8_xml::images::GetCikkKepekAuthResult) -> Self {
        GetProductImagesAuthResult {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Option<partner_xml::defaults::Error>
}

impl From<o8_xml::images::Valasz> for Answer {
    fn from(v: o8_xml::images::Valasz) -> Self {
        Answer {
            version: v.verzio,
            products: v.cikk
                        .into_iter()
                        .collect::<Products>(),
            error: v.hiba.map(|e| e.into())
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl FromIterator<o8_xml::images::Cikk> for Products {
    fn from_iter<T: IntoIterator<Item = o8_xml::images::Cikk>>(iter: T) -> Self {
        Products {
            product: iter
                        .into_iter()
                        .map(|p| p.into())
                        .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Product {
    pub id: u64,
    pub no: String,
    pub images: Images
}

impl From<o8_xml::images::Cikk> for Product {
    fn from(c: o8_xml::images::Cikk) -> Self {
        Product {
            id: c.cikkid,
            no: c.cikkszam,
            images: c.kepek.into()
        }
    }
}


#[derive(Serialize)]
pub struct Images {
    pub image: Vec<Image>
}

impl From<o8_xml::images::Kepek> for Images {
    fn from(kk: o8_xml::images::Kepek) -> Self {
        Images {
            image: kk.kep
                    .into_iter()
                    .map(|k| k.into())
                    .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Image {
    pub gallery: String,
    pub url: String
}

impl From<o8_xml::images::Kep> for Image {
    fn from(k: o8_xml::images::Kep) -> Self {
        Image { 
            gallery: k.galeria,
            url: k.url
        }
    }
}


pub fn error_struct(code: u64, description: &str) -> Envelope {
    Envelope { 
        body: Body { 
            response: GetProductImagesAuthResponse { 
                result: GetProductImagesAuthResult { 
                    answer: Answer {
                        version: "1.0".to_string(), 
                        products: Products {
                            product: vec![]
                        },
                        error: Some(
                            partner_xml::defaults::Error::load(code, description)
                        )
                    }
                }
            }
        }
    }
}
