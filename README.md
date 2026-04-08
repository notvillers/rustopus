# RustOpus

Converts Octopus 8 XML data to a new XML format with english tags and error codes (if possible).

## Configuration
[Config.toml](Config.toml)

Manages the defaults of the webserver

- server
    - `host`: Hostname, `"0.0.0.0"` preferred to be able to connect from outside. Defaults to `"0.0.0.0"`
    - `port`: Server port where the webapp should be available from. Defaults to `8080`
    - `timeout`: Timeout limit in second(s). Defaults to `1200`
    - `workers`: Number of workers available for the webapp, the higher, the faster. Defaults to `std::thread::available_parallelism()`


Soap.json

Manages the defaults of the xml handling.

If the file exists in the repository [root](/) directory, it searches for an "url" tag, if its given, then this will be the default for the gets and posts used for url and xmlns.

- `"url"`: Default wsdl url

## Client documentation

- [Docs](./docs/)
