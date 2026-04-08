// Get-Barcodes RustOpus example

import { writeFile } from "fs/promises";

const AUTHCODE = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB";
const OUTFILE = "get-barcodes.xml";

const response = await fetch(`<url-to-rustopus>/get-barcodes?authcode=${AUTHCODE}`);
const text = await response.text();

await writeFile(OUTFILE, text);
