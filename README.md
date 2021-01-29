# Obnam &ndash; a backup system

Obnam2 is a project to develop a backup system.

You probably want to read the [obnam.md](obnam.md) subplot file.

## Client installation

See instructions at <https://obnam.org/download/> for installing the
client. It's not duplicated here to avoid having to keep the
information in sync in two places.

## Server installation

To install the Obnam server component, you need a Debian host with
sufficient disk space, and Ansible installed locally. Run the
following commands in the Obnam source tree, replacing
`obnam.example.com` with the domain name of your server:

```sh
$ cd ansible
$ printf '[server]\nobnam.example.com\n' > hosts
$ ansible-playbook -i hosts obnam-server.yml -e domain=obnam.example.com
```

The above gets a free TLS certificate from [Let's Encrypt][], but only
works if the server is accessible from the public Internet. For a
private host use the following instead:

```sh
$ cd ansible
$ printf '[server]\nprivate-vm\n' > hosts
$ ansible-playbook -i hosts obnam-server.yml
```

This uses a pre-created self-signed certificate from
`files/server.key` and `files/server.pem` and is probably only good
for trying out Obnam. You may want to generate your own certificates
instead.

To create a self-signed certificate, something like the following
command might work, using [OpenSSL]:

```sh
$ openssl req -x509 -newkey rsa:4096 -passout pass:hunter2 \
  -keyout key.pem -out cert.pem -days 365 -subj /CN=localhost
```


[Let's Encrypt]: https://letsencrypt.org/
[OpenSSL]: https://www.openssl.org/


## Legalese

Copyright 2020-2021  Lars Wirzenius

This program is free software: you can redistribute it and/or modify
it under the terms of the GNU General Public License as published by
the Free Software Foundation, either version 3 of the License, or
(at your option) any later version.

This program is distributed in the hope that it will be useful,
but WITHOUT ANY WARRANTY; without even the implied warranty of
MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
GNU General Public License for more details.

You should have received a copy of the GNU General Public License
along with this program. If not, see <http://www.gnu.org/licenses/>.
