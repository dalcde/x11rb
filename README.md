# X11 rust bindings

[![TravisCI Build Status](https://travis-ci.org/psychon/x11rb.svg?branch=master)](https://travis-ci.org/psychon/x11rb)
[![AppVeyor Build Status](https://ci.appveyor.com/api/projects/status/950g0t6i8hfc9dup/branch/master?svg=true)](https://ci.appveyor.com/project/psychon/x11rb)
[![Crate](https://img.shields.io/crates/v/x11rb.svg)](https://crates.io/crates/x11rb)
[![API](https://docs.rs/x11rb/badge.svg)](https://docs.rs/x11rb)
![Minimum rustc version](https://img.shields.io/badge/rustc-1.37+-lightgray.svg)
[![License](https://img.shields.io/crates/l/x11rb.svg)](https://github.com/psychon/x11rb#license)

Feel free to open issues for any problems or questions you might have.
A comparison with some other Rust X11 libraries is available in an [extra
document](doc/comparison.md).


## Building

This crate uses a code generator that is implemented in Rust. A copy of the
generated code is included, so you do not need to run the generator unless
you have modified the definitions or the generator itself.

The code generator uses the X11 XML description from `xcb-proto`. A copy of
xcb-proto that comes with the source code is used.


## Does this support async/await

No. If you have so many X11 connections that this would matter, you are doing
something wrong. Also, it encourages people to write high-latency code instead
of sending multiple requests and only afterwards wait for the replies.


## Crate features

The following features are enabled by default:

* `allow-unsafe-code`: Without this feature, `forbid(unsafe_code)` forbids all
  unsafe code. With this feature, `XCBConnection` becomes available.


## Current state

The full X11 protocol is supported by this library. All extensions that are
available in `xcb-proto` can be used and even [FD
passing](examples/shared_memory.rs) with the server is supported.

The changelog is available in a [separate file](doc/changelog.md).


## License

Licensed under either of

 * Apache License, Version 2.0
   ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
 * MIT license
   ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

The subdirectory xcb-proto-1.14-1-g2b3559c contains a vendored copy of the
package of the same name. It is covered by the MIT license. See
[xcb-proto-1.14-1-g2b3559c/COPYING](xcb-proto-1.14-1-g2b3559c/COPYING) for details.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
