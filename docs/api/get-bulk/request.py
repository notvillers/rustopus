'''
    Get-Bulk RustOpus example
'''

from urllib import request

# - - - - - -
# PARAMETERS
# - - - - - -
url: str = "<rustopus_url_here>"
get: str = "get-bulk"
authcode: str = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
pid: int = 1234
outfile: str = "get-bulk.xml"

request_url: str = (f"{url}/{get}"
                    f"?authcode={authcode}"
                    f"pid={pid}")


with request.urlopen(url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
