#!/bin/bash
curl --url "smtp://10.149.181.121:587" --mail-from "noreply@3tched.com" --mail-rcpt "test@example.com" \
  --user "jeremy@3tched.com:jeremy123" -T <(echo -e "From: noreply@3tched.com\nTo: test@example.com\nSubject: Test\n\nTest body")
