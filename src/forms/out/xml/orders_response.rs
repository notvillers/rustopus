use crate::{
    macros::out::OutModelDeriveOnly,
    forms::{
        r#in::xml::orders_response as p_orders_response,
        out::xml::defaults as p_defaults
    }
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
        // Absent when Octopus refused the order: no header means nothing was
        // recorded, which an empty `<header>` would only blur.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub header: Option<OrderResponseHeader>,
        #[serde(default)]
        pub items: OrderResponseItems,
        // Charges Octopus added to the order (delivery, deposit, …), each an
        // empty element when there is none — as they always were.
        pub extra_items: OrderCharges,
        pub shipping_cost: OrderCharges,
        pub cash_on_delivery: OrderCharges,
        pub extra_services: OrderCharges,
        pub return_fee: OrderCharges,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub error: Option<p_defaults::Error>,
    }
    
    pub struct OrderResponseHeader {
        pub identifier: String,
        pub web_identifier: String,
        pub document_number: String,
        pub delivery_date: String,
    }

    pub struct OrderCharges {
        #[serde(rename = "item", default)]
        pub item: Vec<OrderCharge>,
    }

    pub struct OrderCharge {
        #[serde(skip_serializing_if = "Option::is_none")]
        pub recorded_item_number: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub product_id: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub product_number: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub product_name: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub quantity: Option<OrderResponseQuantity>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub unit_price_net: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub unit_price_gross: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value_net: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value_gross: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
        /// On a deposit line: the recorded item number it is charged for.
        #[serde(skip_serializing_if = "Option::is_none")]
        pub parent_item_number: Option<String>,
    }

    pub struct OrderResponseItems {
        #[serde(rename = "item", default)]
        pub item: Vec<OrderResponseItem>,
    }

    // Prices and the date are absent on a line Octopus did not record
    // (`recorded_item_number` 0, quantity `type` 4).
    pub struct OrderResponseItem {
        pub item_number: String,
        pub recorded_item_number: String,
        pub product_number: String,
        pub quantity: OrderResponseQuantity,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub unit_price_net: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub unit_price_gross: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value_net: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub value_gross: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        pub currency: Option<String>,
    }

    #[derive(Clone)]
    pub struct OrderResponseQuantity {
        #[serde(rename = "$value")]
        pub value: String,
        #[serde(rename = "@type", skip_serializing_if = "Option::is_none")]
        pub type_indicator: Option<String>,
        #[serde(rename = "@coverage", skip_serializing_if = "Option::is_none")]
        pub coverage: Option<String>,
        #[serde(rename = "@date", skip_serializing_if = "Option::is_none")]
        pub date: Option<String>,
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
            // When no line was recorded Octopus still sends a `<fej>`, but
            // with every identifier empty: that is "no order", not a header.
            header: v.fej
                .filter(|fej| !fej.azonosito.trim().is_empty())
                .map(Into::into),
            items: v.tetelek.into(),
            extra_items: v.extratetelek.into(),
            shipping_cost: v.fuvarkoltseg.into(),
            cash_on_delivery: v.utanvet.into(),
            extra_services: v.extraszolg.into(),
            return_fee: v.visszavaltasi_dij.into(),
            error: v.hiba.map(Into::into),
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


impl From<p_orders_response::ValaszDijak> for OrderCharges {
    fn from(dijak: p_orders_response::ValaszDijak) -> Self {
        Self {
            item: dijak.tetel.into_iter().map(Into::into).collect(),
        }
    }
}


impl From<p_orders_response::ValaszDij> for OrderCharge {
    fn from(dij: p_orders_response::ValaszDij) -> Self {
        Self {
            recorded_item_number: dij.rogzitett_tetelszam,
            product_id: dij.cikkid,
            product_number: dij.cikkszam,
            product_name: dij.cikknev,
            quantity: dij.mennyiseg.map(Into::into),
            unit_price_net: dij.egysegar,
            unit_price_gross: dij.bregysegar,
            value_net: dij.ertek,
            value_gross: dij.brertek,
            currency: dij.dnem,
            parent_item_number: dij.vcapatszam,
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
