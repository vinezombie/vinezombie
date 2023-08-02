Representations of IRC messages and their components.

Unlike some IRC libraries, vinezombie treats
[client-originated messages][ClientMsg] and
[server-originated messages][ServerMsg] as distinct.
Both can contain [message arguments][Args] and [tags][Tags],
but `ServerMsg`s can also be [numeric replies][Numeric]
and can contain a [source][Source].
