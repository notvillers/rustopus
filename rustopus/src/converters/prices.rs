use crate::o8_xml;
use crate::partner_xml;
use crate::service::soap;
use quick_xml;


pub async fn get_prices(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> String {
    let hu_prices_xml = get_prices_xml(url, xmlns, pid, authcode).await;
    let hu_envelope = get_prices_envelope(&hu_prices_xml);
    match hu_envelope {
        Ok(hu_envelope) => {
            convert_prices_envelope_to_xml(hu_envelope)
        }
        Err(e) => {
            println!("Get prices: error {}", e);
            "<Envelope></Envelope>".to_string()
        }
    }
}


fn get_prices_request_string(xmlns: &str, authcode: &str, pid: &i64) -> String {
    format!(r#"<?xml version="1.0" encoding="utf-8"?>
<soap:Envelope xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:soap="http://schemas.xmlsoap.org/soap/envelope/">
  <soap:Body>
    <GetArlistaAuth xmlns="{}">
      <pid>{}</pid>
      <partnerkod>{}</partnerkod>
      <authcode>{}</authcode>
    </GetArlistaAuth>
  </soap:Body>
</soap:Envelope>"#,
        xmlns,
        pid,
        "",
        authcode)
}


async fn get_prices_xml(url: &str, xmlns: &str, pid: &i64, authcode: &str) -> String {
    let soap_request = get_prices_request_string(xmlns, authcode, pid);
    soap::get_response(url, soap_request).await
}


fn get_prices_envelope(response_text: &str) -> Result<o8_xml::prices::Envelope, quick_xml::DeError> {
    quick_xml::de::from_str(response_text)
}


fn convert_prices_envelope_to_xml(hu_envelope: o8_xml::prices::Envelope) -> String {
    let en_envelope: partner_xml::prices::Envelope = hu_envelope.into();
    let eng_xml = quick_xml::se::to_string(&en_envelope);

    match eng_xml {
        Ok(eng_xml) => {
            eng_xml
        }
        Err(e) => {
            println!("Convert prices error: {}", e);
            "<Envelope></Envelope>".to_string()
        }
    }
}