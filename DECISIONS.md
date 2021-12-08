# Big decisions made by the Obnam project

This is a decision log of big decisions, often architectural ones, or
otherwise decisions that impact the project as a whole. They may be
decisions about non-technical aspects of the project.

Decisions should be discussed before being made so that there is a
strong consensus for them. Decisions can be changed or overturned if
they turn out to no longer be good. Overturning is itself a decision.

Each decision should have its own heading. Newest decision should come
first. Updated or overturned decisions should have their section
updated to note their status, without moving them.

## Support at least the version of Rust in Debian "bookworm"

Date: 2021-12-06

What: See discussion in <https://gitlab.com/obnam/obnam/-/issues/137>.
Obnam aims to work in various Linux distributions and other operating
systems. One of these is Debian. At the time of writing, Debian's next
major version (code name bookworm) will have at least Rust 1.56. The
decision for Obnam is that a minimum support Rust version in bookworm.

## Start a decision log

Date: 2021-11-30

What: We decided to start keeping a decision log.

Who: Alexander Batischev, Lars Wirzenius.
