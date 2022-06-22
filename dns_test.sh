#!/bin/env bash

dns_server=$1
domain=wikipedia.org

if [[ -z "${dns_server}" ]]; then
    echo "$0 <dns_server>"
    exit 0
fi

if [[ -n "$2" ]]; then
    domain=$2
fi

echo "use dns_server $dns_server"

ip=$(nslookup $domain $dns_server | awk '/Address/ {print $2}' | head -2 | tail -1)

echo "$domain => $ip"

curl -v --connect-timeout 6 https://$ip/
