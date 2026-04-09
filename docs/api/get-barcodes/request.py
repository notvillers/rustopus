'''
    Get-Barcodes RustOpus example
'''

from urllib import request

# - - - - - -
# PARAMETERS
# - - - - - -
url: str = "<rustopus_url_here>"
get: str = "get-barcodes"
authcode: str = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
outfile: str = "get-barcodes.xml"

request_url: str = (f"{url}/{get}"
                    f"?authcode={authcode}")

with request.urlopen(url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
