use std::sync::atomic::AtomicUsize;
use std::sync::atomic::Ordering;

use proc_macro::TokenStream;
use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::ItemFn;
use syn::Pat;
use syn::PatIdent;

macro_rules! wrap_compile_error {
    ($input:ident, $code:expr) => {{
        let orig_tokens = $input.clone();
        match (|| -> Result<TokenStream, syn::Error> { $code })() {
            Ok(tokens) => tokens,
            Err(_) => return orig_tokens,
        }
    }};
}

fn get_crate_name() -> proc_macro2::TokenStream {
    match crate_name("gmodx") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = syn::Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Err(_) => quote!(::gmodx), // fallback
    }
}

fn parse_lua_ident(input: &syn::FnArg) -> syn::Ident {
    match input {
        syn::FnArg::Typed(arg) => {
            if let Pat::Ident(PatIdent { ident, .. }) = &*arg.pat {
                ident.clone()
            } else {
                panic!("Can't use self for lua functions!")
            }
        }
        syn::FnArg::Receiver(_) => panic!("Needs to be a lua state!"),
    }
}

fn check_lua_function(input: &mut ItemFn) {
    assert!(input.sig.asyncness.is_none(), "Cannot be async");
    assert!(input.sig.constness.is_none(), "Cannot be const");
    assert!(
        input.sig.inputs.len() == 1,
        "There can only be one argument (lua state)"
    );
    assert!(input.sig.abi.is_none(), "Do not specify an ABI");
    assert!(input.sig.unsafety.is_none(), "Cannot be unsafe");
}

#[proc_macro_attribute]
pub fn gmod13_open(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    wrap_compile_error!(tokens, {
        let mut input = syn::parse::<ItemFn>(tokens)?;
        check_gmod13_function(&mut input, "gmod13_open");

        let inputs = &input.sig.inputs;
        let lua_ident = parse_lua_ident(&inputs[0]);
        let param_ty = match &inputs[0] {
            syn::FnArg::Typed(arg) => &arg.ty,
            syn::FnArg::Receiver(_) => panic!("Needs to be a lua state!"),
        };
        let crate_name = get_crate_name();

        let block = input.block;

        let output = quote! {
            #[doc(hidden)]
            pub const __GMOD13_OPEN_EXISTS: () = ();
            const _: () = { let _ = __GMOD13_CLOSE_EXISTS; };

            #[unsafe(no_mangle)]
            pub extern "C-unwind" fn gmod13_open(#lua_ident: #crate_name::lua::State) -> i32 {
                {
                    trait AssertSame<T> {}
                    impl AssertSame<#crate_name::lua::State> for #crate_name::lua::State {}
                    fn assert_impl<T: AssertSame<#crate_name::lua::State>>() {}
                    assert_impl::<#param_ty>();
                }

                #crate_name::open_close::load_all(&#lua_ident);

                #block

                0
            }
        };

        Ok(output.into())
    })
}

#[proc_macro_attribute]
pub fn gmod13_close(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    wrap_compile_error!(tokens, {
        let mut input = syn::parse::<ItemFn>(tokens)?;

        check_gmod13_function(&mut input, "gmod13_close");

        let inputs = &input.sig.inputs;
        let lua_ident = parse_lua_ident(&input.sig.inputs[0]);
        let param_ty = match &inputs[0] {
            syn::FnArg::Typed(arg) => &arg.ty,
            syn::FnArg::Receiver(_) => panic!("Needs to be a lua state!"),
        };
        let crate_name = get_crate_name();

        let block = input.block;

        let output = quote! {
            #[doc(hidden)]
            pub const __GMOD13_CLOSE_EXISTS: () = ();
            const _: () = { let _ = __GMOD13_OPEN_EXISTS; };

            #[unsafe(no_mangle)]
            pub extern "C-unwind" fn gmod13_close(#lua_ident: #crate_name::lua::State) -> i32 {
                {
                    trait AssertSame<T> {}
                    impl AssertSame<#crate_name::lua::State> for #crate_name::lua::State {}
                    fn assert_impl<T: AssertSame<#crate_name::lua::State>>() {}
                    assert_impl::<#param_ty>();
                }

                #block

                #crate_name::open_close::unload_all(&#lua_ident);

                0
            }
        };

        Ok(output.into())
    })
}

fn check_gmod13_function(input: &mut ItemFn, expected_name: &str) {
    check_lua_function(input);

    assert!(
        input.sig.ident == expected_name,
        "Function must be named '{}', found '{}'",
        expected_name,
        input.sig.ident
    );

    match &input.sig.output {
        syn::ReturnType::Default => {}
        syn::ReturnType::Type(_, ty) => {
            // Check if it's the unit type ()
            if let syn::Type::Tuple(tuple) = &**ty {
                assert!(tuple.elems.is_empty(), "Function must not return anything");
            } else {
                panic!("Function must not return anything",);
            }
        }
    }
}

static COUNTER: AtomicUsize = AtomicUsize::new(0);

#[proc_macro]
pub fn unique_id(input: TokenStream) -> TokenStream {
    let unique_str = {
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();

        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "unknown".to_string());
        format!("__GMODX_UNIQUE_ID_{version}_{timestamp}_{counter}")
    };
    let input_str = input.to_string();
    let is_c_string = input_str.trim() == "cstr";
    if is_c_string {
        format!("c\"{unique_str}\"").parse().unwrap()
    } else {
        format!("\"{unique_str}\"").parse().unwrap()
    }
}
