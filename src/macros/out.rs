// Out macro(s)
macro_rules! out_models {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(::serde::Serialize)]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}

pub(crate) use out_models;