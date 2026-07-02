// Defaults
use crate::forms::r#in::xml::defaults::CallData;

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
