// MCP macro(s)

/// Tool-argument models. MCP publishes a JSON Schema for every tool, so these
/// need `JsonSchema` on top of the usual `Deserialize` — the one derive set in
/// this repo that the `in`/`out` macros do not cover.
macro_rules! McpToolArgs {
    ($(
        $(#[$extra:meta])*
        $vis:vis struct $name:ident { $($body:tt)* }
    )*) => {
        $(
            #[derive(Debug, ::serde::Deserialize, ::rmcp::schemars::JsonSchema)]
            // `schemars` is reached through rmcp's re-export rather than a direct
            // dependency, so the generated code has to be told where to find it.
            #[schemars(crate = "::rmcp::schemars")]
            $(#[$extra])*
            $vis struct $name { $($body)* }
        )*
    };
}

pub(crate) use McpToolArgs;
