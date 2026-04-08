// Get-Bulk RustOpus example

import { writeFile } from "fs/promises";

const AUTHCODE = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB";
const PID = 1234;
const OUTFILE = "get-bulk.xml";

const response = await fetch(`<url-to-rustopus>/get-images?authcode=${AUTHCODE}&pid=${PID}`);
const text = await response.text();

await writeFile(OUTFILE, text);
