# Introduction

Obnam2 is a backup system.

In 2004 I started a project to develop a backup program for myself,
which in 2006 I named Obnam. In 2017 I retired the project, because it
was no longer fun. The project had some long-standing, architectural
issues related to performance that had become entrenched and were hard
to fix, without breaking backwards compatibility.

In 2020, with Obnam2 I'm starting over from scratch. The new software
is not, and will not become, compatible with Obnam1 in any way. I aim
the new software to be more reliable and faster than Obnam1, without
sacrificing security or ease of use, while being maintainable in the
long run. I also intend to have fun while developing the new software.

Part of that maintainability is going to be achieved by using Rust as
the programming language (strong, static type system) rather than
Python (dynamic, comparatively weak type system). Another part is more
strongly aiming for simplicity and elegance. Obnam1 used an elegant,
but not very simple copy-on-write B-tree structure; Obnam2 will use
[SQLite][].

[SQLite]: https://sqlite.org/index.html

## Glossary

This document uses some specific terminology related to backups. Here
is a glossary of such terms.

* a **chunk** is a relatively small amount of live data or metadata
  about live data, as chosen by the client
* a **client** is the computer system where the live data lives, also
  the part of Obnam running on that computer
* a **generation** is a snapshot of live data, also known as **a
  backup**
* **live data** is the data that gets backed up
* a **repository** is where the backups get stored
* a **server** is the computer system where the repository resides,
  also the part of Obnam running on that computer


# Requirements

The following high-level requirements are not meant to be verifiable
in an automated way:

* _Not done:_ **Easy to install:** available as a Debian package in an
  APT repository. Other installation packages will also be provided,
  hopefully.
* _Not done:_ **Easy to configure:** only need to configure things
  that are inherently specific to a client, when sensible defaults are
  impossible.
* _Not done:_ **Excellent documentation:** although software ideally
  does not need documentation, in practice is usually does, and Obnam
  should have documentation that is clear, correct, helpful,
  unambiguous, and well-liked.
* _Done_: **Easy to run:** making a backup is a single command line
  that's always the same.
* _Not done:_ **Detects corruption:** if a file in the repository is
  modified or deleted, the software notices it automatically.
* _Not done:_ **Repository is encrypted:** all data stored in the
  repository is encrypted with a key known only to the client.
* _Not done:_ **Fast backups and restores:** when a client and server
  both have sufficient CPU, RAM, and disk bandwidth, the software
  makes a backup or restores a backup over a gigabit Ethernet using at
  least 50% of the network bandwidth.
* _Done:_ **Snapshots:** Each backup is an independent snapshot: it
  can be deleted without affecting any other snapshot.
* _Done:_ **Deduplication:** Identical chunks of data are stored only
  once in the backup repository.
  - Note: The chunking is very simplistic, for now, but that can be
    improved later. The changes will only affect the backup part of
    the client.
* _Not done:_ **Compressed:** Data stored in the backup repository is
  compressed.
* _Not done:_ **Large numbers of live data files:** The system must
  handle at least ten million files of live data. (Preferably much
  more, but I want some concrete number to start with.)
* _Not done:_ **Live data in the terabyte range:** The system must
  handle a terabyte of live data. (Again, preferably more.)
* _Not done:_ **Many clients:** The system must handle a thousand
  total clients and one hundred clients using the server concurrently,
  on one physical server.
* _Not done:_ **Shared repository:** The system should allow people
  who don't trust each other to share a repository without fearing
  that their own data leaks, or even its existence leaks, to anyone.
* _Not done:_ **Shared backups:** People who do trust each other
  should be able to share backed up data in the repository.

The detailed, automatically verified acceptance criteria are
documented below, as _scenarios_ described for the [Subplot][] tool.
The scenarios describe specific sequences of events and the expected
outcomes.

[Subplot]: https://subplot.liw.fi/

# Software architecture

For the minimum viable product, Obnam2 will be split into a server and
one or more clients. The server handles storage of chunks, and access
control to them. The clients make and restore backups. The
communication between the clients and the server is via HTTP.

~~~pikchr
Live1: cylinder "live1" bold big big
move
Live2: cylinder "live2" bold big big
move
Live3: cylinder "live3" bold big big
move
Live4: cylinder "live4" bold big big
move
Live5: cylinder "live5" bold big big

down

