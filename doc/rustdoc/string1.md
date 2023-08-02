Byte strings and string manipulation utilities.

The core text primitive of vinezombie is [`Bytes`]
(not to be confused with `bytes::Bytes`).
This is a borrowing-or-shared-owning immutable string type with
lazy UTF-8 validity checking and a notion of secret values.
It is thread-safe and cheap to construct out of a `Vec` or `String`.

`Bytes` implements `Deref<Target = [u8]>`, so you can use them in many of the same ways
you would use a byte slice. You can also get an `&str` if the `Bytes`
contains valid UTF-8 by using [`to_utf8`][Bytes::to_utf8].

## Newtypes

Additionally, there is a hierarchy of newtypes that have a pseudo-subtyping relationship.
These enforce invariants on the string that can be checked in `const` contexts,
helping to prevent certain classes of logic errors and the construction of invalid messages.

The following diagram shows the pseudo-subtyping relationships between newtypes,
as as well as the additional restriction the "sub"-type imposes over its "super"-type:
