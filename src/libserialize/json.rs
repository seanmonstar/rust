// Copyright 2012-2013 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

// Rust JSON serialization library
// Copyright (c) 2011 Google Inc.

#![forbid(non_camel_case_types)]

/*!
JSON parsing and serialization

# What is JSON?

JSON (JavaScript Object Notation) is a way to write data in Javascript.
Like XML it allows one to encode structured data in a text format that can be read by humans easily.
Its native compatibility with JavaScript and its simple syntax make it used widely.

Json data are encoded in a form of "key":"value".
Data types that can be encoded are JavaScript types :
boolean (`true` or `false`), number (`f64`), string, array, object, null.
An object is a series of string keys mapping to values, in `"key": value` format.
Arrays are enclosed in square brackets ([ ... ]) and objects in curly brackets ({ ... }).
A simple JSON document encoding a person, his/her age, address and phone numbers could look like:

```ignore
{
    "FirstName": "John",
    "LastName": "Doe",
    "Age": 43,
    "Address": {
        "Street": "Downing Street 10",
        "City": "London",
        "Country": "Great Britain"
    },
    "PhoneNumbers": [
        "+44 1234567",
        "+44 2345678"
    ]
}
```

# Rust Type-based Encoding and Decoding

Rust provides a mechanism for low boilerplate encoding & decoding
of values to and from JSON via the serialization API.
To be able to encode a piece of data, it must implement the `serialize::Encodable` trait.
To be able to decode a piece of data, it must implement the `serialize::Decodable` trait.
The Rust compiler provides an annotation to automatically generate
the code for these traits: `#[deriving(Decodable, Encodable)]`

To encode using Encodable :

```rust
use std::io;
use serialize::{json, Encodable};

 #[deriving(Encodable)]
 pub struct TestStruct   {
    data_str: ~str,
 }

fn main() {
    let to_encode_object = TestStruct{data_str:~"example of string to encode"};
    let mut m = io::MemWriter::new();
    {
        let mut encoder = json::Encoder::new(&mut m as &mut std::io::Writer);
        match to_encode_object.encode(&mut encoder) {
            Ok(()) => (),
            Err(e) => fail!("json encoding error: {}", e)
        };
    }
}
```

Two wrapper functions are provided to encode a Encodable object
into a string (~str) or buffer (~[u8]): `str_encode(&m)` and `buffer_encode(&m)`.

```rust
use serialize::json;
let to_encode_object = ~"example of string to encode";
let encoded_str: ~str = json::Encoder::str_encode(&to_encode_object);
```

To decode a JSON string using `Decodable` trait :

```rust
extern crate serialize;
use serialize::{json, Decodable};

#[deriving(Decodable)]
pub struct MyStruct  {
     attr1: u8,
     attr2: ~str,
}

fn main() {
    let json_str_to_decode = ~"{\"attr1\":1,\"attr2\":\"toto\"}";
    let mut decoder = json::Decoder::new(json_str_to_decode);
    let decoded_object: MyStruct = match Decodable::decode(&mut decoder) {
        Ok(v) => v,
        Err(e) => fail!("Decoding error: {}", e)
    }; // create the final object
}
```

Two convenience functions are provided to decode into an object: `from_str` and
`from_reader`. The above example can also be written like so:

```rust
extern crate serialize;
use serialize::{json, Decodable};

#[deriving(Decodable)]
pub struct MyStruct  {
     attr1: u8,
     attr2: ~str,
}

fn main() {
    let json_str = ~"{\"attr1\":1,\"attr2\":\"toto\"}";
    let decoded_object: MyStruct = match json::from_str(json_str) {
        Ok(v) => v,
        Err(e) => fail!("Decoding error: {}", e)
    }; // create the final object
}
```

# Examples of use

## Using Autoserialization

Create a struct called TestStruct1 and serialize and deserialize it to and from JSON
using the serialization API, using the derived serialization code.

```rust
extern crate serialize;
use serialize::{json, Encodable, Decodable};

 #[deriving(Decodable, Encodable)] //generate Decodable, Encodable impl.
 pub struct TestStruct1  {
    data_int: u8,
    data_str: ~str,
    data_vector: Vec<u8>,
 }

// To serialize use the `json::str_encode` to encode an object in a string.
// It calls the generated `Encodable` impl.
fn main() {
    let to_encode_object = TestStruct1{
        data_int: 1,
        data_str: ~"toto",
        data_vector: vec![2,3,4,5]
    };
    let encoded_str: ~str = json::Encoder::str_encode(&to_encode_object);

    // To deserialize use the `json::from_str` and `json::Decoder`

    let decoded1: TestStruct1 = json::from_str(encoded_str).unwrap(); // create the final object
}
```

*/

use std::char;
use std::f64;
use collections::HashMap;
use std::io;
use std::io::MemWriter;
use std::num;
use std::str;
use std::fmt;
use std::vec::Vec;

use Encodable;
use collections::TreeMap;

/// Represents a json value
#[deriving(Clone, Eq)]
pub enum Json {
    Number(f64),
    String(~str),
    Boolean(bool),
    List(List),
    Object(~Object),
    Null,
}

pub type List = Vec<Json>;
pub type Object = TreeMap<~str, Json>;

#[deriving(Eq, Show)]
pub enum Error {
    /// msg, line, col
    ParseError(~str, uint, uint),
    ExpectedError(~str, ~str),
    MissingFieldError(~str),
    UnknownVariantError(~str),
    IoError(io::IoError)
}

pub type EncodeResult = io::IoResult<()>;
pub type DecodeResult<T> = Result<T, Error>;

fn escape_str(s: &str) -> ~str {
    let mut escaped = ~"\"";
    for c in s.chars() {
        match c {
          '"' => escaped.push_str("\\\""),
          '\\' => escaped.push_str("\\\\"),
          '\x08' => escaped.push_str("\\b"),
          '\x0c' => escaped.push_str("\\f"),
          '\n' => escaped.push_str("\\n"),
          '\r' => escaped.push_str("\\r"),
          '\t' => escaped.push_str("\\t"),
          _ => escaped.push_char(c),
        }
    };

    escaped.push_char('"');

    escaped
}

fn spaces(n: uint) -> ~str {
    let mut ss = ~"";
    for _ in range(0, n) { ss.push_str(" "); }
    return ss;
}

/// A structure for implementing serialization to JSON.
pub struct Encoder<'a> {
    priv wr: &'a mut io::Writer,
    priv spaces: uint,
    priv indent: uint,
}

impl<'a> Encoder<'a> {
    /// Creates a new JSON encoder whose output will be written to the writer
    /// specified.
    pub fn new<'a>(wr: &'a mut io::Writer) -> Encoder<'a> {
        Encoder::with_spaces(wr, 0)
    }

    pub fn new_pretty<'a>(wr: &'a mut io::Writer) -> Encoder<'a> {
        Encoder::with_spaces(wr, 2)
    }

    pub fn with_spaces<'a>(wr: &'a mut io::Writer, spaces: uint) -> Encoder<'a> {
        Encoder {
            wr: wr,
            spaces: spaces,
            indent: 0
        }
    }

    /// Encode the specified struct into a json [u8]
    pub fn buffer_encode<T:Encodable<Encoder<'a>, io::IoError>>(to_encode_object: &T) -> ~[u8]  {
       //Serialize the object in a string using a writer
        let mut m = MemWriter::new();
        {
            let mut encoder = Encoder::new(&mut m as &mut io::Writer);
            // MemWriter never Errs
            let _ = to_encode_object.encode(&mut encoder).unwrap();
        }
        m.unwrap()
    }

    /// Encode the specified struct into a json str
    pub fn str_encode<T:Encodable<Encoder<'a>, io::IoError>>(to_encode_object: &T) -> ~str  {
        let buff:~[u8] = Encoder::buffer_encode(to_encode_object);
        str::from_utf8_owned(buff).unwrap()
    }
}

impl<'a> ::Encoder<io::IoError> for Encoder<'a> {
    fn emit_nil(&mut self) -> EncodeResult { write!(self.wr, "null") }

    fn emit_uint(&mut self, v: uint) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_u64(&mut self, v: u64) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_u32(&mut self, v: u32) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_u16(&mut self, v: u16) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_u8(&mut self, v: u8) -> EncodeResult  { self.emit_f64(v as f64) }

