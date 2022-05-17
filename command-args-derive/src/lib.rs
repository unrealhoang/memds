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
        Lifetime, LifetimeDef, LitStr, Path, PathArguments, PathSegment, Result, Type, TypePath, ExprField, Expr, ExprPath,
    };

    pub(crate) fn expand(input: DeriveInput) -> Result<TokenStream> {
        let data_model = parse_data_model(input)?;

        let default_lifetime = LifetimeDef::new(Lifetime::new("'a", Span::call_site()));
        let mut impl_generics = data_model.generics.clone();
        if impl_generics.lifetimes().next().is_none() {
            impl_generics
                .params
                .push(GenericParam::Lifetime(default_lifetime));
        }
        let lifetime = &impl_generics.lifetimes().next().unwrap().lifetime;
        let (_, ty_generics, where_clause) = data_model.generics.split_for_impl();
        let (impl_generics_tok, _, _) = impl_generics.split_for_impl();

        let name = &data_model.name;
        let parse_maybe_fn_content = parse_maybe_fn_content(&data_model)?;

        let parse_fn = quote! {
            fn parse_maybe(args: &mut &[&#lifetime str]) -> Result<Option<Self>, ::command_args::Error> {
                #parse_maybe_fn_content
            }
        };

        let encode_fn_content = encode_fn_content(&data_model)?;
        let encode_fn = quote! {
            fn encode(&self, target: &mut Vec<String>) -> Result<(), ::command_args::Error> {
                #encode_fn_content
            }
        };

        Ok(quote! {
            impl #impl_generics_tok ::command_args::CommandArgs<#lifetime> for #name #ty_generics #where_clause {
                #encode_fn

                #parse_fn
            }
        })
    }

    fn encode_fn_content(data_model: &DataModel) -> Result<TokenStream> {
        match &data_model.data_type {
            DataType::Enum(e) => {
                let span = data_model.span;
                let encode_variants_match_arms = e.variants.iter().map(|v| {
                    encode_variant_match_arm(v)
                }).collect::<Result<Vec<_>>>()?;
                let notoken_catch_arm = e.notoken_variant.as_ref().map(|v| {
                    let span = v.span;
                    let variant_ident = &v.name;
                    quote_spanned! {span=>
                        Self::#variant_ident => {}
                    }
                });

                Ok(quote_spanned! {span=>
                    match self {
                        #(#encode_variants_match_arms)*
                        #notoken_catch_arm
                    }

                    Ok(())
                })
            },
            DataType::Struct(s) => encode_struct(s, data_model.span),
        }
    }

    fn encode_variant_match_arm(v: &Variant) -> Result<TokenStream> {
        let span = v.span;
        let variant_ident = &v.name;
        let token = &v.token;

        match &v.fields {
            Fields::Unit => {
                Ok(quote_spanned! {span=>
                    Self::#variant_ident => {
                        #token.encode(target)?;
                    }
                })
            },
            Fields::Unnamed(t) => {
                let mut field_paths = Vec::new();
                let encode_tuple_fields = t.unnamed.iter().enumerate().map(|(i, f)| {
                    let var_name = Ident::new(&format!("t{}", i), f.span());
                    let field: ExprPath = parse_quote! { #var_name };
                    field_paths.push(field.clone());

                    encode_field(&field.into(), &f.ty)
                }).collect::<Result<Vec<_>>>()?;

                Ok(quote_spanned! {span=>
                    Self::#variant_ident( #(#field_paths)* ) => {
                        #token.encode(target)?;
                        #(#encode_tuple_fields)*
                    }
                })
            },
            Fields::Named(s) => {
                let mut field_paths = Vec::new();
                let encode_tuple_fields = s.named.iter().map(|f| {
                    let var_name = &f.ident;
                    let field: ExprPath = parse_quote! { #var_name };
                    field_paths.push(field.clone());

                    encode_field(&field.into(), &f.ty)
                }).collect::<Result<Vec<_>>>()?;

                Ok(quote_spanned! {span=>
                    Self::#variant_ident{ #(#field_paths),* } => {
                        #token.encode(target)?;
                        #(#encode_tuple_fields)*
                    }
                })
            },
        }
    }

    fn encode_struct(s: &Struct, span: Span) -> Result<TokenStream> {
        let encode_token = s.token.as_ref().map(|tok| {
            quote_spanned! {span=>
                #tok.encode(target)?;
            }
        });
        let encode_fields = match &s.fields {
            Fields::Unit => { TokenStream::new() },
            Fields::Unnamed(u) => {
                // if has token then prepend whitespace
                let encode_tuple_fields = u.unnamed.iter().enumerate().map(|(i, f)| {
                    let field: ExprField = parse_quote! { self.#i };
                    encode_field(&field.into(), &f.ty)
                }).collect::<Result<Vec<_>>>()?;

                quote_spanned! {span=>
                    #(#encode_tuple_fields)*
                }
            },
            Fields::Named(s) => {
                // if has token then prepend whitespace
                let encode_struct_fields = s.named.iter().map(|f| {
                    let ident = &f.ident;
                    let field: ExprField = parse_quote! { self.#ident };
                    encode_field(&field.into(), &f.ty)
                }).collect::<Result<Vec<_>>>()?;

                quote_spanned! {span=>
                    #(#encode_struct_fields)*
                }
            },
        };

        Ok(quote_spanned! {span=>
            #encode_token
            #encode_fields

            Ok(())
        })
    }

    fn encode_field(field_expr: &Expr, ty: &Type) -> Result<TokenStream> {
        let ty_span = ty.span();
        Ok(match option_inner_type(ty) {
            Some(inner_ty) => quote_spanned! {ty_span=>
                if let Some(s) = &#field_expr {
                    <#inner_ty as ::command_args::CommandArgs>::encode(s, target)?;
                }
            },
            None => quote_spanned! {ty_span=>
                <#ty as ::command_args::CommandArgs>::encode(&#field_expr, target)?;
            },
        })
    }

    enum Fields {
        Unit,
        Unnamed(syn::FieldsUnnamed),
        Named(syn::FieldsNamed),
    }

    struct Variant {
        span: proc_macro2::Span,
        name: syn::Ident,
        token: LitStr,
        fields: Fields,
    }

    struct NotokenVariant {
        span: proc_macro2::Span,
        name: syn::Ident,
    }

    struct Enum {
        variants: Vec<Variant>,
        notoken_variant: Option<NotokenVariant>,
    }

    struct Struct {
        token: Option<LitStr>,
        fields: Fields,
    }

    enum DataType {
        Enum(Enum),
        Struct(Struct),
    }

    struct DataModel {
        span: proc_macro2::Span,
        name: syn::Ident,
        generics: syn::Generics,
        data_type: DataType,
    }

    /// Turns input token stream into our DataModel
    fn parse_data_model(input: DeriveInput) -> Result<DataModel> {
        let span = input.span();
        Ok(DataModel {
            span: input.span(),
            data_type: match input.data {
                syn::Data::Struct(s) => {
                    let s = parse_data_model_struct(s, input.attrs)?;
                    DataType::Struct(s)
                }
                syn::Data::Enum(e) => {
                    let e = parse_data_model_enum(e)?;
                    DataType::Enum(e)
                }
                syn::Data::Union(_) => {
                    return Err(Error::new(span, "does not support union"))
                }
            },
            name: input.ident,
            generics: input.generics,
        })
    }

    fn parse_data_model_enum(e: syn::DataEnum) -> Result<Enum> {
        let mut variants = Vec::new();
        let mut notoken_variant = None;

        for variant in e.variants {
            let notoken_path: Path = parse_quote!(argnotoken);
            let is_notoken_variant = variant.attrs.iter().any(|a| a.path == notoken_path);
            let variant_span = variant.span();

            if notoken_variant.is_some() && is_notoken_variant {
                return Err(Error::new(
                    variant.span(),
                    "only one notoken variant allowed",
                ));
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

            if is_notoken_variant {
                let span = variant.span();
                match variant.fields {
                    syn::Fields::Unit => {},
                    _ => return Err(Error::new(span, "notoken variant must be Unit")),
                }
                notoken_variant = Some(NotokenVariant {
                    span: variant_span,
                    name: variant.ident,
                });
            } else {
                let fields = match variant.fields {
                    syn::Fields::Named(s) => Fields::Named(s),
                    syn::Fields::Unnamed(s) => Fields::Unnamed(s),
                    syn::Fields::Unit => Fields::Unit,
                };
                let v = Variant {
                    span: variant_span,
                    name: variant.ident,
                    token: variant_token,
                    fields
                };
                variants.push(v);
            }
        }

        Ok(Enum {
            variants,
            notoken_variant
        })
    }

    fn parse_data_model_struct(s: syn::DataStruct, attrs: Vec<syn::Attribute>) -> Result<Struct> {
        let token_path: Path = parse_quote!(argtoken);
        let token = attrs
            .iter()
            .find(|a| a.path == token_path)
            .map(|a| a.parse_args::<LitStr>())
            .transpose()?;
        let fields = match s.fields {
            syn::Fields::Named(s) => Fields::Named(s),
            syn::Fields::Unnamed(tuple) => Fields::Unnamed(tuple),
            syn::Fields::Unit => Fields::Unit,
        };
        Ok(Struct { token, fields })
    }

    fn parse_maybe_fn_content(input: &DataModel) -> Result<TokenStream> {
        match &input.data_type {
            DataType::Struct(s) => struct_parse_content(s, input.span),
            DataType::Enum(e) => enum_variants_match(e, input.span),
            // syn::Data::Union(_) => Err(Error::new(input.span(), "union not supported")),
        }
    }

    /// Turns an enum into a match expr
    fn enum_variants_match(e: &Enum, span: Span) -> Result<TokenStream> {
        let variant_matches = e.variants.iter().map(|v| variant_match_arm(&v)).collect::<Result<Vec<_>>>()?;
        let catch_all_arm = if let Some(variant) = &e.notoken_variant {
            let notoken_span = variant.span;
            let notoken_ident = &variant.name;
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
    fn variant_match_arm(variant: &Variant) -> Result<TokenStream> {
        let span = variant.span;
        let ident = &variant.name;
        let variant_token = &variant.token;

        Ok(match &variant.fields {
            Fields::Named(named) => {
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
            Fields::Unnamed(tuple) => {
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
            Fields::Unit => {
                quote_spanned! {span=>
                    Some(a) if a.eq_ignore_ascii_case(#variant_token) => {
                        *args = &args[1..];
                        Some(Self::#ident)
                    }
                }
            }
        })
    }

    fn struct_parse_content(s: &Struct, span: Span) -> Result<TokenStream> {
        let parse_token = if let Some(token) = &s.token {
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
            quote_spanned! {span=>
                if args.is_empty() {
                    return Ok(None);
                }
            }
        };
        let parse_fields = match &s.fields {
            Fields::Named(named) => {
                let (field_vars, field_returns) = named_fields_parse(named)?;
                let span = named.span();

                quote_spanned! {span=>
                    #field_vars
                    Ok(Some(Self {#field_returns}))
                }
            }
            Fields::Unnamed(tuple) => {
                let (field_vars, field_returns) = unnamed_fields_parse(tuple)?;
                let span = tuple.span();

                quote_spanned! {span=>
                    #field_vars
                    Ok(Some(Self(#field_returns)))
                }
            }

            Fields::Unit => quote! { Ok(Some(Self)) },
        };

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
        Ok((
            quote_spanned! {span =>
                #(#declare_vars)*
            },
            quote_spanned! {span =>
                #(#return_fields),*
            },
        ))
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
        Ok((
            quote_spanned! {span =>
                #(#declare_vars)*
            },
            quote_spanned! {span =>
                #(#return_fields),*
            },
        ))
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

