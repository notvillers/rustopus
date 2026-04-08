# Get-Bulk RustOpus example

$authcode = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
$partnerid = 1234 # PID
$outfile = "get-bulk.xml"

Invoke-WebRequest "<url-to-rustopus>/get-bulk?authcode=$authcode&pid=$partnerid" -OutFile "$outfile"