    fn emit_int(&mut self, v: int) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_i64(&mut self, v: i64) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_i32(&mut self, v: i32) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_i16(&mut self, v: i16) -> EncodeResult { self.emit_f64(v as f64) }
    fn emit_i8(&mut self, v: i8) -> EncodeResult  { self.emit_f64(v as f64) }

    fn emit_bool(&mut self, v: bool) -> EncodeResult {
        if v {
            write!(self.wr, "true")
        } else {
            write!(self.wr, "false")
        }
    }

    fn emit_f64(&mut self, v: f64) -> EncodeResult {
        write!(self.wr, "{}", f64::to_str_digits(v, 6u))
    }
    fn emit_f32(&mut self, v: f32) -> EncodeResult { self.emit_f64(v as f64) }

    fn emit_char(&mut self, v: char) -> EncodeResult { self.emit_str(str::from_char(v)) }
    fn emit_str(&mut self, v: &str) -> EncodeResult {
        write!(self.wr, "{}", escape_str(v))
    }

    fn emit_enum(&mut self,
                 _name: &str,
                 f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult { f(self) }

    fn emit_enum_variant(&mut self,
                         name: &str,
                         _id: uint,
                         cnt: uint,
                         f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        // enums are encoded as strings or objects
        // Bunny => "Bunny"
        // Kangaroo(34,"William") => {"variant": "Kangaroo", "fields": [34,"William"]}
        if cnt == 0 {
            self.emit_str(name)
        } else {
            self.emit_struct(name, 2, |this| {
                try!(this.emit_struct_field("variant", 0, |this| this.emit_str(name)));
                this.emit_struct_field("fields", 1, |this| this.emit_seq(cnt, |this| f(this)))
            })
        }
    }

    fn emit_enum_variant_arg(&mut self,
                             idx: uint,
                             f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_seq_elt(idx, f)
    }

    fn emit_enum_struct_variant(&mut self,
                                name: &str,
                                id: uint,
                                cnt: uint,
                                f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_enum_variant(name, id, cnt, f)
    }

    fn emit_enum_struct_variant_field(&mut self,
                                      _: &str,
                                      idx: uint,
                                      f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_enum_variant_arg(idx, f)
    }

    fn emit_struct(&mut self,
                   _: &str,
                   _: uint,
                   f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        try!(write!(self.wr, r"\{"));
        self.indent += self.spaces;
        try!(f(self));
        self.indent -= self.spaces;
        if self.spaces > 0 {
            try!(write!(self.wr, "\n{}", spaces(self.indent)));
        }
        write!(self.wr, r"\}")
    }

    fn emit_struct_field(&mut self,
                         name: &str,
                         idx: uint,
                         f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        if idx != 0 { try!(write!(self.wr, ",")); }
        if self.spaces > 0 {
            try!(write!(self.wr, "\n{}", spaces(self.indent)));
        }
        try!(write!(self.wr, "{}:", escape_str(name)));
        f(self)
    }

    fn emit_tuple(&mut self, len: uint, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_seq(len, f)
    }
    fn emit_tuple_arg(&mut self,
                      idx: uint,
                      f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_seq_elt(idx, f)
    }

    fn emit_tuple_struct(&mut self,
                         _name: &str,
                         len: uint,
                         f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_seq(len, f)
    }
    fn emit_tuple_struct_arg(&mut self,
                             idx: uint,
                             f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_seq_elt(idx, f)
    }

    fn emit_option(&mut self, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        f(self)
    }
    fn emit_option_none(&mut self) -> EncodeResult { self.emit_nil() }
    fn emit_option_some(&mut self, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        f(self)
    }

    fn emit_seq(&mut self, len: uint, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        if len == 0 {
            write!(self.wr, "[]")
        } else {
            try!(write!(self.wr, "["));
            self.indent += self.spaces;
            try!(f(self));
            self.indent -= self.spaces;
            if self.spaces > 0 {
                try!(write!(self.wr, "\n{}"), spaces(self.indent));
            }
            write!(self.wr, "]")
        }
    }

    fn emit_seq_elt(&mut self, idx: uint, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        if idx != 0 {
            try!(write!(self.wr, ","));
        }
        f(self)
    }

    fn emit_map(&mut self, len: uint, f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        self.emit_struct("", len, f)
    }

    fn emit_map_elt_key(&mut self,
                        idx: uint,
                        f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        use std::str::from_utf8;
        if idx != 0 { try!(write!(self.wr, ",")) }
        if self.spaces > 0 {
            try!(write!(self.wr, "\n{}", spaces(self.indent)));
        }
        // ref #12967, make sure to wrap a key in double quotes,
        // in the event that its of a type that omits them (eg numbers)
        let mut buf = MemWriter::new();
        let mut check_encoder = Encoder::new(&mut buf);
        try!(f(&mut check_encoder));
        let buf = buf.unwrap();
        let out = from_utf8(buf).unwrap();

        write!(self.wr, "{}:", escape_str(out))
    }

    fn emit_map_elt_val(&mut self,
                        _idx: uint,
                        f: |&mut Encoder<'a>| -> EncodeResult) -> EncodeResult {
        f(self)
    }
}

impl<E: ::Encoder<io::IoError>> Encodable<E, io::IoError> for Json {
    fn encode(&self, e: &mut E) -> EncodeResult {
        match *self {
            Number(v) => v.encode(e),
            String(ref v) => v.encode(e),
            Boolean(v) => v.encode(e),
            List(ref v) => v.encode(e),
            Object(ref v) => v.encode(e),
            Null => e.emit_nil(),
        }
    }
}

impl Decodable<Decoder<Error>, Error> for Json {
    fn decode<T>(d: &mut D) -> DecodeResult<Json> {
        d.pop()
    }
}

impl Json {
    /// Encodes a json value into a io::writer.  Uses a single line.
    pub fn to_writer(&self, wr: &mut io::Writer) -> EncodeResult {
        let mut encoder = Encoder::new(wr);
        self.encode(&mut encoder)
    }

    /// Encodes a json value into a io::writer.
    /// Pretty-prints in a more readable format.
    pub fn to_pretty_writer(&self, wr: &mut io::Writer) -> EncodeResult {
        let mut encoder = Encoder::new_pretty(wr);
        self.encode(&mut encoder)
    }

    /// Encodes a json value into a string
    pub fn to_pretty_str(&self) -> ~str {
        let mut s = MemWriter::new();
        self.to_pretty_writer(&mut s as &mut io::Writer).unwrap();
        str::from_utf8_owned(s.unwrap()).unwrap()
    }

     /// If the Json value is an Object, returns the value associated with the provided key.
    /// Otherwise, returns None.
    pub fn find<'a>(&'a self, key: &~str) -> Option<&'a Json>{
        match self {
            &Object(ref map) => map.find(key),
            _ => None
        }
    }

    /// Attempts to get a nested Json Object for each key in `keys`.
    /// If any key is found not to exist, find_path will return None.
    /// Otherwise, it will return the Json value associated with the final key.
    pub fn find_path<'a>(&'a self, keys: &[&~str]) -> Option<&'a Json>{
        let mut target = self;
        for key in keys.iter() {
            match target.find(*key) {
                Some(t) => { target = t; },
                None => return None
            }
        }
        Some(target)
    }

    /// If the Json value is an Object, performs a depth-first search until
    /// a value associated with the provided key is found. If no value is found
    /// or the Json value is not an Object, returns None.
    pub fn search<'a>(&'a self, key: &~str) -> Option<&'a Json> {
        match self {
            &Object(ref map) => {
                match map.find(key) {
                    Some(json_value) => Some(json_value),
                    None => {
                        let mut value : Option<&'a Json> = None;
                        for (_, v) in map.iter() {
                            value = v.search(key);
                            if value.is_some() {
                                break;
                            }
                        }
                        value
                    }
                }
            },
            _ => None
        }
    }

    /// Returns true if the Json value is an Object. Returns false otherwise.
    pub fn is_object<'a>(&'a self) -> bool {
        self.as_object().is_some()
    }

    /// If the Json value is an Object, returns the associated TreeMap.
    /// Returns None otherwise.
    pub fn as_object<'a>(&'a self) -> Option<&'a Object> {
        match self {
            &Object(ref map) => Some(&**map),
            _ => None
        }
    }

    /// Returns true if the Json value is a List. Returns false otherwise.
    pub fn is_list<'a>(&'a self) -> bool {
        self.as_list().is_some()
    }

    /// If the Json value is a List, returns the associated vector.
    /// Returns None otherwise.
    pub fn as_list<'a>(&'a self) -> Option<&'a List> {
        match self {
            &List(ref list) => Some(&*list),
            _ => None
        }
    }

    /// Returns true if the Json value is a String. Returns false otherwise.
    pub fn is_string<'a>(&'a self) -> bool {
        self.as_string().is_some()
    }

    /// If the Json value is a String, returns the associated str.
    /// Returns None otherwise.
    pub fn as_string<'a>(&'a self) -> Option<&'a str> {
        match *self {
            String(ref s) => Some(s.as_slice()),
            _ => None
        }
    }

    /// Returns true if the Json value is a Number. Returns false otherwise.
    pub fn is_number(&self) -> bool {
        self.as_number().is_some()
    }

    /// If the Json value is a Number, returns the associated f64.
    /// Returns None otherwise.
    pub fn as_number(&self) -> Option<f64> {
        match self {
            &Number(n) => Some(n),
            _ => None
        }
    }

    /// Returns true if the Json value is a Boolean. Returns false otherwise.
    pub fn is_boolean(&self) -> bool {
        self.as_boolean().is_some()
    }

    /// If the Json value is a Boolean, returns the associated bool.
    /// Returns None otherwise.
    pub fn as_boolean(&self) -> Option<bool> {
        match self {
            &Boolean(b) => Some(b),
            _ => None
        }
    }

    /// Returns true if the Json value is a Null. Returns false otherwise.
    pub fn is_null(&self) -> bool {
        self.as_null().is_some()
    }

    /// If the Json value is a Null, returns ().
    /// Returns None otherwise.
    pub fn as_null(&self) -> Option<()> {
        match self {
            &Null => Some(()),
            _ => None
        }
    }
}

