# Acceptance criteria

[Subplot]: https://subplot.liw.fi/

This chapter documents detailed acceptance criteria and how they are
verified as scenarios for the [Subplot][] tool

## Chunk server

These scenarios verify that the chunk server works.

### Chunk management

This scenario verifies that a chunk can be uploaded and then
retrieved, with its metadata, and then deleted. The chunk server has
an API with just one endpoint, `/chunks`, and accepts the the POST,
GET, and DELETE operations on it.

To create a chunk, we use POST.

~~~scenario
given a chunk server
and a file data.dat containing some random data
when I POST data.dat to /chunks, with chunk-meta: {"sha256":"abc"}
then HTTP status code is 201
and content-type is application/json
and the JSON body has a field chunk_id, henceforth ID
~~~

To retrieve a chunk, we use GET, giving the chunk id in the path.

~~~scenario
when I GET /chunks/<ID>
then HTTP status code is 200
and content-type is application/octet-stream
and chunk-meta is {"sha256":"abc","generation":null,"ended":null}
and the body matches file data.dat
~~~




<!-- -------------------------------------------------------------------- -->


---
title: "Obnam2&mdash;a backup system"
author: Lars Wirzenius
bindings:
  - subplot/obnam.yaml
functions:
  - subplot/obnam.py
  - subplot/runcmd.py
  - subplot/daemon.py
...
