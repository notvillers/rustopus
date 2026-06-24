use crate::{
    macros::out::OutModelDeriveOnly,
    forms::r#in::xml::orders_response as p_orders_response
};

OutModelDeriveOnly! {
    pub struct Envelope {
        pub body: Body
    }
    
    pub struct Body {
        pub response: OrderResponse
    }

    pub struct OrderResponse {
        pub result: OrderResult
    }
    
    pub struct OrderResult {
        pub answer: Answer
    }

    pub struct Answer {
        #[serde(rename = "@version")]
        pub version: Option<String>,
        pub header: OrderResponseHeader,
        #[serde(default)]
        pub items: OrderResponseItems,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extra_items: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub shipping_cost: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub cash_on_delivery: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub extra_services: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub return_fee: Option<String>,
    }
    
    pub struct OrderResponseHeader {
        pub identifier: String,
        pub web_identifier: String,
        pub document_number: String,
        pub delivery_date: String,
    }

    pub struct OrderResponseItems {
        #[serde(rename = "item", default)]
        pub item: Vec<OrderResponseItem>,
    }

    pub struct OrderResponseItem {
        pub item_number: String,
        pub recorded_item_number: String,
        pub product_number: String,
        pub quantity: OrderResponseQuantity,
        pub unit_price_net: String,
        pub unit_price_gross: String,
        pub value_net: String,
        pub value_gross: String,
        pub currency: String,
    }

    #[derive(Clone)]
    pub struct OrderResponseQuantity {
        #[serde(rename = "$value")]
        pub value: String,
        #[serde(rename = "@type")]
        pub type_indicator: String,
        #[serde(rename = "@coverage")]
        pub coverage: String,
        #[serde(rename = "@date")]
        pub date: String,
    }
}


impl From<p_orders_response::Envelope> for Envelope {
    fn from(e: p_orders_response::Envelope) -> Self {
        Self {
            body: e.body.into()
        }
    }
}


impl From<p_orders_response::Body> for Body {
    fn from(b: p_orders_response::Body) -> Self {
        Self {
            response: b.rendeles_feladas_auth_response.into()
        }
    }
}


impl From<p_orders_response::RendelesFeladasAuthResponse> for OrderResponse {
    fn from(r: p_orders_response::RendelesFeladasAuthResponse) -> Self {
        Self {
            result: r.rendeles_feladas_auth_result.into()
        }
    }
}


impl From<p_orders_response::RendelesFeladasAuthResult> for OrderResult {
    fn from(r: p_orders_response::RendelesFeladasAuthResult) -> Self {
        Self {
            answer: r.valasz.into()
        }
    }
}


impl From<p_orders_response::Valasz> for Answer {
    fn from(v: p_orders_response::Valasz) -> Self {
        Self {
            version: v.verzio,
            header: v.fej.into(),
            items: v.tetelek.into(),
            extra_items: v.extratetelek,
            shipping_cost: v.fuvarkoltseg,
            cash_on_delivery: v.utanvet,
            extra_services: v.extraszolg,
            return_fee: v.visszavaltasi_dij,
        }
    }
}


impl From<p_orders_response::ValaszFej> for OrderResponseHeader {
    fn from(fej: p_orders_response::ValaszFej) -> Self {
        Self {
            identifier: fej.azonosito,
            web_identifier: fej.webazon,
            document_number: fej.bizonylatszam,
            delivery_date: fej.szalldatum,
        }
    }
}


impl From<p_orders_response::ValaszTetelek> for OrderResponseItems {
    fn from(tetelek: p_orders_response::ValaszTetelek) -> Self {
        Self {
            item: tetelek.tetel.into_iter().map(|t| t.into()).collect(),
        }
    }
}


impl From<p_orders_response::ValaszTetel> for OrderResponseItem {
    fn from(tetel: p_orders_response::ValaszTetel) -> Self {
        Self {
            item_number: tetel.tetelszam,
            recorded_item_number: tetel.rogzitett_tetelszam,
            product_number: tetel.cikkszam,
            quantity: tetel.mennyiseg.into(),
            unit_price_net: tetel.egysegar,
            unit_price_gross: tetel.bregysegar,
            value_net: tetel.ertek,
            value_gross: tetel.brertek,
            currency: tetel.dnem,
        }
    }
}


impl From<p_orders_response::ValaszMennyiseg> for OrderResponseQuantity {
    fn from(mennyiseg: p_orders_response::ValaszMennyiseg) -> Self {
        Self {
            value: mennyiseg.value,
            type_indicator: mennyiseg.tipus,
            coverage: mennyiseg.kenocs,
            date: mennyiseg.datum,
        }
    }
}