pub struct Decoder<T> {
    priv rdr: T,
    priv ch: Option<char>,
    priv line: uint,
    priv col: uint,
    priv parsed: bool,
    priv stack: Vec<Json>
}

impl<T: Iterator<char>> Decoder<T> {
    /// Decode a json value from an Iterator<char>
    pub fn new(rdr: T) -> Decoder<T> {
        let mut p = Decoder {
            rdr: rdr,
            ch: Some('\x00'),
            line: 1,
            col: 0,
            parsed: false,
            stack: Vec::new()
        };
        p.bump();
        p
    }
}

impl<T: Iterator<char>> Decoder<T> {
    pub fn parse(&mut self) -> DecodeResult<Json> {
        let result = match self.parse_value() {
          Ok(value) => {
            // Skip trailing whitespaces.
            self.parse_whitespace();
            // Make sure there is no trailing characters.
            if self.eof() {
                Ok(value)
            } else {
                self.error(~"trailing characters")
            }
          }
          Err(e) => Err(e)
        };
        self.parsed = true;
        result
    }
}

impl<T : Iterator<char>> Decoder<T> {
    fn eof(&self) -> bool { self.ch.is_none() }
    fn ch_or_null(&self) -> char { self.ch.unwrap_or('\x00') }
    fn bump(&mut self) {
        self.ch = self.rdr.next();

        if self.ch_is('\n') {
            self.line += 1u;
            self.col = 1u;
        } else {
            self.col += 1u;
        }
    }

    fn next_char(&mut self) -> Option<char> {
        self.bump();
        self.ch
    }
    fn ch_is(&self, c: char) -> bool {
        self.ch == Some(c)
    }

    fn error<T>(&self, msg: ~str) -> DecodeResult<T> {
        Err(ParseError(msg, self.line, self.col))
    }

    fn parse_value(&mut self) -> DecodeResult<Json> {
        self.parse_whitespace();

        if self.eof() { return self.error(~"EOF while parsing value"); }

        match self.ch_or_null() {
            'n' => self.parse_ident("ull", Null),
            't' => self.parse_ident("rue", Boolean(true)),
            'f' => self.parse_ident("alse", Boolean(false)),
            '0' .. '9' | '-' => self.parse_number(),
            '"' => {
                match self.parse_str() {
                    Ok(s) => Ok(String(s)),
                    Err(e) => Err(e),
                }
            },
            '[' => self.parse_list(),
            '{' => self.parse_object(),
            _ => self.error(~"invalid syntax"),
        }
    }

    fn parse_whitespace(&mut self) {
        while self.ch_is(' ') ||
              self.ch_is('\n') ||
              self.ch_is('\t') ||
              self.ch_is('\r') { self.bump(); }
    }

    fn parse_ident(&mut self, ident: &str, value: Json) -> DecodeResult<Json> {
        if ident.chars().all(|c| Some(c) == self.next_char()) {
            self.bump();
            Ok(value)
        } else {
            self.error(~"invalid syntax")
        }
    }

    fn parse_number(&mut self) -> DecodeResult<Json> {
        let mut neg = 1.0;

        if self.ch_is('-') {
            self.bump();
            neg = -1.0;
        }

        let mut res = match self.parse_integer() {
          Ok(res) => res,
          Err(e) => return Err(e)
        };

        if self.ch_is('.') {
            match self.parse_decimal(res) {
              Ok(r) => res = r,
              Err(e) => return Err(e)
            }
        }

        if self.ch_is('e') || self.ch_is('E') {
            match self.parse_exponent(res) {
              Ok(r) => res = r,
              Err(e) => return Err(e)
            }
        }

        Ok(Number(neg * res))
    }

    fn parse_integer(&mut self) -> DecodeResult<f64> {
        let mut res = 0.0;

        match self.ch_or_null() {
            '0' => {
                self.bump();

                // There can be only one leading '0'.
                match self.ch_or_null() {
                    '0' .. '9' => return self.error(~"invalid number"),
                    _ => ()
                }
            },
            '1' .. '9' => {
                while !self.eof() {
                    match self.ch_or_null() {
                        c @ '0' .. '9' => {
                            res *= 10.0;
                            res += ((c as int) - ('0' as int)) as f64;

                            self.bump();
                        }
                        _ => break,
                    }
                }
            }
            _ => return self.error(~"invalid number"),
        }
        Ok(res)
    }

    fn parse_decimal(&mut self, res: f64) -> DecodeResult<f64> {
        self.bump();

        // Make sure a digit follows the decimal place.
        match self.ch_or_null() {
            '0' .. '9' => (),
             _ => return self.error(~"invalid number")
        }

        let mut res = res;
        let mut dec = 1.0;
        while !self.eof() {
            match self.ch_or_null() {
                c @ '0' .. '9' => {
                    dec /= 10.0;
                    res += (((c as int) - ('0' as int)) as f64) * dec;

                    self.bump();
                }
                _ => break,
            }
        }

        Ok(res)
    }

    fn parse_exponent(&mut self, mut res: f64) -> DecodeResult<f64> {
        self.bump();

        let mut exp = 0u;
        let mut neg_exp = false;

        if self.ch_is('+') {
            self.bump();
        } else if self.ch_is('-') {
            self.bump();
            neg_exp = true;
        }

        // Make sure a digit follows the exponent place.
        match self.ch_or_null() {
            '0' .. '9' => (),
            _ => return self.error(~"invalid number")
        }
        while !self.eof() {
            match self.ch_or_null() {
                c @ '0' .. '9' => {
                    exp *= 10;
                    exp += (c as uint) - ('0' as uint);

                    self.bump();
                }
                _ => break
            }
        }

        let exp: f64 = num::pow(10u as f64, exp);
        if neg_exp {
            res /= exp;
        } else {
            res *= exp;
        }

        Ok(res)
    }

