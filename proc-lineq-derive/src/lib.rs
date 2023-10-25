#![warn(clippy::panic, clippy::str_to_string, clippy::panicking_unwrap)]

use proc_lineq::ClosureInverter;
use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse, parse_macro_input, DeriveInput, LitStr, Meta};

#[proc_macro_derive(ClosureInverter, attributes(invert))]
pub fn is_closure_inverter(tokens: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(tokens as DeriveInput);
    let struct_ident = ast.ident;
    if ast.attrs.len() == 1 {
        let attr = &ast.attrs[0];
        if attr.path().is_ident("invert") {
            attr.meta.require_list().expect("Unwrap to error");
            // Parse the meta into a string
            match &attr.meta {
                Meta::List(meta_list) => {
                    let closure_str = parse::<LitStr>(meta_list.tokens.clone().into())
                        .expect("Did not receive a string");
                    let closure = closure_str.parse::<syn::ExprClosure>().unwrap();
                    let eq = ClosureInverter::new(format_ident!("a"), format_ident!("b"));
                    let result = eq.solve(&closure).unwrap();
                    let return_stream = quote!(
                    impl #struct_ident {
                        fn calculate(value: usize) -> usize {
                            let closure = #result;
                            closure(value)
                        }
                    });
                    return_stream.into()
                }
                _ => unreachable!(),
            }
        } else {
            quote!(compile_error!("ClosureInverter requires a single invert attribute");).into()
        }
    } else {
        quote!(compile_error!("ClosureInverter requires a single invert attribute");).into()
    }
}
