// In macro(s)
macro_rules! O8ModelDeriveOnly {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}
pub(crate) use O8ModelDeriveOnly;


macro_rules! O8ModelLowercase {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
            #[serde(rename_all = "lowercase")]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}
pub(crate) use O8ModelLowercase;


macro_rules! O8ModelPascalcase {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(Debug, ::serde::Deserialize, ::serde::Serialize)]
            #[serde(rename_all = "PascalCase")]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}
pub(crate) use O8ModelPascalcase;
