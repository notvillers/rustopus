use once_cell::sync::Lazy;
use crate::forms::r#in::xml::orders::{Address, Order};

pub struct Country {
    pub name: String,
    pub code: String,
    pub short_name: Option<String>,
    pub hu_name: String,
}


impl From<(&str, &str, Option<&str>, &str)> for Country {
    fn from((name, code, short_name, hu_name): (&str, &str, Option<&str>, &str)) -> Self {
        Self {
            name: name.to_string(),
            code: code.to_string(),
            short_name: short_name.map(str::to_string),
            hu_name: hu_name.to_string(),
        }
    }
}


pub fn get_countries() -> Vec<Country> {
    vec![
        ("Albania", "AL", None, "Albánia").into(),
        ("Algeria", "DZ", None, "Algéria").into(),
        ("Austria", "AT", None, "Ausztria").into(),
        ("Belarus", "BY", None, "Fehéroroszország").into(),
        ("Belgium", "BE", None, "Belgium").into(),
        ("Bosnia and Herzegovina", "BA", None, "Bosznia-Hercegovina").into(),
        ("Brazil", "BR", None, "Brazília").into(),
        ("Bulgaria", "BG", None, "Bulgária").into(),
        ("Cambodia", "KH", None, "Kambodzsa").into(),
        ("Croatia", "HR", None, "Horvátország").into(),
        ("Cyprus", "CY", None, "Ciprus").into(),
        ("Czech Republic", "CZ", Some("Czechia"), "Cseh Köztársaság").into(),
        ("Denmark", "DK", None, "Dánia").into(),
        ("Egypt", "EG", None, "Egyiptom").into(),
        ("Estonia", "EE", None, "Észtország").into(),
        ("EU", "EU", None, "Európai Közösség").into(),
        ("Faroe Islands", "FO", None, "Feröer szigetek").into(),
        ("Finland", "FI", None, "Finnország").into(),
        ("France", "FR", None, "Franciaország").into(),
        ("Germany", "DE", None, "Németország").into(),
        ("Greece", "GR", None, "Görögország").into(),
        ("Hongkong", "HK", Some("Hong Kong"), "Hongkong").into(),
        ("Hungary", "HU", None, "Magyarország").into(),
        ("India", "IN", None, "India").into(),
        ("Ireland", "IE", None, "Írország").into(),
        ("Israel", "IL", None, "Izrael").into(),
        ("Italy", "IT", None, "Olaszország").into(),
        ("Japan", "JP", None, "Japán").into(),
        ("Kenya", "KE", None, "Kenya").into(),
        ("Kosovo", "XK", None, "Koszovó").into(),
        ("Latvia", "LV", None, "Lettország").into(),
        ("Libanon", "LB", Some("Lebanon"), "Libanon").into(),
        ("Lithuania", "LT", None, "Litvánia").into(),
        ("Luxembourg", "LU", None, "Luxemburg").into(),
        ("Macao", "MO", None, "Macao").into(),
        ("Macedonia", "MK", None, "Macedónia").into(),
        ("Malaysia", "MY", None, "Malajzia").into(),
        ("Malta", "MT", None, "Málta").into(),
        ("Mexico", "MX", None, "Mexikó").into(),
        ("Moldova", "MD", None, "Moldova").into(),
        ("Montenegro", "ME", None, "Montenegró").into(),
        ("Morocco", "MA", None, "Marokkó").into(),
        ("Netherlands", "NL", None, "Hollandia").into(),
        ("Nigeria", "NG", None, "Nigéria").into(),
        ("Norway", "NO", None, "Norvégia").into(),
        ("Peoples Republic of China", "CN", Some("China"), "Kínai Népköztársaság").into(),
        ("Peru", "PE", None, "Peru").into(),
        ("Poland", "PL", None, "Lengyelország").into(),
        ("Portugal", "PT", None, "Portugália").into(),
        ("Republic of Korea", "KR", Some("South Korea"), "Koreai Köztársaság").into(),
        ("Republic of South Africa", "ZA", Some("South Africa"), "Dél-afrikai Köztársaság").into(),
        ("Republik Indonesia", "ID", Some("Indonesia"), "Indonézia").into(),
        ("Romania", "RO", None, "Románia").into(),
        ("Ruanda", "RW", Some("Rwanda"), "Ruanda").into(),
        ("Russia", "RU", None, "Oroszország").into(),
        ("Samoa", "WS", None, "Szamoa").into(),
        ("Serbia", "RS", None, "Szerbia").into(),
        ("Singapore", "SG", None, "Szingapúr").into(),
        ("Slovakia", "SK", None, "Szlovákia").into(),
        ("Slovenia", "SI", None, "Szlovénia").into(),
        ("Spain", "ES", None, "Spanyolország").into(),
        ("Sweden", "SE", None, "Svédország").into(),
        ("Switzerland", "CH", None, "Svájc").into(),
        ("Taiwan", "TW", None, "Tajvan").into(),
        ("Tanzania", "TZ", None, "Tanzania").into(),
        ("Thailand", "TH", None, "Thailand").into(),
        ("Tunesia", "TN", Some("Tunisia"), "Tunézia").into(),
        ("Turkey", "TR", None, "Törökország").into(),
        ("Uganda", "UG", None, "Uganda").into(),
        ("Ukraine", "UA", None, "Ukrajna").into(),
        ("United Arab Emirates", "AE", Some("UAE"), "Egyesült Arab Emírségek").into(),
        ("United Kingdom", "GB", Some("UK"), "Egyesült Királyság").into(),
        ("United States", "US", Some("USA"), "Amerikai Egyesült Államok").into(),
        ("Vietnam", "VN", None, "Vietnam").into(),
        ("Western Samoa", "WS", None, "Nyugat-Szamoa").into(),
    ]
}


pub static COUNTRIES: Lazy<Vec<Country>> = Lazy::new(get_countries);

pub fn country_to_hu(mut address: Address) -> Address {
    let Some(original) = address.country.as_deref() else {
        return address;
    };
    let original = original.trim();

    for country in COUNTRIES.iter() {
        let matches = country.name.eq_ignore_ascii_case(original)
            || country.code.eq_ignore_ascii_case(original)
            || country
                .short_name
                .as_deref()
                .is_some_and(|short| short.eq_ignore_ascii_case(original));

        if matches {
            address.country = Some(country.hu_name.clone());
            break;
        }
    }

    address
}


pub fn order_country_to_hu(mut order: Order) -> Order {
    order.header.delivery_address = order.header.delivery_address.map(country_to_hu);
    order.header.invoice_address = order.header.invoice_address.map(country_to_hu);
    order
}