    fn parse_str(&mut self) -> DecodeResult<~str> {
        let mut escape = false;
        let mut res = ~"";

        loop {
            self.bump();
            if self.eof() {
                return self.error(~"EOF while parsing string");
            }

            if escape {
                match self.ch_or_null() {
                    '"' => res.push_char('"'),
                    '\\' => res.push_char('\\'),
                    '/' => res.push_char('/'),
                    'b' => res.push_char('\x08'),
                    'f' => res.push_char('\x0c'),
                    'n' => res.push_char('\n'),
                    'r' => res.push_char('\r'),
                    't' => res.push_char('\t'),
                    'u' => {
                        // Parse \u1234.
                        let mut i = 0u;
                        let mut n = 0u;
                        while i < 4u && !self.eof() {
                            self.bump();
                            n = match self.ch_or_null() {
                                c @ '0' .. '9' => n * 16u + (c as uint) - ('0' as uint),
                                'a' | 'A' => n * 16u + 10u,
                                'b' | 'B' => n * 16u + 11u,
                                'c' | 'C' => n * 16u + 12u,
                                'd' | 'D' => n * 16u + 13u,
                                'e' | 'E' => n * 16u + 14u,
                                'f' | 'F' => n * 16u + 15u,
                                _ => return self.error(
                                    ~"invalid \\u escape (unrecognized hex)")
                            };

                            i += 1u;
                        }

                        // Error out if we didn't parse 4 digits.
                        if i != 4u {
                            return self.error(
                                ~"invalid \\u escape (not four digits)");
                        }

                        res.push_char(char::from_u32(n as u32).unwrap());
                    }
                    _ => return self.error(~"invalid escape"),
                }
                escape = false;
            } else if self.ch_is('\\') {
                escape = true;
            } else {
                match self.ch {
                    Some('"') => { self.bump(); return Ok(res); },
                    Some(c) => res.push_char(c),
                    None => unreachable!()
                }
            }
        }
    }

    fn parse_list(&mut self) -> DecodeResult<Json> {
        self.bump();
        self.parse_whitespace();

        let mut values = Vec::new();

        if self.ch_is(']') {
            self.bump();
            return Ok(List(values));
        }

        loop {
            match self.parse_value() {
              Ok(v) => values.push(v),
              Err(e) => return Err(e)
            }

            self.parse_whitespace();
            if self.eof() {
                return self.error(~"EOF while parsing list");
            }

            if self.ch_is(',') {
                self.bump();
            } else if self.ch_is(']') {
                self.bump();
                return Ok(List(values));
            } else {
                return self.error(~"expected `,` or `]`")
            }
        };
    }

    fn parse_object(&mut self) -> DecodeResult<Json> {
        self.bump();
        self.parse_whitespace();

        let mut values = ~TreeMap::new();

        if self.ch_is('}') {
          self.bump();
          return Ok(Object(values));
        }

        while !self.eof() {
            self.parse_whitespace();

            if !self.ch_is('"') {
                return self.error(~"key must be a string");
            }

            let key = match self.parse_str() {
              Ok(key) => key,
              Err(e) => return Err(e)
            };

            self.parse_whitespace();

            if !self.ch_is(':') {
                if self.eof() { break; }
                return self.error(~"expected `:`");
            }
            self.bump();

            match self.parse_value() {
              Ok(value) => { values.insert(key, value); }
              Err(e) => return Err(e)
            }
            self.parse_whitespace();

            match self.ch_or_null() {
                ',' => self.bump(),
                '}' => { self.bump(); return Ok(Object(values)); },
                _ => {
                    if self.eof() { break; }
                    return self.error(~"expected `,` or `}`");
                }
            }
        }

        return self.error(~"EOF while parsing object");
    }
}

/// Decodes a json value from an `&mut io::Reader`
pub fn from_reader<T>(rdr: &mut io::Reader) -> DecodeResult<T> {
    let contents = match rdr.read_to_end() {
        Ok(c) => c,
        Err(e) => return Err(IoError(e))
    };
    let s = match str::from_utf8_owned(contents) {
        Some(s) => s,
        None => return Err(ParseError(~"contents not utf-8", 0, 0))
    };
    let mut decoder = Decoder::new(s.chars());
    Decodable::decode(decoder)
}

/// Decodes a json value from a string
pub fn from_str<T>(s: &str) -> DecodeResult<T> {
    let mut decoder = Decoder::new(s.chars());
    Decodable::decode(decoder)
}

impl<T> Decoder<T> {
    fn pop(&mut self) -> DecodeResult<Json> {
        if !self.parsed {
            self.stack.push(try!(self.parse()));
        }
        self.stack.pop().unwrap()
    }
}

macro_rules! expect(
    ($e:expr, Null) => ({
        match try!($e) {
            Null => Ok(()),
            other => Err(ExpectedError(~"Null", format!("{}", other)))
        }
    });
    ($e:expr, $t:ident) => ({
        match try!($e) {
            $t(v) => Ok(v),
            other => Err(ExpectedError(stringify!($t).to_owned(), format!("{}", other)))
        }
    })
)

impl<T> ::Decoder<Error> for Decoder<T> {
    fn read_nil(&mut self) -> DecodeResult<()> {
        debug!("read_nil");
        try!(expect!(self.pop(), Null));
        Ok(())
    }

    fn read_u64(&mut self)  -> DecodeResult<u64 > { Ok(try!(self.read_f64()) as u64) }
    fn read_u32(&mut self)  -> DecodeResult<u32 > { Ok(try!(self.read_f64()) as u32) }
    fn read_u16(&mut self)  -> DecodeResult<u16 > { Ok(try!(self.read_f64()) as u16) }
    fn read_u8 (&mut self)  -> DecodeResult<u8  > { Ok(try!(self.read_f64()) as u8) }
    fn read_uint(&mut self) -> DecodeResult<uint> { Ok(try!(self.read_f64()) as uint) }

    fn read_i64(&mut self) -> DecodeResult<i64> { Ok(try!(self.read_f64()) as i64) }
    fn read_i32(&mut self) -> DecodeResult<i32> { Ok(try!(self.read_f64()) as i32) }
    fn read_i16(&mut self) -> DecodeResult<i16> { Ok(try!(self.read_f64()) as i16) }
    fn read_i8 (&mut self) -> DecodeResult<i8 > { Ok(try!(self.read_f64()) as i8) }
    fn read_int(&mut self) -> DecodeResult<int> { Ok(try!(self.read_f64()) as int) }

    fn read_bool(&mut self) -> DecodeResult<bool> {
        debug!("read_bool");
        Ok(try!(expect!(self.pop(), Boolean)))
    }

    fn read_f64(&mut self) -> DecodeResult<f64> {
        use std::from_str::FromStr;
        debug!("read_f64");
        match try!(self.pop()) {
            Number(f) => Ok(f),
            String(s) => {
                // re: #12967.. a type w/ numeric keys (ie HashMap<uint, V> etc)
                // is going to have a string here, as per JSON spec..
                Ok(FromStr::from_str(s).unwrap())
            },
            value => Err(ExpectedError(~"Number", format!("{}", value)))
        }
    }

    fn read_f32(&mut self) -> DecodeResult<f32> { Ok(try!(self.read_f64()) as f32) }

    fn read_char(&mut self) -> DecodeResult<char> {
        let s = try!(self.read_str());
        {
            let mut it = s.chars();
            match (it.next(), it.next()) {
                // exactly one character
                (Some(c), None) => return Ok(c),
                _ => ()
            }
        }
        Err(ExpectedError(~"single character string", format!("{}", s)))
    }

    fn read_str(&mut self) -> DecodeResult<~str> {
        debug!("read_str");
        Ok(try!(expect!(self.pop(), String)))
    }

    fn read_enum<T>(&mut self,
                    name: &str,
                    f: |&mut Decoder| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_enum({})", name);
        f(self)
    }

    fn read_enum_variant<T>(&mut self,
                            names: &[&str],
                            f: |&mut Decoder, uint| -> DecodeResult<T>)
                            -> DecodeResult<T> {
        debug!("read_enum_variant(names={:?})", names);
        let name = match try!(self.pop()) {
            String(s) => s,
            Object(mut o) => {
                let n = match o.pop(&~"variant") {
                    Some(String(s)) => s,
                    Some(val) => return Err(ExpectedError(~"String", format!("{}", val))),
                    None => return Err(MissingFieldError(~"variant"))
                };
                match o.pop(&~"fields") {
                    Some(List(l)) => {
                        for field in l.move_rev_iter() {
                            self.stack.push(field.clone());
                        }
                    },
                    Some(val) => return Err(ExpectedError(~"List", format!("{}", val))),
                    None => return Err(MissingFieldError(~"fields"))
                }
                n
            }
            json => return Err(ExpectedError(~"String or Object", format!("{}", json)))
        };
        let idx = match names.iter().position(|n| str::eq_slice(*n, name)) {
            Some(idx) => idx,
            None => return Err(UnknownVariantError(name))
        };
        f(self, idx)
    }

    fn read_enum_variant_arg<T>(&mut self, idx: uint, f: |&mut Decoder| -> DecodeResult<T>)
                                -> DecodeResult<T> {
        debug!("read_enum_variant_arg(idx={})", idx);
        f(self)
    }

