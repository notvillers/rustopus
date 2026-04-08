# Get-Barcodes RustOpus example

$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$outfile = "barcodes.xml"

Invoke-WebRequest "<url-to-rustopus>/get-barcodes?authcode=$authcode" -OutFile "$outfile"
