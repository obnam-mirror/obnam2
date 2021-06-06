---
title: Release notes for Obnam2
...

This file summarizes changes between releases of the second generation
of Obnam, the backup software. The software is technically called
"obnam2" to distinguish it from the first generation of Obnam, which
ended in 2017 with version number 1.22.


# Version 0.4.0, released 2021-06-06

## Experimental version

This is an experimental release, and is not meant to be relied on for
recovery of important data. The purpose of this release is to get new
features into the hands of intrepid people who want to try out new
things.

## Breaking changes

This release introduces use of encryption in Subplot. Encryption is
not optional, and the new `obnam init` command must always be used
before the first backup to generate an encryption key.

Starting with this version of Obnam, there is no support at all for
cleartext backups any more. A backup, or backup repository, made with
a previous version of Obnam **will not work** with this version: you
can't list backups in a repository, you can't restore a backup, and
you can't make a new backup. You need to start over from scratch, by
emptying the server's chunk directory. Eventually, Obnam will stop
having such breaking, throw-away-everything changes, but it will
take time to build that functionality.

Note: this version add only a very rudimentary approach to encryption.
It is only meant to protect the backups from the server operator
snooping via the server file system. It doesn't protect against most
other threats, including the server operator replacing parts of
backups on the server. Future versions of Obnam will add more
protection.

## New or changed features

* Obnam now by default excludes directories that are marked with a
  `CACHEDIR.TAG` file. Set `exclude_cache_tag_directories` to `false`
  in the configuration file to disable the feature. See the [Cache
  Directory Tagging Specification][] for details of the tag file.

[Cache Directory Tagging Specification]: https://bford.info/cachedir/

* You can now use _tilde notation_ in the configuration file, in fields
  for specifying backup root directories or the log file. This makes
  it easier to files relative to the user's home directory:

  ~~~yaml
  server_url: https://obnam-server
  roots:
    - ~/Maildirs
    ~ ~/src/obnam
  log: ~/log/obnam.log
  ~~~

* Alexander Batischev changed the code that queries the SQL database
  to return an iterator, instead of an array of result. This means
  that if, for example, a backup generation has a very large number of
  files, Obnam no longer needs to keep all of them in memory at once.

* Various error messages are now clearer and more useful. For example,
  if there is a problem reading a file, the name of the file is
  included in the error message.

## Internal changes

* Alexander Batischev added support for GitLab CI, which means that
  changes are tested automatically before they are merged. This will
  make development a little smoother in the future.


## Changes to documentation

* Tigran Zakoyan made a logo for Obnam. It is currently only used on
  the [website](https://obnam.org/), but will find more use later. For
  example, some stickers could be made.

## Thank you

Several people have helped with this release, with changes or
feedback. I want to especially mention the following, in order by
first name, with apologies to anyone I have inadvertently forgotten:
Alexander Batischev, Daniel Silverstone, Neal Walfield, Tigran
Zakoyan.


# Version 0.3.1, released 2021-03-23

This is a minor release to work around a bug in Subplot, which
prevented the 0.3.0 release to have a Debian package built. The
workaround is to rewrite a small table in the "Filenames" section as a
list.


# Version 0.3.0, released 2021-03-14

## Breaking changes

* The format of the data stored on the backup repository has changed.
  The new version can't restore old backups: old generations are now
  useless. You'll have to start over. Sorry.

## New or changed features

* New `obnam config` sub-command writes out the actual configuration
  that the program users, as read from the configuration file.

* The client configuration now has default values for all
  configuration fields that can reasonably have them. For example, it
  is no longer necessary to explicitly set a chunk size.

* Only known fields are now allowed in configuration files. Unknown
  fields cause an error.

* It is now possible to back up multiple, distinct directories with
  one client configuration. The `root` configuration is now `roots`,
  and is a list of directories.

* Problems in backing up a file no longer terminate the backup run.
  Instead, the problem is reported at the end of the backup run, as a
  warning.

* The client now requires an HTTPS URL for the server. Plain HTTP is
  now rejected. The TLS certificate for the server is verified by
  default, but that can be turned off.

* The client progress reporting is now a little clearer.

* Unix domain sockets and named pipes (FIFO files) are now backed up
  and restored.

* The names of the user and group owning a file are backed up, but not
  restored.

* On the Obnam server, the Ansible playbook now installs a cron job to
  renew the Let's Encrypt TLS certificate.

## Bugs fixed

* Temporary files created during backup runs are now automatically
  deleted, even if the Obnam client crashes.

* Symbolic links are now backed up and restored correctly. Previously
  Obnam followed the link when backing up and created the link
  wrongly.

* The Ansible playbook to provision an Obnam server now enables the
  systemd unit so that the Obnam server process starts automatically
  after a reboot.

## Changes to documentation

* A tutorial has been added.

The Obnam subplot (`obnam.md`), which describes the requirements,
acceptance criteria, and architecture of the software, has some
improvements:
  
* a discussion of why Obnam doesn't use content-addressable storage

* a description of the logical structure of backups as stored on the
  backup server

* a rudimentary first sketch of a threat model: the operator of the
  backup server reads the backed up data

* an initial plan for adding support for encryption to backups; this
  is known to be simplistic and inadequate, but the goal is to get
  started, and then iterate to get something acceptable, even if that
  takes months

## Thank you

Several people have helped with this release, with changes or
feedback. I want to especially mention the following, with apologies
to anyone I have inadvertently forgotten: Alexander Batischev, Ossi
Herrala, Daniel Silverstone, Neal Walfield.

# Version 0.2.2, released 2021-01-29

This is the first release of Obnam2. It can just barely make and
restore backups. It's ready for a light trial, but not for real use.
There's no encryption, and backups can't be deleted yet. Restores of
the entire backup work.
