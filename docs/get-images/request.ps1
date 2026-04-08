# Get-Images RustOpus example

$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$outfile = "get-images.xml"

Invoke-WebRequest "<url-to-rustopus>/get-images?authcode=$authcode" -OutFile "$outfile"
