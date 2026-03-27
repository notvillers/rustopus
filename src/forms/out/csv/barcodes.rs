use serde::Serialize;
use crate::forms::r#in::xml::barcode as o8_barcode;

#[derive(Serialize)]
pub struct Barcode {
    pub id: u64,
    pub no: String,
    pub ean: String,
    pub unit: String,
    pub main_ean: bool
}

impl From<o8_barcode::Vonalkod> for Barcode {
    fn from(c: o8_barcode::Vonalkod) -> Self {
        Self {
            id: c.cikkid,
            no: c.cikkszam,
            ean: c.vonalkod,
            unit: c.me,
            main_ean: c.elsean == 1
        }
    }
}


#[derive(Serialize)]
pub struct Barcodes {
    pub barcodes: Vec<Barcode>
}

impl From<o8_barcode::Envelope> for Barcodes {
    fn from(e: o8_barcode::Envelope) -> Self {
        let barcodes = e.body.get_vonalkodok_auth_response.get_vonalkodok_auth_result.valasz.vonalkodok.vonalkod;
        Self {
            barcodes: barcodes
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}
