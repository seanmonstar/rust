// Copyright 2014 The Rust Project Developers. See the COPYRIGHT
// file at the top-level directory of this distribution and at
// http://rust-lang.org/COPYRIGHT.
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use ast;
use ast::{MetaItem, Item, Expr,};
use codemap::Span;
use ext::format;
use ext::base::ExtCtxt;
use ext::build::AstBuilder;
use ext::deriving::generic::*;
use ext::deriving::generic::ty::*;
use parse::token;
use ptr::P;

use std::collections::HashMap;
use std::num::Int;

pub fn expand_deriving_show<F>(cx: &mut ExtCtxt,
                               span: Span,
                               mitem: &MetaItem,
                               item: &Item,
                               push: F) where
    F: FnOnce(P<Item>),
{
    // &mut ::std::fmt::Formatter
    let fmtr = Ptr(box Literal(Path::new(vec!("std", "fmt", "Formatter"))),
                   Borrowed(None, ast::MutMutable));

    let trait_def = TraitDef {
        span: span,
        attributes: Vec::new(),
        path: Path::new(vec!("std", "fmt", "Debug")),
        additional_bounds: Vec::new(),
        generics: LifetimeBounds::empty(),
        methods: vec![
            MethodDef {
                name: "fmt",
                generics: LifetimeBounds::empty(),
                explicit_self: borrowed_explicit_self(),
                args: vec!(fmtr),
                ret_ty: Literal(Path::new(vec!("std", "fmt", "Result"))),
                attributes: Vec::new(),
                combine_substructure: combine_substructure(box |a, b, c| {
                    show_substructure(a, b, c)
                })
            },
            MethodDef {
                name: "size_hint",
                generics: LifetimeBounds::empty(),
                explicit_self: borrowed_explicit_self(),
                args: vec![],
                ret_ty: Literal(Path::new(vec!("std", "fmt", "SizeHint"))),
                attributes: Vec::new(),
                combine_substructure: combine_substructure(box |a, b, c| {
                    size_hint_substructure(a, b, c)
                })
            }
        ]
    };
    trait_def.expand(cx, mitem, item, push)
}

/// We construct a format string and then defer to std::fmt, since that
/// knows what's up with formatting and so on.
fn show_substructure(cx: &mut ExtCtxt, span: Span,
                     substr: &Substructure) -> P<Expr> {
    // build `<name>`, `<name>({}, {}, ...)` or `<name> { <field>: {},
    // <field>: {}, ... }` based on the "shape".
    //
    // Easy start: they all start with the name.
    let name = match *substr.fields {
        Struct(_) => substr.type_ident,
        EnumMatching(_, v, _) => v.node.name,
        EnumNonMatchingCollapsed(..) | StaticStruct(..) | StaticEnum(..) => {
            cx.span_bug(span, "nonsensical .fields in `#[derive(Debug)]`")
        }
    };

    let mut format_string = String::from_str(token::get_ident(name).get());
    // the internal fields we're actually formatting
    let mut exprs = Vec::new();

    // Getting harder... making the format string:
    match *substr.fields {
        // unit struct/nullary variant: no work necessary!
        Struct(ref fields) if fields.len() == 0 => {}
        EnumMatching(_, _, ref fields) if fields.len() == 0 => {}

        Struct(ref fields) | EnumMatching(_, _, ref fields) => {
            if fields[0].name.is_none() {
                // tuple struct/"normal" variant

                format_string.push_str("(");

                for (i, field) in fields.iter().enumerate() {
                    if i != 0 { format_string.push_str(", "); }

                    format_string.push_str("{:?}");

                    exprs.push(field.self_.clone());
                }

                format_string.push_str(")");
            } else {
                // normal struct/struct variant

                format_string.push_str(" {{");

                for (i, field) in fields.iter().enumerate() {
                    if i != 0 { format_string.push_str(","); }

                    let name = token::get_ident(field.name.unwrap());
                    format_string.push_str(" ");
                    format_string.push_str(name.get());
                    format_string.push_str(": {:?}");

                    exprs.push(field.self_.clone());
                }

                format_string.push_str(" }}");
            }
        }
        _ => unreachable!()
    }

    // AST construction!
    // we're basically calling
    //
    // format_arg_method!(fmt, write_fmt, "<format_string>", exprs...)
    //
    // but doing it directly via ext::format.
    let formatter = substr.nonself_args[0].clone();

    let meth = cx.ident_of("write_fmt");
    let s = token::intern_and_get_ident(&format_string[]);
    let format_string = cx.expr_str(span, s);

    // phew, not our responsibility any more!

    let args = vec![
        format::expand_preparsed_format_args(cx, span, format_string,
                                             exprs, vec![], HashMap::new())
    ];
    cx.expr_method_call(span, formatter, meth, args)
}

fn size_hint_substructure(cx: &mut ExtCtxt, span: Span,
                          substr: &Substructure) -> P<Expr> {
    let name = match *substr.fields {
        Struct(_) => substr.type_ident,
        EnumMatching(_, v, _) => v.node.name,
        EnumNonMatchingCollapsed(..) | StaticStruct(..) | StaticEnum(..) => {
            cx.span_bug(span, "nonsensical .fields in `#[derive(Debug)]`")
        }
    };
    let mut min = token::get_ident(name).get().len();
    let mut max = Some(min);
    let mut exprs = Vec::new();

    { // borrow checking
        let mut add = |&mut: num| {
            min = min.saturating_add(num);
            max = max.and_then(|n| n.checked_add(num))
        };

        // the internal fields we're actually formatting

        // Getting harder... making the format string:
        match *substr.fields {
            // unit struct/nullary variant: no work necessary!
            Struct(ref fields) if fields.len() == 0 => {}
            EnumMatching(_, _, ref fields) if fields.len() == 0 => {}

            Struct(ref fields) | EnumMatching(_, _, ref fields) => {
                if fields[0].name.is_none() {
                    // tuple struct/"normal" variant
                    add(1); // '('
                    for (i, field) in fields.iter().enumerate() {
                        if i != 0 { add(2); } // ', '
                        exprs.push(field.self_.clone());
                    }
                    add(1); // ')'
                } else {
                    // normal struct/struct variant

                    add(2); // ' {'
                    for (i, field) in fields.iter().enumerate() {
                        if i != 0 { add(1); } // ','
                        add(1); // ' '
                        add(token::get_ident(field.name.unwrap()).get().len());
                        add(2); // ': '
                        exprs.push(field.self_.clone());
                    }

                    add(2); // ' }'
                }
            }
            _ => unreachable!()
        }
    }

    let max_expr = if let Some(max) = max {
        cx.expr_some(span, cx.expr_usize(span, max))
    } else {
        cx.expr_none(span)
    };
    let hint = cx.expr_struct(span,
                              cx.path_global(span,
                                             vec![cx.ident_of("std"),
                                                  cx.ident_of("fmt"),
                                                  cx.ident_of("SizeHint")]),
                              vec![cx.field_imm(span,
                                                cx.ident_of("min"),
                                                cx.expr_usize(span, min)),
                                   cx.field_imm(span,
                                                cx.ident_of("max"),
                                                max_expr)]);
    exprs.into_iter().fold(hint, |sum, expr| {
        cx.expr_binary(span,
                       ast::BiAdd,
                       sum,
                       cx.expr_call_global(span,
                                           vec![cx.ident_of("std"),
                                                cx.ident_of("fmt"),
                                                cx.ident_of("Debug"),
                                                cx.ident_of("size_hint")],
                                           vec![cx.expr_addr_of(span, expr)]))
    })
}
