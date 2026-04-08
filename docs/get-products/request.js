// Get-Products RustOpus example

import { writeFile } from "fs/promises";

const AUTHCODE = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB";
const OUTFILE = "get-products.xml";

const response = await fetch(`<url-to-rustopus>/get-products?authcode=${AUTHCODE}`);
const text = await response.text();

await writeFile(OUTFILE, text);
