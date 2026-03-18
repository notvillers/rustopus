use crate::o8_xml;
use crate::partner_xml;

#[derive(serde::Serialize)]
pub enum ProductEnvelope {
    Hu(o8_xml::products::Envelope),
    En(partner_xml::products::Envelope)
}
