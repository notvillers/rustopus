// Bulk hungarian struct(s) from XML(s) got from the Octopus calls
use crate::{
    forms::{
        r#in::xml::{
            defaults::Hiba
        },
        out::xml::{
            defaults::Error,
            products::Size,
            bulk
        }
    }, macros::out::OutModelDeriveOnly
};

OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }

    pub struct Body {
        pub response: Response
    }

    pub struct Response {
        pub result: Result
    }

    pub struct Result {
        pub valasz: Valasz
    }

    pub struct Valasz {
        #[serde(rename = "@verzio")]
        pub verzio: String,
        pub cikkek: Cikkek,
        pub hiba: Vec<Hiba>
    }

    pub struct Cikkek {
        pub cikk: Vec<Cikk>
    }

    pub struct Cikk {
        pub cikkid: u64,
        pub cikkszam: String,
        pub cikknev: String,
        pub me: String,
        pub alapme: String,
        pub alapmenny: Option<f64>,
        pub gyarto: String,
        pub gycikkszam: String,
        pub cikkcsoportkod: String,
        pub cikkcsoportnev: String,
        pub leiras: String,
        pub tomeg: Option<f64>,
        pub meret: Option<Meret>,
        pub focsoportkod: String,
        pub focsoportnev: String,
        pub ertmenny: Option<f64>,
        pub szarmorszag: String,
        pub ar: Option<f64>,
        pub dnem: Option<String>,
        pub keszlet: Option<f64>,
        pub ean: Option<String>,
        pub kepek: Kepek
    }

    pub struct Meret {
        pub x: Option<f64>,
        pub y: Option<f64>,
        pub z: Option<f64>
    }

    pub struct Kepek {
        pub kep: Vec<Kep>
    }

    pub struct Kep {
        pub galleria: String,
        pub url: String
    }
}


impl From<bulk::Envelope> for Envelope {
    fn from(e: bulk::Envelope) -> Self {
        Self {
            body: e.body.into()
        }
    }
}


impl From<bulk::Body> for Body {
    fn from(b: bulk::Body) -> Self {
        Self {
            response: b.response.into()
        }
    }
}


impl From<bulk::Response> for Response {
    fn from(r: bulk::Response) -> Self {
        Self {
            result: r.result.into()
        }
    }
}


impl From<bulk::Result> for Result {
    fn from(r: bulk::Result) -> Self {
        Self {
            valasz: r.answer.into()
        }
    }
}


impl From<bulk::Answer> for Valasz {
    fn from(a: bulk::Answer) -> Self {
        Self {
            verzio: a.version,
            cikkek: a.products.into(),
            hiba: a.error
                    .into_iter()
                    .map(Into::into)
                    .collect()
        }
    }
}


impl From<Error> for Hiba {
    fn from(e: Error) -> Self {
        Self {
            kod: e.code,
            leiras: e.description
        }
    }
}


impl From<bulk::Products> for Cikkek {
    fn from(p: bulk::Products) -> Self {
        Self {
            cikk: p.product
                .into_iter()
                .map(Into::into)
                .collect()
        }
    }
}


impl From<bulk::Product> for Cikk {
    fn from(p: bulk::Product) -> Self {
        Self {
            cikkid: p.id,
            cikkszam: p.no,
            cikknev: p.name,
            me: p.unit,
            alapme: p.base_unit,
            alapmenny: p.base_unit_qty,
            gyarto: p.brand,
            gycikkszam: p.oem_code,
            cikkcsoportkod: p.category_code,
            cikkcsoportnev: p.category_name,
            leiras: p.description,
            tomeg: p.weight,
            meret: p.size.map(Into::into),
            focsoportkod: p.main_category_code,
            focsoportnev: p.main_category_name,
            ertmenny: p.sell_unit,
            szarmorszag: p.origin_country,
            ar: p.price,
            dnem: p.currency,
            keszlet: p.stock,
            ean: p.ean,
            kepek: p.images.into()
        }
    }
}


impl From<Size> for Meret {
    fn from(s: Size) -> Self {
        Self {
            x: s.x,
            y: s.y,
            z: s.z
        }
    }
}


impl From<bulk::Images> for Kepek {
    fn from(i: bulk::Images) -> Self {
        Self {
            kep: i.image
                .into_iter()
                .map(Into::into)
                .collect()
        }
    }
}


impl From<bulk::Image> for Kep {
    fn from(i: bulk::Image) -> Self {
        Self {
            galleria: i.gallery,
            url: i.url
        }
    }
}
