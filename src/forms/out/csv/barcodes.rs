// Barcodes CSV
use crate::{
    macros::out::OutModelDeriveSerializeOnly,
    forms::r#in::xml::barcode as o8_barcode
};

OutModelDeriveSerializeOnly! {
    pub struct Barcode {
        pub id: u64,
        pub no: String,
        pub ean: String,
        pub unit: String,
        pub main_ean: bool
    }

    pub struct Barcodes {
        pub barcodes: Vec<Barcode>
    }
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


impl From<o8_barcode::Envelope> for Barcodes {
    fn from(e: o8_barcode::Envelope) -> Self {
        Self {
            barcodes: e.body.get_vonalkodok_auth_response.get_vonalkodok_auth_result.valasz.vonalkodok.vonalkod
                .into_iter()
                .map(|x| x.into())
                .collect()
        }
    }
}