    fn read_enum_struct_variant<T>(&mut self,
                                   names: &[&str],
                                   f: |&mut Decoder, uint| -> DecodeResult<T>)
                                   -> DecodeResult<T> {
        debug!("read_enum_struct_variant(names={:?})", names);
        self.read_enum_variant(names, f)
    }


    fn read_enum_struct_variant_field<T>(&mut self,
                                         name: &str,
                                         idx: uint,
                                         f: |&mut Decoder| -> DecodeResult<T>)
                                         -> DecodeResult<T> {
        debug!("read_enum_struct_variant_field(name={}, idx={})", name, idx);
        self.read_enum_variant_arg(idx, f)
    }

    fn read_struct<T>(&mut self,
                      name: &str,
                      len: uint,
                      f: |&mut Decoder| -> DecodeResult<T>)
                      -> DecodeResult<T> {
        debug!("read_struct(name={}, len={})", name, len);
        let value = try!(f(self));
        try!(self.pop());
        Ok(value)
    }

    fn read_struct_field<T>(&mut self,
                            name: &str,
                            idx: uint,
                            f: |&mut Decoder| -> DecodeResult<T>)
                            -> DecodeResult<T> {
        debug!("read_struct_field(name={}, idx={})", name, idx);
        let mut obj = try!(expect!(self.pop(), Object));

        let value = match obj.pop(&name.to_owned()) {
            None => return Err(MissingFieldError(name.to_owned())),
            Some(json) => {
                self.stack.push(json);
                try!(f(self))
            }
        };
        self.stack.push(Object(obj));
        Ok(value)
    }

    fn read_tuple<T>(&mut self, f: |&mut Decoder, uint| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_tuple()");
        self.read_seq(f)
    }

    fn read_tuple_arg<T>(&mut self,
                         idx: uint,
                         f: |&mut Decoder| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_tuple_arg(idx={})", idx);
        self.read_seq_elt(idx, f)
    }

    fn read_tuple_struct<T>(&mut self,
                            name: &str,
                            f: |&mut Decoder, uint| -> DecodeResult<T>)
                            -> DecodeResult<T> {
        debug!("read_tuple_struct(name={})", name);
        self.read_tuple(f)
    }

    fn read_tuple_struct_arg<T>(&mut self,
                                idx: uint,
                                f: |&mut Decoder| -> DecodeResult<T>)
                                -> DecodeResult<T> {
        debug!("read_tuple_struct_arg(idx={})", idx);
        self.read_tuple_arg(idx, f)
    }

    fn read_option<T>(&mut self, f: |&mut Decoder, bool| -> DecodeResult<T>) -> DecodeResult<T> {
        match try!(self.pop()) {
            Null => f(self, false),
            value => { self.stack.push(value); f(self, true) }
        }
    }

    fn read_seq<T>(&mut self, f: |&mut Decoder, uint| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_seq()");
        let list = try!(expect!(self.pop(), List));
        let len = list.len();
        for v in list.move_rev_iter() {
            self.stack.push(v);
        }
        f(self, len)
    }

    fn read_seq_elt<T>(&mut self,
                       idx: uint,
                       f: |&mut Decoder| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_seq_elt(idx={})", idx);
        f(self)
    }

    fn read_map<T>(&mut self, f: |&mut Decoder, uint| -> DecodeResult<T>) -> DecodeResult<T> {
        debug!("read_map()");
        let obj = try!(expect!(self.pop(), Object));
        let len = obj.len();
        for (key, value) in obj.move_iter() {
            self.stack.push(value);
            self.stack.push(String(key));
        }
        f(self, len)
    }

    fn read_map_elt_key<T>(&mut self, idx: uint, f: |&mut Decoder| -> DecodeResult<T>)
                           -> DecodeResult<T> {
        debug!("read_map_elt_key(idx={})", idx);
        f(self)
    }

    fn read_map_elt_val<T>(&mut self, idx: uint, f: |&mut Decoder| -> DecodeResult<T>)
                           -> DecodeResult<T> {
        debug!("read_map_elt_val(idx={})", idx);
        f(self)
    }
}

/// Test if two json values are less than one another
impl Ord for Json {
    fn lt(&self, other: &Json) -> bool {
        match *self {
            Number(f0) => {
                match *other {
                    Number(f1) => f0 < f1,
                    String(_) | Boolean(_) | List(_) | Object(_) |
                    Null => true
                }
            }

            String(ref s0) => {
                match *other {
                    Number(_) => false,
                    String(ref s1) => s0 < s1,
                    Boolean(_) | List(_) | Object(_) | Null => true
                }
            }

            Boolean(b0) => {
                match *other {
                    Number(_) | String(_) => false,
                    Boolean(b1) => b0 < b1,
                    List(_) | Object(_) | Null => true
                }
            }

            List(ref l0) => {
                match *other {
                    Number(_) | String(_) | Boolean(_) => false,
                    List(ref l1) => (*l0) < (*l1),
                    Object(_) | Null => true
                }
            }

            Object(ref d0) => {
                match *other {
                    Number(_) | String(_) | Boolean(_) | List(_) => false,
                    Object(ref d1) => d0 < d1,
                    Null => true
                }
            }

            Null => {
                match *other {
                    Number(_) | String(_) | Boolean(_) | List(_) |
                    Object(_) =>
                        false,
                    Null => true
                }
            }
        }
    }
}

impl fmt::Show for Json {
    /// Encodes a json value into a string
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        self.to_writer(f.buf)
    }
}

#[cfg(test)]
mod tests {
    use {Encodable, Decodable};
    use super::{Encoder, Decoder, Error, Boolean, Number, List, String, Null,
                PrettyEncoder, Object, Json, from_str, ParseError, ExpectedError,
                MissingFieldError, UnknownVariantError, DecodeResult };
    use std::io;
    use collections::TreeMap;

    #[deriving(Eq, Encodable, Decodable, Show)]
    enum Animal {
        Dog,
        Frog(~str, int)
    }

    #[deriving(Eq, Encodable, Decodable, Show)]
    struct Inner {
        a: (),
        b: uint,
        c: Vec<~str>,
    }

    #[deriving(Eq, Encodable, Decodable, Show)]
    struct Outer {
        inner: Vec<Inner>,
    }

    fn mk_object(items: &[(~str, Json)]) -> Json {
        let mut d = ~TreeMap::new();

        for item in items.iter() {
            match *item {
                (ref key, ref value) => { d.insert((*key).clone(), (*value).clone()); },
            }
        };

        Object(d)
    }

    #[test]
    fn test_write_null() {
        assert_eq!(Null.to_str(), ~"null");
        assert_eq!(Null.to_pretty_str(), ~"null");
    }


    #[test]
    fn test_write_number() {
        assert_eq!(Number(3.0).to_str(), ~"3");
        assert_eq!(Number(3.0).to_pretty_str(), ~"3");

        assert_eq!(Number(3.1).to_str(), ~"3.1");
        assert_eq!(Number(3.1).to_pretty_str(), ~"3.1");

        assert_eq!(Number(-1.5).to_str(), ~"-1.5");
        assert_eq!(Number(-1.5).to_pretty_str(), ~"-1.5");

        assert_eq!(Number(0.5).to_str(), ~"0.5");
        assert_eq!(Number(0.5).to_pretty_str(), ~"0.5");
    }

    #[test]
    fn test_write_str() {
        assert_eq!(String(~"").to_str(), ~"\"\"");
        assert_eq!(String(~"").to_pretty_str(), ~"\"\"");

        assert_eq!(String(~"foo").to_str(), ~"\"foo\"");
        assert_eq!(String(~"foo").to_pretty_str(), ~"\"foo\"");
    }

    #[test]
    fn test_write_bool() {
        assert_eq!(Boolean(true).to_str(), ~"true");
        assert_eq!(Boolean(true).to_pretty_str(), ~"true");

        assert_eq!(Boolean(false).to_str(), ~"false");
        assert_eq!(Boolean(false).to_pretty_str(), ~"false");
    }

    #[test]
    fn test_write_list() {
        assert_eq!(List(vec!()).to_str(), ~"[]");
        assert_eq!(List(vec!()).to_pretty_str(), ~"[]");

        assert_eq!(List(vec![Boolean(true)]).to_str(), ~"[true]");
        assert_eq!(
            List(vec![Boolean(true)]).to_pretty_str(),
            ~"\
            [\n  \
                true\n\
            ]"
        );

        let long_test_list = List(vec![
            Boolean(false),
            Null,
            List(vec![String(~"foo\nbar"), Number(3.5)])]);

        assert_eq!(long_test_list.to_str(),
            ~"[false,null,[\"foo\\nbar\",3.5]]");
        assert_eq!(
            long_test_list.to_pretty_str(),
            ~"\
            [\n  \
                false,\n  \
                null,\n  \
                [\n    \
                    \"foo\\nbar\",\n    \
                    3.5\n  \
                ]\n\
            ]"
        );
    }

    #[test]
    fn test_write_object() {
        assert_eq!(mk_object([]).to_str(), ~"{}");
        assert_eq!(mk_object([]).to_pretty_str(), ~"{}");

        assert_eq!(
            mk_object([(~"a", Boolean(true))]).to_str(),
            ~"{\"a\":true}"
        );
        assert_eq!(
            mk_object([(~"a", Boolean(true))]).to_pretty_str(),
            ~"\
            {\n  \
                \"a\": true\n\
            }"
        );

        let complex_obj = mk_object([
                (~"b", List(vec![
                    mk_object([(~"c", String(~"\x0c\r"))]),
                    mk_object([(~"d", String(~""))])
                ]))
            ]);

        assert_eq!(
            complex_obj.to_str(),
            ~"{\
                \"b\":[\
                    {\"c\":\"\\f\\r\"},\
                    {\"d\":\"\"}\
                ]\
            }"
        );
        assert_eq!(
            complex_obj.to_pretty_str(),
            ~"\
            {\n  \
                \"b\": [\n    \
                    {\n      \
                        \"c\": \"\\f\\r\"\n    \
                    },\n    \
                    {\n      \
                        \"d\": \"\"\n    \
                    }\n  \
                ]\n\
            }"
        );

        let a = mk_object([
            (~"a", Boolean(true)),
            (~"b", List(vec![
                mk_object([(~"c", String(~"\x0c\r"))]),
                mk_object([(~"d", String(~""))])
            ]))
        ]);

        // We can't compare the strings directly because the object fields be
        // printed in a different order.
        assert_eq!(a.clone(), from_str(a.to_str()).unwrap());
        assert_eq!(a.clone(), from_str(a.to_pretty_str()).unwrap());
    }

    fn with_str_writer(f: |&mut io::Writer|) -> ~str {
        use std::io::MemWriter;
        use std::str;

        let mut m = MemWriter::new();
        f(&mut m as &mut io::Writer);
        str::from_utf8_owned(m.unwrap()).unwrap()
    }

    #[test]
    fn test_write_enum() {
        let animal = Dog;
        assert_eq!(
            with_str_writer(|wr| {
                let mut encoder = Encoder::new(wr);
                animal.encode(&mut encoder).unwrap();
            }),
            ~"\"Dog\""
        );
        assert_eq!(
            with_str_writer(|wr| {
                let mut encoder = Encoder::new_pretty(wr);
                animal.encode(&mut encoder).unwrap();
            }),
            ~"\"Dog\""
        );

        let animal = Frog(~"Henry", 349);
        assert_eq!(
            with_str_writer(|wr| {
                let mut encoder = Encoder::new(wr);
                animal.encode(&mut encoder).unwrap();
            }),
            ~"{\"variant\":\"Frog\",\"fields\":[\"Henry\",349]}"
        );
        assert_eq!(
            with_str_writer(|wr| {
                let mut encoder = Encoder::new_pretty(wr);
                animal.encode(&mut encoder).unwrap();
            }),
            ~"\
            [\n  \
                \"Frog\",\n  \
                \"Henry\",\n  \
                349\n\
            ]"
        );
    }

    #[test]
    fn test_write_some() {
        let value = Some(~"jodhpurs");
        let s = with_str_writer(|wr| {
            let mut encoder = Encoder::new(wr);
            value.encode(&mut encoder).unwrap();
        });
        assert_eq!(s, ~"\"jodhpurs\"");

        let value = Some(~"jodhpurs");
        let s = with_str_writer(|wr| {
            let mut encoder = Encoder::new_pretty(wr);
            value.encode(&mut encoder).unwrap();
        });
        assert_eq!(s, ~"\"jodhpurs\"");
    }

    #[test]
    fn test_write_none() {
        let value: Option<~str> = None;
        let s = with_str_writer(|wr| {
            let mut encoder = Encoder::new(wr);
            value.encode(&mut encoder).unwrap();
        });
        assert_eq!(s, ~"null");

        let s = with_str_writer(|wr| {
            let mut encoder = Encoder::new(wr);
            value.encode(&mut encoder).unwrap();
        });
        assert_eq!(s, ~"null");
    }

    #[test]
    fn test_trailing_characters() {
        assert_eq!(from_str("nulla"),
            Err(ParseError(~"trailing characters", 1u, 5u)));
        assert_eq!(from_str("truea"),
            Err(ParseError(~"trailing characters", 1u, 5u)));
        assert_eq!(from_str("falsea"),
            Err(ParseError(~"trailing characters", 1u, 6u)));
        assert_eq!(from_str("1a"),
            Err(ParseError(~"trailing characters", 1u, 2u)));
        assert_eq!(from_str("[]a"),
            Err(ParseError(~"trailing characters", 1u, 3u)));
        assert_eq!(from_str("{}a"),
            Err(ParseError(~"trailing characters", 1u, 3u)));
    }

    #[test]
    fn test_read_identifiers() {
        assert_eq!(from_str("n"),
            Err(ParseError(~"invalid syntax", 1u, 2u)));
        assert_eq!(from_str("nul"),
            Err(ParseError(~"invalid syntax", 1u, 4u)));

        assert_eq!(from_str("t"),
            Err(ParseError(~"invalid syntax", 1u, 2u)));
        assert_eq!(from_str("truz"),
            Err(ParseError(~"invalid syntax", 1u, 4u)));

        assert_eq!(from_str("f"),
            Err(ParseError(~"invalid syntax", 1u, 2u)));
        assert_eq!(from_str("faz"),
            Err(ParseError(~"invalid syntax", 1u, 3u)));

        assert_eq!(from_str("null"), Ok(Null));
        assert_eq!(from_str("true"), Ok(Boolean(true)));
        assert_eq!(from_str("false"), Ok(Boolean(false)));
        assert_eq!(from_str(" null "), Ok(Null));
        assert_eq!(from_str(" true "), Ok(Boolean(true)));
        assert_eq!(from_str(" false "), Ok(Boolean(false)));
    }

    #[test]
    fn test_decode_identifiers() {
        let v: () = from_str("null").unwrap();
        assert_eq!(v, ());

        let v: bool = from_str("true").unwrap();
        assert_eq!(v, true);

        let v: bool = from_str("false").unwrap();
        assert_eq!(v, false);
    }

    #[test]
    fn test_read_number() {
        assert_eq!(from_str("+"),
            Err(ParseError(~"invalid syntax", 1u, 1u)));
        assert_eq!(from_str("."),
            Err(ParseError(~"invalid syntax", 1u, 1u)));

        assert_eq!(from_str("-"),
            Err(ParseError(~"invalid number", 1u, 2u)));
        assert_eq!(from_str("00"),
            Err(ParseError(~"invalid number", 1u, 2u)));
        assert_eq!(from_str("1."),
            Err(ParseError(~"invalid number", 1u, 3u)));
        assert_eq!(from_str("1e"),
            Err(ParseError(~"invalid number", 1u, 3u)));
        assert_eq!(from_str("1e+"),
            Err(ParseError(~"invalid number", 1u, 4u)));

        assert_eq!(from_str("3"), Ok(Number(3.0)));
        assert_eq!(from_str("3.1"), Ok(Number(3.1)));
        assert_eq!(from_str("-1.2"), Ok(Number(-1.2)));
        assert_eq!(from_str("0.4"), Ok(Number(0.4)));
        assert_eq!(from_str("0.4e5"), Ok(Number(0.4e5)));
        assert_eq!(from_str("0.4e+15"), Ok(Number(0.4e15)));
        assert_eq!(from_str("0.4e-01"), Ok(Number(0.4e-01)));
        assert_eq!(from_str(" 3 "), Ok(Number(3.0)));
    }

    #[test]
    fn test_decode_numbers() {
        let v: f64 = from_str("3").unwrap();
        assert_eq!(v, 3.0);

        let v: f64 = from_str("3.1").unwrap();
        assert_eq!(v, 3.1);

        let v: f64 = from_str("-1.2").unwrap();
        let v: f64 = Decodable::decode(&mut decoder).unwrap();
        assert_eq!(v, -1.2);

        let v: f64 = from_str("0.4").unwrap();
        assert_eq!(v, 0.4);

        let v: f64 = from_str("0.4e5").unwrap();
        assert_eq!(v, 0.4e5);

        let v: f64 = from_str("0.4e15").unwrap();
        assert_eq!(v, 0.4e15);

        let v: f64 = from_str("0.4e-01").unwrap();
        assert_eq!(v, 0.4e-01);
    }

    #[test]
    fn test_read_str() {
        assert_eq!(from_str("\""),
            Err(ParseError(~"EOF while parsing string", 1u, 2u)));
        assert_eq!(from_str("\"lol"),
            Err(ParseError(~"EOF while parsing string", 1u, 5u)));

        assert_eq!(from_str("\"\""), Ok(String(~"")));
        assert_eq!(from_str("\"foo\""), Ok(String(~"foo")));
        assert_eq!(from_str("\"\\\"\""), Ok(String(~"\"")));
        assert_eq!(from_str("\"\\b\""), Ok(String(~"\x08")));
        assert_eq!(from_str("\"\\n\""), Ok(String(~"\n")));
        assert_eq!(from_str("\"\\r\""), Ok(String(~"\r")));
        assert_eq!(from_str("\"\\t\""), Ok(String(~"\t")));
        assert_eq!(from_str(" \"foo\" "), Ok(String(~"foo")));
        assert_eq!(from_str("\"\\u12ab\""), Ok(String(~"\u12ab")));
        assert_eq!(from_str("\"\\uAB12\""), Ok(String(~"\uAB12")));
    }

    #[test]
    fn test_decode_str() {
        let v: ~str = from_str("\"\"").unwrap();
        assert_eq!(v, ~"");

        let v: ~str = from_str("\"foo\"").unwrap();
        assert_eq!(v, ~"foo");

        let v: ~str = from_str("\"\\\"\"").unwrap();
        assert_eq!(v, ~"\"");

        let v: ~str = from_str("\"\\b\"").unwrap();
        assert_eq!(v, ~"\x08");

        let v: ~str = from_str("\"\\n\"").unwrap();
        assert_eq!(v, ~"\n");

        let v: ~str = from_str("\"\\r\"").unwrap();
        assert_eq!(v, ~"\r");

        let v: ~str = from_str("\"\\t\"").unwrap();
        assert_eq!(v, ~"\t");

        let v: ~str = from_str("\"\\u12ab\"").unwrap();
        assert_eq!(v, ~"\u12ab");

        let v: ~str = from_str("\"\\uAB12\"").unwrap();
        assert_eq!(v, ~"\uAB12");
    }

    #[test]
    fn test_read_list() {
        assert_eq!(from_str("["),
            Err(ParseError(~"EOF while parsing value", 1u, 2u)));
        assert_eq!(from_str("[1"),
            Err(ParseError(~"EOF while parsing list", 1u, 3u)));
        assert_eq!(from_str("[1,"),
            Err(ParseError(~"EOF while parsing value", 1u, 4u)));
        assert_eq!(from_str("[1,]"),
            Err(ParseError(~"invalid syntax", 1u, 4u)));
        assert_eq!(from_str("[6 7]"),
            Err(ParseError(~"expected `,` or `]`", 1u, 4u)));

        assert_eq!(from_str("[]"), Ok(List(vec![])));
        assert_eq!(from_str("[ ]"), Ok(List(vec![])));
        assert_eq!(from_str("[true]"), Ok(List(vec![Boolean(true)])));
        assert_eq!(from_str("[ false ]"), Ok(List(vec![Boolean(false)])));
        assert_eq!(from_str("[null]"), Ok(List(vec![Null])));
        assert_eq!(from_str("[3, 1]"),
                     Ok(List(vec![Number(3.0), Number(1.0)])));
        assert_eq!(from_str("\n[3, 2]\n"),
                     Ok(List(vec![Number(3.0), Number(2.0)])));
        assert_eq!(from_str("[2, [4, 1]]"),
               Ok(List(vec![Number(2.0), List(vec![Number(4.0), Number(1.0)])])));
    }

    #[test]
    fn test_decode_list() {
        let v: Vec<()> = from_str("[]").unwrap();
        assert_eq!(v, Vec::new());

        let v: Vec<()> = from_str("[null]").unwrap();
        assert_eq!(v, Vec::new());

        let v: Vec<bool> = from_str("[true]").unwrap();
        assert_eq!(v, vec![true]);

        let v: Vec<bool> = from_str("[true]").unwrap();
        assert_eq!(v, vec![true]);

        let v: Vec<int> = from_str("[3, 1]").unwrap();
        assert_eq!(v, vec![3, 1]);

        let v: Vec<Vec<uint>> = from_str("[[3], [1, 2]]").unwrap();
        assert_eq!(v, vec![vec![3], vec![1, 2]]);
    }

    #[test]
    fn test_read_object() {
        assert_eq!(from_str("{"),
            Err(ParseError(~"EOF while parsing object", 1u, 2u)));
        assert_eq!(from_str("{ "),
            Err(ParseError(~"EOF while parsing object", 1u, 3u)));
        assert_eq!(from_str("{1"),
            Err(ParseError(~"key must be a string", 1u, 2u)));
        assert_eq!(from_str("{ \"a\""),
            Err(ParseError(~"EOF while parsing object", 1u, 6u)));
        assert_eq!(from_str("{\"a\""),
            Err(ParseError(~"EOF while parsing object", 1u, 5u)));
        assert_eq!(from_str("{\"a\" "),
            Err(ParseError(~"EOF while parsing object", 1u, 6u)));

        assert_eq!(from_str("{\"a\" 1"),
            Err(ParseError(~"expected `:`", 1u, 6u)));
        assert_eq!(from_str("{\"a\":"),
            Err(ParseError(~"EOF while parsing value", 1u, 6u)));
        assert_eq!(from_str("{\"a\":1"),
            Err(ParseError(~"EOF while parsing object", 1u, 7u)));
        assert_eq!(from_str("{\"a\":1 1"),
            Err(ParseError(~"expected `,` or `}`", 1u, 8u)));
        assert_eq!(from_str("{\"a\":1,"),
            Err(ParseError(~"EOF while parsing object", 1u, 8u)));

        assert_eq!(from_str("{}").unwrap(), mk_object([]));
        assert_eq!(from_str("{\"a\": 3}").unwrap(),
                  mk_object([(~"a", Number(3.0))]));

        assert_eq!(from_str(
                      "{ \"a\": null, \"b\" : true }").unwrap(),
                  mk_object([
                      (~"a", Null),
                      (~"b", Boolean(true))]));
        assert_eq!(from_str("\n{ \"a\": null, \"b\" : true }\n").unwrap(),
                  mk_object([
                      (~"a", Null),
                      (~"b", Boolean(true))]));
        assert_eq!(from_str(
                      "{\"a\" : 1.0 ,\"b\": [ true ]}").unwrap(),
                  mk_object([
                      (~"a", Number(1.0)),
                      (~"b", List(vec![Boolean(true)]))
                  ]));
        assert_eq!(from_str(
                      ~"{" +
                          "\"a\": 1.0, " +
                          "\"b\": [" +
                              "true," +
                              "\"foo\\nbar\", " +
                              "{ \"c\": {\"d\": null} } " +
                          "]" +
                      "}").unwrap(),
                  mk_object([
                      (~"a", Number(1.0)),
                      (~"b", List(vec![
                          Boolean(true),
                          String(~"foo\nbar"),
                          mk_object([
                              (~"c", mk_object([(~"d", Null)]))
                          ])
                      ]))
                  ]));
    }

