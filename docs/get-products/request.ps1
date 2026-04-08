$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$outfile = "get-products.xml"

Invoke-WebRequest "<url-to-rustopus>/get-products?authcode=$authcode" -OutFile "$outfile"
