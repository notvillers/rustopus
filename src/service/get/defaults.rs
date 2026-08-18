// Defaults
use crate::{
    forms::r#in::xml::defaults::{CallData, Hiba},
    service::log::elogger
};


pub enum ReturnType {
    Xml,
    XmlHu,
    Csv,
    Xlsx
}


pub fn get_return_type(call_data: CallData) -> ReturnType {
    match (call_data.is_xlsx(), call_data.is_csv(), call_data.is_hu()) {
        (true, _, _) => ReturnType::Xlsx,
        (_, true, _) => ReturnType::Csv,
        (_, _, true) => ReturnType::XmlHu,
        _ => ReturnType::Xml
    }
}


pub fn check_return_type(call_data: CallData, error: Option<Hiba>, name: &str) -> ReturnType {
    let mut return_type = get_return_type(call_data.clone());
    if let Some(hiba) = &error {
        elogger(format!("{} | {}: Octopus error in {} envelope: '{} - {}'", call_data.pid.unwrap_or_default(), call_data.authcode, name, hiba.kod, hiba.leiras));
        if !matches!(return_type, ReturnType::Xml | ReturnType::XmlHu) {
            return_type = match call_data.is_hu() {
                true => ReturnType::XmlHu,
                _ => ReturnType::Xml
            }
        }
    }
    return_type
}
