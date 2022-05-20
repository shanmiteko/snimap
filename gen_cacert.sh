#!/usr/bin/env bash
set -eu
cert_dir=private
name=disable_sni_reverse_proxy_root_ca

mkdir -p "$cert_dir"

openssl genpkey -algorithm RSA -out "$cert_dir/cakey.pem"

openssl req -x509 -key "$cert_dir/cakey.pem" -out "$cert_dir/ca.pem" \
    -days 3650 \
    -subj "/CN=$name" \
    -config <(
        cat <<END
[ req ]
distinguished_name  = req_distinguished_name

[ req_distinguished_name ]

[ x509_ext ]
basicConstraints = critical,CA:true
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
END
    ) -extensions x509_ext
