# Get-Prices RustOpus example

$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$partnerid = 1234 # pid
$outfile = "get-prices.xml"

Invoke-WebRequest "<url-to-rustopus>/get-prices?authcode=$authcode&pid=$partnerid" -OutFile "$outfile"
