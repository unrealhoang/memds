extern crate proc_macro;
use proc_macro::TokenStream;
use syn::{parse_macro_input, DeriveInput, Error};

#[proc_macro_derive(CommandArgsBlock, attributes(argtoken, argnotoken))]
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
        let mut notoken_variant = None;

        for variant in &e.variants {
            let notoken_path: Path = parse_quote!(argnotoken);
            let is_notoken_variant = variant
                .attrs
                .iter()
                .any(|a| a.path == notoken_path);
            if notoken_variant.is_some() && is_notoken_variant {
                return Err(Error::new(variant.span(), "only one notoken variant allowed"));
            }
            if is_notoken_variant {
                notoken_variant = Some(variant);
                continue
            }

            let token_path: Path = parse_quote!(argtoken);
            let token = variant
                .attrs
                .iter()
                .find(|a| a.path == token_path)
                .map(|a| a.parse_args::<LitStr>())
                .transpose()?;

            let name = LitStr::new(&variant.ident.to_string(), variant.span());
            let variant_token = token.unwrap_or(name);

            result.push((variant, variant_token));
        }

        let mut variant_matches = Vec::new();

        for (variant, variant_token) in result {
            variant_matches.push(enum_variant_parse(variant, variant_token)?);
        }

        let span = input.span();
        let catch_all_arm = if let Some(variant) = notoken_variant {
            let notoken_span = variant.span();
            let notoken_ident = &variant.ident;
            quote_spanned! {notoken_span=>
                _ => Some(Self::#notoken_ident),
            }
        } else {
            quote_spanned! {span=>
                _ => None,
            }
        };
        Ok(quote_spanned! {span=>
            let result = match args.get(0) {
                #(#variant_matches)*
                #catch_all_arm
            };

            Ok(result)
        })
    }

    /// Turns a variant to a match arm
    /// ```rust,ignore
    /// enum NxOrXx {
    ///     Nx,
    ///  // ^ current variant
    ///     Xx,
    /// }
    /// ```
    /// into
    /// ```rust,ignore
    /// Some(a) if a.eq_ignore_ascii_case("NX") => Some(Self::Nx)
    /// ```
    fn enum_variant_parse(variant: &Variant, variant_token: LitStr) -> Result<TokenStream> {
        let span = variant.span();
        let ident = &variant.ident;

        Ok(match &variant.fields {
            syn::Fields::Named(named) => {
                let (field_vars, field_returns) = named_fields_parse(named)?;
                let span = named.span();

                quote_spanned! {span=>
                    Some(a) if a.eq_ignore_ascii_case(#variant_token) => {
                        *args = &args[1..];
                        #field_vars

                        Some(Self::#ident {#field_returns})
                    }
                }
            }
            syn::Fields::Unnamed(tuple) => {
                let (field_vars, field_returns) = unnamed_fields_parse(tuple)?;
                let span = tuple.span();

                quote_spanned! {span=>
                    Some(a) if a.eq_ignore_ascii_case(#variant_token) => {
                        *args = &args[1..];
                        #field_vars

                        Some(Self::#ident(#field_returns))
                    }
                }
            }
            syn::Fields::Unit => {
                quote_spanned! {span=>
                    Some(a) if a.eq_ignore_ascii_case(#variant_token) => {
                        *args = &args[1..];
                        Some(Self::#ident)
                    }
                }
            }
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
        let parse_fields: Result<TokenStream> = match &s.fields {
            syn::Fields::Named(named) => {
                let (field_vars, field_returns) = named_fields_parse(named)?;
                let span = named.span();

                Ok(quote_spanned! {span=>
                    #field_vars
                    Ok(Some(Self {#field_returns}))
                })
            }
            syn::Fields::Unnamed(tuple) => {
                let (field_vars, field_returns) = unnamed_fields_parse(tuple)?;
                let span = tuple.span();

                Ok(quote_spanned! {span=>
                    #field_vars
                    Ok(Some(Self(#field_returns)))
                })
            }

            syn::Fields::Unit => Ok(quote! { Ok(Some(Self)) }),
        };
        let parse_fields = parse_fields?;

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

    /// Turns Unamed fields into code to parse each field element
    /// and list of return field. i.e.
    /// ```rust,ignore
    /// struct A(
    ///   B,
    ///   D
    /// )
    /// ```
    /// =>
    /// (
    /// ```rust,ignore
    ///     let field_0 = <B as ::command_args::CommandArgs>::parse_maybe(args)?
    ///         .ok_or(::command_args::Error::InvalidLength)?;
    ///     let field_1 = <D as ::command_args::CommandArgs>::parse_maybe(args)?
    ///         .ok_or(::command_args::Error::InvalidLength)?;
    /// ```,
    /// ```rust,ignore
    ///     field0, field1
    /// ```
    /// )
    fn unnamed_fields_parse(unnamed: &syn::FieldsUnnamed) -> Result<(TokenStream, TokenStream)> {
        let mut count = 0;
        let declare_vars = unnamed.unnamed.iter().map(|f| {
            let var_name = Ident::new(&format!("field_{}", count), f.ty.span());
            count += 1;
            parse_field_from_type(&f.ty, &var_name)
        });

        let mut count = 0;
        let return_fields = unnamed.unnamed.iter().map(|f| {
            let r = Ident::new(&format!("field_{}", count), f.ty.span());
            count += 1;
            r
        });


        let span = unnamed.span();
        Ok((quote_spanned! {span =>
            #(#declare_vars)*
        }, quote_spanned! {span =>
            #(#return_fields),*
        }))
    }

    /// Turns Unamed fields into code to parse each field element
    /// and list of return field. i.e.
    /// ```rust,ignore
    /// struct A {
    ///   b: B,
    ///   d: D,
    /// }
    /// ```
    /// =>
    /// (
    /// ```rust,ignore
    ///     let b = <B as ::command_args::CommandArgs>::parse_maybe(args)?
    ///         .ok_or(::command_args::Error::InvalidLength)?;
    ///     let d = <D as ::command_args::CommandArgs>::parse_maybe(args)?
    ///         .ok_or(::command_args::Error::InvalidLength)?;
    /// ```,
    /// ```rust,ignore
    ///     b, d
    /// ```
    /// )
    fn named_fields_parse(named: &syn::FieldsNamed) -> Result<(TokenStream, TokenStream)> {
        let declare_vars = named.named.iter().map(|f| {
            let var_name = f.ident.as_ref().unwrap();
            parse_field_from_type(&f.ty, var_name)
        });
        let return_fields = named.named.iter().map(|f| f.ident.as_ref());

        let span = named.span();
        Ok((quote_spanned! {span =>
            #(#declare_vars)*
        }, quote_spanned! {span =>
            #(#return_fields),*
        }))
    }

    /// Turn a type and a var name to code to parse
    /// ty: `B`
    /// var_name: `b`
    /// =>
    /// ```rust,ignore
    ///     let b = <B as ::command_args::CommandArgs>::parse_maybe(args)?
    ///         .ok_or(::command_args::Error::InvalidLength)?;
    /// ```
    /// ty: `Option<B>`
    /// var_name: `field_0`
    /// =>
    /// ```rust,ignore
    ///     let field_0 = <B as ::command_args::CommandArgs>::parse_maybe(args)?;
    /// ```
    ///
    fn parse_field_from_type(ty: &Type, var_name: &Ident) -> TokenStream {
        let ty_span = ty.span();
        match option_inner_type(ty) {
            Some(inner_ty) => quote_spanned! {ty_span=>
                let #var_name = <#inner_ty as ::command_args::CommandArgs>::parse_maybe(args)?;
            },
            None => quote_spanned! {ty_span=>
                let #var_name = <#ty as ::command_args::CommandArgs>::parse_maybe(args)?
                    .ok_or(::command_args::Error::InvalidLength)?;
            },
        }
    }
}
