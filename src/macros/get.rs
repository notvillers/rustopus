/// Out macro(s)
macro_rules! get_models {
    ($(
        $(#[$extra:meta])*
        $vis:vis enum $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(::serde::Serialize)]
            #[serde(untagged)]
            $(#[$extra])*
            $vis enum $name { $($body)* }
        )*
    };
}

pub(crate) use get_models;
