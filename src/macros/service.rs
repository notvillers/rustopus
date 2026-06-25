// Service macro(s)
macro_rules! ConfigModelDerive {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(Debug, ::serde::Deserialize)]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}

pub(crate) use ConfigModelDerive;
