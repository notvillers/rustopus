/// Images english struct(s) for XML(s) got from the Octopus call
use quick_xml;

use crate::{
    macros::out::OutModelDeriveOnly,
    forms::{
        r#in::xml::images as o8_images,
        out::xml::defaults as p_defaults
    }
};

OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }

    pub struct Body {
        pub response: GetProductImagesAuthResponse
    }

    pub struct GetProductImagesAuthResponse {
        pub result: GetProductImagesAuthResult
    }

    pub struct GetProductImagesAuthResult {
        pub answer: Answer
    }
    
    pub struct Answer {
        pub version: String,
        pub products: Products,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>
    }
    
    pub struct Product {
        pub id: u64,
        pub no: String,
        pub images: Images
    }

    pub struct Products {
        pub product: Vec<Product>
    }
    pub struct Images {
        pub image: Vec<Image>
    }
    
    pub struct Image {
        pub gallery: String,
        pub url: String
    }
}


impl From<o8_images::Envelope> for Envelope {
    fn from(e: o8_images::Envelope) -> Self {
        Envelope {
            body: e.body.into()
        }
    }
}


impl From<o8_images::Body> for Body {
    fn from(b: o8_images::Body) -> Self {
        Body {
            response: b.get_cikk_kepek_auth_response.into()
        }
    }
}


impl From<o8_images::GetCikkKepekAuthResponse> for GetProductImagesAuthResponse {
    fn from(r: o8_images::GetCikkKepekAuthResponse) -> Self {
        GetProductImagesAuthResponse {
            result: r.get_cikk_kepek_auth_result.into()
        }
    }
}


impl From<o8_images::GetCikkKepekAuthResult> for GetProductImagesAuthResult{
    fn from(r: o8_images::GetCikkKepekAuthResult) -> Self {
        Self {
            answer: r.valasz.into()
        }
    }
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


impl From<o8_images::Cikk> for Product {
    fn from(c: o8_images::Cikk) -> Self {
        Self {
            id: c.cikkid,
            no: c.cikkszam,
            images: c.kepek.into()
        }
    }
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
