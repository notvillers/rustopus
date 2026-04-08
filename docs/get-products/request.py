'''
    Get-Products request for Rustopus
'''
from urllib import request

authcode: str = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
outfile: str = "get-products.xml"

url: str = f"<url-to-rustopus>/get-products?authcode={authcode}"

with request.urlopen(url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
