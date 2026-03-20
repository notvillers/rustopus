/// Images english struct(s) for XML(s) got from the Octopus call
use serde::Serialize;
use quick_xml;

use crate::forms::r#in::xml::images as o8_images;
use crate::partner_xml::defaults as p_defaults;

#[derive(Serialize)]
pub struct Envelope {
    pub body: Body
}

impl From<o8_images::Envelope> for Envelope {
    fn from(e: o8_images::Envelope) -> Self {
        Envelope {
            body: e.body.into()
        }
    }
}


#[derive(Serialize)]
pub struct Body {
    pub response: GetProductImagesAuthResponse
}

impl From<o8_images::Body> for Body {
    fn from(b: o8_images::Body) -> Self {
        Body {
            response: b.get_cikk_kepek_auth_response.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductImagesAuthResponse {
    pub result: GetProductImagesAuthResult
}

impl From<o8_images::GetCikkKepekAuthResponse> for GetProductImagesAuthResponse {
    fn from(r: o8_images::GetCikkKepekAuthResponse) -> Self {
        GetProductImagesAuthResponse {
            result: r.get_cikk_kepek_auth_result.into()
        }
    }
}


#[derive(Serialize)]
pub struct GetProductImagesAuthResult {
    pub answer: Answer
}

impl From<o8_images::GetCikkKepekAuthResult> for GetProductImagesAuthResult{
    fn from(r: o8_images::GetCikkKepekAuthResult) -> Self {
        Self {
            answer: r.valasz.into()
        }
    }
}


#[derive(Serialize)]
pub struct Answer {
    pub version: String,
    pub products: Products,
    pub error: Option<p_defaults::Error>
}

impl From<o8_images::Valasz> for Answer {
    fn from(v: o8_images::Valasz) -> Self {
        Self {
            version: v.verzio,
            products: v.cikk
                        .into_iter()
                        .collect::<Products>(),
            error: v.hiba.map(|x| x.into())
        }
    }
}


#[derive(Serialize)]
pub struct Products {
    pub product: Vec<Product>
}

impl FromIterator<o8_images::Cikk> for Products {
    fn from_iter<T: IntoIterator<Item = o8_images::Cikk>>(iter: T) -> Self {
        Self {
            product: iter
                        .into_iter()
                        .map(|x| x.into())
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

impl From<o8_images::Cikk> for Product {
    fn from(c: o8_images::Cikk) -> Self {
        Self {
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

impl From<o8_images::Kepek> for Images {
    fn from(kk: o8_images::Kepek) -> Self {
        Self {
            image: kk.kep
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}


#[derive(Serialize)]
pub struct Image {
    pub gallery: String,
    pub url: String
}

impl From<o8_images::Kep> for Image {
    fn from(k: o8_images::Kep) -> Self {
        Self { 
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
                        version: String::from("1.0"), 
                        products: Products {
                            product: vec![]
                        },
                        error: Some(p_defaults::Error::load(code, description))
                    }
                }
            }
        }
    }
}


pub fn error_struct_xml(code: u64, description: &str) -> String {
    quick_xml::se::to_string(&error_struct(code, description)).unwrap_or("<Envelope></Envelope>".into())
}
