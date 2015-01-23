// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

#[derive(Debug)]
struct Unit;

#[derive(Debug)]
struct Tuple(int, uint);

#[derive(Debug)]
struct Struct { x: int, y: uint }

#[derive(Debug)]
enum Enum {
    Nullary,
    Variant(int, uint),
    StructVariant { x: int, y : uint }
}

macro_rules! t {
    ($x:expr, $expected:expr) => {
        assert_eq!(format!("{:?}", $x), $expected.to_string())
    }
}

macro_rules! s {
    ($x:expr, $min:expr, $max:expr) => {
        assert_eq!(::std::fmt::Debug::size_hint(&$x), ::std::fmt::SizeHint { min: $min, max: $max })
    }
}

pub fn main() {
    t!(Unit, "Unit");
    t!(Tuple(1, 2), "Tuple(1, 2)");
    t!(Struct { x: 1, y: 2 }, "Struct { x: 1, y: 2 }");
    t!(Enum::Nullary, "Nullary");
    t!(Enum::Variant(1, 2), "Variant(1, 2)");
    t!(Enum::StructVariant { x: 1, y: 2 }, "StructVariant { x: 1, y: 2 }");

    s!(Unit, 4, Some(4));
    s!(Tuple(1, 2), 11, Some(11));
    s!(Struct { x: 1, y: 2 }, 21, Some(21));
    s!(Enum::Nullary, 7, Some(7));
    s!(Enum::Variant(1, 2), 13, Some(13));
    s!(Enum::StructVariant { x: 1, y: 2 }, 28, Some(28));
}
