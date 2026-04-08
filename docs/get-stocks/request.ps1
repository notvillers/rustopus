$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$outfile = "get-stocks.xml"

Invoke-WebRequest "<url-to-rustopus>/get-stocks?authcode=$authcode" -OutFile "$outfile"
