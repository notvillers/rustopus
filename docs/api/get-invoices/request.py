'''
    Get-Invoices RustOpus example
'''

from datetime import datetime, timezone
from urllib import request

# - - - - - -
# PARAMETERS
# - - - - - -
url: str = "<rustopus_url_here>"
get: str = "get-invoices"
authcode: str = "AAAAAAAA-0000-0000-0000-BBBBBBBBBBBB"
pid: int = 1234
outfile: str = "get-invoices.xml"
date_format: str = "%Y-%m-%dT%H:%M:%SZ"
f_date: str = datetime(2025, 1, 1, 0, 0, 0, tzinfo = timezone.utc).strftime(date_format)
t_date: str = datetime.now().strftime(date_format)
unpaid: int = 0 # if 1 then only returns unpaid invoices

request_url: str = (f"{url}/{get}"
                    f"?authcode={authcode}"
                    f"&pid={pid}"
                    f"&from_date={f_date}"
                    f"&to_date={t_date}"
                    f"&unpaid={unpaid}")

with request.urlopen(request_url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
