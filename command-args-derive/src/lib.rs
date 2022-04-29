extern crate proc_macro;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Error};

#[proc_macro_derive(CommandArgsBlock, attributes(argtoken))]
pub fn derive_command_args_block(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    expand::expand(input)
        .unwrap_or_else(Error::into_compile_error)
        .into()
}

mod expand {
    use proc_macro2::{Span, TokenStream};
    use quote::{quote, quote_spanned};
    use syn::{
        parse_quote, spanned::Spanned, DeriveInput, Error, GenericArgument, GenericParam, Ident,
        Lifetime, LifetimeDef, LitStr, Path, PathArguments, PathSegment, Result, Type, TypePath, Variant,
    };

    pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
        let default_lifetime = LifetimeDef::new(Lifetime::new("'a", Span::call_site()));
        let mut impl_generics = input.generics.clone();
        if impl_generics.lifetimes().next().is_none() {
            impl_generics
                .params
                .push(GenericParam::Lifetime(default_lifetime));
        }
        let lifetime = &impl_generics.lifetimes().next().unwrap().lifetime;
        let (_, ty_generics, where_clause) = input.generics.split_for_impl();
        let (impl_generics_tok, _, _) = impl_generics.split_for_impl();

        let name = &input.ident;
        let parse_maybe_fn_content = parse_maybe_fn_content(&input)?;
        let parse_fn = quote! {
            fn parse_maybe(args: &mut &[&#lifetime str]) -> Result<Option<Self>, ::command_args::Error> {
                #parse_maybe_fn_content
            }
        };

        Ok(quote! {
            impl #impl_generics_tok ::command_args::CommandArgs<#lifetime> for #name #ty_generics #where_clause {
                #parse_fn
            }
        })
    }

    fn parse_maybe_fn_content(input: &DeriveInput) -> Result<TokenStream> {
        match &input.data {
            syn::Data::Struct(s) => Ok(struct_parse(s, input)?),
            syn::Data::Enum(e) => Ok(enum_parse(e, input)?),
            syn::Data::Union(_) => Err(Error::new(input.span(), "union not supported")),
        }
    }

    /// Turns an enum into content of parse_maybe
    fn enum_parse(e: &syn::DataEnum, input: &DeriveInput) -> Result<TokenStream> {
        let mut result = Vec::new();
        for variant in &e.variants {
            let token_path: Path = parse_quote!(argtoken);
            let token = variant
                .attrs
                .iter()
                .find(|a| a.path == token_path)
                .map(|a| a.parse_args::<LitStr>())
                .transpose()?;

            let name = LitStr::new(&variant.ident.to_string(), variant.span());
            let variant_token = token.unwrap_or(name);

            result.push((variant_token, variant));
        }

        let mut variant_matches = Vec::new();

        for (variant_token, variant) in result {
            variant_matches.push(enum_variant_parse(variant, variant_token)?);
        }

        let span = input.span();
        Ok(quote_spanned! {span=>
            let result = match args.get(0) {
                #(#variant_matches)*
                _ => None
            };
            if result.is_some() {
                *args = &args[1..];
            }

            Ok(result)
        })
    }

    /// Turns a variant to a match arm
    /// ```rust
    /// enum NxOrXx {
    ///     Nx,
    ///  // ^ current variant
    ///     Xx,
    /// }
    /// ```
    /// into
    /// ```
    /// Some(a) if a.eq_ignore_ascii_case("NX") => Some(Self::Nx)
    /// ```
    fn enum_variant_parse(variant: &Variant, variant_token: LitStr) -> Result<TokenStream> {
        let span = variant.span();
        let ident = &variant.ident;

        Ok(quote_spanned! {span=>
            Some(a) if a.eq_ignore_ascii_case(#variant_token) => Some(Self::#ident),
        })
    }

    /// Turns a struct into content of parse_maybe
    fn struct_parse(s: &syn::DataStruct, input: &DeriveInput) -> Result<TokenStream> {
        let token_path: Path = parse_quote!(argtoken);
        let token = input
            .attrs
            .iter()
            .find(|a| a.path == token_path)
            .map(|a| a.parse_args::<LitStr>())
            .transpose()?;
        let parse_token = if let Some(token) = token {
            let token_span = token.span();
            quote_spanned! {token_span=>
                match args.get(0) {
                    Some(s) if s.eq_ignore_ascii_case(#token) => {
                        *args = &args[1..];
                    },
                    _ => { return Ok(None); }
                }
            }
        } else {
            // Without token, if args is empty => None
            let span = input.span();
            quote_spanned! {span=>
                if args.is_empty() {
                    return Ok(None);
                }
            }
        };
        let parse_fields = match &s.fields {
            syn::Fields::Named(named) => named_fields_parse(named),
            syn::Fields::Unnamed(tuple) => unnamed_fields_parse(tuple),
            syn::Fields::Unit => Ok(quote! { Ok(Some(Self)) }),
        }?;

        let span = input.span();
        Ok(quote_spanned! {span=>
            #parse_token
            #parse_fields
        })
    }

    // Get last path segment of a type
    // ::std::option::Option<Abc> => Option<Abc>
    fn last_path_segment(ty: &Type) -> Option<&PathSegment> {
        match ty {
            &Type::Path(TypePath {
                qself: None,
                path:
                    Path {
                        segments: ref seg,
                        leading_colon: _,
                    },
            }) => seg.last(),
            _ => None,
        }
    }

    // if this type is Option and return the Wrapped type
    fn option_inner_type(ty: &Type) -> Option<&GenericArgument> {
        match last_path_segment(ty) {
            Some(PathSegment {
                ident,
                arguments: PathArguments::AngleBracketed(ref gen_arg),
            }) if ident == "Option" => gen_arg.args.first(),
            _ => None,
        }
    }

    fn unnamed_fields_parse(unnamed: &syn::FieldsUnnamed) -> Result<TokenStream> {
        let mut count = 0;
        let declare_vars = unnamed.unnamed.iter().map(|f| {
            let ty = &f.ty;
            let ty_span = f.ty.span();
            let var_name = Ident::new(&format!("field_{}", count), ty_span);
            count += 1;

            match option_inner_type(ty) {
                Some(inner_ty) => quote_spanned! {ty_span=>
                    let #var_name = <#inner_ty as ::command_args::CommandArgs>::parse_maybe(args)?;
                },
                None => quote_spanned! {ty_span=>
                    let #var_name = <#ty as ::command_args::CommandArgs>::parse_maybe(args)?
                        .ok_or(::command_args::Error::InvalidLength)?;
                },
            }
        });

        let mut count = 0;
        let return_fields = unnamed.unnamed.iter().map(|f| {
            let r = Ident::new(&format!("field_{}", count), f.ty.span());
            count += 1;
            r
        });

        let span = unnamed.span();
        Ok(quote_spanned! {span =>
            #(#declare_vars)*
            Ok(Some(Self( #(#return_fields),* )))
        })
    }

    fn named_fields_parse(named: &syn::FieldsNamed) -> Result<TokenStream> {
        let declare_vars = named.named.iter().map(|f| {
            let ty = &f.ty;
            let ty_span = f.ty.span();
            let var_name = f.ident.as_ref().unwrap();

            match option_inner_type(ty) {
                Some(inner_ty) => quote_spanned! {ty_span=>
                    let #var_name = <#inner_ty as ::command_args::CommandArgs>::parse_maybe(args)?;
                },
                None => quote_spanned! {ty_span=>
                    let #var_name = <#ty as ::command_args::CommandArgs>::parse_maybe(args)?
                        .ok_or(::command_args::Error::InvalidLength)?;
                },
            }
        });
        let return_fields = named.named.iter().map(|f| f.ident.as_ref());

        let span = named.span();
        Ok(quote_spanned! {span =>
            #(#declare_vars)*
            Ok(Some(Self { #(#return_fields),* }))
        })
    }
}
