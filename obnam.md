# Introduction

Obnam2 is a project to develop a backup system.

In 2004 I started a project to develop a backup program for myself,
which in 2006 I named Obnam. In 2017 I retired the project, because it
was no longer fun. The project had some long-standing, architectural
issues related to performance that had become entrenched and were hard
to fix, without breaking backwards compatibility.

In 2020, with Obnam2 I'm starting over from scratch. The new software
is not, and will not become, compatible with Obnam1 in any way. I aim
the new software to be more reliable and faster than Obnam1, without
sacrificing security or ease of use, while being maintainable in the
long run.

Part of that maintainability is going to be achieved by using Rust as
the programming language (strong, static type system) rather than
Python (dynamic, comparatively weak type system). Another part is more
strongly aiming for simplicity and elegance. Obnam1 used an elegant,
but not very simple copy-on-write B-tree structure; Obnam2 will at
least initially use [SQLite][].

[SQLite]: https://sqlite.org/index.html

## Glossary

This document uses some specific terminology related to backups. Here
is a glossary of such terms.

* **chunk** is a relatively small amount of live data or metadata
  about live data, as chosen by the client
* **client** is the computer system where the live data lives, also the part of
  Obnam2 running on that computer
* **generation** is a snapshot of live data
* **live data** is the data that gets backed up
* **repository** is where the backups get stored
* **server** is the computer system where the repository resides, also
  the part of Obnam2 running on that computer


# Requirements

The following high-level requirements are not meant to be verifiable
in an automated way:

* _Not done:_ **Easy to install:** available as a Debian package in an
  APT repository.
* _Not done:_ **Easy to configure:** only need to configure things
  that are inherently specific to a client and for which sensible
  defaults are impossible.
* _Not done:_ **Easy to run:** a single command line that's always the
  same works for making a backup.
* _Not done:_ **Detects corruption:** if a file in the repository is
  modified or deleted, the software notices it automatically.
* _Not done:_ **Corrects any 1-bit error:** if a file in the
  repository is changed by one bit, the software automatically
  corrects it.
* _Not done:_ **Repository is encrypted:** all data stored in the
  repository is encrypted with a key only the client has.
* _Not done:_ **Fast backups and restores:** when a client and server
  both have sufficient CPU, RAM, and disk bandwidth, the software make
  a backup or restore a backup over a gigabit Ethernet using at least
  50% of the network bandwidth.
* _Not done:_ **Snapshots:** Each backup generation is an independent
  snapshot: it can be deleted without affecting any other generation.
* _Not done:_ **Deduplication:** Identical chunks of data are stored
  only once in the backup repository.
* _Not done:_ **Compressed:** Data stored in the backup repository is
  compressed.
* _Not done:_ **Large numbers of live data files:** The system must
  handle ten million files in live data.
* _Not done:_ **Live data in the terabyte range:** The system must
  handle a terabyte of live data.
* _Not done:_ **Many clients:** The system must handle a thousand
  total clients and one hundred clients using the server concurrently.
* _Not done:_ **Shared repository:** The system should allow people
  who don't trust each other to share a repository without fearing
  their own data leaks, or even its existence leaks, to anyone.
* _Not done:_ **Shared backups:** People who do trust each other
  should be able to share backed up data in the repository.