    #[test]
    fn test_decode_struct() {
        let s = ~"{
            \"inner\": [
                { \"a\": null, \"b\": 2, \"c\": [\"abc\", \"xyz\"] }
            ]
        }";
        let v: Outer = from_str(s).unwrap();
        assert_eq!(
            v,
            Outer {
                inner: vec![
                    Inner { a: (), b: 2, c: vec![~"abc", ~"xyz"] }
                ]
            }
        );
    }

    #[test]
    fn test_decode_option() {
        let value: Option<~str> = from_str("null").unwrap();
        assert_eq!(value, None);

        let value: Option<~str> = from_str("\"jodhpurs\"").unwrap();
        assert_eq!(value, Some(~"jodhpurs"));
    }

    #[test]
    fn test_decode_enum() {
        let value: Animal = from_str("\"Dog\"").unwrap();
        assert_eq!(value, Dog);

        let s = "{\"variant\":\"Frog\",\"fields\":[\"Henry\",349]}";
        let value: Animal = from_str(s).unwrap();
        assert_eq!(value, Frog(~"Henry", 349));
    }

    #[test]
    fn test_decode_map() {
        let s = ~"{\"a\": \"Dog\", \"b\": {\"variant\":\"Frog\",\"fields\":[\"Henry\", 349]}}";
        let mut map: TreeMap<~str, Animal> = from_str(s).unwrap();

        assert_eq!(map.pop(&~"a"), Some(Dog));
        assert_eq!(map.pop(&~"b"), Some(Frog(~"Henry", 349)));
    }

    #[test]
    fn test_multiline_errors() {
        assert_eq!(from_str("{\n  \"foo\":\n \"bar\""),
            Err(ParseError(~"EOF while parsing object", 3u, 8u)));
    }

    #[deriving(Decodable)]
    struct DecodeStruct {
        x: f64,
        y: bool,
        z: ~str,
        w: Vec<DecodeStruct>
    }
    #[deriving(Decodable)]
    enum DecodeEnum {
        A(f64),
        B(~str)
    }
    fn check_err<T: Decodable<Decoder, Error>>(to_parse: &'static str, expected: Error) {
        let res: DecodeResult<T> = from_str(to_parse);
        match res {
            Ok(_) => fail!("`{}` parsed & decoded ok, expecting error `{}`",
                              to_parse, expected),
            Err(ParseError(e, _, _)) => fail!("`{}` is not valid json: {}",
                                           to_parse, e),
            Err(e) => {
                assert_eq!(e, expected);
            }

        }
    }
    #[test]
    fn test_decode_errors_struct() {
        check_err::<DecodeStruct>("[]", ExpectedError(~"Object", ~"[]"));
        check_err::<DecodeStruct>("{\"x\": true, \"y\": true, \"z\": \"\", \"w\": []}",
                                  ExpectedError(~"Number", ~"true"));
        check_err::<DecodeStruct>("{\"x\": 1, \"y\": [], \"z\": \"\", \"w\": []}",
                                  ExpectedError(~"Boolean", ~"[]"));
        check_err::<DecodeStruct>("{\"x\": 1, \"y\": true, \"z\": {}, \"w\": []}",
                                  ExpectedError(~"String", ~"{}"));
        check_err::<DecodeStruct>("{\"x\": 1, \"y\": true, \"z\": \"\", \"w\": null}",
                                  ExpectedError(~"List", ~"null"));
        check_err::<DecodeStruct>("{\"x\": 1, \"y\": true, \"z\": \"\"}",
                                  MissingFieldError(~"w"));
    }
    #[test]
    fn test_decode_errors_enum() {
        check_err::<DecodeEnum>("{}",
                                MissingFieldError(~"variant"));
        check_err::<DecodeEnum>("{\"variant\": 1}",
                                ExpectedError(~"String", ~"1"));
        check_err::<DecodeEnum>("{\"variant\": \"A\"}",
                                MissingFieldError(~"fields"));
        check_err::<DecodeEnum>("{\"variant\": \"A\", \"fields\": null}",
                                ExpectedError(~"List", ~"null"));
        check_err::<DecodeEnum>("{\"variant\": \"C\", \"fields\": []}",
                                UnknownVariantError(~"C"));
    }

    #[test]
    fn test_find(){
        let json_value = from_str("{\"dog\" : \"cat\"}").unwrap();
        let found_str = json_value.find(&~"dog");
        assert!(found_str.is_some() && found_str.unwrap().as_string().unwrap() == &"cat");
    }

    #[test]
    fn test_find_path(){
        let json_value = from_str("{\"dog\":{\"cat\": {\"mouse\" : \"cheese\"}}}").unwrap();
        let found_str = json_value.find_path(&[&~"dog", &~"cat", &~"mouse"]);
        assert!(found_str.is_some() && found_str.unwrap().as_string().unwrap() == &"cheese");
    }

    #[test]
    fn test_search(){
        let json_value = from_str("{\"dog\":{\"cat\": {\"mouse\" : \"cheese\"}}}").unwrap();
        let found_str = json_value.search(&~"mouse").and_then(|j| j.as_string());
        assert!(found_str.is_some());
        assert!(found_str.unwrap() == &"cheese");
    }

    #[test]
    fn test_is_object(){
        let json_value = from_str("{}").unwrap();
        assert!(json_value.is_object());
    }

    #[test]
    fn test_as_object(){
        let json_value = from_str("{}").unwrap();
        let json_object = json_value.as_object();
        assert!(json_object.is_some());
    }

    #[test]
    fn test_is_list(){
        let json_value = from_str("[1, 2, 3]").unwrap();
        assert!(json_value.is_list());
    }

    #[test]
    fn test_as_list(){
        let json_value = from_str("[1, 2, 3]").unwrap();
        let json_list = json_value.as_list();
        let expected_length = 3;
        assert!(json_list.is_some() && json_list.unwrap().len() == expected_length);
    }

    #[test]
    fn test_is_string(){
        let json_value = from_str("\"dog\"").unwrap();
        assert!(json_value.is_string());
    }

    #[test]
    fn test_as_string(){
        let json_value = from_str("\"dog\"").unwrap();
        let json_str = json_value.as_string();
        let expected_str = &"dog";
        assert_eq!(json_str, Some(expected_str));
    }

    #[test]
    fn test_is_number(){
        let json_value = from_str("12").unwrap();
        assert!(json_value.is_number());
    }

    #[test]
    fn test_as_number(){
        let json_value = from_str("12").unwrap();
        let json_num = json_value.as_number();
        let expected_num = 12f64;
        assert!(json_num.is_some() && json_num.unwrap() == expected_num);
    }

    #[test]
    fn test_is_boolean(){
        let json_value = from_str("false").unwrap();
        assert!(json_value.is_boolean());
    }

    #[test]
    fn test_as_boolean(){
        let json_value = from_str("false").unwrap();
        let json_bool = json_value.as_boolean();
        let expected_bool = false;
        assert!(json_bool.is_some() && json_bool.unwrap() == expected_bool);
    }

    #[test]
    fn test_is_null(){
        let json_value = from_str("null").unwrap();
        assert!(json_value.is_null());
    }

    #[test]
    fn test_as_null(){
        let json_value = from_str("null").unwrap();
        let json_null = json_value.as_null();
        let expected_null = ();
        assert!(json_null.is_some() && json_null.unwrap() == expected_null);
    }

    #[test]
    fn test_encode_hashmap_with_numeric_key() {
        use std::str::from_utf8;
        use std::io::Writer;
        use std::io::MemWriter;
        use collections::HashMap;
        let mut hm: HashMap<uint, bool> = HashMap::new();
        hm.insert(1, true);
        let mut mem_buf = MemWriter::new();
        {
            let mut encoder = Encoder::new(&mut mem_buf as &mut io::Writer);
            hm.encode(&mut encoder).unwrap();
        }
        let bytes = mem_buf.unwrap();
        let json_str = from_utf8(bytes).unwrap();
        match from_str(json_str) {
            Err(_) => fail!("Unable to parse json_str: {:?}", json_str),
            _ => {} // it parsed and we are good to go
        }
    }
    #[test]
    fn test_prettyencode_hashmap_with_numeric_key() {
        use std::str::from_utf8;
        use std::io::Writer;
        use std::io::MemWriter;
        use collections::HashMap;
        let mut hm: HashMap<uint, bool> = HashMap::new();
        hm.insert(1, true);
        let mut mem_buf = MemWriter::new();
        {
            let mut encoder = Encoder::new_pretty(&mut mem_buf as &mut io::Writer);
            hm.encode(&mut encoder).unwrap();
        }
        let bytes = mem_buf.unwrap();
        let json_str = from_utf8(bytes).unwrap();
        match from_str(json_str) {
            Err(_) => fail!("Unable to parse json_str: {:?}", json_str),
            _ => {} // it parsed and we are good to go
        }
    }
    #[test]
    fn test_hashmap_with_numeric_key_can_handle_double_quote_delimited_key() {
        use collections::HashMap;
        let json_str = "{\"1\":true}";
        let _hm: HashMap<uint, bool> = from_str(json_str).unwrap();
    }
}
