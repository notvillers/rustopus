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
from_date: str = datetime(2025, 1, 1, 0, 0, 0, tzinfo = timezone.utc).strftime(date_format)
to_date: str = datetime.now().strftime(date_format)
unpaid: int = 0
"""Returns only the unpaid invoices if value is `1`"""

request_url: str = (f"{url}/{get}"
                    f"?authcode={authcode}"
                    f"&pid={pid}"
                    f"&from_date={from_date}"
                    f"&to_date={to_date}"
                    f"&unpaid={unpaid}")

with request.urlopen(request_url) as response:
    with open(file = outfile,
              mode = "wb") as f:
        f.write(response.read())
