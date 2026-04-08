// Get-Bulk RustOpus example
using System.Net.Http;

var authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB";
var pid = 1234;
var outFile = "get-bulk.xml";

using var client = new HttpClient();
var response = await client.GetStringAsync($"https://<url-to-rustopus>/get-bulk?authcode={authcode}&pid={pid}");
await File.WriteAllTextAsync(outFile, response);
