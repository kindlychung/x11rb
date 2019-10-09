# X11 rust bindings

Feel free to open issues for any problems or questions you might have.


## Building

This crate depends on `xcb-proto`. It uses `pkg-config` to find it. In a
nutshell, if you can run `pkg-config --modversion xcb-proto` successfully, you
should be fine.

On Debian, the necessary packages are called `pkg-config` and `xcb-proto`. I
hope that other distros use similarly obvious naming.


## Motivation

![Motivation](https://imgs.xkcd.com/comics/standards.png)

(The image is licensed under a Creative Commons Attribution-NonCommercial 2.5 License)

My main motivation for writing this library is fun and getting some experience
with Rust. As such, "there is already a library" does not count.

However, since you brought this topic up, let us look at some other libraries
that allow accessing an X11 server from Rust. If you know about more libraries
or want me to know that I got something wrong, feel free to tell me about them,
for example by opening an issue.


### xproto-rs

I only found this [on crates.io](https://crates.io/crates/xproto). The
Repository link is broken and documentation looks like someone dumped the result
of `bindgen` on the Xlib headers into a crate.


### xrb

The [Pure Rust bindings for X11](https://github.com/DaMrNelson/xrb) seem to
contain hand-written code for parsing and sending X11 messages. Also, its README
currently claims that this project is in an early state.


### x11-rs

This seems to provide FFI wrappers around Xlib and various related libraries. I
recently heard about this library because [its
unsafe](https://github.com/erlepereira/x11-rs/issues/99) code is
[unsound](https://github.com/rust-lang/rust/issues/52898) and causes undefined
behaviour. This is basically all I know and heard about this library.


### rust-xcb

This project uses xcb-proto, the XML description of the X11 protocol that comes
from the libxcb project. Based on this XML, code is generated that provides a
foreign function interface to the various libxcb libraries. Due to its FFI
nature, this project contains many instances of `unsafe`. Worse, its
`basic_window` example indicates that users of this library must also use
`unsafe` for [handling
events](https://github.com/rtbo/rust-xcb/blob/d7cb614a6fe9f4424ed26939a5720770f84acd05/examples/basic_window.rs#L66).
How can one ever be sure that there is nothing wrong with the `unsafe` one
writes?

I briefly looked at this project and found a [NULL pointer
dereference](https://github.com/rtbo/rust-xcb/issues/64) and re-discovered an
already known [leak](https://github.com/rtbo/rust-xcb/issues/57).


### x11rb (this project)

x11rb, the x11 rust bindings, is based on the XML description of the X11
protocol that comes from the libxcb project, similar to rust-xcb. However,
instead of providing a foreign function interface to libxcb, the generated code
reimplements the serialising and unserialising code that is provided by libxcb.
libxcb is only used for receiving and sending opaque packets.

This reimplementation is done without any uses of `unsafe` and thus should enjoy
Rust's usual safety guarantees. After all, the best way to trust the unsafe code
coming out of your code generator is if your code generator does not generate
any unsafe code.

This means that this project is even safer than libxcb, because libxcb forces
its users to blindly trust length fields that come from the X11 server.

The downside of this is possibly slower code. However, if your bottleneck is in
talking to the X11 server, you are seriously doing something wrong.


## Does this support async/await

No. If you have so many X11 connections that this would matter, you are doing
something wrong. Also, it encourages people to write high-latency code instead
of sending multiple requests and only afterwards wait for the replies.


## Future work

- Support extensions. Currently, only the core protocol is done.
- big requests support
- InternAtom `only_if_exists` argument should be `bool`, but the python code in
  xcb-proto does not provide this information
- The aux structs should be passed via something like AsRef to the request
  functions. This should allow to pass both `obj` and `&obj`.
- code size optimisation: have the generic functions resolve their arguments and
  then call a common (internal) helper function
- FD passing
- checked requests (needed?)
  - Add `connection.check_request(sequence)` and be done?
- thread safety (connection should be Send and Sync)
  - The basic guarantee is provided by libxcb, so this should be easily doable.
- Rewrite it in Rust - a non-ffi based library
  - I know, I know, this may sound crazy, but libxcb is only used for sending
    and receiving opaque packets. This can also be done in pure rust code.


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
