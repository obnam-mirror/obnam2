# Release notes for Obnam2

This file summarizes changes between releases of the second generation
of Obnam, the backup software. The software is technically called
"obnam2" to distinguish it from the first generation of Obnam, which
ended in 2017 with version number 1.22.


# Version 0.7.X, released 2022-07-24

## Breaking changes

Breaking changes are ones that mean existing backups can't be
restored, or new backups can't be created.

* The list of backups is stored in a special "root chunk". This means
  backups are explicitly ordered. This also paves way for a future
  feature to backups: only the root chunk will need to be updated.
  Without a root chunk, the backups formed a linked list, and deleting
  from the middle of the list would updating the whole list.

* The server chunk metadata field `sha256` is now called `label`.
  Labels include a type prefix, to allow for other chunk checksum
  types in the future.

* The server API is now explicitly versioned, to allow future changes
  to cause less breakage.

## New features

* Users can now choose the backup schema version for new backups. A
  repository can have backups with different schemas, and any existing
  backup can be restored. The schema version only applies to new
  backups.

* New command `obnam inspect` shows metadata about a backup. Currently
  only the schema version is shown.

* New command `obnam list-backup-versions` shows all the backup schema
  versions that this version of Obnam supports.

* Obnam now logs some basic performance measurement for each run: how
  many live files were found in total, backed up, chunks uploaded,
  existing chunks reused, and how long various parts of the process
  took.

## Other changes

* The `obnam show-generation` command now outputs data in the JSON
  format. The output now includes data about the generation's SQLite
  database size.

## Thank you

Several people have helped with this release, with changes or
feedback.

* Alexander Batischev
* Lars Wirzenius

# Version 0.7.1, released 2022-03-08

## Bug fixes

* Skipped files are not added to a new backup.

## Other changes

* Obnam is now much faster when backing up files that haven't changed.

## Thank you

Several people have helped with this release, with changes or
feedback.

* Alexander Batischev
* Lars Wirzenius


# Version 0.7.0, released 2022-01-04

## Breaking changes

* No known breaking changes in this release.

## New or changed features

* Command that retrieve and use backups from the server now verify
  that the backup's schema is compatible with the running version of
  Obnam. This means, for example, that `obnam restore` won't try to
  restore a backup it doesn't know it can restore.

## Internal changes

* Update Subplot step bindings with types for captures to allow
  Subplot to verify that embedded files in obnam.md are actually used.

* Tidy up code in various ways.

* The Obnam release process now has a step to run `cargo update` after
  the crate's version number has been updated, so that the
  `Cargo.lock` file gets updated.

## Changes to documentation

* The `obnam` crate now documents all exported symbols. This should
  make the crate somewhat less hostile to use.

* The minimum supported Rust version is whatever is going to be in the
  next Debian stable release (code name bookworm).

## Thank you

Several people have helped with this release, with changes or
feedback.

* Alexander Batischev
* Lars Wirzenius

(Our apologies to anyone who's been forgotten.)



# Version 0.6.0, released 2021-11-20

## Breaking changes

* We no longer test Obnam with Debian 10 (buster) in our continuous
  integration system. The current Debian stable release, Debian 11
  (bullseye), is tested.

## New or changed features

* It is now an error if the backup root directory doesn't exist or
  can't be read. This applies only to the backup roots. Other files
  and directories may go missing or be unreadable, and Obnam only
  warns about that, to allow making backups of live systems where
  files change during the backup.

## Internal changes

* There is now a new "many files" benchmark.

## Changes to documentation

* We've started a decision log for big, important project decisions.

## Thank you

Several people have helped with this release, with changes or
feedback.

* Alexander Batischev
* Lars Wirzenius

(Our apologies to anyone who's been forgotten.)


# Version 0.5.0, released 2021-11-20

## Experimental version

This is an experimental release, and is not meant to be relied on for
recovery of important data. The purpose of this release is to get new
features into the hands of intrepid people who want to try out new
things.

## Breaking changes

* Obnam is now licensed under the GNU Affero General Public License,
  version 3 or later. This mainly affects the Obnam chunk server,
  which has a network API.

* The Obnam client now stores the version of the database schema in
  the per-backup SQLite database. This allows the client to recognize
  when a backup was made with an incompatible version of the client.
  This, in turn, paves way for us to safely making changes that older
  versions of the client do not understand.

  As a result, the backups made with this version may silently break
  older versions of the client. However, this should be the last time
  such silent breakage happens.

## New or changed features

* Obnam now restore metadata of restored symlinks correctly.

* Obnam's handling of `CACHEDIR.TAG` files is more secure against an
  attacker adding such files in directories getting backed up.

* Progress bars so bars for different phases of the backup do not
  interfere with each other anymore.

* The client now has the "obnam resolve" subcommand to resolve a
  generation label (such as "latest") into a generation ID. The labels
  may point at different commits over time, the IDs never change.

* The client now has the "obnam chunkify" subcommand to compute
  checksums of chunks of files. For now, this is for doing performance
  benchmarks, but may eventually evolve into a way to experiment how
  parameters affect sizes of chunks and the ability of the Obnam
  client to find duplicate data.

* A build problem on macOS, where `chmod` needs a different type of
  integer, was fixed.

## Internal changes

* Obnam was migrated to using Docker in GitLab CI and using the new
  Debian stable release (version 11, code name bullseye).

* The Obnam client is now asynchronous code. This is a foundation for
  making the client be faster in the future. This has temporarily made
  the client slower in some cases.

* There is now a simple policy on what is required for changes to be
  merge, in the `DONE.md` file.

* There have been updates to use newer versions of dependencies,
  refactoring of code to be clearer and more tidy, as well as bug
  fixes in the test suite.

## Changes to documentation

* The tutorial now explains the passphrases are ephemeral.

## Thank you

Several people have helped with this release, with changes or
feedback.

* Alexander Batischev
* Daniel Silverstone
* Lars Wirzenius
* Ossi Herrala

(Our apologies to anyone who's been forgotten.)


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
