curl -X POST "https://orink.hu/services/vision.asmx" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -H "SOAPAction: \"https://orink.hu/services/GetCikkekAuth\"" \
  -d @cikk_request.xml