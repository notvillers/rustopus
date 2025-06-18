curl -X POST "https://orink.hu/services/vision.asmx" \
  -H "Content-Type: text/xml; charset=utf-8" \
  -H "SOAPAction: \"https://orink.hu/services/GetCikkKepekAuth\"" \
  -d @cikk_request.xml \
  -o kepek.xml