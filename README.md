# Simple parser for .mrpack files

## Features:
> `default`: All of: `std`, `url`, `fs`<br>
> `resolve`: Enables resolution of a `ModrinthModpack` to an equivalent `.minecraft` folder (TODO)<br>
> `alloc`: Enables `alloc` for `no_std` environments. Incompatible with `std`, obviously<br>
> `std`: Enables `std` support<br>
> `url`: Serialise to `url::Url` rather than `String` where possible<br>
> `fs`: Parse and export `.mrpack` files<br>

## TODO:
- A more ergonomic way of accessing files