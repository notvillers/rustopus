'''
    Get-Prices RustOpus example
'''
from urllib import request

authcode: str = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
pid: int = 1234
outfile: str = "get-prices.xml"

url: str = f"<url-to-rustopus>/get-prices?authcode={authcode}&pid={pid}"

with request.urlopen(url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
