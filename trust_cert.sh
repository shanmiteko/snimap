#!/usr/bin/env bash
set -e

function x509_label() {
    openssl x509 -in "$1" -subject | head -n 1 | awk '{print $(NF)}'
}

case "$1" in
"-r" | "--remove")
    trust list | grep -C 2 $(x509_label $2)
    sudo trust anchor -v --remove "$2"
    ;;
"-s" | "--show")
    openssl x509 -in "$2" -text
    ;;
*)
    sudo trust anchor -v "$1"
    trust list | grep -C 2 $(x509_label $1)
    ;;
esac
