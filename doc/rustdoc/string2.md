## String Manipulation

Immutability and the newtype hierarchy are good for ensuring your data is valid,
but can be difficult to work with efficiently if you need to create new strings.
`vinezombie` includes a few ways of creating new strings from existing ones,
depending on your needs:

- [`Builder`] can be used to construct new strings via concatenation.
- [`Transform`]s provide copy-on-write semantics for string transformation operations.
- [`Splitter`] can be used to split strings.

All of these solutions are newtype-aware and have a notion of UTF-8 validity.
The [`tf`] module includes a few `Transform` implementations that are
relevant to processing IRC messages.
