use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput};
use quote::{format_ident, quote};
use proc_linalg::Variables;

#[proc_macro_derive(ClosureInverter, attributes(invert))]
pub fn is_closure_inverter(tokens: TokenStream) -> TokenStream {
    let ast = parse_macro_input!(tokens as DeriveInput);
    let struct_ident = ast.ident;
    if ast.attrs.len() == 1 {
        let attr = &ast.attrs[0];
        if attr.path.is_ident("invert") {
            let meta = attr.parse_meta().unwrap();
            if let syn::Meta::List(meta_list) = meta {
                assert_eq!(meta_list.nested.len(), 1);
                let lit = meta_list.nested.first().unwrap();
                if let syn::NestedMeta::Lit(syn::Lit::Str(ref ident)) = lit {
                    let closure = ident.parse::<syn::ExprClosure>().unwrap();
                    // can only solve for "a". Needs to expand macro to allow for user defined identifiers
                    let eq = Variables::new(format_ident!("a"), format_ident!("b"));
                    let result = eq.parse_closure(&closure).unwrap();
                    let return_stream = quote!(
                        impl #struct_ident {
                            fn calculate(value: usize) -> usize {
                                let closure = #result;
                                closure(value)
                            }
                        });
                    return_stream.into()
                } else {
                    panic!("Needs to be passed a literal")
                }
            } else {
                panic!("Expected a single string")
            }
        } else {
            panic!("Only can derive with 'invert_me' attribute")
        }
    } else {
        panic!("Requires a single attribute only");
    }
}