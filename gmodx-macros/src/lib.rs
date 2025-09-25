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
        "There can only be one argument, and it should be a pointer to the Lua state (gmodx::lua::State)"
    );
    assert!(input.sig.abi.is_none(), "Do not specify an ABI");
    assert!(input.sig.unsafety.is_none(), "Cannot be unsafe");
}

fn genericify_return(item_fn: &mut ItemFn) -> proc_macro2::TokenStream {
    let ItemFn {
        attrs,
        vis,
        sig,
        block,
    } = item_fn.clone();

    let syn::Signature {
        output: return_type,
        inputs,
        ident: name,
        ..
    } = &sig;

    let return_type = match return_type {
        syn::ReturnType::Default => quote!(()),
        syn::ReturnType::Type(_, ty) => quote!(#ty),
    };

    let lua_ident = parse_lua_ident(&inputs[0]);
    let param_ty = match &inputs[0] {
        syn::FnArg::Typed(arg) => &arg.ty,
        syn::FnArg::Receiver(_) => panic!("Needs to be a lua state!"),
    };
    let crate_name = get_crate_name();

    let internal_name = syn::Ident::new(
        &format!("__{name}_internal__"),
        proc_macro2::Span::call_site(),
    );

    let output = quote! {
        #(#attrs)*
        #vis fn #name(#lua_ident: #crate_name::lua::State) -> #crate_name::lua::RustFunctionResult
        {
            {
                trait AssertSame<T> {}
                impl AssertSame<#crate_name::lua::State> for #crate_name::lua::State {}
                fn assert_impl<T: AssertSame<#crate_name::lua::State>>() {}
                assert_impl::<#param_ty>();
            }


            #[inline(always)]
            #(#attrs)*
            fn #internal_name(#lua_ident: #crate_name::lua::State) -> #return_type
            {
                #block
            }

            use #crate_name::lua::FunctionReturn;
            {
                fn assert_send<T: #crate_name::lua::FunctionReturn>() {}
                fn assert() {
                    assert_send::<#return_type>();
                }
            }

            #internal_name(#lua_ident).handle_result(#lua_ident)
        }
    };

    output
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
            #[unsafe(no_mangle)]
            pub extern "C-unwind" fn gmod13_open(#lua_ident: #crate_name::lua::State) -> i32 {
                {
                    trait AssertSame<T> {}
                    impl AssertSame<#crate_name::lua::State> for #crate_name::lua::State {}
                    fn assert_impl<T: AssertSame<#crate_name::lua::State>>() {}
                    assert_impl::<#param_ty>();
                }

                #crate_name::open_close::load_all(#lua_ident);

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
            #[unsafe(no_mangle)]
            pub extern "C-unwind" fn gmod13_close(#lua_ident: #crate_name::lua::State) -> i32 {
                {
                    trait AssertSame<T> {}
                    impl AssertSame<#crate_name::lua::State> for #crate_name::lua::State {}
                    fn assert_impl<T: AssertSame<#crate_name::lua::State>>() {}
                    assert_impl::<#param_ty>();
                }

                #block

                #crate_name::open_close::unload_all(#lua_ident);

                0
            }
        };

        Ok(output.into())
    })
}

#[proc_macro_attribute]
pub fn lua_function(_attr: TokenStream, tokens: TokenStream) -> TokenStream {
    wrap_compile_error!(tokens, {
        let mut input = syn::parse::<ItemFn>(tokens)?;

        check_lua_function(&mut input);

        Ok(genericify_return(&mut input).into())
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
                if !tuple.elems.is_empty() {
                    panic!("Function must not return anything",);
                }
            } else {
                panic!("Function must not return anything",);
            }
        }
    }
}

#[proc_macro]
pub fn compile_timestamp(_input: TokenStream) -> TokenStream {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();

    let expanded = quote! {
        #timestamp
    };

    TokenStream::from(expanded)
}