The detailed, automatically verified acceptance criteria are
documented in the ["Acceptance criteria"](#acceptance) chapter.


## Requirements for a minimum viable product

The first milestone for the Obnam2 &ndash; the minimum viable product
&ndash; does not try to fulfil all the requirements for Obnam2.
Instead, the following semi-subset is the goal:

* _Not done:_ **Can do a backup of my own data and restore it:** This
  is the minimum functionality for a backup program.
* _Not done:_ **Fast backups and restores:** a backup or restore of 10
  GiB of live data, between two VMs on my big home server take less
  than 200 seconds.
* _Not done:_ **Snapshots:** Each backup generation is an independent
  snapshot: it can be deleted without affecting any other generation.
* _Not done:_ **Deduplication:** Identical files are stored only once
  in the backup repository.
* _Not done:_ **Single client:** Only a single client per server is
  supported.
* _Not done:_ **No authentication:** The client does note authenticate
  itself to the server.
* _Not done:_ **No encryption:** Client sends data to the server in
  cleartext.

This document currently only documents the detailed acceptance
criteria for the MVP. When the MVP is finished, this document will
start documenting more.


# Architecture

For the minimum viable product, Obnam2 will be split into a server and
one or more clients. The server handles storage of chunks, and access
control to them. The clients make and restore backups. The
communication between the clients and the server is via HTTP.

~~~dot
digraph "arch" {
  live1 -> client1;
  live2 -> client2;
  live3 -> client3;
  live4 -> client4;
  live5 -> client5;
  client1 -> server [label="HTTP"];
  client2 -> server;
  client3 -> server;
  client4 -> server;
  client5 -> server;
  server -> disk;
  live1 [shape=cylinder]
  live2 [shape=cylinder]
  live3 [shape=cylinder]
  live4 [shape=cylinder]
  live5 [shape=cylinder]
  disk [shape=cylinder]
}
~~~

The server side is not very smart. It handles storage of chunks and
their metadata only. The client is smarter:

* it scans live data for files to back up
* it splits those files into chunks, and stores the chunks on the
  server
* it constructs an SQLite database file, with all filenames, file
  metadata, and the chunks associated with each live data file
* it stores the database on the server, as chunks
* it stores a chunk specially marked as a generation on the server

The generation chunk contains a list of the chunks for the SQLite
database. When the client needs to restore data:

* it gets a list of generation chunks from the server
* it lets the user choose a generation
* it downloads the generation chunk, and the associated SQLite
  database, and then all the backed up files, as listed in the
  database

This is the simplest architecture I can think of for the MVP.

## Chunk server API

The chunk server has the following API:

* `POST /chunks` &ndash; store a new chunk (and its metadata) on the
  server, return its randomly chosen identifier
* `GET /chunks/<ID>` &ndash; retrieve a chunk (and its metadata) from
  the server, given a chunk identifier
* `GET /chunks?sha256=xyzzy` &ndash; find chunks on the server whose
  metadata indicates their contents has a given SHA256 checksum
* `GET /chunks?generation=true` &ndash; find generation chunks

When creating or retrieving a chunk, its metadata is carried in a
`Chunk-Meta` header as a JSON object. The following keys are allowed:

* `sha256` &ndash; the SHA256 checksum of the chunk contents as
  determined by the client
  - this must be set for every chunk, including generation chunks
  - note that the server doesn't verify this in any way
* `generation` &ndash; set to `true` if the chunk represents a
  generation
  - may also be set to `false` or `null` or be missing entirely
* `ended` &ndash; the timestamp of when the backup generation ended
  - note that the server doesn't process this in anyway, the contents
    is entirely up to the client
  - may be set to the empty string, `null`, or be missing entirely

HTTP status codes are used to indicate if a request succeeded or not,
using the customary meanings.

When creating a chunk, chunk's metadata is sent in the `Chunk-Meta`
header, and the contents in the request body. The new chunk gets a
randomly assigned identifier, and if the request is successful, the
response is a JSON object with the identifier:

~~~json
{
    "chunk_id": "fe20734b-edb3-432f-83c3-d35fe15969dd"
}
~~~

The identifier is a [UUID4][], but the client should not assume that.

[UUID4]: https://en.wikipedia.org/wiki/Universally_unique_identifier#Version_4_(random)

When a chunk is retrieved, the chunk metadata is returned in the
`Chunk-Meta` header, and the contents in the response body.

Note that it is not possible to update a chunk or its metadata.

When searching for chunks, any matching chunk's identifiers and
metadata are returned in a JSON object:

~~~json
{
  "fe20734b-edb3-432f-83c3-d35fe15969dd": {
     "sha256": "09ca7e4eaa6e8ae9c7d261167129184883644d07dfba7cbfbc4c8a2e08360d5b",
     "generation": null,
	 "ended: null,
  }
}
~~~

There can be any number of chunks in the response.

# Acceptance criteria {#acceptance}

[Subplot]: https://subplot.liw.fi/

This chapter documents detailed acceptance criteria and how they are
verified as scenarios for the [Subplot][] tool. At this time, only
criteria for the minimum viable product are included.

## Chunk server

These scenarios verify that the chunk server works on its own. The
scenarios start a fresh, empty chunk server, and do some operations on
it, and verify the results, and finally terminate the server.

### Chunk management happy path

We must be able to create a new chunk.

~~~scenario
given a chunk server
and a file data.dat containing some random data
when I POST data.dat to /chunks, with chunk-meta: {"sha256":"abc"}
then HTTP status code is 201
and content-type is application/json
and the JSON body has a field chunk_id, henceforth ID
~~~

We must be able to retrieve it.

~~~scenario
when I GET /chunks/<ID>
then HTTP status code is 200
and content-type is application/octet-stream
and chunk-meta is {"sha256":"abc","generation":null,"ended":null}
and the body matches file data.dat
~~~

We must also be able to find it based on metadata.

~~~scenario
when I GET /chunks?sha256=abc
then HTTP status code is 200
and content-type is application/json
and the JSON body matches {"<ID>":{"sha256":"abc","generation":null,"ended":null}}
~~~

Finally, we must be able to delete it. After that, we must not be able
to retrieve it, or find it using metadata.

~~~scenario
when I DELETE /chunks/<ID>
then HTTP status code is 200

when I GET /chunks/<ID>
then HTTP status code is 404

when I GET /chunks?sha256=abc
then HTTP status code is 200
and content-type is application/json
and the JSON body matches {}
~~~

### Retrieve a chunk that does not exist

We must get the right error if we try to retrieve a chunk that does
not exist.

~~~scenario
given a chunk server
when I try to GET /chunks/any.random.string
then HTTP status code is 404
~~~

### Search without matches

We must get an empty result if searching for chunks that don't exist.

~~~scenario
given a chunk server
when I GET /chunks?sha256=abc
then HTTP status code is 200
and content-type is application/json
and the JSON body matches {}
~~~

### Delete chunk that does not exist

We must get the right error when deleting a chunk that doesn't exist.

~~~scenario
given a chunk server
when I try to DELETE /chunks/any.random.string
then HTTP status code is 404
~~~

## Smoke test

This scenario verifies that a small amount of data in simple files in
one directory can be backed up and restored, and the restored files
and their metadata are identical to the original. This is the simplest
possible, but still useful requirement for a backup system.

~~~scenario
given a chunk server
and a file live/data.dat containing some random data
when I back up live with obnam-backup
then backup command is successful
~~~

## Backups and restores

These scenarios verify that every kind of file system object can be
backed up and restored.

### All kinds of files and metadata

This scenario verifies that all kinds of files (regular, hard link,
symbolic link, directory, etc) and metadata can be backed up and
restored.

### Duplicate files are stored once

This scenario verifies that if the live data has two copies of the
same file, it is stored only once.

### Snapshots are independent

This scenario verifies that generation snapshots are independent of
each other, by making three backup generations, deleting the middle
one, and restoring the others.


## Performance

These scenarios verify that system performance is at an expected
level, at least in simple cases. To keep the implementation of the
scenario manageable, communication is over `localhost`, not between
hosts. A more thorough benchmark suite will need to be implemented
separately.

### Can back up 10 GiB in 200 seconds

This scenario verifies that the system can back up data at an
acceptable speed. 

### Can restore 10 GiB in 200 seconds

This scenario verifies that the system can restore backed up data at
an acceptable speed.




<!-- -------------------------------------------------------------------- -->


---
title: "Obnam2&mdash;a backup system"
author: Lars Wirzenius
documentclass: report
bindings:
  - subplot/obnam.yaml
functions:
  - subplot/obnam.py
  - subplot/runcmd.py
  - subplot/daemon.py
classes:
  - json
...
