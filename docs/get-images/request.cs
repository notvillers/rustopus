// Get-Images RustOpus example
using System.Net.Http;

var authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB";
var outFile = "get-images.xml";

using var client = new HttpClient();
var response = await client.GetStringAsync($"https://<url-to-rustopus>/get-images?authcode={authcode}");
await File.WriteAllTextAsync(outFile, response);