arrow from Live1.s
C1: ellipse "client1" bold big big
arrow from Live2.s
C2: ellipse "client2" bold big big
arrow from Live3.s
C3: ellipse "client3" bold big big
arrow from Live4.s
C4: ellipse "client4" bold big big
arrow from Live5.s
C5: ellipse "client5" bold big big

S: ellipse "server" bold big big at 2*boxwid south of C3
arrow from C1.s to S.n "HTTPS" bold big big aligned below
arrow from C2.s to S.n
arrow from C3.s to S.n
arrow from C4.s to S.n
arrow from C5.s to S.n

arrow from S.s
cylinder "disk" bold big big
~~~

The server side is not very smart. It handles storage of chunks and
their metadata only. The client is smarter:

* it scans live data for files to back up
* it splits those files into chunks, and stores the chunks on the
  server
* it constructs an SQLite database file, with all filenames, file
  metadata, and the identifiers of chunks for each live data file
* it stores the database on the server, as chunks
* it stores a chunk specially marked as a generation on the server

The generation chunk contains a list of the chunks for the SQLite
database. When the client needs to restore data:

* it gets a list of generation chunks from the server
* it lets the user choose a generation
* it downloads the generation chunk, and the associated SQLite
  database, and then all the backed up files, as listed in the
  database

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
  - this MUST be set for every chunk, including generation chunks
  - note that the server doesn't verify this in any way, to pave way
    for future client-side encryption of the chunk data
* `generation` &ndash; set to `true` if the chunk represents a
  generation
  - may also be set to `false` or `null` or be missing entirely
* `ended` &ndash; the timestamp of when the backup generation ended
  - note that the server doesn't process this in anyway, the contents
    is entirely up to the client
  - may be set to the empty string, `null`, or be missing entirely
  - this can't be used in searches

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

It is not possible to update a chunk or its metadata.

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

# Acceptance criteria for the chunk server

These scenarios verify that the chunk server works on its own. The
scenarios start a fresh, empty chunk server, and do some operations on
it, and verify the results, and finally terminate the server.

### Chunk management happy path

We must be able to create a new chunk, retrieve it, find it via a
search, and delete it. This is needed so the client can manage the
storage of backed up data.

~~~scenario
given an installed obnam
and a running chunk server
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
given an installed obnam
and a running chunk server
when I try to GET /chunks/any.random.string
then HTTP status code is 404
~~~

### Search without matches

We must get an empty result if searching for chunks that don't exist.

~~~scenario
given an installed obnam
and a running chunk server
when I GET /chunks?sha256=abc
then HTTP status code is 200
and content-type is application/json
and the JSON body matches {}
~~~

### Delete chunk that does not exist

We must get the right error when deleting a chunk that doesn't exist.

~~~scenario
given an installed obnam
and a running chunk server
when I try to DELETE /chunks/any.random.string
then HTTP status code is 404
~~~


# Smoke test for Obnam as a whole

This scenario verifies that a small amount of data in simple files in
one directory can be backed up and restored, and the restored files
and their metadata are identical to the original. This is the simplest
possible useful use case for a backup system.

~~~scenario
given an installed obnam
and a running chunk server
and a client config based on smoke.yaml
and a file live/data.dat containing some random data
when I run obnam backup smoke.yaml
then backup generation is GEN
when I run obnam list smoke.yaml
then generation list contains <GEN>
when I invoke obnam restore smoke.yaml <GEN> restore.db rest
then data in live and rest match
~~~

~~~{#smoke.yaml .file .yaml .numberLines}
root: live
dbname: tmp.db
~~~





<!-- -------------------------------------------------------------------- -->


---
title: "Obnam2&mdash;a backup system"
author: Lars Wirzenius
documentclass: report
bindings:
  - subplot/server.yaml
  - subplot/client.yaml
  - subplot/data.yaml
  - subplot/vendored/runcmd.yaml
functions:
  - subplot/server.py
  - subplot/client.py
  - subplot/data.py
  - subplot/daemon.py
  - subplot/vendored/runcmd.py
classes:
  - json
abstract: |
  Obnam is a backup system, consisting of a not very smart server for
  storing chunks of backup data, and a client that splits the user's
  data into chunks. They communicate via HTTP.
  
  This document describes the architecture and acceptance criteria for
  Obnam, as well as how the acceptance criteria are verified.
...
