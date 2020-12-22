#!/bin/sh
#
# Install the Obnam Debian package from http://ci-prod-controller.vm.liw.fi:8080/debian/

set -eu

install_sources_list()
{
    cat > /etc/apt/sources.list.d/obnam.list <<EOF
deb [arch=amd64] http://ci-prod-controller.vm.liw.fi/debian unstable-ci main
EOF
}

install_signing_key()
{
    cat > /etc/apt/trusted.gpg.d/obnam.asc <<EOF
-----BEGIN PGP PUBLIC KEY BLOCK-----

mQINBFrLO7kBEADdz6mHstYmKU5Dp6OSjxWtWaqTDOX1sJdmmaIK/9EKVIH0Maxp
5kvVO5G6mULLAjv/kLG0MxasHPrq8I2A/y8AqKAGVL8QelwLjQMIFZ30/VbGQPHS
+T5TZXEnoQtNce1GUhFwJ38ZyjjwHBFV9tSec7rZ2Q3YeM3nNnGPf6DacXGfEOPO
HIN4sXAN2hzNXNjKRzTIvxQseb6nr7afUh/SlZ3yhQOCrIzmYlD7tP9WJe7ofL0p
JY4pDQYw8rT6nC2BE/ioemh84kERCT1vCe+OVFlSRuMlqfEv+ZpKQ+itOmPDQ/lM
jpUm1K2hrW/lWpxT/ZxHKo/w1K36J5WshgMZxfUu5BMCL9LMqMcrXNhNjDMfxDMM
3yBPOvQ4ls6fecOZ/bsFo1p8VzMk/w/eG8vPs5yuNa5XxN95yFMXoOHGb5Xbu8D4
6yiW+Af70LbiSNpGdmNdneiGB2fY38NxBukPw5u3S5qG8HedSmMr1RvSr5kHoAAe
UbOY+BYaaKsTAT7+1skUW1o3FJSqoRKCHAzTsMWC6zzhR8hRn7jVrrguH1hGbqq5
TZSCFQZExuTJ7uXrTLG0WoBXIjB5wWNcSeXn8myUWYB51nJNF4tJBouZOz9JwWGl
kiAQkrHnBttLQWdW9FyjbIoTZMtpvVx+m6ObGTGdGL1cNlLAvWprMXGc+QARAQAB
tDJJY2sgQVBUIHJlcG9zaXRvcnkgc2lnbmluZyBrZXkgKDIwMTgpIDxsaXdAbGl3
LmZpPokCTgQTAQgAOBYhBKL1uyDoXyxUH3O717Wr+TZVS6PGBQJayzu5AhsDBQsJ
CAcCBhUICQoLAgQWAgMBAh4BAheAAAoJELWr+TZVS6PGB5QQANTcikhRUHwt9N4h
dGc/Hp6CbqdshMoWlwpFskttoVDxQG5OAobuZl5XyzGcmja1lT85RGkZFfbca0IZ
LnXOLLSAu51QBkXNaj4OhjK/0uQ+ITrvL6RQSXNgHiUTR/W2XD1GIUq6nBqe2GSN
31S1baYKKVj5QIMsi7Dq8ls3BBXuPCE+xTSaNmGWjes2t9pPidcRvxsksCLY1qgw
P1GFXBeMkBQ29kBP87SUL15SIk7OiQLlEURCy5iRls5rt/YEsdEpRWIb0Tm5Nrjv
2M3VM+iBhfNXTwj0rJ34mlycF1qQmA7YcTEobT7z587GPY0VWzBpQUnEQj7rQWPM
cDYY0b+I6kQ8VKOaL4wVAtE98d7HzFIrIrwhTKufnrWrVDPYsmLZ+LPC1jiF7JBD
SR6Vftb+SdDR9xoE1yRuXbC6IfoW+5/qQNrdQ2mm9BFw5jOonBqchs18HTTf3441
6SWwP9fY3Vi+IZphPPi0Gf85oMStgnv/Wnw6LacEL32ek39Desero/D8iGLZernK
Q2mC9mua5A/bYGVhsNWyURNFkKdbFa+/wW3NfdKYyZnsSfo+jJ2luNewrhAY7Kod
GWXTer9RxzTGA3EXFGvNr+BBOOxSj0SfWTl0Olo7J5dnxof+jLAUS1VHpceHGHps
GSJSdir7NkZidgwoCPA7BTqsb5LN
=dXB0
-----END PGP PUBLIC KEY BLOCK-----
EOF
}

main()
{
    install_signing_key
    install_sources_list
    apt update
    apt install -y --no-remove obnam
}

main
